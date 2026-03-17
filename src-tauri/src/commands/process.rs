use arcctl_core::process::SessionInfo;
use tauri::State;

use crate::state::AppState;

#[tauri::command]
pub fn list_sessions(state: State<'_, AppState>) -> Vec<SessionInfo> {
    state.registry.list()
}
