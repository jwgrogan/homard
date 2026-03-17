use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{ArcctlError, Result};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClaudeSettings {
    #[serde(default)]
    pub permissions: PermissionsConfig,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default, rename = "mcpServers")]
    pub mcp_servers: HashMap<String, McpServerConfig>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PermissionsConfig {
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub deny: Vec<String>,
    #[serde(default, rename = "bypassPermissions")]
    pub bypass_permissions: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub url: Option<String>,
    #[serde(rename = "type")]
    pub server_type: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

/// Returns `~/.claude/settings.json`.
pub fn claude_global_settings_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".claude")
        .join("settings.json")
}

/// Returns `<project_dir>/.claude/settings.json`.
pub fn claude_project_settings_path(project_dir: &Path) -> PathBuf {
    project_dir.join(".claude").join("settings.json")
}

// ---------------------------------------------------------------------------
// Load / Save
// ---------------------------------------------------------------------------

impl ClaudeSettings {
    /// Read and parse settings from `path`. Returns `Default` if the file is missing.
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(contents) => {
                if contents.trim().is_empty() {
                    return Ok(Self::default());
                }
                let settings: ClaudeSettings =
                    serde_json::from_str(&contents).map_err(ArcctlError::Json)?;
                Ok(settings)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(ArcctlError::Io(e)),
        }
    }

    /// Serialize as pretty JSON and write to `path`, creating parent dirs as needed.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(ArcctlError::Io)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(ArcctlError::Json)?;
        std::fs::write(path, json).map_err(ArcctlError::Io)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Mutation methods
// ---------------------------------------------------------------------------

impl ClaudeSettings {
    pub fn add_permission_allow(&mut self, pattern: String) {
        if !self.permissions.allow.contains(&pattern) {
            self.permissions.allow.push(pattern);
        }
    }

    pub fn remove_permission_allow(&mut self, pattern: &str) {
        self.permissions.allow.retain(|p| p != pattern);
    }

    pub fn add_permission_deny(&mut self, pattern: String) {
        if !self.permissions.deny.contains(&pattern) {
            self.permissions.deny.push(pattern);
        }
    }

    pub fn remove_permission_deny(&mut self, pattern: &str) {
        self.permissions.deny.retain(|p| p != pattern);
    }

    pub fn set_bypass_permissions(&mut self, bypass: bool) {
        self.permissions.bypass_permissions = bypass;
    }

    pub fn add_mcp_server(&mut self, name: String, config: McpServerConfig) {
        self.mcp_servers.insert(name, config);
    }

    pub fn remove_mcp_server(&mut self, name: &str) -> Option<McpServerConfig> {
        self.mcp_servers.remove(name)
    }

    pub fn set_env_var(&mut self, key: String, value: String) {
        self.env.insert(key, value);
    }

    pub fn remove_env_var(&mut self, key: &str) -> Option<String> {
        self.env.remove(key)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_mcp_server(command: &str) -> McpServerConfig {
        McpServerConfig {
            command: Some(command.to_string()),
            args: Some(vec!["--flag".to_string()]),
            url: None,
            server_type: Some("stdio".to_string()),
            extra: HashMap::new(),
        }
    }

    #[test]
    fn test_round_trip_save_load() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");

        let mut settings = ClaudeSettings::default();
        settings.add_permission_allow("Bash(*)".to_string());
        settings.add_permission_deny("Bash(rm*)".to_string());
        settings.set_bypass_permissions(true);
        settings.set_env_var("FOO".to_string(), "bar".to_string());
        settings.add_mcp_server("my-server".to_string(), make_mcp_server("npx"));

        settings.save(&path).expect("save should succeed");

        let loaded = ClaudeSettings::load(&path).expect("load should succeed");
        assert_eq!(loaded.permissions.allow, vec!["Bash(*)"]);
        assert_eq!(loaded.permissions.deny, vec!["Bash(rm*)"]);
        assert!(loaded.permissions.bypass_permissions);
        assert_eq!(loaded.env.get("FOO").unwrap(), "bar");
        assert!(loaded.mcp_servers.contains_key("my-server"));
        let srv = loaded.mcp_servers.get("my-server").unwrap();
        assert_eq!(srv.command.as_deref(), Some("npx"));
        assert_eq!(srv.server_type.as_deref(), Some("stdio"));
    }

    #[test]
    fn test_default_on_missing_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nonexistent.json");
        let settings = ClaudeSettings::load(&path).expect("should return default");
        assert!(settings.permissions.allow.is_empty());
        assert!(settings.permissions.deny.is_empty());
        assert!(!settings.permissions.bypass_permissions);
        assert!(settings.env.is_empty());
        assert!(settings.mcp_servers.is_empty());
    }

    #[test]
    fn test_flatten_preserves_unknown_keys() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");

        let raw = serde_json::json!({
            "permissions": { "allow": [] },
            "env": {},
            "mcpServers": {},
            "unknownTopLevel": "preserved",
            "anotherKey": 42
        });
        std::fs::write(&path, serde_json::to_string_pretty(&raw).unwrap()).unwrap();

        let loaded = ClaudeSettings::load(&path).expect("load should succeed");
        assert_eq!(
            loaded.extra.get("unknownTopLevel").and_then(|v| v.as_str()),
            Some("preserved")
        );
        assert_eq!(
            loaded.extra.get("anotherKey").and_then(|v| v.as_i64()),
            Some(42)
        );

        // Round-trip: unknown keys survive save+load
        loaded.save(&path).expect("save should succeed");
        let reloaded = ClaudeSettings::load(&path).expect("reload should succeed");
        assert_eq!(
            reloaded
                .extra
                .get("unknownTopLevel")
                .and_then(|v| v.as_str()),
            Some("preserved")
        );
    }

    #[test]
    fn test_add_remove_permission_allow() {
        let mut s = ClaudeSettings::default();
        s.add_permission_allow("Bash(*)".to_string());
        s.add_permission_allow("Read(*)".to_string());
        // Duplicate should not be added
        s.add_permission_allow("Bash(*)".to_string());
        assert_eq!(s.permissions.allow.len(), 2);

        s.remove_permission_allow("Bash(*)");
        assert_eq!(s.permissions.allow, vec!["Read(*)"]);
    }

    #[test]
    fn test_add_remove_permission_deny() {
        let mut s = ClaudeSettings::default();
        s.add_permission_deny("Bash(rm*)".to_string());
        assert_eq!(s.permissions.deny.len(), 1);
        s.remove_permission_deny("Bash(rm*)");
        assert!(s.permissions.deny.is_empty());
    }

    #[test]
    fn test_set_bypass_permissions() {
        let mut s = ClaudeSettings::default();
        assert!(!s.permissions.bypass_permissions);
        s.set_bypass_permissions(true);
        assert!(s.permissions.bypass_permissions);
        s.set_bypass_permissions(false);
        assert!(!s.permissions.bypass_permissions);
    }

    #[test]
    fn test_bypass_permissions_camel_case_rename() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");

        let mut s = ClaudeSettings::default();
        s.set_bypass_permissions(true);
        s.save(&path).unwrap();

        let json = std::fs::read_to_string(&path).unwrap();
        assert!(json.contains("bypassPermissions"), "JSON should use camelCase key");

        let loaded = ClaudeSettings::load(&path).unwrap();
        assert!(loaded.permissions.bypass_permissions);
    }

    #[test]
    fn test_mcp_servers_camel_case_rename() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");

        let mut s = ClaudeSettings::default();
        s.add_mcp_server("test-srv".to_string(), make_mcp_server("node"));
        s.save(&path).unwrap();

        let json = std::fs::read_to_string(&path).unwrap();
        assert!(json.contains("mcpServers"), "JSON should use camelCase key");

        let loaded = ClaudeSettings::load(&path).unwrap();
        assert!(loaded.mcp_servers.contains_key("test-srv"));
    }

    #[test]
    fn test_add_remove_mcp_server() {
        let mut s = ClaudeSettings::default();
        s.add_mcp_server("srv".to_string(), make_mcp_server("npx"));
        assert!(s.mcp_servers.contains_key("srv"));

        let removed = s.remove_mcp_server("srv");
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().command.as_deref(), Some("npx"));
        assert!(!s.mcp_servers.contains_key("srv"));

        let missing = s.remove_mcp_server("nope");
        assert!(missing.is_none());
    }

    #[test]
    fn test_set_remove_env_var() {
        let mut s = ClaudeSettings::default();
        s.set_env_var("KEY".to_string(), "value".to_string());
        assert_eq!(s.env.get("KEY").unwrap(), "value");

        // Overwrite
        s.set_env_var("KEY".to_string(), "new_value".to_string());
        assert_eq!(s.env.get("KEY").unwrap(), "new_value");

        let removed = s.remove_env_var("KEY");
        assert_eq!(removed.as_deref(), Some("new_value"));
        assert!(s.env.get("KEY").is_none());

        let missing = s.remove_env_var("NOPE");
        assert!(missing.is_none());
    }

    #[test]
    fn test_empty_file_loads_as_defaults() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "").unwrap();

        let loaded = ClaudeSettings::load(&path).expect("empty file should return defaults");
        assert!(loaded.permissions.allow.is_empty());
        assert!(loaded.env.is_empty());
        assert!(loaded.mcp_servers.is_empty());
    }

    #[test]
    fn test_creates_parent_dirs_on_save() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("deeply").join("nested").join("settings.json");

        let s = ClaudeSettings::default();
        s.save(&path).expect("save should create parent dirs");
        assert!(path.exists());
    }
}
