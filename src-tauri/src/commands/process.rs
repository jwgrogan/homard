use arcctl_core::process::{kill_session as kill_process, spawn_claude, SessionInfo};
use arcctl_core::types::{Run, RunStatus, Trigger};
use chrono::Utc;
use tauri::{Emitter, Manager, State};
use uuid::Uuid;

use crate::state::AppState;

#[tauri::command]
pub fn list_sessions(state: State<'_, AppState>) -> Vec<SessionInfo> {
    state.registry.list()
}

#[tauri::command]
pub async fn spawn_session(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    prompt: String,
    directory: String,
    profile: Option<String>,
    agent: Option<String>,
) -> Result<SessionInfo, String> {
    let session_id = Uuid::new_v4().to_string();

    // Switch profile if requested (file-based restore + keychain on macOS)
    if let Some(ref profile_name) = profile {
        crate::commands::profile::switch_profile(state.clone(), profile_name.clone())?;
    }

    // Spawn the claude process
    let child = spawn_claude(&prompt, &directory, agent.as_deref(), Some("stream-json"))
        .await
        .map_err(|e| e.to_string())?;

    let pid = child.id();

    // Build SessionInfo
    let info = SessionInfo {
        id: session_id.clone(),
        agent: agent.clone(),
        profile: profile.clone(),
        directory: directory.clone(),
        trigger: Trigger::Manual,
        started_at: Utc::now(),
        pid,
    };

    // Register in process registry
    state.registry.register(&session_id, info.clone());

    // Build Run record and insert into store
    let run = Run {
        id: session_id.clone(),
        schedule_id: None,
        agent: agent.clone(),
        profile: profile.clone(),
        directory: Some(directory.clone()),
        trigger: Trigger::Manual,
        status: RunStatus::Running,
        started_at: Utc::now(),
        finished_at: None,
        duration_ms: None,
        error_message: None,
        delivery_status: None,
    };
    {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        store.insert_run(&run).map_err(|e| e.to_string())?;
    }

    // Store the child handle
    {
        let mut children = state.children.lock().map_err(|e| e.to_string())?;
        children.insert(session_id.clone(), child);
    }

    // Take the child back out for the background streaming task
    let child_for_task = {
        let mut children = state.children.lock().map_err(|e| e.to_string())?;
        children.remove(&session_id)
    };

    if let Some(child) = child_for_task {
        let session_id_bg = session_id.clone();
        let registry_bg = state.registry.clone();
        let app_handle = app.clone();

        tokio::spawn(async move {
            arcctl_core::process::stream_jsonl(child, |value| {
                let _ = app_handle.emit(
                    &format!("session-output:{}", session_id_bg),
                    value,
                );
            })
            .await;

            // After streaming ends, remove from registry
            registry_bg.remove(&session_id_bg);

            // Update run status to complete via app state
            if let Some(state_ref) = app_handle.try_state::<AppState>() {
                if let Ok(store) = state_ref.store.lock() {
                    let _ = store.complete_run(
                        &session_id_bg,
                        RunStatus::Complete,
                        None,
                    );
                }
            }

            // Emit a completion event
            let _ = app_handle.emit(
                &format!("session-done:{}", session_id_bg),
                serde_json::json!({ "session_id": session_id_bg }),
            );
        });
    }

    Ok(info)
}

#[tauri::command]
pub async fn kill_session(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    // Remove child handle
    let child = {
        let mut children = state.children.lock().map_err(|e| e.to_string())?;
        children.remove(&session_id)
    };

    if let Some(child) = child {
        kill_process(child).await;
    }

    // Mark run as killed in store
    {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        let _ = store.complete_run(&session_id, RunStatus::Killed, None);
    }

    // Remove from registry
    state.registry.remove(&session_id);

    Ok(())
}

#[tauri::command]
pub fn list_runs(
    state: State<'_, AppState>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<Run>, String> {
    let limit = limit.unwrap_or(50) as i64;
    let offset = offset.unwrap_or(0) as i64;
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.list_runs(limit, offset).map_err(|e| e.to_string())
}
