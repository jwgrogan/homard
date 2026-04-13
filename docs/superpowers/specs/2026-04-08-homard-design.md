# Homard — Lightweight macOS Personal AI Assistant

**Date:** 2026-04-08
**Status:** Design approved, pending implementation plan
**Origin:** Fork of [homard](https://github.com/jwgrogan/homard) with agent core ported from [ysz/nanoClaw](https://github.com/ysz/nanoClaw)

---

## Overview

Homard is an always-on macOS personal AI assistant that lives in your menu bar and is reachable via Telegram. It combines homard's Tauri tray shell, Telegram bridge, and launchd scheduling with a ReAct-style agent loop ported from nanoClaw. OpenClaw-style identity files provide deep context and autonomy.

**Core tenets:**
- Lightweight: Rust daemon, low memory footprint, runs on a MacBook Air
- Always-on: launchd-managed daemon survives tray quit/crash
- Chat-first: tray popover for quick chat, not a dashboard
- Remote access: full Telegram integration for control from anywhere
- Identity-driven: persistent personality, user knowledge, and memory across all conversations

---

## Architecture

### Two-Binary Split

```
homard (daemon)                      Homard.app (Tauri tray)
├── Agent loop (ReAct)               ├── Popover UI (React)
├── LLM client (OpenAI/Anthropic)    │   ├── Chat view (default)
├── Tool registry + execution        │   ├── Activity view
├── Telegram poller (teloxide)       │   └── Settings view
├── Cron scheduler                   └── Thin REST client to daemon
├── Identity file loader             
├── Memory (FTS5)                    Communicates via localhost:17700
├── Security sandbox                 
├── REST API (axum, :17700)          
└── launchd managed                  
```

**Daemon** (`homard` binary): All business logic. Runs as a launchd service. Exposes REST API on localhost:17700. Manages Telegram, cron, agent loop, LLM calls, tool execution, memory.

**Tray app** (`Homard.app`): Pure UI shell. Tauri 2 with React frontend. Talks to daemon via REST. Manages tray icon, popover window, settings views. Can be quit without stopping the agent.

**Data directory:** `~/.homard/`

### Data Storage

```
~/.homard/
├── config.json              # Provider auth refs, permissions, telegram, settings
├── homard.db                # SQLite WAL (conversations, memory FTS5, audit log)
├── schedules/               # JSON schedule definitions
├── logs/                    # Run logs
├── BOOTSTRAP.md             # One-time first-run setup prompt
├── SOUL.md                  # Personality, tone, communication style
├── IDENTITY.md              # Name, avatar emoji, tagline
├── USER.md                  # User profile (agent-maintained)
├── AGENTS.md                # Operational rules and policies
├── TOOLS.md                 # Environment-specific tool notes
├── MEMORY.md                # Learned patterns and preferences
├── HEARTBEAT.md             # Periodic task checklists
└── conversations/           # Per-channel thread history (JSONL)
    ├── chat.jsonl
    ├── telegram_{user_id}.jsonl
    └── cron_{job_name}.jsonl
```

---

## Identity Files

All identity files are markdown, stored in `~/.homard/`, loaded into every agent call as context.

### BOOTSTRAP.md
Runs once on first launch (before `bootstrapped = true` is set in config). Guides the agent through initial setup: learn user's name, role, preferences, populate USER.md.

```markdown
# Bootstrap
Introduce yourself as Homard. Ask the user:
- Their name and what they do
- What they'll primarily use you for
- Their preferred communication style (terse vs detailed)
- Which Telegram chat to pair with
Populate USER.md with what you learn. Set a welcoming tone.
```

### IDENTITY.md
Agent name, emoji, tagline. User-editable. Agent reads but does not modify.

```markdown
name: Homard
emoji: 🦞
tagline: Your personal crustacean
```

### SOUL.md
Personality and communication style. User-editable. Agent reads but does not modify.

```markdown
You are direct and competent. You bias toward action over discussion.
You don't over-explain or hedge. When unsure, you say so and ask.
You have dry wit but never at the user's expense.
You remember context between conversations and reference it naturally.
```

### USER.md
Deep user profile. Agent maintains via `update_user_profile` tool. Grows over time as the agent learns.

### AGENTS.md
Operational rules and policies. User-editable. Agent reads but does not modify.

```markdown
# Policies
- Always confirm before sending messages to others (Slack, email)
- Never commit code without explicit approval
- For scheduled tasks, report results to Telegram
- When a task takes >30s, send a "working on it" acknowledgment
- If a run hangs, explain what happened and ask how to proceed
```

### TOOLS.md
Environment-specific notes about available tools. User-editable. Agent reads for context before using tools.

```markdown
# Shell
- site-factory deploys via `npm run deploy` in ~/GitHub/site-factory
- d1201 CI is GitHub Actions, check with `gh run list`

# MCP
- Gmail: personal account, not work
- Calendar: shared with spouse, check before creating events
```

### MEMORY.md
Learned patterns and preferences. Agent maintains via `save_memory` and `maintain_memory` tools. Structured sections:

```markdown
# Active Context
(Current focus — agent overwrites freely)

# Decisions & Preferences
(Accumulated learnings — append-only, summarized when >2k tokens)

# Important Facts
(Key dates, accounts, relationships — agent curates)

# Recent Events
(Last ~20 interactions summarized — rolling window)
```

Self-maintenance: agent runs `maintain_memory` when file exceeds ~4k tokens, reorganizing, summarizing, and pruning.

### HEARTBEAT.md
Periodic task checklists. Cron scheduler reads this and executes items on schedule.

```markdown
# Every Morning (9am)
- Check calendar for today's meetings
- Check GitHub notifications
- Summarize any overnight Telegram messages I missed

# Every Friday (5pm)
- Weekly summary of what got done
- Any open PRs that need attention
```

### Context Builder Load Order

```
IDENTITY.md → SOUL.md → USER.md → AGENTS.md → TOOLS.md → MEMORY.md
  ~50 tok     ~200 tok   ~500 tok   ~300 tok    ~300 tok   ~2000 tok
                                                      Total: ~3-4k tokens
```

All files loaded every call. No conditional loading. Simple concatenation into system prompt. Leaves ~120k+ tokens for conversation + tool results in a 128k context window.

---

## Agent Core

### ReAct Loop

Ported from nanoClaw's agent.py to Rust.

```
Input (chat / telegram / cron)
  → Load identity files (SOUL, USER, AGENTS, TOOLS, MEMORY, IDENTITY)
  → Load channel-specific conversation history (windowed)
  → Select relevant tools (keyword filtering)
  → Loop (no hard iteration limit):
    → Call LLM (OpenAI chat/completions format)
    → If text response, no tool calls → done, return
    → If tool calls → execute all in parallel (tokio::join!)
    → Append assistant msg + tool results to messages
    → Hang detection check (see Security section)
  → Save to channel conversation history
  → Optionally update USER.md (if learned something new)
  → Optionally append to MEMORY.md (if noteworthy event)
  → If MEMORY.md > threshold → trigger maintain_memory
  → Return response to caller
```

### LLM Client

- Primary format: OpenAI `chat/completions` (works for OpenAI and any compatible endpoint)
- Anthropic adapter: translates to/from Messages API format
- OAuth PKCE middleware: checks token expiry before each call, auto-refreshes if needed
- Retry: 3 attempts with exponential backoff (1s, 2s, 4s) for 429/529
- Connection pooling via `reqwest::Client` (shared across app lifetime)
- No streaming for v1 — request/response. Streaming is a future polish item.

### Context Builder

- System prompt built from concatenated identity files + current datetime + platform info
- History window: last 4 messages always included, messages 5-15 if substantive (>100 chars), older dropped
- Dynamic tool selection: keyword match in user message determines which tools to send (5-7 per call to save tokens)
- Tool output truncation: web_fetch 4000 chars, shell_exec 2000, others 1000

### Conversation Threading

Each input channel gets its own conversation history file (`~/.homard/conversations/{channel}.jsonl`). All threads share the same identity files. Memory updates from any thread are visible to all others.

---

## Tool System

### Two Tool Sources

**Built-in tools** (Rust functions):
- `web_search` — search the web
- `web_fetch` — fetch and extract content from a URL
- `shell_exec` — execute shell command (sandboxed)
- `file_read` — read a file
- `file_write` — write a file
- `memory_save` — save a fact to MEMORY.md
- `memory_search` — search conversation history and MEMORY.md via FTS5
- `maintain_memory` — reorganize/summarize/prune MEMORY.md
- `update_user_profile` — update USER.md with learned information

**Shell tools** (user-defined in config.json):
```json
{
  "shell_tools": [
    {
      "name": "deploy_site",
      "description": "Deploy the site factory",
      "command": "cd ~/GitHub/site-factory && npm run deploy"
    },
    {
      "name": "check_ci",
      "description": "Check GitHub Actions status for a repo",
      "command": "gh run list --repo {repo} --limit 5"
    }
  ]
}
```

**MCP bridge** (future, phase 2):
Connects to user's MCP servers, translates their tools into the registry format. Enables Gmail, Calendar, Slack, etc.

### Tool Registry

Tools register as name + description + parameters JSON schema + async handler. Registry produces OpenAI-compatible tool schemas for LLM calls. Tool results fed back as `{"role": "tool", "tool_call_id": ..., "content": ...}` messages.

---

## Providers & Authentication

### OpenAI — OAuth PKCE

| Field | Value |
|-------|-------|
| Auth type | OAuth 2.0 Authorization Code + PKCE (S256) |
| Authorize URL | `https://auth.openai.com/oauth/authorize` |
| Token URL | `https://auth.openai.com/oauth/token` |
| Client ID | `app_EMoamEEZ73f0CkXaXp7hrann` |
| Scopes | `openid profile email offline_access` |
| Models | `gpt-5.4`, `gpt-5.4-mini`, `gpt-5.3-codex` |
| Billing | Subscription credits (ChatGPT Plus/Pro) |

### Anthropic — OAuth PKCE

| Field | Value |
|-------|-------|
| Auth type | OAuth 2.0 Authorization Code + PKCE (S256) |
| Authorize URL | `https://claude.ai/oauth/authorize` |
| Token URL | `https://console.anthropic.com/v1/oauth/token` |
| Client ID | `9d1c250a-e61b-44d9-88ed-5944d1962f5e` |
| Scopes | `org:create_api_key user:profile user:inference` |
| Models | `claude-opus-4-6`, `claude-sonnet-4-6` |
| Billing | Extra usage (pay-as-you-go, not subscription) |

### OpenRouter — API Key (fallback)

| Field | Value |
|-------|-------|
| Auth type | API key |
| Endpoint | `https://openrouter.ai/api/v1/chat/completions` |
| Models | Any model on OpenRouter |
| Billing | Per-token via OpenRouter account |

### OAuth Flow (shared by OpenAI and Anthropic)

1. User clicks "Sign in with {Provider}" in tray Settings
2. Daemon generates PKCE code_verifier + code_challenge (S256)
3. Daemon starts temporary localhost HTTP server on ephemeral port
4. Opens browser to provider's authorize URL with client_id, redirect_uri, code_challenge, scopes
5. User logs in and authorizes
6. Browser redirects to localhost callback with `?code=...`
7. Daemon exchanges code + code_verifier for access_token + refresh_token
8. Tokens stored in macOS Keychain
9. Temporary HTTP server shuts down

Token refresh: before each LLM call, check expiry. If within 5 min of expiry, refresh silently. If refresh fails, surface alert to chat + Telegram.

### Config Structure

```json
{
  "providers": {
    "openai": {
      "auth_type": "oauth_pkce",
      "client_id": "app_EMoamEEZ73f0CkXaXp7hrann",
      "token_keychain_ref": "homard.openai.tokens",
      "model": "gpt-5.4"
    },
    "anthropic": {
      "auth_type": "oauth_pkce",
      "client_id": "9d1c250a-e61b-44d9-88ed-5944d1962f5e",
      "token_keychain_ref": "homard.anthropic.tokens",
      "model": "claude-sonnet-4-6"
    },
    "openrouter": {
      "auth_type": "api_key",
      "api_key_keychain_ref": "homard.openrouter.api_key",
      "model": "anthropic/claude-sonnet-4-6"
    }
  },
  "active_provider": "openai"
}
```

All secrets in macOS Keychain, never on disk. Config stores only references.

### Settings UI

```
OpenAI     [Connected ✓]  gpt-5.4         (subscription credits)
Anthropic  [Connected ✓]  claude-sonnet   (extra usage billing)
OpenRouter [Not connected]                 (API key)
```

Info badge on Anthropic: "Usage billed to extra usage, not your subscription."

---

## Security & Permissions

### Three Permission Levels

Togglable in tray Settings → Permissions. Also switchable via Telegram: `/perms supervised|autonomous|locked`

**Supervised** (default):
- All actions either execute silently (safe) or surface for user approval (everything else)
- Nothing is ever auto-rejected — user always gets the choice
- Hang detection: after 10 iterations or 5 min with no meaningful progress, pause run and ask "Still working on this — continue?" via chat + Telegram. Blocks until response.

**Autonomous:**
- All actions auto-approved
- Hang detection: after 10 iterations or 5 min with no meaningful progress, surface alert: "Heads up — this run has been looping for a while. /stop to end run." Run continues, never pauses.

**Locked:**
- Read-only. No shell, no file writes, no outbound messages
- Agent can only chat, search memory, and web search/fetch (GET only)

### Key Rules

- **Never kill a running job.** In any mode. Jobs stop only by: natural completion, or user sends `/stop`.
- "No meaningful progress" = repeated tool calls with same/similar arguments, or iterations producing no content. A legitimately complex multi-step task does not trigger hang detection.
- Audit log (`~/.homard/audit.log`) records every tool execution regardless of permission level.

### Approval Flow

In supervised mode, when a tool call needs approval:
- **Telegram:** Inline keyboard with Approve / Deny buttons
- **Chat (tray):** Confirm/Deny buttons in the chat view
- 5-minute timeout, defaults to deny

---

## Telegram Integration

Built on teloxide (Rust Telegram framework). Polling mode, no webhooks, no open ports.

### Commands

| Command | Action |
|---------|--------|
| (any text) | Send to agent, get response |
| `/status` | Daemon health + current run info |
| `/stop` | Stop current run |
| `/perms <level>` | Switch permission level |
| `/pair <code>` | Pair this chat with Homard |

### Pairing Flow

1. User opens tray Settings → Telegram
2. Enters bot token from @BotFather, clicks Save
3. Clicks "Generate Pairing Code" → shows 8-char code (10-min expiry)
4. User sends `/pair ABC12345` in Telegram chat
5. Chat ID stored in config, all future messages from that chat go to the agent

### Message Delivery

- Agent responses chunked at 4000 chars (Telegram limit)
- Markdown formatting attempted, plaintext fallback on parse error
- Cron job results delivered to all paired chats
- Approval prompts sent as inline keyboard messages

---

## Cron Scheduler

### Schedule Definitions

Stored as JSON in `~/.homard/schedules/`:

```json
{
  "id": "uuid",
  "name": "Morning brief",
  "message": "Run the morning checklist from HEARTBEAT.md",
  "schedule": "0 9 * * *",
  "enabled": true,
  "deliver_to": ["telegram", "chat"]
}
```

### HEARTBEAT.md Integration

The cron scheduler parses HEARTBEAT.md sections (e.g., "Every Morning (9am)") and creates/updates corresponding schedule entries. User edits HEARTBEAT.md, schedules auto-sync. Section headers are matched via regex for common patterns: "Every Morning", "Every Evening", "Every Monday", "Every Friday (5pm)", "Every Hour", "Daily", "Weekly". Unrecognized patterns are logged as warnings. Users can also specify explicit cron syntax in headers: "# Cron: 0 9 * * *".

### Execution

Cron jobs route through the same agent loop as chat/Telegram messages. The job's message becomes the user input. Response delivered to configured channels.

---

## REST API

Axum on `localhost:17700`. No auth (localhost-only, single user).

```
POST   /chat                    Send message, get response (JSON)
GET    /conversations           List conversation threads
GET    /conversations/:id       Get thread history
POST   /stop                    Stop current run

GET    /status                  Daemon health + active run info
GET    /activity                Recent runs with status

GET    /schedules               List cron jobs
POST   /schedules               Create/update schedule
DELETE /schedules/:id           Delete schedule

GET    /settings                Read config
PUT    /settings                Update config
GET    /settings/permissions    Get current permission level
PUT    /settings/permissions    Set permission level

POST   /auth/:provider/start   Initiate OAuth flow (returns browser URL)
GET    /auth/:provider/callback OAuth redirect handler

POST   /telegram/pair           Start pairing flow
GET    /telegram/status         Connection status

GET    /files/:name             Read identity file
PUT    /files/:name             Update identity file
```

---

## UI Design

### Tray Popover (not a full window)

~400x500px popover anchored to tray icon. Three views via bottom tab bar.

### Color Palette (Light mode only)

| Role | Color | Hex |
|------|-------|-----|
| App background | Light cream | `#FAF5ED` |
| Card/input background | Cream | `#FDF8F0` |
| Surface (headers, sidebar) | Sage | `#E8F0EC` |
| Primary accent | Coral | `#E85D4A` |
| Text primary | Navy | `#1B2D4F` |
| Text secondary | Muted navy | `#4A6180` |
| Borders | Sage-tinted | `#C2D1C8` |
| Code blocks | Navy bg + cream text | `#1B2D4F` / `#FDF8F0` |

### Tray Icon

Lobster claw silhouette, 16x16, navy color. Green dot overlay when daemon is running.

### Chat View (default)

- Message input at bottom (like iMessage/Telegram)
- Conversation thread above with responses
- Provider indicator badge on each response
- Messages from cron and Telegram visible (unified inbox)
- Approval prompts appear inline as interactive cards

### Activity View

- Compact list of recent runs: status dot, name/source, duration, timestamp
- Tap to expand and see output
- Scheduled jobs with next-run time

### Settings View

- **Providers:** Sign in buttons (OpenAI, Anthropic), API key input (OpenRouter), model selector, active provider dropdown, billing info badges
- **Permissions:** Three-way toggle (Supervised / Autonomous / Locked) with descriptions
- **Telegram:** Bot token, pairing code, connection status, paired chats
- **Tools:** Shell tools editor, MCP server list (future)
- **Identity:** Edit SOUL.md, AGENTS.md, TOOLS.md, HEARTBEAT.md inline
- **Daemon:** Start/stop, launchd install/uninstall, log viewer

---

## Cargo Workspace

```
homard/
├── Cargo.toml                    # Workspace root
├── crates/
│   └── homard-core/              # Shared library
│       └── src/
│           ├── lib.rs
│           ├── config.rs          # Config + dirs (~/.homard/)
│           ├── store.rs           # SQLite (conversations, memory FTS5, audit)
│           ├── error.rs           # Error types
│           ├── types.rs           # Shared types
│           ├── keychain.rs        # macOS Keychain
│           ├── agent/
│           │   ├── mod.rs
│           │   ├── loop.rs        # ReAct agent loop
│           │   ├── context.rs     # Identity file loader + context builder
│           │   └── hang.rs        # Hang detection (soft pause/alert)
│           ├── llm/
│           │   ├── mod.rs
│           │   ├── client.rs      # HTTP client + connection pooling
│           │   ├── openai.rs      # OpenAI format
│           │   ├── anthropic.rs   # Anthropic format adapter
│           │   └── oauth.rs       # PKCE OAuth flow (shared by providers)
│           ├── tools/
│           │   ├── mod.rs
│           │   ├── registry.rs    # Tool registration + schema generation
│           │   ├── shell.rs       # shell_exec (sandboxed)
│           │   ├── web.rs         # web_search, web_fetch
│           │   ├── files.rs       # file_read, file_write
│           │   ├── memory.rs      # memory_save, memory_search, maintain_memory
│           │   ├── user_profile.rs # update_user_profile
│           │   └── mcp_bridge.rs  # MCP server connector (phase 2)
│           ├── security/
│           │   ├── mod.rs
│           │   ├── sandbox.rs     # Permission levels + approval flow
│           │   └── prompt_guard.rs # Injection detection
│           ├── telegram/
│           │   ├── mod.rs
│           │   ├── client.rs      # Send/edit messages, chunking
│           │   └── poller.rs      # Long-poll loop, command parsing
│           ├── scheduler/
│           │   ├── mod.rs
│           │   ├── cron.rs        # Cron check loop
│           │   ├── heartbeat.rs   # HEARTBEAT.md parser
│           │   └── launchd.rs     # Plist generation + install
│           └── api/
│               ├── mod.rs
│               └── routes.rs      # Axum route handlers
├── cli/                           # CLI binary
│   └── src/
│       └── main.rs               # homard serve|chat|status|stop
├── src-tauri/                     # Tauri tray app
│   └── src/
│       ├── main.rs
│       ├── lib.rs                # Tray setup, window management
│       ├── tray.rs               # Tray icon + menu
│       └── state.rs              # Minimal app state
└── src/                           # React frontend
    ├── main.tsx
    ├── App.tsx                   # Tab bar: Chat | Activity | Settings
    ├── pages/
    │   ├── Chat.tsx              # Chat view (primary)
    │   ├── Activity.tsx          # Run history
    │   └── Settings.tsx          # All settings panels
    ├── components/
    │   ├── MessageBubble.tsx
    │   ├── ApprovalCard.tsx
    │   ├── ProviderBadge.tsx
    │   ├── RunCard.tsx
    │   └── settings/
    │       ├── ProvidersPanel.tsx
    │       ├── PermissionsPanel.tsx
    │       ├── TelegramPanel.tsx
    │       ├── ToolsPanel.tsx
    │       ├── IdentityPanel.tsx
    │       └── DaemonPanel.tsx
    └── lib/
        ├── api.ts                # REST client (fetch to localhost:17700)
        ├── store.ts              # Zustand stores
        └── types.ts              # TypeScript types
```

---

## What Gets Stripped from homard

**Deleted:**
- `agents.rs` — agent/command discovery
- `parsers/` — Claude/Gemini JSONL parsers
- `mcp_sync.rs` — MCP sync stub
- `profile.rs` — multi-profile management
- `provider.rs` — CLI provider abstraction
- `session_monitor.rs` — PID polling
- `process.rs` — process registry
- `project_defaults.rs` — template constants
- `Health.tsx`, `Sessions.tsx` — dashboard pages
- `AgentTree.tsx`, `SessionDetail.tsx`, `NewSessionModal.tsx`, `ProfileSwitcher.tsx`
- `EmailPanel.tsx`, `AgentsBrowser.tsx`, `ProfilesPanel.tsx`
- `App.css` — Tauri boilerplate

**Kept and adapted:**
- `config.rs` → `~/.homard/` paths
- `store.rs` → new schema
- `keychain.rs` → as-is
- `telegram.rs` → agent loop integration
- `schedule.rs` → HEARTBEAT.md support
- `launchd.rs` → renamed identifiers
- `health.rs` → daemon health only
- `terminal.rs` → as-is
- `types.rs` → pruned
- `error.rs` → as-is
- `tray.rs` → reskinned
- `TelegramPanel.tsx` → restyled
- `McpServersPanel.tsx` → becomes ToolsPanel
- `QuickPrompt.tsx` → becomes Chat view foundation

---

## CLI Commands

```
homard serve          Start the daemon (foreground, or use launchd)
homard chat           Interactive CLI chat (one-shot: homard chat -m "...")
homard status         Show daemon health, active runs, connected channels
homard stop           Stop current run
homard install        Install launchd plist for always-on daemon
homard uninstall      Remove launchd plist
```

---

## Testing Strategy

- **Unit tests:** Agent loop (mock LLM responses), tool registry, context builder, OAuth token handling, security sandbox, cron parser
- **Integration tests:** Full agent run with mock HTTP server (LLM + tool endpoints), Telegram message flow, REST API endpoints
- **Manual QA:** Telegram pairing flow, OAuth browser flow, tray popover UX, identity file editing, cron job execution
