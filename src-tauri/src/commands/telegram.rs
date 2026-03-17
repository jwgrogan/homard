use arcctl_core::config::{
    add_paired_chat, generate_pairing_code, remove_paired_chat, ArcctlConfig,
};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::AppState;

#[derive(Serialize, Deserialize, Debug)]
pub struct TelegramStatus {
    pub enabled: bool,
    pub bot_username: Option<String>,
    pub paired_chat_ids: Vec<String>,
    pub is_polling: bool,
}

/// Save a Telegram bot token to Keychain and enable Telegram in config.
#[tauri::command]
pub async fn save_telegram_token_cmd(
    token: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use arcctl_core::config::save_telegram_token;
        let dirs = state.dirs.clone();
        save_telegram_token(&dirs, &token).map_err(|e| e.to_string())?;
        let new_config = ArcctlConfig::load_or_default(&dirs.config_path());
        let mut config = state.config.lock().unwrap();
        *config = new_config;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (token, state);
        Err("Telegram not supported on this platform".to_string())
    }
}

/// Verify a bot token by calling Telegram getMe. Returns bot username on success.
#[tauri::command]
pub async fn verify_telegram_token(token: String) -> Result<String, String> {
    let client = arcctl_core::telegram::TelegramClient::new(&token);
    client.verify().await.map_err(|e| e.to_string())
}

/// Get current Telegram bridge status.
#[tauri::command]
pub async fn get_telegram_status(state: State<'_, AppState>) -> Result<TelegramStatus, String> {
    let dirs = state.dirs.clone();
    let config = ArcctlConfig::load_or_default(&dirs.config_path());

    let is_polling = {
        let handle = state.telegram_poll_handle.lock().unwrap();
        handle.as_ref().map_or(false, |h| !h.is_finished())
    };

    Ok(TelegramStatus {
        enabled: config.telegram.enabled,
        bot_username: None, // Use verify_telegram_token command for live connectivity check
        paired_chat_ids: config.telegram.paired_chat_ids.clone(),
        is_polling,
    })
}

/// Add a chat_id to the paired list.
#[tauri::command]
pub async fn add_paired_chat_cmd(
    chat_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let dirs = state.dirs.clone();
    add_paired_chat(&dirs, &chat_id).map_err(|e| e.to_string())?;
    let new_config = ArcctlConfig::load_or_default(&dirs.config_path());
    let mut config = state.config.lock().unwrap();
    *config = new_config;
    Ok(())
}

/// Remove a chat_id from the paired list.
#[tauri::command]
pub async fn remove_paired_chat_cmd(
    chat_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let dirs = state.dirs.clone();
    remove_paired_chat(&dirs, &chat_id).map_err(|e| e.to_string())?;
    let new_config = ArcctlConfig::load_or_default(&dirs.config_path());
    let mut config = state.config.lock().unwrap();
    *config = new_config;
    Ok(())
}

/// Generate a pairing code for Telegram bot pairing.
#[tauri::command]
pub async fn generate_pairing_code_cmd(
    state: State<'_, AppState>,
) -> Result<String, String> {
    let dirs = state.dirs.clone();
    generate_pairing_code(&dirs).map_err(|e| e.to_string())
}

/// Start the Telegram polling background task.
#[tauri::command]
pub async fn start_telegram_polling(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Stop any existing poller first
    stop_polling_inner(&state);

    let dirs = state.dirs.clone();
    let cancel_token = tokio_util::sync::CancellationToken::new();
    let cancel_clone = cancel_token.clone();

    let handle = tokio::spawn(async move {
        if let Err(e) = crate::telegram_poller::run_poller(dirs, app_handle, cancel_clone).await {
            tracing::error!("Telegram poller error: {}", e);
        }
    });

    *state.telegram_poll_handle.lock().unwrap() = Some(handle);
    *state.telegram_cancel.lock().unwrap() = Some(cancel_token);

    Ok(())
}

/// Stop the Telegram polling background task.
#[tauri::command]
pub async fn stop_telegram_polling(state: State<'_, AppState>) -> Result<(), String> {
    stop_polling_inner(&state);
    Ok(())
}

fn stop_polling_inner(state: &AppState) {
    if let Some(token) = state.telegram_cancel.lock().unwrap().take() {
        token.cancel();
    }
    let _ = state.telegram_poll_handle.lock().unwrap().take();
}
