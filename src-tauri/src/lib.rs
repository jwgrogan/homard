mod commands;
mod state;
mod telegram_poller;
mod tray;

use arcctl_core::config::{ArcctlConfig, ArcctlDirs};
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
        config: Mutex::new(config),
        dirs,
        preferred_terminal: Mutex::new(None),
        telegram_poll_handle: Mutex::new(None),
        telegram_cancel: Mutex::new(None),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    use tauri::{Emitter, Manager};
                    use tauri_plugin_global_shortcut::ShortcutState;
                    if event.state() == ShortcutState::Pressed {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                            let _ = window.emit("open-quick-prompt", ());
                        }
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state)
        .setup(|app| {
            tray::create_tray(app)?;
            use tauri_plugin_global_shortcut::GlobalShortcutExt;
            app.global_shortcut().register("CmdOrCtrl+Shift+C")?;
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
            commands::settings::set_default_mode,
            commands::settings::add_mcp_server,
            commands::settings::remove_mcp_server,
            commands::settings::set_env_var,
            commands::settings::remove_env_var,
            commands::settings::get_agents,
            commands::settings::get_commands,
            // scheduler
            commands::scheduler::create_schedule,
            commands::scheduler::update_schedule,
            commands::scheduler::delete_schedule,
            commands::scheduler::get_schedule,
            commands::scheduler::list_schedules,
            commands::scheduler::pause_schedule,
            commands::scheduler::resume_schedule,
            commands::scheduler::discover_launchd_jobs,
            commands::scheduler::import_launchd_job_cmd,
            commands::scheduler::list_schedule_runs,
            // telegram
            commands::telegram::save_telegram_token_cmd,
            commands::telegram::verify_telegram_token,
            commands::telegram::get_telegram_status,
            commands::telegram::add_paired_chat_cmd,
            commands::telegram::remove_paired_chat_cmd,
            commands::telegram::generate_pairing_code_cmd,
            commands::telegram::start_telegram_polling,
            commands::telegram::stop_telegram_polling,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
