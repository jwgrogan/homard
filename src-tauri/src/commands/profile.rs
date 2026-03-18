use arcctl_core::profile::ProfileManager;
use arcctl_core::types::{CredentialHealth, Profile};
use tauri::State;

use crate::state::AppState;

fn make_manager(state: &AppState) -> ProfileManager {
    let dirs = state.dirs.clone();
    let claude_dir = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".claude");
    let home_dir = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    ProfileManager::new(dirs.profiles_dir(), claude_dir, home_dir)
}

#[tauri::command]
pub fn list_profiles(state: State<'_, AppState>) -> Result<Vec<Profile>, String> {
    let manager = make_manager(&state);
    manager.list().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn switch_profile(state: State<'_, AppState>, name: String) -> Result<(), String> {
    let manager = make_manager(&state);

    // Restore credential files to live locations
    manager.restore_files(&name).map_err(|e| e.to_string())?;

    // On macOS, read .credentials.json from the profile dir and write to keychain
    #[cfg(target_os = "macos")]
    {
        let profile_dir = state.dirs.profiles_dir().join(&name);
        let creds_path = profile_dir.join(".credentials.json");
        if creds_path.exists() {
            let data =
                std::fs::read_to_string(&creds_path).map_err(|e| e.to_string())?;
            arcctl_core::profile::keychain::write_credentials(&data)
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

#[tauri::command]
pub fn import_profile(
    state: State<'_, AppState>,
    name: String,
) -> Result<Profile, String> {
    let manager = make_manager(&state);
    manager.import(&name).map_err(|e| e.to_string())?;

    // Return the newly created profile by looking it up from the list
    let profiles = manager.list().map_err(|e| e.to_string())?;
    profiles
        .into_iter()
        .find(|p| p.name == name)
        .ok_or_else(|| format!("Profile '{}' not found after import", name))
}

#[tauri::command]
pub fn check_profile_health(
    state: State<'_, AppState>,
    name: String,
) -> CredentialHealth {
    let mgr = make_manager(&state);
    mgr.check_health(&name)
}

#[tauri::command]
pub fn check_all_profile_health(
    state: State<'_, AppState>,
) -> Vec<(String, CredentialHealth)> {
    let mgr = make_manager(&state);
    match mgr.list() {
        Ok(profiles) => profiles
            .iter()
            .map(|p| (p.name.clone(), mgr.check_health(&p.name)))
            .collect(),
        Err(_) => Vec::new(),
    }
}

#[tauri::command]
pub fn delete_profile(state: State<'_, AppState>, name: String) -> Result<(), String> {
    let manager = make_manager(&state);
    manager.delete(&name).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn detect_claude_switch() -> bool {
    std::path::Path::new("/usr/local/bin/claude-switch").exists()
        || std::path::Path::new("/opt/homebrew/bin/claude-switch").exists()
}
