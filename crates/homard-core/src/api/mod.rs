pub mod routes;

use std::sync::Arc;
use axum::{Router, routing::{get, post, put, delete}};
use tower_http::cors::{CorsLayer, Any};
use crate::agent::r#loop::AgentLoop;
use crate::store::Store;
use crate::config::HomardConfig;
use crate::security::SecurityManager;
use crate::llm::oauth::OAuthManager;

#[derive(Clone)]
pub struct AppState {
    pub agent: Arc<AgentLoop>,
    pub store: Arc<tokio::sync::Mutex<Store>>,
    pub config: Arc<tokio::sync::RwLock<HomardConfig>>,
    pub security: Arc<SecurityManager>,
    pub oauth: Arc<OAuthManager>,
    pub homard_dir: std::path::PathBuf,
    pub stop_tx: tokio::sync::watch::Sender<bool>,
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/chat", post(routes::chat))
        .route("/conversations", get(routes::list_conversations))
        .route("/conversations/{id}", get(routes::get_conversation))
        .route("/stop", post(routes::stop_run))
        .route("/status", get(routes::status))
        .route("/activity", get(routes::activity))
        .route("/cron/health", get(routes::cron_health))
        .route("/schedules", get(routes::list_schedules).post(routes::create_schedule))
        .route("/schedules/{id}", delete(routes::delete_schedule))
        .route("/settings", get(routes::get_settings).put(routes::update_settings))
        .route("/settings/permissions", get(routes::get_permissions).put(routes::set_permissions))
        .route("/auth/{provider}/start", post(routes::start_auth))
        .route("/auth/{provider}/callback", get(routes::auth_callback))
        .route("/telegram/pair", post(routes::telegram_pair))
        .route("/telegram/status", get(routes::telegram_status))
        .route("/server", get(routes::get_server_mode).put(routes::set_server_mode))
        .route("/sessions", get(routes::list_cli_sessions))
        .route("/sessions/{id}", delete(routes::kill_cli_session))
        .route("/files/{name}", get(routes::read_file).put(routes::write_file))
        .layer(CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any))
        .with_state(state)
}

pub async fn serve(state: AppState, port: u16) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await?;
    tracing::info!("Homard daemon listening on 127.0.0.1:{}", port);
    axum::serve(listener, create_router(state)).await?;
    Ok(())
}
