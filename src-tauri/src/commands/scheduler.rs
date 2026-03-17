use arcctl_core::launchd::DiscoveredPlist;
use arcctl_core::types::{Run, Schedule};
use tauri::State;

use crate::state::AppState;

#[tauri::command]
pub fn create_schedule(state: State<'_, AppState>, schedule: Schedule) -> Result<(), String> {
    arcctl_core::schedule::create_schedule(&state.dirs, &schedule).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_schedule(state: State<'_, AppState>, schedule: Schedule) -> Result<(), String> {
    arcctl_core::schedule::update_schedule(&state.dirs, &schedule).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_schedule(state: State<'_, AppState>, id: String) -> Result<(), String> {
    arcctl_core::schedule::delete_schedule(&state.dirs, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_schedule(state: State<'_, AppState>, id: String) -> Result<Schedule, String> {
    arcctl_core::schedule::load_schedule(&state.dirs, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_schedules(state: State<'_, AppState>) -> Result<Vec<Schedule>, String> {
    arcctl_core::schedule::list_schedules(&state.dirs).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn pause_schedule(state: State<'_, AppState>, id: String) -> Result<(), String> {
    arcctl_core::schedule::pause_schedule(&state.dirs, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn resume_schedule(state: State<'_, AppState>, id: String) -> Result<(), String> {
    arcctl_core::schedule::resume_schedule(&state.dirs, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn discover_launchd_jobs(state: State<'_, AppState>) -> Result<Vec<DiscoveredPlist>, String> {
    arcctl_core::schedule::discover_importable_jobs(&state.dirs).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn import_launchd_job_cmd(
    state: State<'_, AppState>,
    label: String,
) -> Result<Schedule, String> {
    let jobs = arcctl_core::schedule::discover_importable_jobs(&state.dirs)
        .map_err(|e| e.to_string())?;

    let discovered = jobs
        .into_iter()
        .find(|p| p.label == label)
        .ok_or_else(|| format!("launchd job not found: {}", label))?;

    arcctl_core::schedule::import_launchd_job(&state.dirs, &discovered).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_schedule_runs(
    state: State<'_, AppState>,
    schedule_id: String,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<Run>, String> {
    let limit = limit.unwrap_or(50);
    let offset = offset.unwrap_or(0);
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store
        .list_runs_by_schedule(&schedule_id, limit, offset)
        .map_err(|e| e.to_string())
}
