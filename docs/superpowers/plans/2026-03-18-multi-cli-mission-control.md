# Multi-CLI Mission Control Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform arcctl from a Claude-only menu bar app into a multi-CLI mission control that spawns terminal sessions, monitors agent teams, manages profiles across providers, and unifies MCP configuration.

**Architecture:** Log Tailer approach — arcctl reads CLI session files (Claude JSONL, Gemini JSON) via file watchers, tracks metadata in SQLite, and opens real terminal windows. No terminal emulation. Provider abstraction via Rust traits/enums enables extensibility.

**Tech Stack:** Rust (Tauri 2, rusqlite, notify, tokio), React 18 + TypeScript + Zustand + Tailwind CSS 4

**Spec:** `docs/superpowers/specs/2026-03-18-multi-cli-mission-control-design.md`

---

## File Structure

### New Files (Phase 1)
- `crates/arcctl-core/src/provider.rs` — Provider enum, ProviderConfig, credential trait
- `crates/arcctl-core/src/terminal.rs` — Terminal app detection and launch
- `crates/arcctl-core/src/mcp_sync.rs` — Unified MCP sync logic
- `crates/arcctl-core/src/project_defaults.rs` — Directory → profile mapping
- `src-tauri/src/commands/mcp_sync.rs` — Tauri commands for MCP sync
- `src-tauri/icons/tray-icon.png` — 22x22 monochrome menu bar icon
- `src/components/ProfileSwitcher.tsx` — Bottom-left dropdown with usage
- `src/components/NewSessionModal.tsx` — Session spawning form

### New Files (Phase 2)
- `crates/arcctl-core/src/session_monitor.rs` — File watcher + agent tree extraction
- `crates/arcctl-core/src/parsers/mod.rs` — Parser trait
- `crates/arcctl-core/src/parsers/claude.rs` — Claude JSONL session parser
- `crates/arcctl-core/src/parsers/gemini.rs` — Gemini JSON session parser
- `src/components/AgentTree.tsx` — Tree visualization component
- `src/components/SessionDetail.tsx` — Session detail panel with tree

### Modified Files
- `crates/arcctl-core/Cargo.toml` — Add `notify` crate
- `crates/arcctl-core/src/types.rs` — Session struct, Provider enum, SessionStatus
- `crates/arcctl-core/src/profile.rs` — Add provider field, credential trait
- `crates/arcctl-core/src/store.rs` — Migrate runs→sessions table, new queries
- `crates/arcctl-core/src/settings.rs` — Fix bypass permissions (defaultMode)
- `crates/arcctl-core/src/process.rs` — Terminal spawning replaces headless
- `src-tauri/src/tray.rs` — Add icon
- `src-tauri/src/state.rs` — Replace ProcessRegistry with session tracking
- `src-tauri/src/lib.rs` — Register new commands, session monitor setup
- `src-tauri/src/commands/process.rs` — Terminal-based session spawning
- `src-tauri/src/commands/profile.rs` — Provider-aware profiles
- `src-tauri/src/commands/settings.rs` — defaultMode fix
- `src/lib/types.ts` — Updated interfaces
- `src/lib/tauri.ts` — New invoke wrappers
- `src/lib/store.ts` — Updated stores
- `src/App.tsx` — Profile switcher, sessions as primary page
- `src/pages/Sessions.tsx` — Redesigned session list
- `src/components/settings/McpServersPanel.tsx` — Unified MCP UI
- `src/components/settings/PermissionsPanel.tsx` — defaultMode toggle

---

## Phase 1: Foundation

### Task 1: Fix Tray Icon

**Files:**
- Modify: `src-tauri/src/tray.rs`
- Create: `src-tauri/icons/tray-icon.png`

- [ ] **Step 1: Create a 22x22 monochrome tray icon**

Create a simple 22x22 PNG. For now, use the existing `icon.png` resized, or generate a minimal one. The icon needs to be a template image (monochrome with alpha) for macOS menu bar.

```bash
# Use sips to resize the existing icon as a placeholder
sips -z 22 22 src-tauri/icons/icon.png --out src-tauri/icons/tray-icon.png
```

- [ ] **Step 2: Add .icon() to TrayIconBuilder**

In `src-tauri/src/tray.rs`, add the icon to the builder:

```rust
use tauri::image::Image;

pub fn create_tray(app: &App) -> tauri::Result<()> {
    let open = MenuItemBuilder::new("Open arcctl")
        .id("open")
        .build(app)?;
    let quit = MenuItemBuilder::new("Quit arcctl")
        .id("quit")
        .build(app)?;

    let menu = MenuBuilder::new(app).item(&open).item(&quit).build()?;

    let icon = Image::from_bytes(include_bytes!("../icons/tray-icon.png"))?;

    TrayIconBuilder::new()
        .icon(icon)
        .icon_as_template(true)
        .tooltip("arcctl")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "quit" => {
                app.exit(0);
            }
            "open" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}
```

- [ ] **Step 3: Build and verify tray icon appears**

```bash
cd /Users/jwgrogan/GitHub/arcctl && cargo build 2>&1 | tail -5
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/tray.rs src-tauri/icons/tray-icon.png
git commit -m "fix: add tray icon to menu bar"
```

---

### Task 2: Fix Bypass Permissions (defaultMode)

**Files:**
- Modify: `crates/arcctl-core/src/settings.rs`
- Modify: `src-tauri/src/commands/settings.rs`
- Modify: `src/lib/types.ts`
- Modify: `src/components/settings/PermissionsPanel.tsx`
- Modify: `src/lib/store.ts`
- Modify: `src/lib/tauri.ts`

- [ ] **Step 1: Write failing test for defaultMode serialization**

In `crates/arcctl-core/src/settings.rs`, add test:

```rust
#[test]
fn test_default_mode_serialization() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("settings.json");

    let mut s = ClaudeSettings::default();
    s.set_default_mode(Some("bypassPermissions".to_string()));
    s.save(&path).unwrap();

    let json = std::fs::read_to_string(&path).unwrap();
    assert!(json.contains(r#""defaultMode": "bypassPermissions""#),
        "defaultMode should be a top-level key, got: {}", json);
    assert!(!json.contains(r#""bypassPermissions": true"#),
        "should not have old nested bypassPermissions");

    let loaded = ClaudeSettings::load(&path).unwrap();
    assert_eq!(loaded.default_mode.as_deref(), Some("bypassPermissions"));
}

#[test]
fn test_default_mode_none_omitted() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("settings.json");

    let s = ClaudeSettings::default();
    s.save(&path).unwrap();

    let json = std::fs::read_to_string(&path).unwrap();
    assert!(!json.contains("defaultMode"), "defaultMode should be omitted when None");
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cd /Users/jwgrogan/GitHub/arcctl && cargo test -p arcctl-core test_default_mode 2>&1 | tail -10
```

Expected: FAIL — `set_default_mode` and `default_mode` field don't exist yet.

- [ ] **Step 3: Implement defaultMode in settings.rs**

In `crates/arcctl-core/src/settings.rs`:

1. Add `default_mode` field to `ClaudeSettings`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClaudeSettings {
    #[serde(default)]
    pub permissions: PermissionsConfig,
    #[serde(default, rename = "defaultMode", skip_serializing_if = "Option::is_none")]
    pub default_mode: Option<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    // ... rest unchanged
}
```

2. Remove `bypass_permissions` from `PermissionsConfig`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PermissionsConfig {
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub deny: Vec<String>,
}
```

3. Replace `set_bypass_permissions` with `set_default_mode`:

```rust
pub fn set_default_mode(&mut self, mode: Option<String>) {
    self.default_mode = mode;
}
```

4. Remove `set_bypass_permissions` method.

5. Update existing tests that reference `bypass_permissions` — remove `test_set_bypass_permissions` and `test_bypass_permissions_camel_case_rename`, and update `test_round_trip_save_load` to use `set_default_mode`.

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd /Users/jwgrogan/GitHub/arcctl && cargo test -p arcctl-core 2>&1 | tail -10
```

- [ ] **Step 5: Update Tauri command**

In `src-tauri/src/commands/settings.rs`, rename the command:

```rust
#[tauri::command]
pub fn set_default_mode(
    state: State<'_, AppState>,
    scope: String,
    mode: Option<String>,
    project_dir: Option<String>,
) -> Result<(), String> {
    let path = resolve_settings_path(&state, &scope, project_dir.as_deref())?;
    let mut settings = ClaudeSettings::load(&path).map_err(|e| e.to_string())?;
    settings.set_default_mode(mode);
    settings.save(&path).map_err(|e| e.to_string())?;
    Ok(())
}
```

Remove the old `set_bypass_permissions` command.

In `src-tauri/src/lib.rs`, update the handler registration:
- Replace `commands::settings::set_bypass_permissions` with `commands::settings::set_default_mode`

- [ ] **Step 6: Update frontend types and API**

In `src/lib/types.ts`, update `PermissionsConfig`:

```typescript
export interface PermissionsConfig {
  allow: string[];
  deny: string[];
}

export interface ClaudeSettings {
  permissions: PermissionsConfig;
  defaultMode?: string;
  env: Record<string, string>;
  mcpServers: Record<string, McpServerConfig>;
  enabledPlugins?: Record<string, boolean>;
  enabledMcpjsonServers?: string[];
  [key: string]: unknown;
}
```

In `src/lib/tauri.ts`, update:

```typescript
export async function setDefaultMode(scope: string, mode: string | null, projectDir?: string): Promise<void> {
  return invoke("set_default_mode", { scope, mode, projectDir });
}
```

Remove `setBypassPermissions`.

In `src/lib/store.ts`, update `SettingsActions` and `useSettingsStore`:
- Replace `setBypassPermissions` with `setDefaultMode`

```typescript
setDefaultMode: async (mode: string | null) => {
  const { scope, projectDir } = get();
  await api.setDefaultMode(scope, mode, projectDir ?? undefined);
  await get().fetchSettings();
},
```

- [ ] **Step 7: Update PermissionsPanel.tsx toggle**

In `src/components/settings/PermissionsPanel.tsx`, update the bypass toggle to use `defaultMode`:

```typescript
const isBypassed = settings?.defaultMode === "bypassPermissions";

// Toggle handler:
onClick={() => setDefaultMode(isBypassed ? null : "bypassPermissions")}
```

Update the conditional rendering to check `settings?.defaultMode === "bypassPermissions"` instead of `perms.bypassPermissions`.

- [ ] **Step 8: Build frontend and verify**

```bash
cd /Users/jwgrogan/GitHub/arcctl && npm run build 2>&1 | tail -5
```

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "fix: use top-level defaultMode instead of nested bypassPermissions"
```

---

### Task 3: Provider Types and Enum

**Files:**
- Create: `crates/arcctl-core/src/provider.rs`
- Modify: `crates/arcctl-core/src/lib.rs` (add `pub mod provider;`)

- [ ] **Step 1: Write test for Provider**

Create `crates/arcctl-core/src/provider.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderId {
    Claude,
    Gemini,
}

impl ProviderId {
    pub fn cli_command(&self) -> &'static str {
        match self {
            ProviderId::Claude => "claude",
            ProviderId::Gemini => "gemini",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            ProviderId::Claude => "Claude Code",
            ProviderId::Gemini => "Gemini CLI",
        }
    }

    pub fn supports_session_id_flag(&self) -> bool {
        match self {
            ProviderId::Claude => true,   // --session-id <uuid>
            ProviderId::Gemini => false,
        }
    }

    pub fn supports_resume(&self) -> bool {
        match self {
            ProviderId::Claude => true,
            ProviderId::Gemini => true,
        }
    }

    pub fn resume_flag(&self) -> &'static str {
        match self {
            ProviderId::Claude => "--resume",
            ProviderId::Gemini => "--resume",
        }
    }

    /// Directory where this CLI stores session files.
    pub fn session_dir(&self) -> Option<PathBuf> {
        let home = dirs::home_dir()?;
        match self {
            ProviderId::Claude => Some(home.join(".claude").join("projects")),
            ProviderId::Gemini => Some(home.join(".gemini").join("tmp")),
        }
    }

    /// Check if the CLI is installed by looking for the binary.
    pub fn is_installed(&self) -> bool {
        which::which(self.cli_command()).is_ok()
    }
}

impl std::fmt::Display for ProviderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_cli_commands() {
        assert_eq!(ProviderId::Claude.cli_command(), "claude");
        assert_eq!(ProviderId::Gemini.cli_command(), "gemini");
    }

    #[test]
    fn test_provider_serialization() {
        let claude = ProviderId::Claude;
        let json = serde_json::to_string(&claude).unwrap();
        assert_eq!(json, r#""claude""#);

        let gemini: ProviderId = serde_json::from_str(r#""gemini""#).unwrap();
        assert_eq!(gemini, ProviderId::Gemini);
    }

    #[test]
    fn test_session_dir_returns_path() {
        // Just verify it returns Some, actual path depends on environment
        assert!(ProviderId::Claude.session_dir().is_some());
        assert!(ProviderId::Gemini.session_dir().is_some());
    }
}
```

- [ ] **Step 2: Add `which` crate and module declaration**

In `crates/arcctl-core/Cargo.toml`, add:
```toml
which = "7"
```

In `crates/arcctl-core/src/lib.rs`, add `pub mod provider;`

- [ ] **Step 3: Run tests**

```bash
cd /Users/jwgrogan/GitHub/arcctl && cargo test -p arcctl-core provider 2>&1 | tail -10
```

- [ ] **Step 4: Commit**

```bash
git add crates/arcctl-core/src/provider.rs crates/arcctl-core/src/lib.rs crates/arcctl-core/Cargo.toml
git commit -m "feat: add Provider enum with CLI metadata"
```

---

### Task 4: Update Profile Model with Provider

**Files:**
- Modify: `crates/arcctl-core/src/types.rs`
- Modify: `crates/arcctl-core/src/profile.rs`
- Modify: `src-tauri/src/commands/profile.rs`

- [ ] **Step 1: Write test for provider-aware Profile**

In `crates/arcctl-core/src/types.rs`, update Profile:

```rust
use crate::provider::ProviderId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub provider: ProviderId,
    pub email: Option<String>,
    pub is_active: bool,
}
```

In `crates/arcctl-core/src/profile.rs` tests, update `test_list_profiles`:

```rust
#[test]
fn test_list_profiles_with_provider() {
    let tmp = TempDir::new().unwrap();
    let mgr = make_manager(&tmp);

    write_credentials(&mgr.claude_dir, "alice@example.com");
    mgr.import("alice").unwrap();

    let profiles = mgr.list().unwrap();
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].provider, ProviderId::Claude);
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cd /Users/jwgrogan/GitHub/arcctl && cargo test -p arcctl-core test_list_profiles_with_provider 2>&1 | tail -10
```

- [ ] **Step 3: Update ProfileManager to include provider**

The current `ProfileManager` is Claude-only. For now, all imported profiles get `ProviderId::Claude`. We'll store a `provider.json` in each profile dir to persist the provider choice.

In `crates/arcctl-core/src/profile.rs`:

1. Add import: `use crate::provider::ProviderId;`
2. In `import()`, also write a `provider.json` file:

```rust
let provider_path = dest_dir.join("provider.json");
let provider_json = serde_json::to_string(&ProviderId::Claude).unwrap();
std::fs::write(&provider_path, provider_json)?;
```

3. In `list()`, read provider from `provider.json` with fallback to `Claude`:

```rust
let provider = std::fs::read_to_string(path.join("provider.json"))
    .ok()
    .and_then(|s| serde_json::from_str(&s).ok())
    .unwrap_or(ProviderId::Claude);

profiles.push(Profile {
    name,
    provider,
    email,
    is_active: false,
});
```

- [ ] **Step 4: Run all profile tests**

```bash
cd /Users/jwgrogan/GitHub/arcctl && cargo test -p arcctl-core profile 2>&1 | tail -15
```

Fix any compilation errors from the Profile struct change (add `provider: ProviderId::Claude` to test helpers).

- [ ] **Step 5: Update Tauri profile command and frontend types**

In `src-tauri/src/commands/profile.rs`, ensure the updated `Profile` struct is returned correctly (it's already serialized via serde).

In `src/lib/types.ts`:

```typescript
export interface Profile {
  name: string;
  provider: "claude" | "gemini";
  email: string | null;
  is_active: boolean;
}
```

- [ ] **Step 6: Build to verify compilation**

```bash
cd /Users/jwgrogan/GitHub/arcctl && cargo build 2>&1 | tail -5 && npm run build 2>&1 | tail -5
```

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: add provider field to Profile model"
```

---

### Task 5: Terminal Launcher

**Files:**
- Create: `crates/arcctl-core/src/terminal.rs`

- [ ] **Step 1: Write tests for terminal detection**

Create `crates/arcctl-core/src/terminal.rs`:

```rust
use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::error::{ArcctlError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TerminalApp {
    Iterm,
    Warp,
    Ghostty,
    Kitty,
    TerminalApp,
}

impl TerminalApp {
    /// Detect installed terminals, return in preference order.
    pub fn detect_installed() -> Vec<TerminalApp> {
        let mut found = Vec::new();
        if Path::new("/Applications/iTerm.app").exists() {
            found.push(TerminalApp::Iterm);
        }
        if Path::new("/Applications/Ghostty.app").exists() {
            found.push(TerminalApp::Ghostty);
        }
        if Path::new("/Applications/Warp.app").exists() {
            found.push(TerminalApp::Warp);
        }
        if Path::new("/Applications/kitty.app").exists() {
            found.push(TerminalApp::Kitty);
        }
        // Terminal.app is always available on macOS
        found.push(TerminalApp::TerminalApp);
        found
    }

    /// Launch a command in a new terminal window.
    /// Returns the PID of the terminal process (best effort).
    pub fn launch(&self, shell_command: &str) -> Result<Option<u32>> {
        match self {
            TerminalApp::TerminalApp => {
                let script = format!(
                    r#"tell application "Terminal"
                        activate
                        do script "{}"
                    end tell"#,
                    shell_command.replace('\\', "\\\\").replace('"', "\\\"")
                );
                let output = Command::new("osascript")
                    .arg("-e")
                    .arg(&script)
                    .output()
                    .map_err(ArcctlError::Io)?;
                if !output.status.success() {
                    return Err(ArcctlError::Terminal(
                        String::from_utf8_lossy(&output.stderr).to_string()
                    ));
                }
                Ok(None) // AppleScript doesn't easily return PID
            }
            TerminalApp::Iterm => {
                let script = format!(
                    r#"tell application "iTerm"
                        activate
                        set newWindow to (create window with default profile command "{}")
                    end tell"#,
                    shell_command.replace('\\', "\\\\").replace('"', "\\\"")
                );
                let output = Command::new("osascript")
                    .arg("-e")
                    .arg(&script)
                    .output()
                    .map_err(ArcctlError::Io)?;
                if !output.status.success() {
                    return Err(ArcctlError::Terminal(
                        String::from_utf8_lossy(&output.stderr).to_string()
                    ));
                }
                Ok(None)
            }
            TerminalApp::Ghostty => {
                let child = Command::new("open")
                    .args(["-a", "Ghostty", "--args", "-e", shell_command])
                    .spawn()
                    .map_err(ArcctlError::Io)?;
                Ok(child.id().map(|id| id))
            }
            TerminalApp::Kitty => {
                let child = Command::new("open")
                    .args(["-a", "kitty", "--args", "sh", "-c", shell_command])
                    .spawn()
                    .map_err(ArcctlError::Io)?;
                Ok(child.id().map(|id| id))
            }
            TerminalApp::Warp => {
                // Warp doesn't have great AppleScript support yet
                // Fall back to open -a with the command
                let child = Command::new("open")
                    .args(["-a", "Warp"])
                    .spawn()
                    .map_err(ArcctlError::Io)?;
                Ok(child.id().map(|id| id))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_installed_always_includes_terminal_app() {
        let found = TerminalApp::detect_installed();
        assert!(!found.is_empty());
        assert_eq!(*found.last().unwrap(), TerminalApp::TerminalApp);
    }

    #[test]
    fn test_terminal_serialization() {
        let t = TerminalApp::Iterm;
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(json, r#""iterm""#);
    }
}
```

- [ ] **Step 2: Add Terminal error variant and module**

In `crates/arcctl-core/src/error.rs`, add:
```rust
Terminal(String),
```

In `crates/arcctl-core/src/lib.rs`, add `pub mod terminal;`

- [ ] **Step 3: Run tests**

```bash
cd /Users/jwgrogan/GitHub/arcctl && cargo test -p arcctl-core terminal 2>&1 | tail -10
```

- [ ] **Step 4: Commit**

```bash
git add crates/arcctl-core/src/terminal.rs crates/arcctl-core/src/lib.rs crates/arcctl-core/src/error.rs
git commit -m "feat: add terminal app detection and launcher"
```

---

### Task 6: Project Directory Defaults

**Files:**
- Create: `crates/arcctl-core/src/project_defaults.rs`

- [ ] **Step 1: Write tests**

```rust
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectDefaults {
    #[serde(flatten)]
    pub mappings: HashMap<String, String>,  // directory path -> profile name
}

impl ProjectDefaults {
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(contents) => {
                if contents.trim().is_empty() {
                    return Ok(Self::default());
                }
                Ok(serde_json::from_str(&contents)?)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&self.mappings)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn get_profile(&self, directory: &str) -> Option<&str> {
        self.mappings.get(directory).map(|s| s.as_str())
    }

    pub fn set_profile(&mut self, directory: String, profile: String) {
        self.mappings.insert(directory, profile);
    }

    pub fn remove(&mut self, directory: &str) {
        self.mappings.remove(directory);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_round_trip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("project-defaults.json");

        let mut defaults = ProjectDefaults::default();
        defaults.set_profile("/Users/test/repo".to_string(), "Work Claude".to_string());
        defaults.save(&path).unwrap();

        let loaded = ProjectDefaults::load(&path).unwrap();
        assert_eq!(loaded.get_profile("/Users/test/repo"), Some("Work Claude"));
    }

    #[test]
    fn test_missing_file_returns_empty() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nonexistent.json");
        let defaults = ProjectDefaults::load(&path).unwrap();
        assert!(defaults.mappings.is_empty());
    }
}
```

- [ ] **Step 2: Add module declaration**

In `crates/arcctl-core/src/lib.rs`, add `pub mod project_defaults;`

- [ ] **Step 3: Run tests**

```bash
cd /Users/jwgrogan/GitHub/arcctl && cargo test -p arcctl-core project_defaults 2>&1 | tail -10
```

- [ ] **Step 4: Commit**

```bash
git add crates/arcctl-core/src/project_defaults.rs crates/arcctl-core/src/lib.rs
git commit -m "feat: add project directory defaults (directory → profile mapping)"
```

---

### Task 7: Database Migration (runs → sessions)

**Files:**
- Modify: `crates/arcctl-core/src/types.rs`
- Modify: `crates/arcctl-core/src/store.rs`

- [ ] **Step 1: Write test for new Session type and store**

In `crates/arcctl-core/src/types.rs`, add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Running,
    Stopped,
    Error,
    Killed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub cli_session_id: Option<String>,
    pub profile_name: Option<String>,
    pub provider: String,
    pub directory: Option<String>,
    pub terminal_pid: Option<u32>,
    pub trigger: Trigger,
    pub status: SessionStatus,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<i64>,
    pub error_message: Option<String>,
    pub agent: Option<String>,
    pub parent_session_id: Option<String>,
    pub forked_from: Option<String>,
}
```

In `crates/arcctl-core/src/store.rs` tests:

```rust
#[test]
fn test_insert_and_get_session() {
    let store = Store::open_in_memory().unwrap();
    let session = Session {
        id: "sess-001".to_string(),
        cli_session_id: Some("cli-abc".to_string()),
        profile_name: Some("Work Claude".to_string()),
        provider: "claude".to_string(),
        directory: Some("/tmp/repo".to_string()),
        terminal_pid: Some(12345),
        trigger: Trigger::Manual,
        status: SessionStatus::Running,
        started_at: Utc::now(),
        ended_at: None,
        duration_ms: None,
        error_message: None,
        agent: None,
        parent_session_id: None,
        forked_from: None,
    };
    store.insert_session(&session).unwrap();

    let fetched = store.get_session("sess-001").unwrap().unwrap();
    assert_eq!(fetched.id, "sess-001");
    assert_eq!(fetched.cli_session_id.as_deref(), Some("cli-abc"));
    assert_eq!(fetched.provider, "claude");
    assert_eq!(fetched.status, SessionStatus::Running);
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cd /Users/jwgrogan/GitHub/arcctl && cargo test -p arcctl-core test_insert_and_get_session 2>&1 | tail -10
```

- [ ] **Step 3: Implement sessions table and migration**

In `store.rs`, update the `migrate()` method to add a `sessions` table alongside (not replacing) `runs` for backward compatibility:

```rust
pub fn migrate(&mut self) -> Result<()> {
    self.conn.execute_batch("
        -- Existing tables...
        CREATE TABLE IF NOT EXISTS runs ( ... );
        -- ... existing tables unchanged ...

        -- New sessions table
        CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            cli_session_id TEXT,
            profile_name TEXT,
            provider TEXT NOT NULL DEFAULT 'claude',
            directory TEXT,
            terminal_pid INTEGER,
            trigger TEXT NOT NULL DEFAULT 'manual',
            status TEXT NOT NULL DEFAULT 'running',
            started_at TEXT NOT NULL,
            ended_at TEXT,
            duration_ms INTEGER,
            error_message TEXT,
            agent TEXT,
            parent_session_id TEXT,
            forked_from TEXT
        );
    ")?;
    Ok(())
}
```

Add `insert_session`, `get_session`, `complete_session`, `list_sessions` methods following the same pattern as the existing `Run` methods.

- [ ] **Step 4: Run tests**

```bash
cd /Users/jwgrogan/GitHub/arcctl && cargo test -p arcctl-core store 2>&1 | tail -15
```

- [ ] **Step 5: Commit**

```bash
git add crates/arcctl-core/src/types.rs crates/arcctl-core/src/store.rs
git commit -m "feat: add sessions table and Session type for multi-CLI tracking"
```

---

### Task 8: Terminal-Based Session Spawning (Tauri Commands)

**Files:**
- Modify: `src-tauri/src/commands/process.rs`
- Modify: `src-tauri/src/state.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/lib/tauri.ts`
- Modify: `src/lib/types.ts`
- Modify: `src/lib/store.ts`

- [ ] **Step 1: Update AppState to remove ProcessRegistry and children**

In `src-tauri/src/state.rs`, replace `ProcessRegistry` and `children` with session-aware state:

```rust
use arcctl_core::config::{ArcctlConfig, ArcctlDirs};
use arcctl_core::store::Store;
use arcctl_core::terminal::TerminalApp;
use std::sync::Mutex;

pub struct AppState {
    pub store: Mutex<Store>,
    pub config: Mutex<ArcctlConfig>,
    pub dirs: ArcctlDirs,
    pub preferred_terminal: Mutex<Option<TerminalApp>>,
    pub telegram_poll_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    pub telegram_cancel: Mutex<Option<tokio_util::sync::CancellationToken>>,
}
```

- [ ] **Step 2: Rewrite spawn_session to open terminal**

In `src-tauri/src/commands/process.rs`:

```rust
use arcctl_core::provider::ProviderId;
use arcctl_core::terminal::TerminalApp;
use arcctl_core::types::{Session, SessionStatus, Trigger};
use chrono::Utc;
use tauri::State;
use uuid::Uuid;

use crate::state::AppState;

#[tauri::command]
pub fn spawn_session(
    state: State<'_, AppState>,
    directory: String,
    profile: Option<String>,
    provider: String,
    agent: Option<String>,
    prompt: Option<String>,
) -> Result<Session, String> {
    let session_id = Uuid::new_v4().to_string();

    let provider_id: ProviderId = serde_json::from_str(&format!("\"{}\"", provider))
        .map_err(|_| format!("Unknown provider: {}", provider))?;

    // Build the CLI command
    let mut cmd_parts = vec![format!("cd {} &&", shell_escape(&directory))];
    cmd_parts.push(provider_id.cli_command().to_string());

    if provider_id.supports_session_id_flag() {
        cmd_parts.push(format!("--session-id {}", session_id));
    }

    if let Some(ref a) = agent {
        cmd_parts.push(format!("--agent {}", a));
    }

    if let Some(ref p) = prompt {
        cmd_parts.push(format!("-p \"{}\"", p.replace('"', "\\\"")));
    }

    let shell_command = cmd_parts.join(" ");

    // Get preferred terminal or detect
    let terminal = {
        let pref = state.preferred_terminal.lock().map_err(|e| e.to_string())?;
        pref.clone().unwrap_or_else(|| {
            TerminalApp::detect_installed().into_iter().next()
                .unwrap_or(TerminalApp::TerminalApp)
        })
    };

    let terminal_pid = terminal.launch(&shell_command)
        .map_err(|e| e.to_string())?;

    // Record in database
    let session = Session {
        id: session_id,
        cli_session_id: if provider_id.supports_session_id_flag() {
            Some(session_id.clone())
        } else {
            None
        },
        profile_name: profile,
        provider: provider.clone(),
        directory: Some(directory),
        terminal_pid,
        trigger: Trigger::Manual,
        status: SessionStatus::Running,
        started_at: Utc::now(),
        ended_at: None,
        duration_ms: None,
        error_message: None,
        agent,
        parent_session_id: None,
        forked_from: None,
    };

    {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        store.insert_session(&session).map_err(|e| e.to_string())?;
    }

    Ok(session)
}

fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[tauri::command]
pub fn list_sessions(state: State<'_, AppState>) -> Result<Vec<Session>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.list_sessions(50, 0).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn kill_session(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;

    // Get the session to find its PID
    if let Some(session) = store.get_session(&session_id).map_err(|e| e.to_string())? {
        if let Some(pid) = session.terminal_pid {
            // Send SIGTERM to the terminal process
            unsafe {
                libc::kill(pid as libc::pid_t, libc::SIGTERM);
            }
        }
    }

    store.complete_session(&session_id, SessionStatus::Killed, None)
        .map_err(|e| e.to_string())?;
    Ok(())
}
```

- [ ] **Step 3: Update lib.rs AppState construction and command registration**

Remove `ProcessRegistry`, `children` from app state creation. Update invoke_handler to use new command signatures.

- [ ] **Step 4: Update frontend types, API, and store**

In `src/lib/types.ts`:

```typescript
export interface Session {
  id: string;
  cli_session_id: string | null;
  profile_name: string | null;
  provider: string;
  directory: string | null;
  terminal_pid: number | null;
  trigger: "manual" | "cron" | "telegram" | "email";
  status: "running" | "stopped" | "error" | "killed";
  started_at: string;
  ended_at: string | null;
  duration_ms: number | null;
  error_message: string | null;
  agent: string | null;
  parent_session_id: string | null;
  forked_from: string | null;
}
```

Update `src/lib/tauri.ts` and `src/lib/store.ts` to use `Session` instead of `SessionInfo` + `Run`.

- [ ] **Step 5: Build to verify**

```bash
cd /Users/jwgrogan/GitHub/arcctl && cargo build 2>&1 | tail -10 && npm run build 2>&1 | tail -5
```

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: terminal-based session spawning replaces headless process model"
```

---

### Task 9: Profile Switcher Component

**Files:**
- Create: `src/components/ProfileSwitcher.tsx`
- Modify: `src/App.tsx`

- [ ] **Step 1: Build the ProfileSwitcher component**

Create `src/components/ProfileSwitcher.tsx`:

```tsx
import { useState, useRef, useEffect } from "react";
import { useProfilesStore } from "../lib/store";
import type { Profile } from "../lib/types";

export default function ProfileSwitcher() {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const { profiles, fetchProfiles, switchProfile, importProfile } = useProfilesStore();
  const [importing, setImporting] = useState(false);
  const [newName, setNewName] = useState("");

  const activeProfile = profiles.find((p) => p.is_active) ?? null;

  // Close on click outside
  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
      }
    }
    if (open) document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, [open]);

  // Group profiles by provider
  const grouped = profiles.reduce<Record<string, Profile[]>>((acc, p) => {
    const key = p.provider;
    if (!acc[key]) acc[key] = [];
    acc[key].push(p);
    return acc;
  }, {});

  const handleImport = async () => {
    if (!newName.trim()) return;
    await importProfile(newName.trim());
    setNewName("");
    setImporting(false);
  };

  return (
    <div className="relative" ref={ref}>
      {/* Indicator button */}
      <button
        onClick={() => { setOpen(!open); fetchProfiles(); }}
        className="w-full text-left px-3 py-2 rounded text-xs text-zinc-400 hover:bg-zinc-800"
      >
        <div className="flex items-center gap-2">
          <span className={`w-2 h-2 rounded-full shrink-0 ${
            activeProfile ? "bg-green-500" : "bg-zinc-600"
          }`} />
          <span className="truncate">
            {activeProfile?.name ?? "No profile"}
          </span>
        </div>
        {activeProfile?.email && (
          <div className="text-zinc-500 text-xs mt-0.5 truncate pl-4">
            {activeProfile.email}
          </div>
        )}
      </button>

      {/* Popover */}
      {open && (
        <div className="absolute bottom-full left-0 mb-2 w-64 bg-zinc-800 border border-zinc-700 rounded-lg shadow-xl z-50 max-h-80 overflow-y-auto">
          {Object.entries(grouped).map(([provider, profs]) => (
            <div key={provider}>
              <div className="px-3 py-1.5 text-xs font-medium text-zinc-500 uppercase tracking-wide border-b border-zinc-700">
                {provider === "claude" ? "Claude Code" : provider === "gemini" ? "Gemini CLI" : provider}
              </div>
              {profs.map((p) => (
                <button
                  key={p.name}
                  onClick={async () => {
                    await switchProfile(p.name);
                    setOpen(false);
                  }}
                  className={`w-full text-left px-3 py-2 text-sm hover:bg-zinc-700 flex items-center gap-2 ${
                    p.is_active ? "bg-zinc-700/50" : ""
                  }`}
                >
                  <span className={`w-2 h-2 rounded-full shrink-0 ${
                    p.is_active ? "bg-green-500" : "bg-zinc-600"
                  }`} />
                  <div className="flex-1 min-w-0">
                    <div className="text-zinc-200 truncate">{p.name}</div>
                    {p.email && <div className="text-zinc-500 text-xs truncate">{p.email}</div>}
                  </div>
                </button>
              ))}
            </div>
          ))}

          {/* Add profile */}
          <div className="border-t border-zinc-700 p-2">
            {importing ? (
              <div className="flex gap-2">
                <input
                  value={newName}
                  onChange={(e) => setNewName(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && handleImport()}
                  placeholder="Profile name"
                  className="flex-1 px-2 py-1 text-xs bg-zinc-900 border border-zinc-600 rounded text-zinc-200"
                  autoFocus
                />
                <button
                  onClick={handleImport}
                  className="px-2 py-1 text-xs bg-blue-600 hover:bg-blue-500 rounded text-white"
                >
                  Save
                </button>
              </div>
            ) : (
              <button
                onClick={() => setImporting(true)}
                className="w-full text-center text-xs text-zinc-400 hover:text-zinc-200 py-1"
              >
                + Add Profile
              </button>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 2: Replace profile indicator in App.tsx with ProfileSwitcher**

In `src/App.tsx`:

```tsx
import ProfileSwitcher from "./components/ProfileSwitcher";

// Replace the bottom profile section in the nav:
<div className="pt-4 border-t border-zinc-700">
  <ProfileSwitcher />
</div>
```

Remove the old inline profile display and the direct `useProfilesStore` usage for active profile display from App.tsx.

- [ ] **Step 3: Build and verify**

```bash
cd /Users/jwgrogan/GitHub/arcctl && npm run build 2>&1 | tail -5
```

- [ ] **Step 4: Commit**

```bash
git add src/components/ProfileSwitcher.tsx src/App.tsx
git commit -m "feat: add profile switcher dropdown in bottom-left nav"
```

---

### Task 10: New Session Modal and Sessions Page Redesign

**Files:**
- Create: `src/components/NewSessionModal.tsx`
- Modify: `src/pages/Sessions.tsx`

- [ ] **Step 1: Create NewSessionModal**

Create `src/components/NewSessionModal.tsx` with:
- Directory picker (text input + browse button using Tauri dialog)
- Profile/provider selector dropdown (populated from profiles store)
- Optional initial prompt textarea
- Optional agent selector
- "Start Session" button that calls `spawn_session` and closes modal

- [ ] **Step 2: Redesign Sessions.tsx**

Update `src/pages/Sessions.tsx`:
- "New Session" button at top that opens the modal
- Session cards (not table rows) showing: provider icon, profile, directory, status badge, duration, "Open Terminal" button
- Running sessions section at top
- History section below (from database, paginated)
- Remove the old `LiveSessionRow` component and `Run History` table

- [ ] **Step 3: Build and verify**

```bash
cd /Users/jwgrogan/GitHub/arcctl && npm run build 2>&1 | tail -5
```

- [ ] **Step 4: Commit**

```bash
git add src/components/NewSessionModal.tsx src/pages/Sessions.tsx
git commit -m "feat: redesign sessions page with terminal-based session spawning"
```

---

### Task 11: Unified MCP Sync

**Files:**
- Create: `crates/arcctl-core/src/mcp_sync.rs`
- Create: `src-tauri/src/commands/mcp_sync.rs`
- Modify: `src/components/settings/McpServersPanel.tsx`

- [ ] **Step 1: Write MCP sync core logic**

Create `crates/arcctl-core/src/mcp_sync.rs`:
- `McpSyncManager` that loads `~/.arcctl/mcp-servers.json`
- Methods: `add_server`, `remove_server`, `list_servers`
- `sync_to_provider(provider)` — writes MCP config to each CLI's settings file
- `detect_drift(provider)` — compares arcctl's config with CLI's config
- Tests for round-trip and sync

- [ ] **Step 2: Add Tauri commands**

Create `src-tauri/src/commands/mcp_sync.rs`:
- `list_managed_mcps` — returns the unified list
- `add_managed_mcp` — adds and syncs
- `remove_managed_mcp` — removes and syncs
- `sync_mcps` — force sync all providers

- [ ] **Step 3: Update McpServersPanel.tsx**

Split into two sections:
1. **Managed MCPs** — from arcctl's unified config, with sync status badges
2. **Cloud Services** — read-only, from each CLI's settings (enabledMcpjsonServers + permission patterns)

- [ ] **Step 4: Build and test**

```bash
cd /Users/jwgrogan/GitHub/arcctl && cargo test -p arcctl-core mcp_sync 2>&1 | tail -10
cd /Users/jwgrogan/GitHub/arcctl && npm run build 2>&1 | tail -5
```

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: unified MCP server management synced across CLIs"
```

---

## Phase 2: Mission Control

### Task 12: Session File Parsers

**Files:**
- Create: `crates/arcctl-core/src/parsers/mod.rs`
- Create: `crates/arcctl-core/src/parsers/claude.rs`
- Create: `crates/arcctl-core/src/parsers/gemini.rs`
- Modify: `crates/arcctl-core/Cargo.toml` (no new deps — uses serde_json already)

- [ ] **Step 1: Define AgentNode and parser trait**

In `crates/arcctl-core/src/parsers/mod.rs`:

```rust
pub mod claude;
pub mod gemini;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentNode {
    pub id: String,
    pub name: String,
    pub agent_type: Option<String>,  // "Explore", "Team Implementer", etc.
    pub status: AgentStatus,
    pub current_activity: Option<String>,
    pub files_touched: Vec<String>,
    pub children: Vec<AgentNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Working,
    Waiting,
    Done,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTree {
    pub session_id: String,
    pub root: AgentNode,
}

pub trait SessionParser: Send + Sync {
    /// Parse session file content and return the agent tree.
    /// Returns None if the file is not parseable.
    fn parse(&self, content: &str) -> Option<SessionTree>;
}
```

- [ ] **Step 2: Implement Claude JSONL parser**

In `crates/arcctl-core/src/parsers/claude.rs`:
- Read JSONL lines
- Track `toolUseID` → `parentToolUseID` relationships
- When tool name is `Agent`, create a child AgentNode with the description/name from the tool args
- Track `text` content for current activity
- Track file paths mentioned in `Read`, `Write`, `Edit`, `Glob` tool calls

Include tests with sample JSONL input.

- [ ] **Step 3: Implement Gemini JSON parser**

In `crates/arcctl-core/src/parsers/gemini.rs`:
- Parse full JSON session file
- Extract `toolCalls` with `displayName` and `status`
- Build flat tree (most Gemini sessions are single-agent with tool calls)

Include tests with sample JSON input.

- [ ] **Step 4: Run tests**

```bash
cd /Users/jwgrogan/GitHub/arcctl && cargo test -p arcctl-core parsers 2>&1 | tail -15
```

- [ ] **Step 5: Commit**

```bash
git add crates/arcctl-core/src/parsers/
git commit -m "feat: session file parsers for Claude JSONL and Gemini JSON"
```

---

### Task 13: Session Monitor (File Watcher)

**Files:**
- Create: `crates/arcctl-core/src/session_monitor.rs`
- Modify: `crates/arcctl-core/Cargo.toml` (add `notify = "7"`)
- Modify: `src-tauri/src/lib.rs` (start monitor on setup)

- [ ] **Step 1: Write SessionMonitor**

```rust
// crates/arcctl-core/src/session_monitor.rs
// - Watches session files using notify crate
// - On file change, reads new content, parses via provider-specific parser
// - Maintains current SessionTree per session
// - Exposes get_tree(session_id) -> Option<SessionTree>
// - Runs in a background tokio task
```

Key methods:
- `start_monitoring(session_id, file_path, provider)` — begins watching
- `stop_monitoring(session_id)` — stops watching
- `get_tree(session_id)` — returns latest parsed tree
- Callback mechanism for Tauri events

- [ ] **Step 2: Integrate with Tauri**

In `src-tauri/src/lib.rs` setup, create the SessionMonitor and store in AppState.

Add Tauri command:
```rust
#[tauri::command]
fn get_session_tree(state: State<'_, AppState>, session_id: String) -> Option<SessionTree>
```

When a session is spawned (Task 8), also start monitoring its session file.

- [ ] **Step 3: Add `notify` dependency**

In `crates/arcctl-core/Cargo.toml`:
```toml
notify = "7"
```

- [ ] **Step 4: Test with a real session file**

```bash
cd /Users/jwgrogan/GitHub/arcctl && cargo test -p arcctl-core session_monitor 2>&1 | tail -10
```

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: session monitor watches CLI session files for agent tree updates"
```

---

### Task 14: Agent Tree UI Component

**Files:**
- Create: `src/components/AgentTree.tsx`
- Create: `src/components/SessionDetail.tsx`
- Modify: `src/pages/Sessions.tsx`
- Modify: `src/lib/tauri.ts`
- Modify: `src/lib/types.ts`

- [ ] **Step 1: Add TypeScript types for agent tree**

In `src/lib/types.ts`:

```typescript
export interface AgentNode {
  id: string;
  name: string;
  agent_type: string | null;
  status: "working" | "waiting" | "done" | "error";
  current_activity: string | null;
  files_touched: string[];
  children: AgentNode[];
}

export interface SessionTree {
  session_id: string;
  root: AgentNode;
}
```

Add Tauri API wrapper in `src/lib/tauri.ts`:

```typescript
export async function getSessionTree(sessionId: string): Promise<SessionTree | null> {
  return invoke("get_session_tree", { sessionId });
}
```

- [ ] **Step 2: Build AgentTree component**

Create `src/components/AgentTree.tsx`:
- Recursive tree rendering
- Each node shows: name, type badge, status indicator (colored dot), current activity text, file list
- Indentation for hierarchy
- Auto-refreshes via polling or Tauri event listener

- [ ] **Step 3: Build SessionDetail component**

Create `src/components/SessionDetail.tsx`:
- Shows when a session card is clicked in Sessions.tsx
- Displays: session metadata, AgentTree, "Open Terminal" button, files touched aggregate
- Slide-in panel or modal

- [ ] **Step 4: Wire into Sessions page**

In `src/pages/Sessions.tsx`:
- Clicking a session card opens SessionDetail
- Session detail shows the live agent tree

- [ ] **Step 5: Build and verify**

```bash
cd /Users/jwgrogan/GitHub/arcctl && npm run build 2>&1 | tail -5
```

- [ ] **Step 6: Commit**

```bash
git add src/components/AgentTree.tsx src/components/SessionDetail.tsx src/pages/Sessions.tsx src/lib/types.ts src/lib/tauri.ts
git commit -m "feat: agent team tree visualization in session detail view"
```

---

## Phase 3: Power Features

### Task 15: Session Resume

**Files:**
- Modify: `src-tauri/src/commands/process.rs`
- Modify: `src/components/SessionDetail.tsx`
- Modify: `src/lib/tauri.ts`

- [ ] **Step 1: Add resume_session Tauri command**

```rust
#[tauri::command]
pub fn resume_session(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<Session, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    let original = store.get_session(&session_id)
        .map_err(|e| e.to_string())?
        .ok_or("Session not found")?;

    let provider_id: ProviderId = serde_json::from_str(&format!("\"{}\"", original.provider))
        .map_err(|e| e.to_string())?;

    let cli_session_id = original.cli_session_id
        .ok_or("No CLI session ID — cannot resume")?;

    let dir = original.directory.unwrap_or_else(|| ".".to_string());

    let cmd = format!("cd {} && {} {} {}",
        shell_escape(&dir),
        provider_id.cli_command(),
        provider_id.resume_flag(),
        cli_session_id,
    );

    // Launch terminal, create new session record linked to original
    // ... (similar to spawn_session but with parent_session_id set)
}
```

- [ ] **Step 2: Add Resume button to SessionDetail**

In `src/components/SessionDetail.tsx`, add a "Resume" button for stopped sessions that calls `resumeSession`.

- [ ] **Step 3: Test manually**

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat: session resume opens terminal with CLI --resume flag"
```

---

### Task 16: Session Fork

**Files:**
- Modify: `src-tauri/src/commands/process.rs`
- Modify: `src/components/SessionDetail.tsx`

- [ ] **Step 1: Add fork_session command**

Similar to resume but uses `--fork-session` flag for Claude, and creates a session with `forked_from` set.

- [ ] **Step 2: Add Fork button to UI**

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat: session fork creates branched conversation in new terminal"
```

---

### Task 17: Session History with Filters

**Files:**
- Modify: `crates/arcctl-core/src/store.rs`
- Modify: `src/pages/Sessions.tsx`
- Modify: `src/lib/tauri.ts`

- [ ] **Step 1: Add filtered session queries**

Add `list_sessions_filtered` to store:
- Filter by directory, provider, date range
- Pagination

- [ ] **Step 2: Add filter UI to Sessions page**

Below running sessions:
- Filter bar: directory (text/dropdown), provider (toggle), date range
- Paginated table/card list of history
- "Show all directories" toggle (default: scoped to current)

- [ ] **Step 3: Test and commit**

```bash
git add -A
git commit -m "feat: filterable session history view"
```

---

### Task 18: Dock/Tray Hybrid

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/tray.rs`

- [ ] **Step 1: Implement activation policy switching**

In `src-tauri/src/lib.rs` setup:

```rust
// On window show: switch to Regular (dock visible)
// On window hide/close: switch to Accessory (tray only)
```

Use Tauri's window event listeners:
```rust
use tauri::ActivationPolicy;

window.on_window_event(|event| {
    match event {
        WindowEvent::Focused(true) => {
            app.set_activation_policy(ActivationPolicy::Regular);
        }
        WindowEvent::CloseRequested { .. } => {
            app.set_activation_policy(ActivationPolicy::Accessory);
            // Don't actually close — just hide
        }
        _ => {}
    }
});
```

- [ ] **Step 2: Test behavior**

Verify: app shows in dock when window is open, disappears when closed, tray icon always visible.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/tray.rs
git commit -m "feat: dock/tray hybrid — shows in dock when window open, tray-only when hidden"
```

---

## Additional Tasks (from review)

### Task 19: Session PID Polling (Status Detection)

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `crates/arcctl-core/src/store.rs`

Background task that runs every 5 seconds, checks if `terminal_pid` is still alive for each running session via `kill(pid, 0)`. If PID is gone, marks session as `Stopped` with `ended_at` timestamp. Started on Tauri app setup, runs for the lifetime of the app.

- [ ] **Step 1: Add `list_running_sessions` to store** (returns only `status = Running`)
- [ ] **Step 2: Implement polling loop in lib.rs setup** — `tokio::spawn` a task that loops every 5s, checks PIDs, updates store
- [ ] **Step 3: Test** — unit test for `list_running_sessions`, manual test for PID polling
- [ ] **Step 4: Commit**

---

### Task 20: OAuth Health Monitoring + CredentialManager Trait

**Files:**
- Modify: `crates/arcctl-core/src/profile.rs`
- Modify: `src/components/ProfileSwitcher.tsx`
- Modify: `src-tauri/src/commands/profile.rs`

- [ ] **Step 1: Define CredentialManager trait**

```rust
pub trait CredentialManager {
    fn import(&self, profile_dir: &Path, live_dirs: &LiveDirs) -> Result<()>;
    fn restore(&self, profile_dir: &Path, live_dirs: &LiveDirs) -> Result<()>;
    fn detect_active(&self, profile_dir: &Path, live_dirs: &LiveDirs) -> Result<bool>;
    fn check_health(&self, profile_dir: &Path) -> Result<HealthStatus>;
}

pub enum CredentialHealth { Valid, Expiring, Expired, Unknown }
```

- [ ] **Step 2: Implement ClaudeCredentialManager** — wraps existing logic, adds health check (read `.credentials.json`, check `expiresAt` field)
- [ ] **Step 3: Implement GeminiCredentialManager** — stub with `Unknown` health status until we investigate Gemini's auth storage
- [ ] **Step 4: Add Tauri command** — `check_profile_health(name) -> CredentialHealth`
- [ ] **Step 5: Add periodic health polling** — every 5 minutes, check all profiles, emit Tauri event with health status
- [ ] **Step 6: Update ProfileSwitcher** — use health status for dot colors (green/yellow/red), show "Re-authenticate" for expired
- [ ] **Step 7: Test and commit**

---

### Task 21: claude-switch Import and Removal

**Files:**
- Modify: `src-tauri/src/commands/profile.rs`
- Modify: `src/pages/Health.tsx`

- [ ] **Step 1: Add `detect_claude_switch` command** — checks if `/usr/local/bin/claude-switch` exists
- [ ] **Step 2: Add migration prompt in Health page** — if detected, show banner: "claude-switch detected. Import profiles and uninstall?"
- [ ] **Step 3: Add `import_claude_switch_profiles` command** — scans claude-switch's profile storage, imports into arcctl
- [ ] **Step 4: Test and commit**

---

### Task 22: Data Migration (runs → sessions backfill)

**Files:**
- Modify: `crates/arcctl-core/src/store.rs`

- [ ] **Step 1: Add migration step** — in `migrate()`, after creating `sessions` table, check if `runs` has data and `sessions` is empty. If so, copy runs into sessions with `provider: "claude"` and map `RunStatus::Complete` → `SessionStatus::Stopped`.
- [ ] **Step 2: Write test** — insert runs, run migrate, verify sessions populated
- [ ] **Step 3: Commit**

---

## Known Code Fixes (apply during implementation)

These bugs exist in plan code snippets and must be fixed when implementing:

1. **ProfileSwitcher.tsx:** Rename `const ref` to `const popoverRef` (ref is reserved in JSX)
2. **terminal.rs:** Rename `TerminalApp::TerminalApp` to `TerminalApp::AppleTerminal` to avoid shadowing
3. **terminal.rs:** `std::process::Child::id()` returns `u32` not `Option<u32>` — use `Some(child.id())`
4. **terminal.rs:** Ghostty launch should use direct CLI (`ghostty -e`) not `open -a`
5. **process.rs (Task 8):** `session_id` is moved then cloned — separate `cli_session_id` generation before constructing Session
6. **process.rs (Task 8):** Replace `serde_json::from_str(&format!(...))` with `ProviderId::from_str()` impl
7. **lib.rs module declarations:** Add `pub mod parsers;` (Task 12), `pub mod session_monitor;` (Task 13), `pub mod mcp_sync;` (Task 11) to `crates/arcctl-core/src/lib.rs`
8. **commands/mod.rs:** Register `mcp_sync` module in Tauri commands module
9. **libc dependency:** Already in `Cargo.toml` — verify it's available in `src-tauri` crate

---

## Verification

After all tasks:

```bash
cd /Users/jwgrogan/GitHub/arcctl
cargo test                     # All Rust tests pass
cargo clippy                   # No warnings
npm run build                  # Frontend builds clean
cargo tauri dev                # App launches with tray icon, sessions page works
```
