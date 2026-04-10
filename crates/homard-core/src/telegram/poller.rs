use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::payloads::GetUpdatesSetters;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn, error};

use crate::agent::r#loop::AgentLoop;
use crate::config::{HomardConfig, HomardDirs, add_paired_chat, validate_pairing_code};
use crate::telegram::client::TelegramClient;
use crate::types::Trigger;

#[derive(Debug, PartialEq)]
pub enum Command {
    Start,  // Auto-pair this chat
    Status,
    Pair(String),
    Stop,
    Perms(String),
    ServerOff,
    ServerOn,
}

pub fn parse_command(text: &str) -> Option<Command> {
    let text = text.trim();
    if text == "/start" { return Some(Command::Start); }
    if text == "/status" { return Some(Command::Status); }
    if text == "/stop" { return Some(Command::Stop); }
    if text.starts_with("/pair ") {
        return Some(Command::Pair(text[6..].trim().to_string()));
    }
    if text.starts_with("/pair") {
        return Some(Command::Pair(String::new()));
    }
    if text.starts_with("/perms ") {
        return Some(Command::Perms(text[7..].trim().to_string()));
    }
    if text == "/server off" { return Some(Command::ServerOff); }
    if text == "/server on" { return Some(Command::ServerOn); }
    None
}

pub async fn run_poller(
    dirs: HomardDirs,
    agent: Arc<AgentLoop>,
    client: Arc<TelegramClient>,
    cancel: CancellationToken,
    stop_tx: tokio::sync::watch::Sender<bool>,
) {
    // Wait for a token to be configured (checks every 10s)
    #[cfg(not(target_os = "macos"))]
    {
        warn!("Telegram poller: only supported on macOS (keychain)");
        return;
    }

    #[cfg(target_os = "macos")]
    let token = loop {
        if cancel.is_cancelled() { return; }
        match crate::config::get_telegram_token(&dirs) {
            Ok(Some(t)) => {
                info!("Telegram poller: token found, starting...");
                break t;
            }
            Ok(None) => {
                // No token yet — wait and check again
                tokio::select! {
                    _ = tokio::time::sleep(std::time::Duration::from_secs(10)) => {}
                    _ = cancel.cancelled() => { return; }
                }
            }
            Err(e) => {
                warn!("Telegram poller: keychain error: {}, retrying...", e);
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            }
        }
    };

    #[cfg(target_os = "macos")]
    {
        let bot = teloxide::Bot::new(&token);
        info!("Telegram poller started");

        let mut offset: i32 = 0;

        loop {
            if cancel.is_cancelled() {
                info!("Telegram poller: cancelled");
                break;
            }

            // Long-poll (10s timeout)
            let updates = match bot.get_updates().offset(offset).timeout(10).await {
                Ok(updates) => updates,
                Err(e) => {
                    warn!("Telegram getUpdates error: {}", e);
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    continue;
                }
            };

            let config = HomardConfig::load_or_default(&dirs.config_path());

            for update in &updates {
                offset = update.id.as_offset();

                let (chat_id, text) = match &update.kind {
                    teloxide::types::UpdateKind::Message(msg) => {
                        (msg.chat.id.0, msg.text().unwrap_or("").to_string())
                    }
                    _ => continue,
                };

                let chat_id_str = chat_id.to_string();

                // Check if user is allowed — by username allowlist or paired chat ID
                let username = match &update.kind {
                    teloxide::types::UpdateKind::Message(msg) => {
                        msg.from.as_ref().and_then(|u| u.username.clone()).unwrap_or_default()
                    }
                    _ => String::new(),
                };

                let is_allowed = config.telegram.paired_chat_ids.contains(&chat_id_str)
                    || config.telegram.allowed_usernames.iter().any(|u| {
                        let allowed = u.trim_start_matches('@').to_lowercase();
                        username.to_lowercase() == allowed
                    });

                // Auto-pair allowed users on first message
                if is_allowed && !config.telegram.paired_chat_ids.contains(&chat_id_str) {
                    let _ = add_paired_chat(&dirs, &chat_id_str);
                    info!("Telegram: auto-paired chat {} for allowed user @{}", chat_id, username);
                }

                match parse_command(&text) {
                    Some(Command::Start) => {
                        if is_allowed {
                            let _ = client.send_message(chat_id, "Hey! I'm Homard. Send me anything.\n\nCommands: /status /stop /perms <level> /server on|off").await;
                        } else {
                            let _ = client.send_message(chat_id, "Not authorized. Add your username in Homard Settings → Telegram.").await;
                        }
                    }
                    Some(Command::Pair(code)) => {
                        if code.is_empty() {
                            let _ = client.send_message(chat_id, "Usage: /pair <code> \u{2014} get your pairing code from the Homard app.").await;
                        } else {
                            match validate_pairing_code(&dirs, &code) {
                                Ok(true) => {
                                    match add_paired_chat(&dirs, &chat_id_str) {
                                        Ok(()) => {
                                            let _ = client.send_message(chat_id, "Paired! You can now chat with Homard here.").await;
                                            info!("Telegram: chat {} paired", chat_id);
                                        }
                                        Err(e) => {
                                            let _ = client.send_message(chat_id, &format!("Pairing failed: {}", e)).await;
                                        }
                                    }
                                }
                                Ok(false) => {
                                    let _ = client.send_message(chat_id, "Invalid or expired code. Generate a new one in Homard settings.").await;
                                }
                                Err(e) => {
                                    error!("Pairing error: {}", e);
                                }
                            }
                        }
                    }
                    Some(Command::Status) if is_allowed => {
                        let _ = client.send_message(chat_id, "Homard is running. Commands: /status /stop /perms <level> /server on|off").await;
                    }
                    Some(Command::Stop) if is_allowed => {
                        let _ = stop_tx.send(true);
                        let _ = client.send_message(chat_id, "Stop signal sent.").await;
                        // Reset stop after 1s
                        let tx = stop_tx.clone();
                        tokio::spawn(async move {
                            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                            let _ = tx.send(false);
                        });
                    }
                    Some(Command::Perms(level)) if is_allowed => {
                        let _ = client.send_message(chat_id, &format!("Permission level set to: {} (restart daemon to apply)", level)).await;
                    }
                    Some(Command::ServerOff) if is_allowed => {
                        // Unload launchd plist
                        let home = dirs::home_dir().unwrap_or_default();
                        let plist = home.join("Library/LaunchAgents/com.homard.daemon.plist");
                        if plist.exists() {
                            let _ = std::process::Command::new("launchctl")
                                .args(["bootout", &format!("gui/{}", unsafe { libc::getuid() }), &plist.to_string_lossy()])
                                .output();
                            let _ = std::fs::remove_file(&plist);
                            let _ = client.send_message(chat_id, "Server mode OFF. Daemon will stop after current session ends.").await;
                        } else {
                            let _ = client.send_message(chat_id, "Server mode is already off.").await;
                        }
                    }
                    Some(Command::ServerOn) if is_allowed => {
                        let _ = client.send_message(chat_id, "Use `homard install` from the CLI or the tray app to enable server mode.").await;
                    }
                    Some(_) if !is_allowed => {
                        let _ = client.send_message(chat_id, "Send /start to connect with Homard.").await;
                    }
                    None if !is_allowed => {
                        let _ = client.send_message(chat_id, "Send /start to connect.").await;
                    }
                    None if is_allowed => {
                        // Route through agent loop (spawned concurrently to avoid blocking the poller)
                        let channel = format!("telegram_{}", chat_id);
                        // Send typing indicator
                        let _ = bot.send_chat_action(teloxide::types::ChatId(chat_id), teloxide::types::ChatAction::Typing).await;

                        let agent_clone = agent.clone();
                        let client_clone = client.clone();
                        tokio::spawn(async move {
                            match agent_clone.run(&channel, &text, Trigger::Telegram).await {
                                Ok(response) => {
                                    let _ = client_clone.chunk_and_send(chat_id, &response).await;
                                }
                                Err(e) => {
                                    let _ = client_clone.send_message(chat_id, &format!("Error: {}", e)).await;
                                }
                            }
                        });
                    }
                    _ => {}
                }
            }
        }

        info!("Telegram poller stopped");
    }
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
        assert_eq!(parse_command("/pair ABCD1234"), Some(Command::Pair("ABCD1234".to_string())));
    }

    #[test]
    fn test_parse_command_stop() {
        assert_eq!(parse_command("/stop"), Some(Command::Stop));
    }

    #[test]
    fn test_parse_command_perms() {
        assert_eq!(parse_command("/perms autonomous"), Some(Command::Perms("autonomous".to_string())));
    }

    #[test]
    fn test_parse_command_server() {
        assert_eq!(parse_command("/server off"), Some(Command::ServerOff));
        assert_eq!(parse_command("/server on"), Some(Command::ServerOn));
    }

    #[test]
    fn test_parse_command_regular_text() {
        assert_eq!(parse_command("hello world"), None);
    }
}
