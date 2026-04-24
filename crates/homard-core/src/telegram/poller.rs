use std::sync::Arc;
#[cfg(target_os = "macos")]
use teloxide::payloads::GetUpdatesSetters;
#[cfg(target_os = "macos")]
use teloxide::prelude::*;
use tokio_util::sync::CancellationToken;
#[allow(unused_imports)]
use tracing::{error, info, warn};

use crate::agent::r#loop::AgentLoop;
#[allow(unused_imports)]
use crate::config::{add_paired_chat, validate_pairing_code, HomardDirs};
use crate::telegram::client::TelegramClient;
#[cfg(target_os = "macos")]
use crate::types::Trigger;

#[derive(Debug, PartialEq)]
pub enum Command {
    Start,
    Status,
    Pair(String),
    Stop,
    Perms(String),
    Claude(String), // /claude <prompt> — spawn a Claude session
    ServerOff,
    ServerOn,
}

pub fn parse_command(text: &str) -> Option<Command> {
    let text = text.trim();
    if text == "/start" {
        return Some(Command::Start);
    }
    if text == "/status" {
        return Some(Command::Status);
    }
    if text == "/stop" {
        return Some(Command::Stop);
    }
    if let Some(stripped) = text.strip_prefix("/pair") {
        if stripped.is_empty() || stripped.starts_with(' ') {
            return Some(Command::Pair(stripped.trim().to_string()));
        }
    }
    if let Some(stripped) = text.strip_prefix("/perms") {
        if stripped.is_empty() || stripped.starts_with(' ') {
            return Some(Command::Perms(stripped.trim().to_string()));
        }
    }
    if let Some(stripped) = text.strip_prefix("/claude") {
        if stripped.is_empty() || stripped.starts_with(' ') {
            return Some(Command::Claude(stripped.trim().to_string()));
        }
    }
    if text == "/server off" {
        return Some(Command::ServerOff);
    }
    if text == "/server on" {
        return Some(Command::ServerOn);
    }
    None
}

pub async fn run_poller(
    #[allow(unused_variables)] dirs: HomardDirs,
    #[allow(unused_variables)] agent: Arc<AgentLoop>,
    #[allow(unused_variables)] client: Arc<TelegramClient>,
    #[allow(unused_variables)] cancel: CancellationToken,
    #[allow(unused_variables)] stop_tx: tokio::sync::watch::Sender<bool>,
    #[allow(unused_variables)] security: Arc<crate::security::SecurityManager>,
    #[allow(unused_variables)] shared_config: Arc<tokio::sync::RwLock<crate::config::HomardConfig>>,
) {
    // Wait for a token to be configured (checks every 10s)
    #[cfg(not(target_os = "macos"))]
    {
        warn!("Telegram poller: only supported on macOS (keychain)");
        return;
    }

    #[cfg(target_os = "macos")]
    let token = loop {
        if cancel.is_cancelled() {
            return;
        }
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

            let config = shared_config.read().await.clone();

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
                    teloxide::types::UpdateKind::Message(msg) => msg
                        .from
                        .as_ref()
                        .and_then(|u| u.username.clone())
                        .unwrap_or_default(),
                    _ => String::new(),
                };

                let is_allowed = config.telegram.paired_chat_ids.contains(&chat_id_str)
                    || config.telegram.allowed_usernames.iter().any(|u| {
                        let allowed = u.trim_start_matches('@').to_lowercase();
                        username.to_lowercase() == allowed
                    });

                // Auto-pair allowed users on first message
                if is_allowed && !config.telegram.paired_chat_ids.contains(&chat_id_str) {
                    if add_paired_chat(&dirs, &chat_id_str).is_ok() {
                        let mut cfg = shared_config.write().await;
                        if !cfg.telegram.paired_chat_ids.contains(&chat_id_str) {
                            cfg.telegram.paired_chat_ids.push(chat_id_str.clone());
                        }
                    }
                    info!(
                        "Telegram: auto-paired chat {} for allowed user @{}",
                        chat_id, username
                    );
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
                                Ok(true) => match add_paired_chat(&dirs, &chat_id_str) {
                                    Ok(()) => {
                                        {
                                            let mut cfg = shared_config.write().await;
                                            if !cfg.telegram.paired_chat_ids.contains(&chat_id_str) {
                                                cfg.telegram.paired_chat_ids.push(chat_id_str.clone());
                                            }
                                        }
                                        let _ = client
                                            .send_message(
                                                chat_id,
                                                "Paired! You can now chat with Homard here.",
                                            )
                                            .await;
                                        info!("Telegram: chat {} paired", chat_id);
                                    }
                                    Err(e) => {
                                        let _ = client
                                            .send_message(
                                                chat_id,
                                                &format!("Pairing failed: {}", e),
                                            )
                                            .await;
                                    }
                                },
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
                        let new_level = match level.as_str() {
                            "autonomous" | "auto" => crate::types::PermissionLevel::Autonomous,
                            "locked" | "lock" => crate::types::PermissionLevel::Locked,
                            _ => crate::types::PermissionLevel::Supervised,
                        };
                        security.set_permission_level(new_level.clone()).await;
                        // Persist to config
                        {
                            let mut config = shared_config.write().await;
                            config.permission_level = new_level.clone();
                            let _ = config.save(&dirs.config_path());
                        }
                        let name = match new_level {
                            crate::types::PermissionLevel::Supervised => "supervised",
                            crate::types::PermissionLevel::Autonomous => "autonomous",
                            crate::types::PermissionLevel::Locked => "locked",
                        };
                        let _ = client
                            .send_message(chat_id, &format!("Permission level: {}", name))
                            .await;
                    }
                    Some(Command::Claude(prompt)) if is_allowed => {
                        if prompt.is_empty() {
                            let _ = client.send_message(chat_id, "Usage: /claude <prompt> [--dir path]\nExample: /claude fix the tests in site-factory\nExample: /claude refactor auth --dir ~/GitHub/d1201\n\nLaunches a Claude Code session you manage from the terminal or Claude app.").await;
                        } else {
                            // Parse optional --dir flag
                            let (task, dir) = if let Some(idx) = prompt.find("--dir") {
                                let task = prompt[..idx].trim().to_string();
                                let dir = prompt[idx + 5..].trim().to_string();
                                (task, if dir.is_empty() { ".".to_string() } else { dir })
                            } else {
                                (prompt.clone(), ".".to_string())
                            };

                            // Generate a session name from the prompt
                            let name: String = task
                                .split_whitespace()
                                .take(4)
                                .collect::<Vec<_>>()
                                .join("-")
                                .chars()
                                .filter(|c| c.is_alphanumeric() || *c == '-')
                                .take(30)
                                .collect();

                            let _ = client.send_message(
                                chat_id,
                                &format!("Launching Claude session '{}'\nDir: {}\n\nManage it:\n  claude --resume (terminal)\n  Or open Claude desktop app", name, dir)
                            ).await;

                            let client_clone = client.clone();
                            let name_clone = name.clone();
                            tokio::spawn(async move {
                                // Launch Claude in background — user manages from Claude app
                                let escaped = crate::llm::cli_backend::shell_escape_pub(&task);
                                let dir_expanded = if dir.starts_with("~/") {
                                    dirs::home_dir()
                                        .map(|h| h.join(&dir[2..]).to_string_lossy().to_string())
                                        .unwrap_or(dir.clone())
                                } else {
                                    dir.clone()
                                };

                                let child = tokio::process::Command::new("claude")
                                    .arg("-p")
                                    .arg(&task)
                                    .arg("--name")
                                    .arg(&name_clone)
                                    .arg("--output-format")
                                    .arg("text")
                                    .current_dir(&dir_expanded)
                                    .stdin(std::process::Stdio::null())
                                    .stdout(std::process::Stdio::piped())
                                    .stderr(std::process::Stdio::piped())
                                    .spawn();

                                match child {
                                    Ok(child) => {
                                        // Wait for completion in background
                                        match tokio::time::timeout(
                                            std::time::Duration::from_secs(600),
                                            child.wait_with_output(),
                                        )
                                        .await
                                        {
                                            Ok(Ok(output)) => {
                                                let stdout =
                                                    String::from_utf8_lossy(&output.stdout);
                                                let summary = if stdout.len() > 500 {
                                                    format!("{}...", &stdout[..500])
                                                } else {
                                                    stdout.trim().to_string()
                                                };
                                                let status = if output.status.success() {
                                                    "completed"
                                                } else {
                                                    "failed"
                                                };
                                                let msg = if summary.is_empty() {
                                                    format!("Session '{}' {}.", name_clone, status)
                                                } else {
                                                    format!(
                                                        "Session '{}' {}.\n\n{}",
                                                        name_clone, status, summary
                                                    )
                                                };
                                                let _ = client_clone
                                                    .chunk_and_send(chat_id, &msg)
                                                    .await;
                                            }
                                            Ok(Err(e)) => {
                                                let _ = client_clone
                                                    .send_message(
                                                        chat_id,
                                                        &format!(
                                                            "Session '{}' error: {}",
                                                            name_clone, e
                                                        ),
                                                    )
                                                    .await;
                                            }
                                            Err(_) => {
                                                let _ = client_clone
                                                    .send_message(
                                                        chat_id,
                                                        &format!(
                                                            "Session '{}' timed out (10 min).",
                                                            name_clone
                                                        ),
                                                    )
                                                    .await;
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        let _ = client_clone
                                            .send_message(
                                                chat_id,
                                                &format!("Failed to start Claude: {}", e),
                                            )
                                            .await;
                                    }
                                }
                            });
                        }
                    }
                    Some(Command::ServerOff) if is_allowed => {
                        // Unload launchd plist
                        let home = dirs::home_dir().unwrap_or_default();
                        let plist = home.join("Library/LaunchAgents/com.homard.daemon.plist");
                        if plist.exists() {
                            let _ = std::process::Command::new("launchctl")
                                .args([
                                    "bootout",
                                    &format!("gui/{}", unsafe { libc::getuid() }),
                                    &plist.to_string_lossy(),
                                ])
                                .output();
                            let _ = std::fs::remove_file(&plist);

                            // Persist to config
                            {
                                let mut config = shared_config.write().await;
                                config.server_mode = crate::types::ServerMode::Off;
                                let _ = config.save(&dirs.config_path());
                            }

                            let _ = client
                                .send_message(
                                    chat_id,
                                    "Server mode OFF. Daemon will stop after current session ends.",
                                )
                                .await;
                        } else {
                            // Even if plist is missing, ensure config is in sync
                            {
                                let mut config = shared_config.write().await;
                                if config.server_mode != crate::types::ServerMode::Off {
                                    config.server_mode = crate::types::ServerMode::Off;
                                    let _ = config.save(&dirs.config_path());
                                }
                            }

                            let _ = client
                                .send_message(chat_id, "Server mode is already off.")
                                .await;
                        }
                    }
                    Some(Command::ServerOn) if is_allowed => {
                        let _ = client.send_message(chat_id, "Use `homard install` from the CLI or the tray app to enable server mode.").await;
                    }
                    Some(_) if !is_allowed => {
                        let _ = client
                            .send_message(chat_id, "Send /start to connect with Homard.")
                            .await;
                    }
                    None if !is_allowed => {
                        let _ = client
                            .send_message(chat_id, "Send /start to connect.")
                            .await;
                    }
                    None if is_allowed => {
                        // Route through agent loop (spawned concurrently to avoid blocking the poller)
                        let channel = format!("telegram_{}", chat_id);
                        // Send typing indicator
                        let _ = bot
                            .send_chat_action(
                                teloxide::types::ChatId(chat_id),
                                teloxide::types::ChatAction::Typing,
                            )
                            .await;

                        let agent_clone = agent.clone();
                        let client_clone = client.clone();
                        tokio::spawn(async move {
                            match agent_clone.run(&channel, &text, Trigger::Telegram).await {
                                Ok((response, _run_id)) => {
                                    let _ = client_clone.chunk_and_send(chat_id, &response).await;
                                }
                                Err(e) => {
                                    let _ = client_clone
                                        .send_message(chat_id, &format!("Error: {}", e))
                                        .await;
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
    fn test_parse_command_claude() {
        assert_eq!(parse_command("/claude"), Some(Command::Claude(String::new())));
        assert_eq!(parse_command("/claude  "), Some(Command::Claude(String::new())));
        assert_eq!(parse_command("/claude fix tests"), Some(Command::Claude("fix tests".to_string())));
    }

    #[test]
    fn test_parse_command_pair() {
        assert_eq!(
            parse_command("/pair ABCD1234"),
            Some(Command::Pair("ABCD1234".to_string()))
        );
        assert_eq!(
            parse_command("/pair"),
            Some(Command::Pair(String::new()))
        );
        assert_eq!(
            parse_command("/pair "),
            Some(Command::Pair(String::new()))
        );
    }

    #[test]
    fn test_parse_command_stop() {
        assert_eq!(parse_command("/stop"), Some(Command::Stop));
    }

    #[test]
    fn test_parse_command_perms() {
        assert_eq!(
            parse_command("/perms autonomous"),
            Some(Command::Perms("autonomous".to_string()))
        );
    }

    #[test]
    fn test_parse_command_server() {
        assert_eq!(parse_command("/server off"), Some(Command::ServerOff));
        assert_eq!(parse_command("/server on"), Some(Command::ServerOn));
    }

    #[test]
    fn test_parse_command_regular_text() {
        assert_eq!(parse_command("hello world"), None);
        assert_eq!(parse_command("/unknown"), None);
    }

    #[test]
    fn test_parse_command_claude_with_dir() {
        assert_eq!(
            parse_command("/claude fix tests --dir ./site"),
            Some(Command::Claude("fix tests --dir ./site".to_string()))
        );
    }

    #[test]
    fn test_parse_command_greedy_bug() {
        // These should NOT match
        assert_eq!(parse_command("/pairing"), None);
        assert_eq!(parse_command("/statuses"), None);
        assert_eq!(parse_command("/claudette"), None);
        assert_eq!(parse_command("/permissions"), None);
    }
}
