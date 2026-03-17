use arcctl_core::profile::ProfileManager;
use arcctl_core::types::Profile;
use tauri::State;

use crate::state::AppState;

#[tauri::command]
pub fn list_profiles(state: State<'_, AppState>) -> Result<Vec<Profile>, String> {
    let dirs = state.dirs.clone();
    let claude_dir = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".claude");
    let home_dir = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));

    let manager = ProfileManager::new(dirs.profiles_dir(), claude_dir, home_dir);
    manager.list().map_err(|e| e.to_string())
}
