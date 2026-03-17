use arcctl_core::profile::ProfileManager;
use arcctl_core::types::HealthStatus;
use tauri::State;

use crate::state::AppState;

#[tauri::command]
pub async fn run_health_check(state: State<'_, AppState>) -> Result<HealthStatus, String> {
    let mut status = arcctl_core::health::run_health_check().await;

    // Augment with active profile info from ProfileManager
    let dirs = state.dirs.clone();
    let claude_dir = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".claude");
    let home_dir = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));

    let manager = ProfileManager::new(dirs.profiles_dir(), claude_dir, home_dir);
    if let Ok(profiles) = manager.list() {
        status.active_profile = profiles.into_iter().find(|p| p.is_active);
    }

    Ok(status)
}
