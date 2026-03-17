mod commands;
mod state;
mod tray;

use arcctl_core::config::{ArcctlConfig, ArcctlDirs};
use arcctl_core::process::ProcessRegistry;
use arcctl_core::store::Store;
use state::AppState;
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
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(app_state)
        .setup(|app| {
            tray::create_tray(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::health::run_health_check,
            commands::process::list_sessions,
            commands::profile::list_profiles,
            commands::config::read_claude_settings_cmd,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
