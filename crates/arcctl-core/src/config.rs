use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::error::ArcctlError;

// ---------------------------------------------------------------------------
// Config structs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    pub enabled: bool,
    pub token_keychain_ref: Option<String>,
    pub paired_chat_ids: Vec<String>,
    pub throttle_ms: u64,
}

impl Default for TelegramConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            token_keychain_ref: None,
            paired_chat_ids: Vec::new(),
            throttle_ms: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailConfig {
    pub enabled: bool,
    pub bot_address: Option<String>,
}

impl Default for EmailConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            bot_address: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerConfig {
    pub enabled: bool,
    pub stagger_ms: u64,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            stagger_ms: 5000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageConfig {
    pub track_tokens: bool,
    pub track_cost_estimate: bool,
}

impl Default for UsageConfig {
    fn default() -> Self {
        Self {
            track_tokens: true,
            track_cost_estimate: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArcctlConfig {
    pub version: u32,
    pub telegram: TelegramConfig,
    pub email: EmailConfig,
    pub scheduler: SchedulerConfig,
    pub usage: UsageConfig,
}

impl Default for ArcctlConfig {
    fn default() -> Self {
        Self {
            version: 1,
            telegram: TelegramConfig::default(),
            email: EmailConfig::default(),
            scheduler: SchedulerConfig::default(),
            usage: UsageConfig::default(),
        }
    }
}

impl ArcctlConfig {
    /// Read and deserialize from a JSON file at `path`.
    pub fn load(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path).map_err(ArcctlError::Io)?;
        let config: ArcctlConfig = serde_json::from_str(&contents).map_err(ArcctlError::Json)?;
        Ok(config)
    }

    /// Load from `path`, returning `Default` if the file is missing or invalid.
    pub fn load_or_default(path: &Path) -> Self {
        Self::load(path).unwrap_or_default()
    }

    /// Serialize as pretty JSON and write to `path`, creating parent directories as needed.
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
// Directory management
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ArcctlDirs {
    root: PathBuf,
}

impl ArcctlDirs {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Uses `$HOME/.arcctl` as the root.
    pub fn default_path() -> Self {
        let root = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".arcctl");
        Self::new(root)
    }

    pub fn config_path(&self) -> PathBuf {
        self.root.join("config.json")
    }

    pub fn schedules_dir(&self) -> PathBuf {
        self.root.join("schedules")
    }

    pub fn profiles_dir(&self) -> PathBuf {
        self.root.join("profiles")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }

    pub fn backups_dir(&self) -> PathBuf {
        self.root.join("backups")
    }

    pub fn db_path(&self) -> PathBuf {
        self.root.join("arcctl.db")
    }

    /// Create all required directories (not the db file itself).
    pub fn ensure_all(&self) -> Result<()> {
        for dir in &[
            &self.root,
            &self.schedules_dir(),
            &self.profiles_dir(),
            &self.logs_dir(),
            &self.backups_dir(),
        ] {
            std::fs::create_dir_all(dir).map_err(ArcctlError::Io)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Claude settings helpers
// ---------------------------------------------------------------------------

/// Read any JSON file and return its parsed value.
pub fn read_claude_settings(path: &Path) -> Result<serde_json::Value> {
    let contents = std::fs::read_to_string(path).map_err(ArcctlError::Io)?;
    let value: serde_json::Value = serde_json::from_str(&contents).map_err(ArcctlError::Json)?;
    Ok(value)
}

/// Write a `serde_json::Value` as pretty JSON, creating parent dirs as needed.
pub fn write_claude_settings(path: &Path, value: &serde_json::Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(ArcctlError::Io)?;
    }
    let json = serde_json::to_string_pretty(value).map_err(ArcctlError::Json)?;
    std::fs::write(path, json).map_err(ArcctlError::Io)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default_config() {
        let cfg = ArcctlConfig::default();
        assert_eq!(cfg.version, 1);
        assert!(!cfg.telegram.enabled);
        assert!(cfg.telegram.token_keychain_ref.is_none());
        assert!(cfg.telegram.paired_chat_ids.is_empty());
        assert!(!cfg.email.enabled);
        assert!(cfg.email.bot_address.is_none());
        assert!(cfg.scheduler.enabled);
        assert_eq!(cfg.scheduler.stagger_ms, 5000);
        assert!(cfg.usage.track_tokens);
        assert!(!cfg.usage.track_cost_estimate);
    }

    #[test]
    fn test_save_and_load() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("subdir").join("config.json");

        let mut cfg = ArcctlConfig::default();
        cfg.version = 42;
        cfg.save(&path).expect("save should succeed");

        let loaded = ArcctlConfig::load(&path).expect("load should succeed");
        assert_eq!(loaded.version, 42);
    }

    #[test]
    fn test_load_missing_returns_default() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nonexistent.json");
        let cfg = ArcctlConfig::load_or_default(&path);
        assert_eq!(cfg.version, 1);
    }

    #[test]
    fn test_arcctl_dirs() {
        let dir = TempDir::new().unwrap();
        let dirs = ArcctlDirs::new(dir.path().to_path_buf());

        dirs.ensure_all().expect("ensure_all should succeed");

        assert!(dirs.schedules_dir().exists());
        assert!(dirs.profiles_dir().exists());
        assert!(dirs.logs_dir().exists());
        assert!(dirs.backups_dir().exists());
        // db_path is a file path — parent dir (root) should exist
        assert!(dirs.db_path().parent().unwrap().exists());
    }

    #[test]
    fn test_read_claude_settings() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");

        let value = serde_json::json!({
            "model": "claude-opus-4",
            "max_tokens": 8192
        });
        write_claude_settings(&path, &value).expect("write should succeed");

        let loaded = read_claude_settings(&path).expect("read should succeed");
        assert_eq!(loaded["model"], "claude-opus-4");
        assert_eq!(loaded["max_tokens"], 8192);
    }
}
