use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::settings::{ClaudeSettings, McpServerConfig};

/// Manages unified MCP server configuration, synced to each CLI.
pub struct McpSyncManager {
    config_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ManagedMcpConfig {
    #[serde(flatten)]
    pub servers: HashMap<String, McpServerConfig>,
}

impl McpSyncManager {
    pub fn new(config_path: PathBuf) -> Self {
        Self { config_path }
    }

    pub fn load(&self) -> Result<ManagedMcpConfig> {
        match std::fs::read_to_string(&self.config_path) {
            Ok(contents) => {
                if contents.trim().is_empty() {
                    return Ok(ManagedMcpConfig::default());
                }
                Ok(serde_json::from_str(&contents)?)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(ManagedMcpConfig::default()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn save(&self, config: &ManagedMcpConfig) -> Result<()> {
        if let Some(parent) = self.config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&config.servers)?;
        std::fs::write(&self.config_path, json)?;
        Ok(())
    }

    pub fn add_server(&self, name: String, config: McpServerConfig) -> Result<()> {
        let mut managed = self.load()?;
        managed.servers.insert(name, config);
        self.save(&managed)?;
        Ok(())
    }

    pub fn remove_server(&self, name: &str) -> Result<Option<McpServerConfig>> {
        let mut managed = self.load()?;
        let removed = managed.servers.remove(name);
        self.save(&managed)?;
        Ok(removed)
    }

    pub fn list_servers(&self) -> Result<ManagedMcpConfig> {
        self.load()
    }

    /// Sync managed MCP servers to Claude's settings.json.
    ///
    /// This performs a full reconciliation:
    ///   1. Adds/updates every server in arcctl's managed list.
    ///   2. Removes servers that were previously synced by arcctl but are no
    ///      longer in the managed list (tracked via a `.arcctl-managed` marker
    ///      file next to the managed config).
    pub fn sync_to_claude(&self, claude_settings_path: &Path) -> Result<()> {
        let managed = self.load()?;
        let mut settings = ClaudeSettings::load(claude_settings_path)?;

        let prev_names = self.load_synced_names();

        // Remove servers that were previously synced but are no longer managed
        for old_name in &prev_names {
            if !managed.servers.contains_key(old_name) {
                settings.mcp_servers.remove(old_name);
            }
        }

        // Add/update managed servers
        for (name, config) in &managed.servers {
            settings.mcp_servers.insert(name.clone(), config.clone());
        }

        settings.save(claude_settings_path)?;

        // Persist the set of names we just synced so future removals work
        self.save_synced_names(&managed.servers.keys().cloned().collect::<Vec<_>>());

        Ok(())
    }

    // -- helpers for tracking previously-synced server names ------------------

    fn synced_names_path(&self) -> PathBuf {
        self.config_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(".arcctl-synced-mcps.json")
    }

    fn load_synced_names(&self) -> Vec<String> {
        let path = self.synced_names_path();
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save_synced_names(&self, names: &[String]) {
        let path = self.synced_names_path();
        let _ = serde_json::to_string(names)
            .map(|json| std::fs::write(&path, json));
    }

    /// Detect if a CLI's MCP config has drifted from arcctl's managed config.
    /// Returns server names that exist in the CLI but not in arcctl's managed config.
    pub fn detect_drift_claude(&self, claude_settings_path: &Path) -> Result<Vec<String>> {
        let managed = self.load()?;
        let settings = ClaudeSettings::load(claude_settings_path)?;

        let mut drifted = Vec::new();
        for name in settings.mcp_servers.keys() {
            if !managed.servers.contains_key(name) {
                drifted.push(name.clone());
            }
        }
        Ok(drifted)
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
    fn test_round_trip_empty() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("mcp-servers.json");
        let manager = McpSyncManager::new(path);

        let config = manager.load().unwrap();
        assert!(config.servers.is_empty());
    }

    #[test]
    fn test_round_trip_save_load() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("mcp-servers.json");
        let manager = McpSyncManager::new(path);

        let mut config = ManagedMcpConfig::default();
        config
            .servers
            .insert("test-srv".to_string(), make_mcp_server("npx"));
        manager.save(&config).unwrap();

        let loaded = manager.load().unwrap();
        assert!(loaded.servers.contains_key("test-srv"));
        let srv = loaded.servers.get("test-srv").unwrap();
        assert_eq!(srv.command.as_deref(), Some("npx"));
        assert_eq!(srv.server_type.as_deref(), Some("stdio"));
    }

    #[test]
    fn test_add_server() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("mcp-servers.json");
        let manager = McpSyncManager::new(path);

        manager
            .add_server("my-server".to_string(), make_mcp_server("node"))
            .unwrap();

        let config = manager.load().unwrap();
        assert!(config.servers.contains_key("my-server"));
        assert_eq!(
            config.servers["my-server"].command.as_deref(),
            Some("node")
        );
    }

    #[test]
    fn test_remove_server() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("mcp-servers.json");
        let manager = McpSyncManager::new(path);

        manager
            .add_server("srv".to_string(), make_mcp_server("npx"))
            .unwrap();
        let removed = manager.remove_server("srv").unwrap();
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().command.as_deref(), Some("npx"));

        let config = manager.load().unwrap();
        assert!(!config.servers.contains_key("srv"));
    }

    #[test]
    fn test_remove_nonexistent_server() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("mcp-servers.json");
        let manager = McpSyncManager::new(path);

        let removed = manager.remove_server("nope").unwrap();
        assert!(removed.is_none());
    }

    #[test]
    fn test_list_servers() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("mcp-servers.json");
        let manager = McpSyncManager::new(path);

        manager
            .add_server("a".to_string(), make_mcp_server("cmd-a"))
            .unwrap();
        manager
            .add_server("b".to_string(), make_mcp_server("cmd-b"))
            .unwrap();

        let config = manager.list_servers().unwrap();
        assert_eq!(config.servers.len(), 2);
        assert!(config.servers.contains_key("a"));
        assert!(config.servers.contains_key("b"));
    }

    #[test]
    fn test_sync_to_claude() {
        let dir = TempDir::new().unwrap();
        let mcp_path = dir.path().join("mcp-servers.json");
        let claude_path = dir.path().join("settings.json");

        let manager = McpSyncManager::new(mcp_path);
        manager
            .add_server("synced-srv".to_string(), make_mcp_server("npx"))
            .unwrap();

        manager.sync_to_claude(&claude_path).unwrap();

        let settings = ClaudeSettings::load(&claude_path).unwrap();
        assert!(settings.mcp_servers.contains_key("synced-srv"));
        assert_eq!(
            settings.mcp_servers["synced-srv"].command.as_deref(),
            Some("npx")
        );
    }

    #[test]
    fn test_sync_to_claude_preserves_existing() {
        let dir = TempDir::new().unwrap();
        let mcp_path = dir.path().join("mcp-servers.json");
        let claude_path = dir.path().join("settings.json");

        // Pre-populate Claude settings with an existing server
        let mut settings = ClaudeSettings::default();
        settings
            .mcp_servers
            .insert("existing-srv".to_string(), make_mcp_server("existing-cmd"));
        settings.save(&claude_path).unwrap();

        let manager = McpSyncManager::new(mcp_path);
        manager
            .add_server("new-srv".to_string(), make_mcp_server("new-cmd"))
            .unwrap();
        manager.sync_to_claude(&claude_path).unwrap();

        let reloaded = ClaudeSettings::load(&claude_path).unwrap();
        assert!(reloaded.mcp_servers.contains_key("existing-srv"));
        assert!(reloaded.mcp_servers.contains_key("new-srv"));
    }

    #[test]
    fn test_detect_drift_claude() {
        let dir = TempDir::new().unwrap();
        let mcp_path = dir.path().join("mcp-servers.json");
        let claude_path = dir.path().join("settings.json");

        // Claude has two servers, arcctl only knows about one
        let mut settings = ClaudeSettings::default();
        settings
            .mcp_servers
            .insert("managed-srv".to_string(), make_mcp_server("cmd1"));
        settings
            .mcp_servers
            .insert("drifted-srv".to_string(), make_mcp_server("cmd2"));
        settings.save(&claude_path).unwrap();

        let manager = McpSyncManager::new(mcp_path);
        manager
            .add_server("managed-srv".to_string(), make_mcp_server("cmd1"))
            .unwrap();

        let drifted = manager.detect_drift_claude(&claude_path).unwrap();
        assert_eq!(drifted, vec!["drifted-srv"]);
    }

    #[test]
    fn test_sync_to_claude_removes_deleted_managed_server() {
        let dir = TempDir::new().unwrap();
        let mcp_path = dir.path().join("mcp-servers.json");
        let claude_path = dir.path().join("settings.json");

        // Pre-populate Claude settings with an unmanaged server
        let mut settings = ClaudeSettings::default();
        settings
            .mcp_servers
            .insert("unmanaged-srv".to_string(), make_mcp_server("unmanaged-cmd"));
        settings.save(&claude_path).unwrap();

        let manager = McpSyncManager::new(mcp_path);

        // Add two managed servers and sync
        manager
            .add_server("srv-a".to_string(), make_mcp_server("cmd-a"))
            .unwrap();
        manager
            .add_server("srv-b".to_string(), make_mcp_server("cmd-b"))
            .unwrap();
        manager.sync_to_claude(&claude_path).unwrap();

        let reloaded = ClaudeSettings::load(&claude_path).unwrap();
        assert_eq!(reloaded.mcp_servers.len(), 3); // unmanaged + a + b

        // Now remove srv-b from managed and re-sync
        manager.remove_server("srv-b").unwrap();
        manager.sync_to_claude(&claude_path).unwrap();

        let reloaded = ClaudeSettings::load(&claude_path).unwrap();
        assert!(reloaded.mcp_servers.contains_key("unmanaged-srv"), "unmanaged server should be preserved");
        assert!(reloaded.mcp_servers.contains_key("srv-a"), "still-managed server should remain");
        assert!(!reloaded.mcp_servers.contains_key("srv-b"), "removed managed server should be deleted from Claude settings");
    }

    #[test]
    fn test_creates_parent_dirs_on_save() {
        let dir = TempDir::new().unwrap();
        let path = dir
            .path()
            .join("deeply")
            .join("nested")
            .join("mcp-servers.json");
        let manager = McpSyncManager::new(path.clone());

        let config = ManagedMcpConfig::default();
        manager.save(&config).unwrap();
        assert!(path.exists());
    }
}
