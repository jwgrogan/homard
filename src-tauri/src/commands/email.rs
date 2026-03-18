use arcctl_core::config::ArcctlConfig;
use serde::Serialize;
use tauri::State;

use crate::state::AppState;

#[derive(Serialize)]
pub struct EmailConfigResponse {
    pub enabled: bool,
    pub bot_address: Option<String>,
}

#[tauri::command]
pub fn get_email_config(state: State<'_, AppState>) -> Result<EmailConfigResponse, String> {
    let cfg = ArcctlConfig::load_or_default(&state.dirs.config_path());
    Ok(EmailConfigResponse {
        enabled: cfg.email.enabled,
        bot_address: cfg.email.bot_address,
    })
}

#[tauri::command]
pub fn save_email_config(
    state: State<'_, AppState>,
    enabled: bool,
    bot_address: Option<String>,
) -> Result<(), String> {
    let mut cfg = ArcctlConfig::load_or_default(&state.dirs.config_path());
    cfg.email.enabled = enabled;
    cfg.email.bot_address = bot_address;
    cfg.save(&state.dirs.config_path()).map_err(|e| e.to_string())
}
