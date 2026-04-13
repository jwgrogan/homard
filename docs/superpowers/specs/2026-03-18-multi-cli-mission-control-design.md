# homard Multi-CLI Mission Control Redesign

**Date:** 2026-03-18
**Status:** Draft

## Overview

homard evolves from a Claude Code-only menu bar app into a multi-CLI mission control dashboard. It launches terminal sessions across providers (Claude Code, Gemini CLI, extensible), monitors agent activity via session file tailing, manages profiles and MCP servers as a unified layer, and gives users a bird's eye view across all their AI coding sessions.

homard is not a terminal emulator. Users interact with CLIs in real terminal windows. homard's value is orchestration, visibility, and the cross-session view no single terminal can provide.

## Architecture

**Approach: Log Tailer.** homard never interferes with CLI processes. It reads session files written by each CLI, tracks metadata in SQLite, and opens real terminal windows. Agent team tree depth varies by provider — graceful degradation when parsing is unavailable.

**Key principle:** Each CLI works exactly as its developers intended. homard adds the layer above.

### Migration from Current Architecture

The existing codebase has `Run` (types.rs) and `ProcessRegistry` (process.rs) for headless CLI process management, plus a `runs` table and `chat_sessions` table in SQLite.

**Migration path:**
- `Run` is replaced by `Session`. The `runs` table is migrated to a `sessions` table via SQLite `ALTER TABLE` + data migration on first launch of the new version.
- `ProcessRegistry` is replaced by `SessionMonitor`. The registry tracked in-memory PIDs of child processes homard owned — the new model tracks PIDs of terminal windows homard launched but does not own.
- `chat_sessions` table is dropped (it was used for headless streaming, which is no longer the interaction model).
- Existing `Run` history is preserved in the migrated `sessions` table with `provider: "claude"` backfilled and `cli_session_id: null` (we didn't capture it before).

### Scope Boundaries

- **Phase 1 only monitors sessions spawned by homard.** Discovery of externally-started sessions (user typing `claude` directly in a terminal) is out of scope. It could be added later by scanning CLI session directories for untracked session files.

---

## Phase 1 — Foundation

### 1.1 Profile = CLI Identity

A profile is a (provider, account) tuple. No separate CLI selection — the profile determines which CLI runs.

**Profile struct:**

```
Profile {
  name: String,           // "Work Claude", "Personal Gemini"
  provider: Provider,     // Claude | Gemini | (extensible enum)
  email: String,          // account identifier
  credential_dir: Path,   // ~/.homard/profiles/<name>/
  created_at: DateTime,
}
```

**Provider struct:**

```
Provider {
  id: String,             // "claude", "gemini"
  cli_command: String,    // "claude", "gemini"
  session_dir: Path,      // where this CLI stores sessions
  supports_stream_json: bool,
  supports_resume: bool,
  resume_flag: String,    // "--resume" for both currently
}
```

Provider definitions are hardcoded initially (Claude, Gemini). Adding a new provider means adding a new variant and its session file parser.

**Per-provider credential strategy:**

Credential management is provider-specific. The `ProfileManager` uses a trait-based approach:

```
trait CredentialManager {
  fn import(&self, profile_dir: &Path) -> Result<()>;   // copy live creds to profile dir
  fn restore(&self, profile_dir: &Path) -> Result<()>;  // restore profile creds to live locations
  fn detect_active(&self, profile_dir: &Path) -> Result<bool>;  // is this profile active?
  fn check_health(&self, profile_dir: &Path) -> Result<HealthStatus>;  // are creds valid?
}
```

**Claude:** Copies `.credentials.json`, `statsig`, `statsig_metadata`, `home_.claude.json` from `~/.claude/`. Writes credentials to macOS Keychain (`security` CLI, service `"Claude Code-credentials"`). Health check: verify `.credentials.json` exists and contains a non-expired `accessToken` (check `expiresAt` field).

**Gemini:** Credentials managed via `gcloud` auth or Gemini's own auth. Credential files TBD — need to investigate `~/.gemini/` structure. Health check: run `gemini --version` or check for valid auth token in Gemini's config. Import/restore may involve `gcloud auth` commands rather than file copying.

This is a known gap for Gemini — Phase 1 implementation should investigate and document the actual credential storage before building the Gemini credential manager.

### 1.2 Project-Directory Defaults

Stored in `~/.homard/project-defaults.json`:

```json
{
  "/Users/jwgrogan/GitHub/homard": "Work Claude",
  "/Users/jwgrogan/GitHub/other-repo": "Personal Gemini"
}
```

When spawning a session for a directory, the saved profile is pre-selected. User can override and is asked whether to update the default.

### 1.3 Profile Switcher UX

**Bottom-left indicator** replaces current nav sidebar profile display:
- Shows: provider icon + profile name + status dot (green = healthy, yellow = expiring, red = expired)
- Click expands an upward popover

**Popover contents:**
- Profiles grouped by provider section (Claude, Gemini)
- Each entry: name, email, usage indicator (tokens/requests today, rate limit status if available)
- Active profile highlighted
- Click to switch
- `+` button at bottom to add new profile
  - Inline form: name, provider select
  - Triggers the CLI's login command in a terminal (`claude login`, `gemini auth login`)
- Popover dismisses on click-outside

**Profile management** (deeper actions) remains in Settings tab: delete, rename, view detailed usage history, re-authenticate.

### 1.4 OAuth Health Monitoring

On app launch and every 5 minutes, validate credentials per profile:
- Attempt a lightweight auth check per provider (provider-specific)
- Status reflected as colored dot on the profile indicator
- If expired: yellow dot, "Re-authenticate" action in the popover opens terminal with the CLI's login command
- No silent failures — status is always visible

### 1.5 Remove claude-switch

`claude-switch` (installed at `/usr/local/bin/claude-switch` via Homebrew) is replaced by homard's profile manager.

- On first launch, detect if `claude-switch` is installed
- Offer to auto-import any profiles it created
- Suggest `brew uninstall claude-switch`
- homard's profile manager handles all credential switching going forward

### 1.6 Session Spawning

**Flow:**

1. User clicks "New Session" (prominent, top of Sessions page)
2. Form shows:
   - **Directory picker** — recent dirs + browse
   - **Profile selector** — pre-filled from project default, dropdown to override
   - **Optional: initial prompt** — passed as `-p` or positional arg
   - **Optional: agent** — select from available agents for the chosen CLI
3. homard opens a real terminal window:
   - Detect installed terminal: iTerm, Warp, Ghostty, Kitty, Terminal.app (configurable in settings)
   - Command: `cd <dir> && claude` or `cd <dir> && gemini` (with flags as appropriate)
   - If initial prompt: append to command
4. homard records the session in SQLite
5. Session appears in the session list immediately

**Terminal launch mechanism:**
- **Terminal.app:** AppleScript via `osascript` — `tell application "Terminal" to do script "cd <dir> && <cmd>"`
- **iTerm:** AppleScript — `tell application "iTerm" to create window with default profile command "cd <dir> && <cmd>"`
- **Warp:** `open -a Warp <dir>` then run command (Warp supports URL schemes and CLI launch)
- **Ghostty/Kitty:** Direct CLI — `ghostty -e "cd <dir> && <cmd>"` / `kitty sh -c "cd <dir> && <cmd>"`
- **Fallback:** If preferred terminal not found, fall back to Terminal.app
- **Setting:** `~/.homard/config.json` → `"terminal": "iterm"` (auto-detected on first launch, user can change in settings)

**Session status detection:**
- Poll `terminal_pid` liveness every 5 seconds via `kill(pid, 0)` (signal 0 checks existence without sending a signal)
- If PID is gone: mark session as `Stopped`, record `ended_at`
- If `terminal_pid` is `None` (failed to capture): check if the CLI's session file has stopped updating for >60 seconds as a secondary signal

**Capturing CLI session ID:**
- **Claude:** Pass `--session-id <uuid>` when spawning — homard generates the ID, always knows it
- **Gemini:** Start a file watcher (`notify` crate) on `~/.gemini/tmp/*/chats/` *before* spawning the terminal. The first new `session-*.json` file that appears after spawn is matched to the session. Session ID is extracted from the filename (`session-<timestamp>-<id>.json`). If multiple files appear simultaneously (unlikely but possible), match by closest timestamp to spawn time. Fallback: if no session file is detected within 30 seconds, the session is tracked without a `cli_session_id` (resume will be unavailable for that session).

**Session data model (SQLite):**

```
Session {
  id: UUID,                    // homard's session ID
  cli_session_id: String?,     // the CLI's own session ID (for resume)
  profile_name: String,
  provider: String,            // "claude" | "gemini"
  directory: String,
  terminal_pid: u32?,
  status: Running | Stopped | Error | Killed,
  started_at: DateTime,
  ended_at: DateTime?,
  parent_session_id: UUID?,    // for resumed/forked sessions
  forked_from: UUID?,          // for forked sessions
}
```

**Session list view (primary page):**
- Cards or rows: provider icon, profile name, directory (short path), status badge, duration
- "Open Terminal" button per session (focuses/raises the terminal window)
- Running sessions at top, recent stopped sessions below
- Clicking a session opens the detail view (Phase 2: agent team tree)

### 1.7 Unified Local MCP Management

homard maintains a single source of truth for local MCP servers, synced to each CLI.

**Source of truth:** `~/.homard/mcp-servers.json`

```json
{
  "whatsapp": {
    "command": "npx",
    "args": ["whatsapp-mcp"],
    "type": "stdio"
  },
  "my-custom-server": {
    "url": "http://localhost:8080",
    "type": "http"
  }
}
```

**Sync targets:**
- Claude: `~/.claude/settings.json` → `mcpServers` field
- Gemini: Gemini's MCP config location (to be verified — likely `~/.gemini/settings.json` or via `gemini mcp` CLI)
- Future CLIs: add a sync adapter per provider

**Sync behavior:**
- Sync on change (when user adds/removes/edits in homard) and on app launch (reconcile)
- homard is the authority for local MCPs
- If external drift detected: prompt "MCP config for [CLI] was modified externally. Adopt changes or overwrite?" — "Adopt" imports external changes into homard's config, then syncs outward to all providers. "Overwrite" pushes homard's config to the CLI. Unknown keys in provider configs are preserved during sync (only homard-managed server entries are touched).

**MCP settings panel:**
1. **Local MCP Servers** (managed by homard) — add/edit/remove, shows sync status per provider
2. **Cloud-Connected Services** (read-only, per CLI) — informational display of what's connected via Claude.ai, Gemini extensions, etc.

### 1.8 Quick Fixes

**Tray icon:**
- Add `.icon()` call to `TrayIconBuilder` in `tray.rs`
- Use a 22x22 monochrome template PNG suitable for macOS menu bar
- Reference icon file from `src-tauri/icons/`

**Bypass permissions:**
- Claude Code CLI reads `"defaultMode"` at the top level of `settings.json`, not `permissions.bypassPermissions`. The current implementation writes to the wrong location.
- Remove `bypass_permissions: bool` from `PermissionsConfig`
- Add `default_mode: Option<String>` to top-level `ClaudeSettings`, serialized as `"defaultMode"`
- `set_bypass_permissions(true)` → `set_default_mode(Some("bypassPermissions".into()))`
- Unsetting removes the key entirely
- Update frontend toggle and tests

**MCP visibility:**
- Display `enabledMcpjsonServers` (already in struct, not shown)
- Show three sections: local (homard-managed), plugins (`enabledPlugins`), cloud services (`enabledMcpjsonServers` + permission-pattern extraction as fallback)

---

## Phase 2 — Mission Control

### 2.1 Agent Team Tree

The core value-add: a tree view of what agents are doing within a session.

**Tree structure:**

```
Session: "homard" (Claude, ~/GitHub/homard)
├─ Main Agent (working)
│  ├─ current: "Editing src/pages/Sessions.tsx"
│  └─ files: [Sessions.tsx, store.ts]
├─ Explore Agent "codebase-search" (done)
│  └─ result: "Found 3 matching files"
├─ Team Implementer "auth-module" (working)
│  ├─ current: "Writing tests for OAuth flow"
│  └─ files: [auth.rs, auth_test.rs]
└─ Team Implementer "profile-ui" (working)
   ├─ current: "Building profile dropdown component"
   └─ files: [ProfileDropdown.tsx]
```

### 2.2 Session File Monitoring

**Implementation:**
- Rust backend: `SessionMonitor` struct per active session
- Uses `notify` crate (file watcher) on the session file
- On change: read new lines/diff, parse events, extract agent tree state
- Emits Tauri events: `session-tree-update:{session_id}` with current tree snapshot
- Frontend: React component subscribes, renders tree with status badges and file lists

**Per-provider parsing:**

**Claude Code** — tail `~/.claude/projects/<project-hash>/<session-id>.jsonl`:
- Scan for `tool_use` blocks where tool name is `Agent` or `SendMessage` — these define tree edges
- Track `toolUseID` → agent mapping via `parentToolUseID` for hierarchy
- Tool results update agent status to "done"
- Assistant message `text` content provides "current activity" label

**Gemini CLI** — tail `~/.gemini/tmp/<user>/chats/session-<id>.json`:
- Scan `toolCalls` entries within messages
- `displayName` provides agent label, `description` provides task context
- `status` field gives completion state
- Tree is flatter (tool calls within messages) but named agents (subagents) are still visible

### 2.3 Parse Error Handling & Graceful Degradation

**JSONL (Claude):** Read only complete lines (lines ending with `\n`). Ignore incomplete trailing data — the CLI will finish writing the line. If a complete line fails to parse as JSON, skip it and log a warning.

**JSON (Gemini):** The session file is a single JSON document that gets rewritten as the conversation progresses. If parse fails (partial write in progress), retry after 500ms, up to 3 attempts. On persistent failure, fall back to "Session running" status.

**General degradation:** If a provider's session file format is unreadable, changes, or is unavailable:
- Tree view shows "Session running" with provider icon and status — no agent breakdown
- No errors, no crashes
- Provider parser can be updated independently without affecting other providers

### 2.4 Session Detail View

Clicking a session in the list opens a detail panel:
- Agent team tree (live-updating for running sessions)
- Session metadata: provider, profile, directory, duration, CLI session ID
- "Open Terminal" button
- "Resume" button (Phase 3, grayed out until implemented)
- Files touched (aggregated from all agents)

---

## Phase 3 — Power Features

### 3.1 Session Resume

**Flow:**
1. Stopped sessions show a "Resume" button in the session list
2. Click opens a new terminal window with the resume command:
   - Claude: `cd <dir> && claude --resume <session-id>`
   - Gemini: `cd <dir> && gemini --resume <session-id>`
3. homard creates a new Session record with `parent_session_id` pointing to the original, same `cli_session_id`
4. Agent tree monitor picks up the resumed session automatically

### 3.2 Session Fork

- "Fork" button next to Resume
- Claude: spawns with `--resume <id> --fork-session`
- Gemini: spawns a new session (fork semantics TBD based on Gemini CLI support)
- Creates an independent Session record with `forked_from` reference

### 3.3 Session History

Below running sessions, a searchable/filterable history:
- Filters: by directory, by provider, by date range
- Each row: provider icon, directory, profile, start time, duration, status, summary (first prompt)
- Click to see agent tree snapshot (read from stored session file if still available)
- Scoped to project directory by default, toggle to show all

**Data retention:** homard stores metadata only (SQLite). The CLI owns conversation data. If a CLI's session file is cleaned up, homard shows "Session data unavailable" for the tree view.

### 3.4 Dock/Tray Hybrid

- Default: `NSApplicationActivationPolicy::Accessory` (tray only, background manager)
- When main window opens: switch to `Regular` (appears in dock, Cmd+Tab accessible)
- When main window closes: switch back to `Accessory`
- Tauri exposes this via `app.set_activation_policy()` on macOS
- Optional setting: "Always show in dock" for users who prefer it

---

## Files Affected

### New Files
- `~/.homard/mcp-servers.json` — unified MCP config
- `~/.homard/project-defaults.json` — directory → profile mapping
- `src-tauri/src/commands/mcp_sync.rs` — MCP sync logic per provider
- `src-tauri/src/session_monitor.rs` — file watcher + parser for agent tree
- `src-tauri/src/providers/mod.rs` — provider abstraction
- `src-tauri/src/providers/claude.rs` — Claude session parser
- `src-tauri/src/providers/gemini.rs` — Gemini session parser
- `src/components/ProfileSwitcher.tsx` — bottom-left dropdown
- `src/components/AgentTree.tsx` — tree visualization component
- `src/components/SessionDetail.tsx` — session detail panel
- `src-tauri/icons/tray-icon.png` — 22x22 menu bar icon
- `crates/homard-core/src/provider.rs` — provider types and traits

### Modified Files
- `crates/homard-core/src/profile.rs` — add `provider` field, project defaults
- `crates/homard-core/src/settings.rs` — fix bypass permissions (`defaultMode`), MCP visibility
- `crates/homard-core/src/types.rs` — Session struct updates, Provider enum
- `crates/homard-core/src/store.rs` — session table schema updates
- `src-tauri/src/tray.rs` — add `.icon()` call
- `src-tauri/src/lib.rs` — register new commands, session monitor setup
- `src-tauri/src/commands/profile.rs` — provider-aware profile operations
- `src-tauri/src/commands/process.rs` — terminal spawning instead of headless
- `src-tauri/src/commands/settings.rs` — bypass permissions fix, MCP sync
- `src/App.tsx` — profile switcher in nav, updated routing
- `src/pages/Sessions.tsx` — redesigned session list + detail view
- `src/components/settings/McpServersPanel.tsx` — unified MCP UI
- `src/components/settings/ProfilesPanel.tsx` — management actions (deeper settings)
- `src/components/QuickPrompt.tsx` — integrate with new session spawning
- `src/lib/types.ts` — updated TypeScript interfaces
- `src/lib/store.ts` — updated Zustand stores
- `src/lib/tauri.ts` — new invoke wrappers

### Removed
- `claude-switch` dependency (external, `brew uninstall`)
- `bypass_permissions` field from `PermissionsConfig`
