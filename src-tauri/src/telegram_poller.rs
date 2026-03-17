use arcctl_core::config::{
    add_paired_chat, validate_pairing_code, ArcctlConfig, ArcctlDirs,
};
use arcctl_core::telegram::TelegramClient;
use tauri::Emitter;
use teloxide::payloads::GetUpdatesSetters;
use teloxide::prelude::Requester;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

#[derive(Debug, PartialEq)]
pub enum Command {
    Status,
    Pair(String),
    Stop(String),
}

pub fn parse_command(text: &str) -> Option<Command> {
    let text = text.trim();
    if text.starts_with("/status") {
        return Some(Command::Status);
    }
    if text.starts_with("/pair") {
        let code = text.strip_prefix("/pair").unwrap_or("").trim().to_string();
        return Some(Command::Pair(code));
    }
    if text.starts_with("/stop") {
        let session_id = text.strip_prefix("/stop").unwrap_or("").trim().to_string();
        return Some(Command::Stop(session_id));
    }
    None
}

pub async fn run_poller(
    dirs: ArcctlDirs,
    app_handle: tauri::AppHandle,
    cancel: CancellationToken,
) -> Result<(), String> {
    // Non-macOS platforms don't have keychain support; bail out early.
    #[cfg(not(target_os = "macos"))]
    {
        warn!("Telegram poller: only supported on macOS");
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        // Get token from config — macOS only (keychain)
        use arcctl_core::config::get_telegram_token;
        let token = match get_telegram_token(&dirs) {
            Ok(Some(t)) => t,
            Ok(None) => {
                warn!("Telegram poller: no token configured, stopping");
                return Ok(());
            }
            Err(e) => {
                error!("Telegram poller: failed to read token: {}", e);
                return Err(e.to_string());
            }
        };

        let client = TelegramClient::new(&token);
        let bot = teloxide::Bot::new(&token);
        info!("Telegram poller started");

        let mut offset: i32 = 0;

        loop {
            if cancel.is_cancelled() {
                info!("Telegram poller: cancelled");
                break;
            }

            // Long-poll for updates (10s timeout)
            let updates = match bot.get_updates().offset(offset).timeout(10).await {
                Ok(updates) => updates,
                Err(e) => {
                    warn!("Telegram getUpdates error: {}", e);
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    continue;
                }
            };

            // Load config once per polling round (not per message)
            let config = ArcctlConfig::load_or_default(&dirs.config_path());

            for update in &updates {
                // Advance offset past this update so it won't be returned again
                offset = update.id.as_offset();

                // Extract chat_id and text from message updates only
                let (chat_id, text) = match &update.kind {
                    teloxide::types::UpdateKind::Message(msg) => {
                        let chat_id = msg.chat.id.0;
                        let text = msg.text().unwrap_or("").to_string();
                        (chat_id, text)
                    }
                    _ => continue,
                };

                let chat_id_str = chat_id.to_string();
                let is_paired = config.telegram.paired_chat_ids.contains(&chat_id_str);

                match parse_command(&text) {
                Some(Command::Pair(code)) => {
                    handle_pair(&dirs, &client, &app_handle, chat_id, &code).await;
                }
                Some(Command::Status) if is_paired => {
                    handle_status(&client, chat_id).await;
                }
                Some(Command::Stop(session_id)) if is_paired => {
                    let _ = app_handle.emit("telegram-stop-session", session_id.clone());
                    let reply = if session_id.is_empty() {
                        "⏹ Sent stop signal to all running sessions.".to_string()
                    } else {
                        format!("⏹ Sent stop signal to session `{}`.", session_id)
                    };
                    let _ = client.send_message(chat_id, &reply).await;
                }
                Some(Command::Status) | Some(Command::Stop(_)) => {
                    // Recognized command but sender not paired
                    send_pairing_required(&client, chat_id).await;
                }
                None if !is_paired => {
                    // Unknown sender, non-command message
                    send_pairing_required(&client, chat_id).await;
                }
                None => {
                    // Paired sender, non-command message — give a hint
                    let _ = client
                        .send_message(
                            chat_id,
                            "I'll notify you when jobs complete. Commands: /status, /stop [session_id]",
                        )
                        .await;
                }
            }
        }
    } // end loop

        info!("Telegram poller stopped");
        Ok(())
    } // end #[cfg(target_os = "macos")]
}

async fn handle_pair(
    dirs: &ArcctlDirs,
    client: &TelegramClient,
    app_handle: &tauri::AppHandle,
    chat_id: i64,
    code: &str,
) {
    if code.is_empty() {
        let _ = client
            .send_message(
                chat_id,
                "Usage: `/pair <code>` — get your pairing code from the arcctl app.",
            )
            .await;
        return;
    }

    match validate_pairing_code(dirs, code) {
        Ok(true) => match add_paired_chat(dirs, &chat_id.to_string()) {
            Ok(()) => {
                let _ = client
                    .send_message(
                        chat_id,
                        "✅ Paired! You'll receive job notifications here. Use /status to check the assistant.",
                    )
                    .await;
                let _ = app_handle.emit("telegram-paired", chat_id.to_string());
                info!("Telegram: chat {} successfully paired", chat_id);
            }
            Err(e) => {
                let _ = client
                    .send_message(chat_id, &format!("Pairing failed: {}", e))
                    .await;
            }
        },
        Ok(false) => {
            let _ = client
                .send_message(
                    chat_id,
                    "❌ Invalid or expired pairing code. Generate a new one in the arcctl app.",
                )
                .await;
        }
        Err(e) => {
            error!("Pairing validation error: {}", e);
        }
    }
}

async fn handle_status(client: &TelegramClient, chat_id: i64) {
    let _ = client
        .send_message(
            chat_id,
            "✅ arcctl is running. Use the app to see session details.\n\nCommands: /status /stop [session_id]",
        )
        .await;
}

async fn send_pairing_required(client: &TelegramClient, chat_id: i64) {
    let msg = "🔒 Open arcctl and generate a pairing code, then send `/pair <code>`.".to_string();
    let _ = client.send_message(chat_id, &msg).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_command_status() {
        assert_eq!(parse_command("/status"), Some(Command::Status));
    }

    #[test]
    fn test_parse_command_pair() {
        assert_eq!(
            parse_command("/pair ABCD1234"),
            Some(Command::Pair("ABCD1234".to_string()))
        );
    }

    #[test]
    fn test_parse_command_unknown() {
        assert_eq!(parse_command("hello world"), None);
    }

    #[test]
    fn test_parse_command_pair_missing_code() {
        assert_eq!(parse_command("/pair"), Some(Command::Pair("".to_string())));
    }

    #[test]
    fn test_parse_command_stop_with_session() {
        assert_eq!(
            parse_command("/stop abc-session-123"),
            Some(Command::Stop("abc-session-123".to_string()))
        );
    }

    #[test]
    fn test_parse_command_stop_no_session() {
        assert_eq!(parse_command("/stop"), Some(Command::Stop("".to_string())));
    }
}
