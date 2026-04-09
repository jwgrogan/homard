use axum::{
    extract::{Path, State, Query},
    response::Json,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use super::AppState;
use crate::types::*;

#[derive(Deserialize)]
pub struct ChatRequest {
    pub message: String,
    pub channel: Option<String>,
}

#[derive(Serialize)]
pub struct ChatResponse {
    pub response: String,
    pub run_id: String,
}

pub async fn chat(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> std::result::Result<Json<ChatResponse>, (StatusCode, String)> {
    let channel = req.channel.unwrap_or_else(|| "chat".to_string());
    let response = state.agent.run(&channel, &req.message, Trigger::Chat).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(ChatResponse { response, run_id: String::new() }))
}

pub async fn stop_run(State(state): State<AppState>) -> StatusCode {
    let _ = state.stop_tx.send(true);
    // Reset after a moment
    let tx = state.stop_tx.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        let _ = tx.send(false);
    });
    StatusCode::OK
}

pub async fn status(State(state): State<AppState>) -> Json<DaemonStatus> {
    let config = state.config.read().await;
    Json(DaemonStatus {
        running: true,
        uptime_secs: None,
        active_provider: Some(config.active_provider.clone()),
        active_model: config.providers.get(&config.active_provider).map(|p| p.model.clone()),
        permission_level: config.permission_level.clone(),
        telegram_connected: config.telegram.enabled,
        current_run: None,
    })
}

pub async fn activity(State(state): State<AppState>) -> std::result::Result<Json<Vec<AgentRun>>, (StatusCode, String)> {
    let store = state.store.lock().await;
    let runs = store.list_runs(20).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(runs))
}

#[derive(Deserialize)]
pub struct ConversationQuery {
    pub limit: Option<usize>,
}

pub async fn list_conversations(State(_state): State<AppState>) -> Json<Vec<String>> {
    // List conversation channels from the database
    Json(vec!["chat".to_string()])
}

pub async fn get_conversation(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<ConversationQuery>,
) -> std::result::Result<Json<Vec<ChatMessage>>, (StatusCode, String)> {
    let store = state.store.lock().await;
    let limit = query.limit.unwrap_or(50);
    let history = store.get_history(&id, limit).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(history))
}

pub async fn list_schedules(State(_state): State<AppState>) -> std::result::Result<Json<Vec<Schedule>>, (StatusCode, String)> {
    let dirs = crate::config::HomardDirs::default_path();
    let schedules = crate::schedule::list_schedules(&dirs).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(schedules))
}

pub async fn create_schedule(
    State(_state): State<AppState>,
    Json(schedule): Json<Schedule>,
) -> std::result::Result<Json<Schedule>, (StatusCode, String)> {
    let dirs = crate::config::HomardDirs::default_path();
    crate::schedule::save_schedule(&dirs, &schedule).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(schedule))
}

pub async fn delete_schedule(
    State(_state): State<AppState>,
    Path(id): Path<String>,
) -> StatusCode {
    let dirs = crate::config::HomardDirs::default_path();
    let _ = crate::schedule::delete_schedule(&dirs, &id);
    StatusCode::OK
}

pub async fn cron_health(State(state): State<AppState>) -> std::result::Result<Json<Vec<crate::types::CronHealth>>, (StatusCode, String)> {
    let store = state.store.lock().await;
    store.get_cron_health().map(Json).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

pub async fn get_settings(State(state): State<AppState>) -> Json<serde_json::Value> {
    let config = state.config.read().await;
    Json(serde_json::to_value(&*config).unwrap_or_default())
}

pub async fn update_settings(
    State(state): State<AppState>,
    Json(new_config): Json<crate::config::HomardConfig>,
) -> StatusCode {
    let dirs = crate::config::HomardDirs::default_path();
    *state.config.write().await = new_config.clone();
    let _ = new_config.save(&dirs.config_path());
    StatusCode::OK
}

pub async fn get_permissions(State(state): State<AppState>) -> Json<PermissionLevel> {
    Json(state.security.permission_level())
}

#[derive(Deserialize)]
pub struct PermissionRequest {
    pub level: PermissionLevel,
}

pub async fn set_permissions(
    State(state): State<AppState>,
    Json(req): Json<PermissionRequest>,
) -> StatusCode {
    state.security.set_permission_level(req.level).await;
    StatusCode::OK
}

pub async fn start_auth(
    State(state): State<AppState>,
    Path(provider): Path<String>,
) -> std::result::Result<Json<serde_json::Value>, (StatusCode, String)> {
    let (auth_url, _port) = state.oauth.start_auth(&provider).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::json!({ "auth_url": auth_url })))
}

#[derive(Deserialize)]
pub struct AuthCallbackQuery {
    pub code: String,
}

pub async fn auth_callback(
    State(state): State<AppState>,
    Path(provider): Path<String>,
    Query(query): Query<AuthCallbackQuery>,
) -> std::result::Result<Json<serde_json::Value>, (StatusCode, String)> {
    let verifier = state.oauth.take_verifier(&provider).await
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "No pending auth flow".to_string()))?;
    let tokens = state.oauth.exchange_code(&provider, &query.code, &verifier, 17700).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::json!({"status": "connected", "expires_at": tokens.expires_at})))
}

pub async fn telegram_pair(State(_state): State<AppState>) -> std::result::Result<Json<serde_json::Value>, (StatusCode, String)> {
    let dirs = crate::config::HomardDirs::default_path();
    let code = crate::config::generate_pairing_code(&dirs)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::json!({"code": code})))
}

pub async fn telegram_status(State(state): State<AppState>) -> Json<serde_json::Value> {
    let config = state.config.read().await;
    Json(serde_json::json!({
        "enabled": config.telegram.enabled,
        "paired_chats": config.telegram.paired_chat_ids.len(),
    }))
}

pub async fn get_server_mode(State(state): State<AppState>) -> Json<serde_json::Value> {
    let config = state.config.read().await;
    let home = dirs::home_dir().unwrap_or_default();
    let plist_exists = home.join("Library/LaunchAgents/com.homard.daemon.plist").exists();
    Json(serde_json::json!({
        "mode": config.server_mode,
        "launchd_installed": plist_exists,
    }))
}

pub async fn set_server_mode(
    State(state): State<AppState>,
    Json(req): Json<serde_json::Value>,
) -> std::result::Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mode = req.get("mode").and_then(|m| m.as_str()).unwrap_or("off");
    let dirs = crate::config::HomardDirs::default_path();

    let mut config = state.config.write().await;

    if mode == "on" {
        config.server_mode = crate::types::ServerMode::On;
        config.save(&dirs.config_path()).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        // Install launchd plist
        let bin_path = crate::schedule::resolve_homard_bin().unwrap_or_else(|_| "homard".to_string());
        let home = dirs::home_dir().unwrap_or_default();
        let uid = std::process::Command::new("id")
            .arg("-u")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "501".to_string());
        let plist_content = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.homard.daemon</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>serve</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>ThrottleInterval</key>
    <integer>10</integer>
    <key>StandardOutPath</key>
    <string>{}/.homard/logs/daemon.stdout.log</string>
    <key>StandardErrorPath</key>
    <string>{}/.homard/logs/daemon.stderr.log</string>
    <key>EnvironmentVariables</key>
    <dict>
        <key>PATH</key>
        <string>/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin</string>
    </dict>
</dict>
</plist>"#,
            bin_path, home.display(), home.display(),
        );

        let plist_path = home.join("Library/LaunchAgents/com.homard.daemon.plist");
        std::fs::create_dir_all(plist_path.parent().unwrap()).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        std::fs::write(&plist_path, plist_content).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        // Bootstrap the plist (modern launchctl)
        let _ = std::process::Command::new("launchctl")
            .args(["bootstrap", &format!("gui/{}", uid), &plist_path.to_string_lossy()])
            .output();

        Ok(Json(serde_json::json!({"status": "on", "message": "Server mode enabled. Homard will restart on crash and start on boot."})))
    } else {
        config.server_mode = crate::types::ServerMode::Off;
        config.save(&dirs.config_path()).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        // Remove launchd plist
        let home = dirs::home_dir().unwrap_or_default();
        let plist_path = home.join("Library/LaunchAgents/com.homard.daemon.plist");
        if plist_path.exists() {
            let uid = std::process::Command::new("id")
                .arg("-u")
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|| "501".to_string());
            let _ = std::process::Command::new("launchctl")
                .args(["bootout", &format!("gui/{}", uid), &plist_path.to_string_lossy()])
                .output();
            let _ = std::fs::remove_file(&plist_path);
        }

        Ok(Json(serde_json::json!({"status": "off", "message": "Server mode disabled. Daemon will stop when you close it."})))
    }
}

pub async fn list_cli_sessions(State(state): State<AppState>) -> std::result::Result<Json<Vec<crate::types::CliSession>>, (StatusCode, String)> {
    let store = state.store.lock().await;
    let sessions = store.list_sessions(20).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(sessions))
}

pub async fn kill_cli_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> std::result::Result<StatusCode, (StatusCode, String)> {
    let store = state.store.lock().await;
    let sessions = store.get_running_sessions().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    drop(store);

    let session = sessions.iter().find(|s| s.id.starts_with(&id))
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Session not found".to_string()))?;

    if let Some(pid) = session.pid {
        unsafe { libc::kill(pid as i32, libc::SIGTERM); }
        let store = state.store.lock().await;
        let _ = store.complete_session(&session.id, crate::types::SessionStatus::Killed, None, Some("Killed via API"));
    }

    Ok(StatusCode::OK)
}

pub async fn read_file(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> std::result::Result<String, (StatusCode, String)> {
    let path = state.homard_dir.join(&name);
    tokio::fs::read_to_string(&path).await
        .map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))
}

pub async fn write_file(
    State(state): State<AppState>,
    Path(name): Path<String>,
    body: String,
) -> StatusCode {
    let path = state.homard_dir.join(&name);
    match tokio::fs::write(&path, body).await {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
