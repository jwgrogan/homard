use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::error::HomardError;
use crate::types::{PermissionLevel, ProviderConfig, ServerMode, ShellTool};

// ---------------------------------------------------------------------------
// Config structs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingCode {
    pub code: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    pub enabled: bool,
    pub token_keychain_ref: Option<String>,
    pub paired_chat_ids: Vec<String>,
    pub throttle_ms: u64,
    #[serde(default)]
    pub pending_pairing_codes: Vec<PairingCode>,
}

impl Default for TelegramConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            token_keychain_ref: None,
            paired_chat_ids: Vec::new(),
            throttle_ms: 0,
            pending_pairing_codes: Vec::new(),
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
pub struct HomardConfig {
    pub version: u32,
    pub telegram: TelegramConfig,
    pub scheduler: SchedulerConfig,
    pub usage: UsageConfig,
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
    #[serde(default = "default_active_provider")]
    pub active_provider: String,
    #[serde(default)]
    pub permission_level: PermissionLevel,
    #[serde(default)]
    pub shell_tools: Vec<ShellTool>,
    #[serde(default)]
    pub bootstrapped: bool,
    #[serde(default)]
    pub server_mode: ServerMode,
    /// Preferred CLI for coding delegation: "claude" or "codex"
    #[serde(default = "default_coding_cli")]
    pub preferred_coding_cli: String,
    /// Fallback CLI if preferred is unavailable
    #[serde(default = "default_coding_cli_fallback")]
    pub coding_cli_fallback: String,
}

fn default_coding_cli() -> String {
    "claude".to_string()
}

fn default_coding_cli_fallback() -> String {
    "codex".to_string()
}

fn default_active_provider() -> String {
    "anthropic".to_string()
}

impl Default for HomardConfig {
    fn default() -> Self {
        Self {
            version: 1,
            telegram: TelegramConfig::default(),
            scheduler: SchedulerConfig::default(),
            usage: UsageConfig::default(),
            providers: HashMap::new(),
            active_provider: default_active_provider(),
            permission_level: PermissionLevel::default(),
            shell_tools: Vec::new(),
            bootstrapped: false,
            server_mode: ServerMode::default(),
            preferred_coding_cli: default_coding_cli(),
            coding_cli_fallback: default_coding_cli_fallback(),
        }
    }
}

impl HomardConfig {
    /// Read and deserialize from a JSON file at `path`.
    pub fn load(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path).map_err(HomardError::Io)?;
        let config: HomardConfig = serde_json::from_str(&contents).map_err(HomardError::Json)?;
        Ok(config)
    }

    /// Load from `path`, returning `Default` if the file is missing or invalid.
    pub fn load_or_default(path: &Path) -> Self {
        Self::load(path).unwrap_or_default()
    }

    /// Serialize as pretty JSON and write to `path`, creating parent directories as needed.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(HomardError::Io)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(HomardError::Json)?;
        std::fs::write(path, json).map_err(HomardError::Io)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Directory management
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct HomardDirs {
    root: PathBuf,
}

impl HomardDirs {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Uses `$HOME/.homard` as the root.
    pub fn default_path() -> Self {
        let root = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".homard");
        Self::new(root)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn config_path(&self) -> PathBuf {
        self.root.join("config.json")
    }

    pub fn schedules_dir(&self) -> PathBuf {
        self.root.join("schedules")
    }

    pub fn conversations_dir(&self) -> PathBuf {
        self.root.join("conversations")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }

    pub fn db_path(&self) -> PathBuf {
        self.root.join("homard.db")
    }

    /// Create all required directories (not the db file itself).
    pub fn ensure_all(&self) -> Result<()> {
        for dir in &[
            &self.root,
            &self.schedules_dir(),
            &self.conversations_dir(),
            &self.logs_dir(),
        ] {
            std::fs::create_dir_all(dir).map_err(HomardError::Io)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Telegram config helpers
// ---------------------------------------------------------------------------

const TELEGRAM_KEYCHAIN_SERVICE: &str = "homard-telegram";
const TELEGRAM_KEYCHAIN_ACCOUNT: &str = "bot-token";

/// Store Telegram bot token in Keychain and update config.json with the ref.
#[cfg(target_os = "macos")]
pub fn save_telegram_token(dirs: &HomardDirs, token: &str) -> Result<()> {
    crate::keychain::store_secret(TELEGRAM_KEYCHAIN_SERVICE, TELEGRAM_KEYCHAIN_ACCOUNT, token)?;
    let mut cfg = HomardConfig::load_or_default(&dirs.config_path());
    cfg.telegram.enabled = true;
    cfg.telegram.token_keychain_ref = Some(format!("{}/{}", TELEGRAM_KEYCHAIN_SERVICE, TELEGRAM_KEYCHAIN_ACCOUNT));
    cfg.save(&dirs.config_path())?;
    Ok(())
}

/// Read Telegram bot token from Keychain. Returns None if not configured.
#[cfg(target_os = "macos")]
pub fn get_telegram_token(dirs: &HomardDirs) -> Result<Option<String>> {
    let cfg = HomardConfig::load_or_default(&dirs.config_path());
    if cfg.telegram.token_keychain_ref.is_none() {
        return Ok(None);
    }
    crate::keychain::read_secret(TELEGRAM_KEYCHAIN_SERVICE, TELEGRAM_KEYCHAIN_ACCOUNT)
}

/// Add a chat_id to the paired list (idempotent).
pub fn add_paired_chat(dirs: &HomardDirs, chat_id: &str) -> Result<()> {
    let mut cfg = HomardConfig::load_or_default(&dirs.config_path());
    if !cfg.telegram.paired_chat_ids.contains(&chat_id.to_string()) {
        cfg.telegram.paired_chat_ids.push(chat_id.to_string());
        cfg.save(&dirs.config_path())?;
    }
    Ok(())
}

/// Remove a chat_id from the paired list.
pub fn remove_paired_chat(dirs: &HomardDirs, chat_id: &str) -> Result<()> {
    let mut cfg = HomardConfig::load_or_default(&dirs.config_path());
    cfg.telegram.paired_chat_ids.retain(|id| id != chat_id);
    cfg.save(&dirs.config_path())?;
    Ok(())
}

/// Generate a random 8-char alphanumeric pairing code with 10-min expiry.
pub fn generate_pairing_code(dirs: &HomardDirs) -> Result<String> {
    use rand::Rng;
    let code: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(8)
        .map(|c| char::from(c).to_ascii_uppercase())
        .collect();
    let now = Utc::now();
    let pairing_code = PairingCode {
        code: code.clone(),
        created_at: now,
        expires_at: now + chrono::Duration::minutes(10),
    };
    let mut cfg = HomardConfig::load_or_default(&dirs.config_path());
    cfg.telegram.pending_pairing_codes.retain(|p| p.expires_at > now);
    cfg.telegram.pending_pairing_codes.push(pairing_code);
    cfg.save(&dirs.config_path())?;
    Ok(code)
}

/// Validate and consume a pairing code. Returns true if valid+unexpired, false otherwise.
pub fn validate_pairing_code(dirs: &HomardDirs, code: &str) -> Result<bool> {
    let now = Utc::now();
    let mut cfg = HomardConfig::load_or_default(&dirs.config_path());
    let found = cfg.telegram.pending_pairing_codes
        .iter()
        .any(|p| p.code == code && p.expires_at > now);
    if found {
        cfg.telegram.pending_pairing_codes.retain(|p| p.code != code);
        cfg.save(&dirs.config_path())?;
    }
    Ok(found)
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
        let cfg = HomardConfig::default();
        assert_eq!(cfg.version, 1);
        assert!(!cfg.telegram.enabled);
        assert!(cfg.telegram.token_keychain_ref.is_none());
        assert!(cfg.telegram.paired_chat_ids.is_empty());
        assert!(cfg.scheduler.enabled);
        assert_eq!(cfg.scheduler.stagger_ms, 5000);
        assert!(cfg.usage.track_tokens);
        assert!(!cfg.usage.track_cost_estimate);
        assert_eq!(cfg.active_provider, "anthropic");
        assert_eq!(cfg.permission_level, PermissionLevel::Supervised);
        assert!(cfg.providers.is_empty());
        assert!(cfg.shell_tools.is_empty());
        assert!(!cfg.bootstrapped);
    }

    #[test]
    fn test_save_and_load() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("subdir").join("config.json");

        let mut cfg = HomardConfig::default();
        cfg.version = 42;
        cfg.save(&path).expect("save should succeed");

        let loaded = HomardConfig::load(&path).expect("load should succeed");
        assert_eq!(loaded.version, 42);
    }

    #[test]
    fn test_load_missing_returns_default() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nonexistent.json");
        let cfg = HomardConfig::load_or_default(&path);
        assert_eq!(cfg.version, 1);
    }

    #[test]
    fn test_homard_dirs() {
        let dir = TempDir::new().unwrap();
        let dirs = HomardDirs::new(dir.path().to_path_buf());

        dirs.ensure_all().expect("ensure_all should succeed");

        assert!(dirs.schedules_dir().exists());
        assert!(dirs.conversations_dir().exists());
        assert!(dirs.logs_dir().exists());
        // db_path is a file path -- parent dir (root) should exist
        assert!(dirs.db_path().parent().unwrap().exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_save_telegram_token() {
        use crate::keychain;
        let dir = TempDir::new().unwrap();
        let dirs = HomardDirs::new(dir.path().to_path_buf());
        dirs.ensure_all().unwrap();

        save_telegram_token(&dirs, "test-bot-token-12345").unwrap();

        let cfg = HomardConfig::load(&dirs.config_path()).unwrap();
        assert!(cfg.telegram.enabled);
        assert_eq!(
            cfg.telegram.token_keychain_ref,
            Some("homard-telegram/bot-token".to_string())
        );

        let token = keychain::read_secret("homard-telegram", "bot-token").unwrap();
        assert_eq!(token, Some("test-bot-token-12345".to_string()));

        // Cleanup
        let _ = keychain::delete_secret("homard-telegram", "bot-token");
    }

    #[test]
    fn test_add_and_remove_paired_chat() {
        let dir = TempDir::new().unwrap();
        let dirs = HomardDirs::new(dir.path().to_path_buf());
        dirs.ensure_all().unwrap();

        add_paired_chat(&dirs, "123456789").unwrap();
        let cfg = HomardConfig::load_or_default(&dirs.config_path());
        assert!(cfg.telegram.paired_chat_ids.contains(&"123456789".to_string()));

        // Idempotent -- add twice
        add_paired_chat(&dirs, "123456789").unwrap();
        let cfg = HomardConfig::load_or_default(&dirs.config_path());
        assert_eq!(cfg.telegram.paired_chat_ids.len(), 1);

        remove_paired_chat(&dirs, "123456789").unwrap();
        let cfg = HomardConfig::load_or_default(&dirs.config_path());
        assert!(!cfg.telegram.paired_chat_ids.contains(&"123456789".to_string()));
    }

    #[test]
    fn test_generate_and_validate_pairing_code() {
        let dir = TempDir::new().unwrap();
        let dirs = HomardDirs::new(dir.path().to_path_buf());
        dirs.ensure_all().unwrap();

        let code = generate_pairing_code(&dirs).unwrap();
        assert_eq!(code.len(), 8, "pairing code should be 8 chars");
        assert!(code.chars().all(|c| c.is_alphanumeric()));

        assert!(validate_pairing_code(&dirs, &code).unwrap());
        // Code should be consumed after first use -- calling again returns false
        assert!(!validate_pairing_code(&dirs, &code).unwrap());
        assert!(!validate_pairing_code(&dirs, "WRONGCODE").unwrap());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_get_telegram_token_unconfigured() {
        let dir = TempDir::new().unwrap();
        let dirs = HomardDirs::new(dir.path().to_path_buf());
        dirs.ensure_all().unwrap();
        let token = get_telegram_token(&dirs).unwrap();
        assert_eq!(token, None);
    }
}
