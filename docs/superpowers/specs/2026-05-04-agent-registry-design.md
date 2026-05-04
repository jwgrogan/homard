# Agent Registry — Skill-as-Agent Feature
**Date:** 2026-05-04
**Status:** WIP — brainstorm in progress, picking up in Codex
**Owner:** JWG

---

## What This Is

A UI-friendly agent-building layer that lets users upload a skill file to create a named, stateful, persistent agent accessible from the Homard sidebar. Agents are not a new runtime — they are skill-defined CLI session profiles (wrapping Claude Code / Codex via the existing control-plane architecture). This is an "easier and more secure" approach vs. remembering to invoke skill calls manually each time.

---

## Decisions Locked

### Architecture — agent lifecycle (§1, approved)

- **Agent = profile on disk** → **session spawned per chat** → **session torn down when chat closes/idles**.
- `AgentRegistry` is the only genuinely new Rust component. Chat rooms reuse existing `Session` machinery from the multi-CLI mission control spec.
- On each user turn addressed to an agent, daemon spawns a fresh CLI session with:
  ```
  system_prompt = render(agent.SKILL.md + agent.MEMORY.md + optional_global_identity + recent_transcript)
  ```
- CLI returns → appended to transcript.
- On idle (5 min after last turn): **memory consolidation pass** runs (see §4).
- Python hooks: **cut from v1**. Leave tool composition to the CLI's MCP path.

---

### Skill file format (§2 partial, approved)

Anthropic SKILL.md frontmatter with optional `homard:` extension block.

```markdown
---
name: morning-briefing
description: Daily summary of calendar + email + open PRs
homard:
  provider: claude        # which CLI backend
  repo: ~/code/work       # default repo (optional)
  worktree: main          # optional
  channels: [tray, telegram, schedule]  # default: [tray] only
  icon: 🌅
  shared_memory: true     # also reads global SOUL/USER/MEMORY (default: true)
---

[body → seed system prompt for the agent]
```

**Defaults if `homard:` block is absent:**
- `provider` → user's current default
- `repo` → unbound
- `channels` → `[tray]` only (Telegram/schedule require explicit opt-in per agent)
- `icon` → derived from name
- `shared_memory` → `true`

**Per-agent folder layout:**
```
~/.homard/agents/<slug>/
  SKILL.md            # uploaded source of truth for seed prompt
  MEMORY.md           # accumulated by consolidation loop
  NOTES.md            # user-authored; agent reads, consolidation never writes
  history.jsonl       # session transcripts (rotated)
  consolidation.log   # one-line per consolidation run (debug)
```

---

### Sidebar / navigation redesign (§2, approved)

- **App.tsx top tab bar `[Chat | Activity | Settings]` is removed.**
- Replaced with a **left sidebar** that is the primary navigator.
- Sidebar structure:
  ```
  AGENTS
  ├── 🏠 Homard          ← built-in default agent (pinned, existing chat behavior preserved)
  ├── 🌅 morning-briefing
  ├── 🧪 test-doctor
  └── + Upload skill…

  ROOMS (placeholder, "coming soon" or hidden — v1.1)

  ⚡ Activity
  ⚙️ Settings
  ```
- **"Homard" default agent is pinned at top.** Existing users see zero migration change.
- Telegram threads no longer get their own tab strip in Chat. Telegram messages addressed to a specific agent (`/morning-briefing …`) appear in that agent's transcript. Unscoped Telegram messages land in the default Homard agent's transcript.
- Old Chat header provider/system config → moves fully into Settings.

**Upload flow:**
- `+ Upload skill` → file picker (`*.md`)
- Daemon validates frontmatter: missing `name` → reject with inline error; unknown `homard:` keys → warn, accept (forward-compat); collision → ask rename or replace
- On success → agent appears in sidebar

**Agent detail panel (right pane when agent is selected in Agents tab):**
- Name + description
- Channels section:
  - ☑ Tray (always on, non-toggleable)
  - ☐ Telegram `[ Enable ]` — three states: off / enabled-not-paired (shows "Pair Telegram first → Settings") / enabled
  - ☐ Schedule `[ Add cron… ]` — opens existing schedule editor pre-bound to agent
- Memory section:
  - `MEMORY.md (4.2 KB)  [ Open ]`
  - `NOTES.md (empty)    [ Open ]`
  - `[ Reset memory ]`
  - `☐ Freeze memory` checkbox — disables consolidation for this agent
  - `[ View archives ]` — visible only if archive has ever fired
- Repo binding: `~/code/work  [ Change ]`
- Actions: `[ Chat ]  [ Edit skill ]  [ … ]`
  - `…` → Delete (soft-delete: moves to `~/.homard/agents/.trash/<slug>-<timestamp>/`)

---

### Multi-agent (§3, approved)

- **No rooms in tray UI.**
- **Telegram multi-agent emerges for free** via per-agent channel toggles: if multiple agents have `telegram` enabled and the Homard bot is in a Telegram group, users address agents by `/agent-slug` or `@mention`. No new Homard code beyond what per-agent toggles give.
- **Workflows** (user-defined chaining of agents, input → A → A's output → B → final): **deferred to v1.1.** Spec appendix will sketch the `WORKFLOW.md` format and `~/.homard/workflows/` directory for forward-compat.

---

### Memory consolidation (§4, mostly approved — two open items below)

**Trigger:** idle-based, per-chat, debounced.
- 5-minute idle timer resets on each new turn.
- Fires when 5 minutes pass with no new turn.
- Pending consolidations survive daemon restart (persisted).
- `[ Consolidate now ]` manual button in agent detail panel.

**What runs:** same CLI machinery as the agent (not a direct-API call, not a different provider). One-shot CLI session with fixed consolidation system prompt + current MEMORY.md + new turns since last consolidation.

**Consolidation system prompt (fixed, in code):**
> You are maintaining a small persistent memory file for an agent. Read the existing MEMORY.md and the new conversation turns. Output a new MEMORY.md that incorporates lasting facts, preferences, recurring patterns, and decisions the agent should remember next time. Drop ephemera and anything the user explicitly said to forget. Resolve contradictions in favor of the most recent turn. Stay under 2,000 tokens. Output the full new MEMORY.md content only — no commentary.

**Write strategy:** atomic (write to `MEMORY.md.tmp` → fsync → rename). On error: log, leave existing untouched, retry next idle.

**Cost:** ~1-1.5k tokens per active agent per idle window. Negligible against chat tokens.

**Size discipline:**
- Soft cap (2k tokens): enforced by consolidation prompt.
- Hard cap (8k tokens): [OPEN — see below]

**User controls:**
- `[ Open ]` MEMORY.md / NOTES.md → opens in default editor, picked up by file watcher
- `[ Reset memory ]` → confirm → wipe to empty
- `☐ Freeze memory` → skip consolidation for this agent
- `[ View archives ]` → list of `MEMORY.archive.md.*` files

**NOTES.md distinction:** user-authored; agent reads it every turn; consolidation never writes to it. UI label: "Memory: managed by Homard. Notes: managed by you."

**Debug log:** each consolidation appends one line to `consolidation.log`:
```
2026-04-29T18:32:14Z  turns=12  in=1042 tok  out=487 tok  duration=4.1s  result=ok
```

**Synaptic-graph seam:** the consolidation prompt + file write is explicitly the future swap-in point for synaptic-graph. No structural change needed — replace the CLI call and file write with `synaptic_graph.consolidate(agent_id, transcript)`.

---

## Open Items (resolve before spec is final)

1. **Hard cap behavior on MEMORY.md > 8k tokens:** auto-archive to `MEMORY.archive.md.<timestamp>` + fresh consolidation (self-healing, preferred) vs. stop + warn only. **Recommendation: auto-archive.** Pending confirmation.
2. **Idle window duration:** 5 minutes is the recommended default. Pending confirmation.
3. **§5 — Storage / daemon changes:** not yet designed. Need to cover:
   - SQLite schema additions for agent registry (agents table, last_consolidated_at, etc.)
   - `AgentRegistry` Rust struct and file watcher setup
   - API routes the frontend needs (`GET /agents`, `POST /agents`, `DELETE /agents/:slug`, `PATCH /agents/:slug/channels`, `GET /agents/:slug/memory`)
4. **§6 — Security / channel-trust model fit:** how per-agent channel restrictions are enforced at the API layer (existing `permission_level` system, or per-agent policy?)
5. **§7 — Migration:** no migration needed for existing users — Homard default agent is synthetic (no file on disk, hardcoded behavior). But the sidebar redesign is a breaking UI change and needs a "first launch" state where users with no uploaded agents see a useful empty state.

---

## Deferred to v1.1

- **Workflows:** user-defined linear pipelines chaining agents. `~/.homard/workflows/` directory created in v1 (empty). `WORKFLOW.md` format sketched in spec appendix.
- **Sidebar tags/folders:** flat list in v1; grouping when >10 agents is common.
- **Synaptic-graph memory backend:** swap-in ready at the consolidation seam.
- **Parallel fan-out in Telegram rooms:** agents addressable individually today; parallel dispatch to N agents in v1.1.
- **Manager-agent pattern (C-style orchestration):** an agent whose skill prompt uses `@agent-slug` syntax in its responses to dispatch to siblings. Daemon parses and queues (one level deep, no recursion). Explicit v1.1 hook — no orchestration logic built in v1.
