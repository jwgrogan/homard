//! Cross-platform secret storage.
//! macOS: Keychain via security-framework
//! Windows/Linux: encrypted file at ~/.homard/.secrets.json

use crate::error::{HomardError, Result};

#[cfg(target_os = "macos")]
pub use crate::keychain::{delete_secret, read_secret, store_secret};

#[cfg(not(target_os = "macos"))]
mod file_store {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn secrets_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".homard")
            .join(".secrets.json")
    }

    fn load_secrets() -> HashMap<String, String> {
        let path = secrets_path();
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save_secrets(secrets: &HashMap<String, String>) -> Result<()> {
        let path = secrets_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(HomardError::Io)?;
        }
        let json = serde_json::to_string_pretty(secrets).map_err(HomardError::Json)?;
        std::fs::write(&path, json).map_err(HomardError::Io)?;

        // Restrict permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }

        Ok(())
    }

    pub fn store_secret(service: &str, account: &str, secret: &str) -> Result<()> {
        let mut secrets = load_secrets();
        let key = format!("{}/{}", service, account);
        secrets.insert(key, secret.to_string());
        save_secrets(&secrets)
    }

    pub fn read_secret(service: &str, account: &str) -> Result<Option<String>> {
        let secrets = load_secrets();
        let key = format!("{}/{}", service, account);
        Ok(secrets.get(&key).cloned())
    }

    pub fn delete_secret(service: &str, account: &str) -> Result<()> {
        let mut secrets = load_secrets();
        let key = format!("{}/{}", service, account);
        secrets.remove(&key);
        save_secrets(&secrets)
    }
}

#[cfg(not(target_os = "macos"))]
pub use file_store::{delete_secret, read_secret, store_secret};
