pub mod routes;

use crate::agent::r#loop::AgentLoop;
use crate::config::HomardConfig;
use crate::llm::oauth::OAuthManager;
use crate::security::SecurityManager;
use crate::store::Store;
use axum::http::Request;
use axum::middleware;
use axum::response::Response;
use axum::{
    routing::{delete, get, post},
    Router,
};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

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

async fn auth_check(
    expected: String,
    req: Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> Response {
    let auth_header = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let origin = req
        .headers()
        .get("origin")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let is_tauri = origin == "tauri://localhost";
    let is_local_dev =
        origin.starts_with("http://localhost:") || origin.starts_with("http://127.0.0.1:");
    let is_auth_callback = req.uri().path().starts_with("/auth/");
    let has_valid_token = !expected.is_empty() && auth_header == format!("Bearer {}", expected);

    if has_valid_token || is_tauri || is_local_dev || is_auth_callback || expected.is_empty() {
        next.run(req).await
    } else {
        Response::builder()
            .status(401)
            .body(axum::body::Body::from("Unauthorized"))
            .unwrap()
    }
}

pub fn create_router(state: AppState, api_token: String) -> Router {
    let token = api_token;
    Router::new()
        .route("/chat", post(routes::chat))
        .route("/conversations", get(routes::list_conversations))
        .route("/conversations/{id}", get(routes::get_conversation))
        .route("/stop", post(routes::stop_run))
        .route("/status", get(routes::status))
        .route("/activity", get(routes::activity))
        .route("/cron/health", get(routes::cron_health))
        .route(
            "/schedules",
            get(routes::list_schedules).post(routes::create_schedule),
        )
        .route("/schedules/{id}", delete(routes::delete_schedule))
        .route(
            "/settings",
            get(routes::get_settings).put(routes::update_settings),
        )
        .route("/settings/snapshot", get(routes::settings_snapshot))
        .route(
            "/settings/permissions",
            get(routes::get_permissions).put(routes::set_permissions),
        )
        .route(
            "/providers/{provider}/api-key",
            post(routes::save_provider_api_key),
        )
        .route("/auth/{provider}/start", post(routes::start_auth))
        .route("/auth/{provider}/callback", get(routes::auth_callback))
        .route("/telegram/token", post(routes::save_telegram_token))
        .route("/telegram/pair", post(routes::telegram_pair))
        .route("/telegram/status", get(routes::telegram_status))
        .route(
            "/server",
            get(routes::get_server_mode).put(routes::set_server_mode),
        )
        .route("/sessions", get(routes::list_cli_sessions))
        .route("/sessions/{id}", delete(routes::kill_cli_session))
        .route(
            "/files/{name}",
            get(routes::read_file).put(routes::write_file),
        )
        .layer(middleware::from_fn(move |req, next| {
            let t = token.clone();
            auth_check(t, req, next)
        }))
        .layer(
            CorsLayer::new()
                .allow_origin([
                    "tauri://localhost".parse().unwrap(),
                    "http://localhost:5173".parse().unwrap(),
                    "http://127.0.0.1:5173".parse().unwrap(),
                    "http://localhost:4173".parse().unwrap(),
                    "http://127.0.0.1:4173".parse().unwrap(),
                ])
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(state)
}

pub async fn serve(
    state: AppState,
    port: u16,
) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
    axum::serve(listener, create_router(state, token)).await?;
    Ok(())
}
