mod commands;
mod state;
mod telegram_poller;
mod tray;

use arcctl_core::config::{ArcctlConfig, ArcctlDirs};
use arcctl_core::session_monitor::SessionMonitor;
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
        session_monitor: SessionMonitor::new(),
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
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            tray::create_tray(app)?;
            use tauri_plugin_global_shortcut::GlobalShortcutExt;
            app.global_shortcut().register("CmdOrCtrl+Shift+C")?;

            // Background task: poll PIDs of running sessions every 5 seconds
            // and mark them as stopped when the process is no longer alive.
            let app_handle = app.handle().clone();
            tokio::spawn(async move {
                use tauri::Manager;
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    if let Some(state) = app_handle.try_state::<AppState>() {
                        if let Ok(store) = state.store.lock() {
                            if let Ok(running) = store.list_running_sessions() {
                                for session in running {
                                    if let Some(pid) = session.terminal_pid {
                                        if !arcctl_core::process::is_pid_alive(pid) {
                                            let _ = store.complete_session(
                                                &session.id,
                                                arcctl_core::types::SessionStatus::Stopped,
                                                None::<String>,
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            });

            {
                use tauri::Manager;
                if let Some(window) = app.get_webview_window("main") {
                let app_handle = app.handle().clone();
                window.on_window_event(move |event| {
                    use tauri::WindowEvent;
                    match event {
                        WindowEvent::Focused(true) => {
                            #[cfg(target_os = "macos")]
                            {
                                let _ = app_handle.set_activation_policy(tauri::ActivationPolicy::Regular);
                            }
                        }
                        WindowEvent::CloseRequested { api, .. } => {
                            // Don't actually close — just hide the window
                            api.prevent_close();
                            if let Some(w) = app_handle.get_webview_window("main") {
                                let _ = w.hide();
                            }
                            #[cfg(target_os = "macos")]
                            {
                                let _ = app_handle.set_activation_policy(tauri::ActivationPolicy::Accessory);
                            }
                        }
                        _ => {}
                    }
                });
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // health
            commands::health::run_health_check,
            // process / sessions
            commands::process::list_sessions,
            commands::process::list_sessions_filtered,
            commands::process::spawn_session,
            commands::process::kill_session,
            commands::process::resume_session,
            commands::process::fork_session,
            commands::process::list_runs,
            commands::process::get_session_tree,
            // profiles
            commands::profile::list_profiles,
            commands::profile::switch_profile,
            commands::profile::import_profile,
            commands::profile::check_profile_health,
            commands::profile::check_all_profile_health,
            commands::profile::detect_claude_switch,
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
            // mcp sync
            commands::mcp_sync::list_managed_mcps,
            commands::mcp_sync::add_managed_mcp,
            commands::mcp_sync::remove_managed_mcp,
            commands::mcp_sync::sync_all_mcps,
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
