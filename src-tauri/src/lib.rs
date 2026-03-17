mod commands;
mod state;
mod tray;

use arcctl_core::config::{ArcctlConfig, ArcctlDirs};
use arcctl_core::process::ProcessRegistry;
use arcctl_core::store::Store;
use state::AppState;
use std::collections::HashMap;
use std::sync::Mutex;

pub fn run() {
    let dirs = ArcctlDirs::default_path();
    dirs.ensure_all().expect("failed to create arcctl directories");

    let config = ArcctlConfig::load_or_default(&dirs.config_path());
    let db_path = dirs.db_path();
    let store = Store::open(&db_path).expect("failed to open arcctl database");

    let app_state = AppState {
        store: Mutex::new(store),
        registry: ProcessRegistry::new(),
        config: Mutex::new(config),
        dirs,
        children: Mutex::new(HashMap::new()),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state)
        .setup(|app| {
            tray::create_tray(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // health
            commands::health::run_health_check,
            // process / sessions
            commands::process::list_sessions,
            commands::process::spawn_session,
            commands::process::kill_session,
            commands::process::list_runs,
            // profiles
            commands::profile::list_profiles,
            commands::profile::switch_profile,
            commands::profile::import_profile,
            // settings (legacy)
            commands::config::read_claude_settings_cmd,
            // settings (new)
            commands::settings::get_claude_settings,
            commands::settings::add_permission,
            commands::settings::remove_permission,
            commands::settings::set_bypass_permissions,
            commands::settings::add_mcp_server,
            commands::settings::remove_mcp_server,
            commands::settings::set_env_var,
            commands::settings::remove_env_var,
            commands::settings::get_agents,
            commands::settings::get_commands,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
