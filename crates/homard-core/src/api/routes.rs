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
