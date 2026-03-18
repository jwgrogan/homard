use arcctl_core::provider::ProviderId;
use arcctl_core::terminal::TerminalApp;
use arcctl_core::types::{Run, Session, SessionStatus, Trigger};
use chrono::Utc;
use std::str::FromStr;
use tauri::State;
use uuid::Uuid;

use crate::state::AppState;

fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[tauri::command]
pub fn list_sessions(state: State<'_, AppState>) -> Result<Vec<Session>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.list_sessions(100, 0).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn spawn_session(
    state: State<'_, AppState>,
    directory: String,
    provider: String,
    profile: Option<String>,
    agent: Option<String>,
    prompt: Option<String>,
) -> Result<Session, String> {
    let session_id = Uuid::new_v4().to_string();

    let provider_id = ProviderId::from_str(&provider).map_err(|e| e.to_string())?;
    let cli_command = provider_id.cli_command();

    // Determine if we should generate a CLI session ID
    let cli_session_id = if provider_id.supports_session_id_flag() {
        Some(Uuid::new_v4().to_string())
    } else {
        None
    };

    // Build the shell command
    let mut parts = vec![format!("cd {}", shell_escape(&directory))];
    let mut cmd_parts = vec![cli_command.to_string()];

    if let Some(ref sid) = cli_session_id {
        cmd_parts.push("--session-id".to_string());
        cmd_parts.push(shell_escape(sid));
    }

    if let Some(ref ag) = agent {
        cmd_parts.push("--agent".to_string());
        cmd_parts.push(shell_escape(ag));
    }

    if let Some(ref p) = prompt {
        cmd_parts.push("-p".to_string());
        cmd_parts.push(shell_escape(p));
    }

    parts.push(cmd_parts.join(" "));
    let shell_command = parts.join(" && ");

    // Determine which terminal to use
    let terminal = {
        let preferred = state.preferred_terminal.lock().map_err(|e| e.to_string())?;
        if let Some(t) = preferred.clone() {
            t
        } else {
            TerminalApp::detect_installed()
                .into_iter()
                .next()
                .ok_or_else(|| "No terminal found".to_string())?
        }
    };

    // Launch the terminal
    let terminal_pid = terminal.launch(&shell_command).map_err(|e| e.to_string())?;

    // Build Session record
    let session = Session {
        id: session_id.clone(),
        cli_session_id,
        profile_name: profile,
        provider,
        directory: Some(directory),
        terminal_pid,
        trigger: Trigger::Manual,
        status: SessionStatus::Running,
        started_at: Utc::now(),
        ended_at: None,
        duration_ms: None,
        error_message: None,
        agent,
        parent_session_id: None,
        forked_from: None,
    };

    // Insert into store
    {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        store.insert_session(&session).map_err(|e| e.to_string())?;
    }

    Ok(session)
}

#[tauri::command]
pub async fn kill_session(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    // Fetch the session to get the terminal_pid
    let terminal_pid = {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        store
            .get_session(&session_id)
            .map_err(|e| e.to_string())?
            .and_then(|s| s.terminal_pid)
    };

    // Send SIGTERM to the terminal PID if available
    if let Some(pid) = terminal_pid {
        unsafe {
            libc::kill(pid as libc::pid_t, libc::SIGTERM);
        }
    }

    // Mark session as killed in store
    {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        let _ = store.complete_session(&session_id, SessionStatus::Killed, None);
    }

    Ok(())
}

#[tauri::command]
pub fn resume_session(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<Session, String> {
    let original = {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        store.get_session(&session_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Session not found".to_string())?
    };

    let provider_id = ProviderId::from_str(&original.provider)
        .map_err(|e| e.to_string())?;

    let cli_session_id = original.cli_session_id
        .ok_or_else(|| "No CLI session ID — cannot resume".to_string())?;

    let dir = original.directory.unwrap_or_else(|| ".".to_string());

    let cmd = format!("cd {} && {} {} {}",
        shell_escape(&dir),
        provider_id.cli_command(),
        provider_id.resume_flag(),
        cli_session_id,
    );

    let terminal = {
        let pref = state.preferred_terminal.lock().map_err(|e| e.to_string())?;
        pref.clone().unwrap_or_else(|| {
            TerminalApp::detect_installed().into_iter().next()
                .unwrap_or(TerminalApp::AppleTerminal)
        })
    };

    let terminal_pid = terminal.launch(&cmd).map_err(|e| e.to_string())?;

    let new_session_id = Uuid::new_v4().to_string();
    let session = Session {
        id: new_session_id,
        cli_session_id: Some(cli_session_id),
        profile_name: original.profile_name,
        provider: original.provider,
        directory: Some(dir),
        terminal_pid,
        trigger: Trigger::Manual,
        status: SessionStatus::Running,
        started_at: Utc::now(),
        ended_at: None,
        duration_ms: None,
        error_message: None,
        agent: original.agent,
        parent_session_id: Some(session_id),
        forked_from: None,
    };

    {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        store.insert_session(&session).map_err(|e| e.to_string())?;
    }

    Ok(session)
}

#[tauri::command]
pub fn fork_session(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<Session, String> {
    let original = {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        store.get_session(&session_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Session not found".to_string())?
    };

    let provider_id = ProviderId::from_str(&original.provider)
        .map_err(|e| e.to_string())?;

    let cli_session_id = original.cli_session_id
        .ok_or_else(|| "No CLI session ID — cannot fork".to_string())?;

    let dir = original.directory.unwrap_or_else(|| ".".to_string());

    // Claude supports --fork-session, Gemini starts a new session
    let cmd = match provider_id {
        ProviderId::Claude => format!("cd {} && {} {} {} --fork-session",
            shell_escape(&dir),
            provider_id.cli_command(),
            provider_id.resume_flag(),
            cli_session_id,
        ),
        ProviderId::Gemini => format!("cd {} && {}",
            shell_escape(&dir),
            provider_id.cli_command(),
        ),
    };

    let terminal = {
        let pref = state.preferred_terminal.lock().map_err(|e| e.to_string())?;
        pref.clone().unwrap_or_else(|| {
            TerminalApp::detect_installed().into_iter().next()
                .unwrap_or(TerminalApp::AppleTerminal)
        })
    };

    let terminal_pid = terminal.launch(&cmd).map_err(|e| e.to_string())?;

    let new_session_id = Uuid::new_v4().to_string();
    let session = Session {
        id: new_session_id,
        cli_session_id: None, // Fork creates a new CLI session ID
        profile_name: original.profile_name,
        provider: original.provider,
        directory: Some(dir),
        terminal_pid,
        trigger: Trigger::Manual,
        status: SessionStatus::Running,
        started_at: Utc::now(),
        ended_at: None,
        duration_ms: None,
        error_message: None,
        agent: original.agent,
        parent_session_id: None,
        forked_from: Some(session_id),
    };

    {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        store.insert_session(&session).map_err(|e| e.to_string())?;
    }

    Ok(session)
}

#[tauri::command]
pub fn get_session_tree(
    state: State<'_, AppState>,
    session_id: String,
) -> Option<arcctl_core::parsers::SessionTree> {
    state.session_monitor.get_tree(&session_id)
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
