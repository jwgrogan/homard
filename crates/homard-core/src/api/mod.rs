pub mod routes;

use std::sync::Arc;
use axum::{Router, routing::{get, post, put, delete}};
use axum::middleware;
use axum::http::Request;
use axum::response::Response;
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

async fn auth_middleware(
    req: Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> Response {
    let dirs = crate::config::HomardDirs::default_path();
    let token_path = dirs.root().join("api.token");
    let expected = std::fs::read_to_string(&token_path).unwrap_or_default().trim().to_string();

    if expected.is_empty() {
        return next.run(req).await; // No token configured, skip auth
    }

    let auth_header = req.headers().get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if auth_header == format!("Bearer {}", expected) || auth_header.is_empty() && req.uri().path().starts_with("/auth/") {
        // Allow auth callback without token (browser redirect)
        next.run(req).await
    } else {
        axum::response::Response::builder()
            .status(401)
            .body(axum::body::Body::from("Unauthorized"))
            .unwrap()
    }
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
        .layer(middleware::from_fn(auth_middleware))
        .layer(CorsLayer::new()
            .allow_origin(["tauri://localhost".parse().unwrap(), "http://localhost:5173".parse().unwrap()])
            .allow_methods(Any)
            .allow_headers(Any))
        .with_state(state)
}

pub async fn serve(state: AppState, port: u16) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Generate API auth token
    let token: String = {
        use rand::Rng;
        rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(48)
            .map(char::from)
            .collect()
    };
    let token_path = state.homard_dir.join("api.token");
    let _ = std::fs::write(&token_path, &token);
    // Set file permissions to owner-only
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&token_path, std::fs::Permissions::from_mode(0o600));
    }
    tracing::info!("API auth token written to {}", token_path.display());

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await?;
    tracing::info!("Homard daemon listening on 127.0.0.1:{}", port);
    axum::serve(listener, create_router(state)).await?;
    Ok(())
}
