use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::error::{ArcctlError, Result};
use crate::provider::ProviderId;
use crate::types::{CredentialHealth, Profile};

const CLAUDE_DIR_FILES: &[&str] = &[".credentials.json", "statsig", "statsig_metadata"];
const HOME_FILE: &str = ".claude.json";
const HOME_FILE_STORED: &str = "home_.claude.json";

pub struct ProfileManager {
    pub profiles_dir: PathBuf,
    pub claude_dir: PathBuf,
    pub home_dir: PathBuf,
}

impl ProfileManager {
    pub fn new(profiles_dir: PathBuf, claude_dir: PathBuf, home_dir: PathBuf) -> Self {
        Self {
            profiles_dir,
            claude_dir,
            home_dir,
        }
    }

    /// Copy credential files from claude_dir + home_dir into profiles_dir/<name>/
    pub fn import(&self, name: &str) -> Result<()> {
        let dest_dir = self.profiles_dir.join(name);
        fs::create_dir_all(&dest_dir)?;

        // Copy files from claude_dir
        for filename in CLAUDE_DIR_FILES {
            let src = self.claude_dir.join(filename);
            if src.exists() {
                let dest = dest_dir.join(filename);
                // If src is a directory, copy recursively; otherwise copy as file
                if src.is_dir() {
                    copy_dir_all(&src, &dest)?;
                } else {
                    fs::copy(&src, &dest)?;
                }
            }
        }

        // Copy home .claude.json stored as home_.claude.json
        let home_src = self.home_dir.join(HOME_FILE);
        if home_src.exists() {
            let dest = dest_dir.join(HOME_FILE_STORED);
            fs::copy(&home_src, &dest)?;
        }

        // Write provider.json (default to Claude)
        let provider_json = serde_json::to_string(&ProviderId::Claude).unwrap();
        std::fs::write(dest_dir.join("provider.json"), provider_json)?;

        Ok(())
    }

    /// List all profile dirs, extract email from .credentials.json, sort by name
    pub fn list(&self) -> Result<Vec<Profile>> {
        if !self.profiles_dir.exists() {
            return Ok(vec![]);
        }

        let mut profiles: Vec<Profile> = Vec::new();

        let entries = fs::read_dir(&self.profiles_dir)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            if name.is_empty() {
                continue;
            }

            let email = extract_email_from_credentials(&path.join(".credentials.json"));

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
        }

        profiles.sort_by(|a, b| a.name.cmp(&b.name));

        self.detect_active(&mut profiles);

        Ok(profiles)
    }

    /// Determine which profile is active by comparing the live .credentials.json
    /// against each profile's stored copy. If the contents match, that profile is active.
    fn detect_active(&self, profiles: &mut Vec<Profile>) {
        let current = fs::read_to_string(self.claude_dir.join(".credentials.json")).ok();
        for profile in profiles.iter_mut() {
            let stored = fs::read_to_string(
                self.profiles_dir
                    .join(&profile.name)
                    .join(".credentials.json"),
            )
            .ok();
            profile.is_active = current.is_some() && current == stored;
        }
    }

    /// Check if a profile's credentials are still valid.
    /// For Claude: reads .credentials.json and checks expiresAt field.
    /// Returns Unknown for non-Claude profiles (Gemini health check TBD).
    pub fn check_health(&self, name: &str) -> CredentialHealth {
        let profile_dir = self.profiles_dir.join(name);

        // Read provider
        let provider = std::fs::read_to_string(profile_dir.join("provider.json"))
            .ok()
            .and_then(|s| serde_json::from_str::<ProviderId>(&s).ok())
            .unwrap_or(ProviderId::Claude);

        match provider {
            ProviderId::Claude => {
                let creds_path = profile_dir.join(".credentials.json");
                match std::fs::read_to_string(&creds_path) {
                    Ok(contents) => {
                        match serde_json::from_str::<serde_json::Value>(&contents) {
                            Ok(v) => {
                                // Check expiresAt field
                                if let Some(expires_at) = v.get("expiresAt").and_then(|e| e.as_str()) {
                                    if let Ok(expires) = chrono::DateTime::parse_from_rfc3339(expires_at) {
                                        let now = chrono::Utc::now();
                                        let until_expiry = expires.signed_duration_since(now);
                                        if until_expiry.num_seconds() < 0 {
                                            return CredentialHealth::Expired;
                                        } else if until_expiry.num_hours() < 24 {
                                            return CredentialHealth::Expiring;
                                        } else {
                                            return CredentialHealth::Valid;
                                        }
                                    }
                                }
                                // Has credentials but no expiresAt — assume valid
                                CredentialHealth::Valid
                            }
                            Err(_) => CredentialHealth::Unknown,
                        }
                    }
                    Err(_) => CredentialHealth::Expired, // No credentials file
                }
            }
            ProviderId::Gemini => CredentialHealth::Unknown, // Gemini health check TBD
        }
    }

    /// Delete a profile directory. Returns error if profile doesn't exist.
    pub fn delete(&self, name: &str) -> Result<()> {
        let profile_dir = self.profiles_dir.join(name);
        if !profile_dir.exists() {
            return Err(ArcctlError::NotFound(format!("Profile '{}' not found", name)));
        }
        std::fs::remove_dir_all(&profile_dir)?;
        Ok(())
    }

    /// Copy profile files back to live locations
    pub fn restore_files(&self, name: &str) -> Result<()> {
        let src_dir = self.profiles_dir.join(name);
        if !src_dir.exists() {
            return Err(ArcctlError::NotFound(format!("Profile '{}' not found", name)));
        }

        // Restore files to claude_dir
        for filename in CLAUDE_DIR_FILES {
            let src = src_dir.join(filename);
            if src.exists() {
                let dest = self.claude_dir.join(filename);
                if let Some(parent) = dest.parent() {
                    fs::create_dir_all(parent)?;
                }
                if src.is_dir() {
                    copy_dir_all(&src, &dest)?;
                } else {
                    fs::copy(&src, &dest)?;
                }
            }
        }

        // Restore home_.claude.json back to home_dir/.claude.json
        let stored = src_dir.join(HOME_FILE_STORED);
        if stored.exists() {
            let dest = self.home_dir.join(HOME_FILE);
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&stored, &dest)?;
        }

        Ok(())
    }
}

/// Extract the email field from a .credentials.json file.
fn extract_email_from_credentials(path: &Path) -> Option<String> {
    let contents = fs::read_to_string(path).ok()?;
    let v: Value = serde_json::from_str(&contents).ok()?;
    v.get("email")
        .and_then(|e| e.as_str())
        .map(|s| s.to_string())
}

/// Recursively copy a directory from src to dst.
fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dest_path)?;
        } else {
            fs::copy(entry.path(), &dest_path)?;
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub mod keychain {
    use crate::error::{ArcctlError, Result};
    use std::process::Command;

    const SERVICE_NAME: &str = "Claude Code-credentials";

    pub fn read_credentials() -> Result<String> {
        let username = whoami::username();
        let output = Command::new("security")
            .args([
                "find-generic-password",
                "-s",
                SERVICE_NAME,
                "-a",
                &username,
                "-w",
            ])
            .output()
            .map_err(|e| ArcctlError::Profile(format!("Failed to run security CLI: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ArcctlError::Profile(format!(
                "security find-generic-password failed: {}",
                stderr.trim()
            )));
        }

        let data = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(data)
    }

    pub fn write_credentials(data: &str) -> Result<()> {
        let username = whoami::username();

        // Delete existing entry (ignore errors — may not exist)
        let _ = Command::new("security")
            .args([
                "delete-generic-password",
                "-s",
                SERVICE_NAME,
                "-a",
                &username,
            ])
            .output();

        // Add new entry
        let output = Command::new("security")
            .args([
                "add-generic-password",
                "-s",
                SERVICE_NAME,
                "-a",
                &username,
                "-w",
                data,
            ])
            .output()
            .map_err(|e| ArcctlError::Profile(format!("Failed to run security CLI: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ArcctlError::Profile(format!(
                "security add-generic-password failed: {}",
                stderr.trim()
            )));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_manager(tmp: &TempDir) -> ProfileManager {
        let profiles_dir = tmp.path().join("profiles");
        let claude_dir = tmp.path().join("claude");
        let home_dir = tmp.path().join("home");
        fs::create_dir_all(&profiles_dir).unwrap();
        fs::create_dir_all(&claude_dir).unwrap();
        fs::create_dir_all(&home_dir).unwrap();
        ProfileManager::new(profiles_dir, claude_dir, home_dir)
    }

    fn write_credentials(dir: &Path, email: &str) {
        let contents = format!(r#"{{"email":"{}","token":"abc123"}}"#, email);
        fs::write(dir.join(".credentials.json"), contents).unwrap();
    }

    #[test]
    fn test_import_profile() {
        let tmp = TempDir::new().unwrap();
        let mgr = make_manager(&tmp);

        // Create fake credential files in claude_dir
        write_credentials(&mgr.claude_dir, "test@example.com");
        fs::write(mgr.claude_dir.join("statsig"), b"statsig-data").unwrap();
        fs::write(mgr.home_dir.join(".claude.json"), b"{}").unwrap();

        mgr.import("personal").unwrap();

        let profile_dir = mgr.profiles_dir.join("personal");
        assert!(
            profile_dir.join(".credentials.json").exists(),
            ".credentials.json should be copied"
        );
        assert!(
            profile_dir.join("statsig").exists(),
            "statsig should be copied"
        );
        assert!(
            profile_dir.join(HOME_FILE_STORED).exists(),
            "home_.claude.json should be copied"
        );

        // Verify email content preserved
        let creds = fs::read_to_string(profile_dir.join(".credentials.json")).unwrap();
        assert!(creds.contains("test@example.com"));
    }

    #[test]
    fn test_list_profiles() {
        let tmp = TempDir::new().unwrap();
        let mgr = make_manager(&tmp);

        // Import two profiles by setting up separate credential files
        write_credentials(&mgr.claude_dir, "alice@example.com");
        mgr.import("alice").unwrap();

        // Overwrite with different email for second profile
        write_credentials(&mgr.claude_dir, "bob@example.com");
        mgr.import("bob").unwrap();

        let profiles = mgr.list().unwrap();
        assert_eq!(profiles.len(), 2, "should list 2 profiles");

        // Sorted by name: alice, bob
        assert_eq!(profiles[0].name, "alice");
        assert_eq!(profiles[0].email.as_deref(), Some("alice@example.com"));
        assert_eq!(profiles[1].name, "bob");
        assert_eq!(profiles[1].email.as_deref(), Some("bob@example.com"));
    }

    #[test]
    fn test_restore_profile_files() {
        let tmp = TempDir::new().unwrap();
        let mgr = make_manager(&tmp);

        // Set up original credential files and import
        let original_creds = r#"{"email":"original@example.com","token":"original-token"}"#;
        fs::write(mgr.claude_dir.join(".credentials.json"), original_creds).unwrap();
        fs::write(mgr.home_dir.join(".claude.json"), b"original-home").unwrap();

        mgr.import("work").unwrap();

        // Modify live files to simulate a different profile being active
        fs::write(
            mgr.claude_dir.join(".credentials.json"),
            r#"{"email":"other@example.com","token":"other-token"}"#,
        )
        .unwrap();
        fs::write(mgr.home_dir.join(".claude.json"), b"modified-home").unwrap();

        // Restore the "work" profile
        mgr.restore_files("work").unwrap();

        // Verify the original content is back
        let restored_creds =
            fs::read_to_string(mgr.claude_dir.join(".credentials.json")).unwrap();
        assert!(
            restored_creds.contains("original@example.com"),
            "credentials should be restored to original"
        );

        let restored_home = fs::read_to_string(mgr.home_dir.join(".claude.json")).unwrap();
        assert_eq!(
            restored_home, "original-home",
            "home .claude.json should be restored to original"
        );
    }

    #[test]
    fn test_restore_nonexistent_profile_returns_error() {
        let tmp = TempDir::new().unwrap();
        let mgr = make_manager(&tmp);

        let result = mgr.restore_files("nonexistent");
        assert!(result.is_err(), "restoring a missing profile should error");
        match result.unwrap_err() {
            ArcctlError::NotFound(_) => {}
            other => panic!("expected NotFound, got {:?}", other),
        }
    }
}
