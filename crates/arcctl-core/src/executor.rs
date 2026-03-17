use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;
use rand::Rng;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{debug, info, warn};

use crate::config::{ArcctlConfig, ArcctlDirs};
#[cfg(target_os = "macos")]
use crate::config::get_telegram_token;
use crate::error::{ArcctlError, Result};
use crate::schedule::{load_schedule, update_last_session_id};
use crate::store::Store;
use crate::types::{Run, RunStatus, SessionMode, Trigger};

// ---------------------------------------------------------------------------
// JobExecutor
// ---------------------------------------------------------------------------

pub struct JobExecutor {
    dirs: ArcctlDirs,
    store: Store,
    schedule: crate::types::Schedule,
    run_id: String,
    log_path: PathBuf,
    captured_session_id: Option<String>,
    last_error: Option<String>,
    telegram_reporter: Option<Arc<crate::telegram::TelegramStreamReporter>>,
    telegram_delivered: bool,
}

impl JobExecutor {
    pub fn new(dirs: ArcctlDirs, job_id: &str) -> Result<Self> {
        let schedule = load_schedule(&dirs, job_id)?;

        let store = Store::open(dirs.db_path())?;

        let run_id = uuid::Uuid::new_v4().to_string();
        let log_path = dirs.logs_dir().join(format!("{}.log", run_id));

        // Initialize Telegram reporter if telegram is in delivery channels
        #[cfg(target_os = "macos")]
        let telegram_reporter = if schedule.delivery.channels.contains(&"telegram".to_string()) {
            let config = ArcctlConfig::load_or_default(&dirs.config_path());
            if config.telegram.enabled && !config.telegram.paired_chat_ids.is_empty() {
                if let Ok(Some(token)) = get_telegram_token(&dirs) {
                    let chat_ids: Vec<i64> = config.telegram.paired_chat_ids.iter()
                        .filter_map(|s| s.parse().ok())
                        .collect();
                    let client = Arc::new(crate::telegram::TelegramClient::new(token));
                    Some(Arc::new(crate::telegram::TelegramStreamReporter::new(client, chat_ids)))
                } else { None }
            } else { None }
        } else { None };

        #[cfg(not(target_os = "macos"))]
        let telegram_reporter: Option<Arc<crate::telegram::TelegramStreamReporter>> = None;

        Ok(Self {
            dirs,
            store,
            schedule,
            run_id,
            log_path,
            captured_session_id: None,
            last_error: None,
            telegram_reporter,
            telegram_delivered: false,
        })
    }

    pub async fn execute(&mut self) -> Result<()> {
        // 1. Apply random stagger delay
        let config = ArcctlConfig::load_or_default(&self.dirs.config_path());
        if config.scheduler.stagger_ms > 0 {
            let delay = {
                let mut rng = rand::thread_rng();
                rng.gen_range(0..=config.scheduler.stagger_ms)
            };
            if delay > 0 {
                debug!("Stagger delay: {}ms", delay);
                tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
            }
        }

        // 2. Switch profile if needed
        if let Err(e) = self.switch_profile_if_needed() {
            warn!("Profile switch failed: {}", e);
        }

        // 3. Send Telegram start message for streaming preview
        if let Some(ref reporter) = self.telegram_reporter {
            reporter.send_start(&self.schedule.name).await;
        }

        // 4. Retry loop
        let max_attempts = self.schedule.retry.max_attempts.max(1);
        let backoff_seconds = self.schedule.retry.backoff_seconds.clone();

        let mut final_status = RunStatus::Error;

        for attempt in 0..max_attempts {
            if attempt > 0 {
                let backoff_idx = (attempt as usize - 1).min(backoff_seconds.len().saturating_sub(1));
                let backoff = backoff_seconds.get(backoff_idx).copied().unwrap_or(60);
                info!("Retry attempt {} after {}s backoff", attempt + 1, backoff);
                tokio::time::sleep(std::time::Duration::from_secs(backoff)).await;
            }

            // a. Insert Run record
            let run = Run {
                id: self.run_id.clone(),
                schedule_id: Some(self.schedule.id.clone()),
                agent: self.schedule.agent.clone(),
                profile: self.schedule.profile.clone(),
                directory: Some(self.schedule.directory.clone()),
                trigger: Trigger::Cron,
                status: RunStatus::Running,
                started_at: Utc::now(),
                finished_at: None,
                duration_ms: None,
                error_message: None,
                delivery_status: None,
            };

            // Only insert on first attempt; subsequent attempts update the same run_id
            if attempt == 0 {
                self.store.insert_run(&run)?;
            }

            // b-d. Spawn and stream
            match self.spawn_and_stream().await {
                Ok(status) => {
                    final_status = status.clone();

                    // e. On success: update last_session_id for persistent mode
                    if matches!(final_status, RunStatus::Complete) {
                        if matches!(self.schedule.session_mode, SessionMode::Persistent) {
                            if let Some(sid) = &self.captured_session_id {
                                let _ = update_last_session_id(&self.dirs, &self.schedule.id, sid);
                            }
                        }
                    }

                    // Send Telegram final result via streaming reporter
                    if let Some(ref reporter) = self.telegram_reporter {
                        reporter.send_final(
                            &self.schedule.name,
                            self.last_error.as_deref(),
                            None,
                        ).await;
                        self.telegram_delivered = true;
                    }

                    // Finalize and deliver
                    self.finalize_run(status, None)?;
                    self.deliver_results().await?;
                    return Ok(());
                }
                Err(e) => {
                    let err_msg = e.to_string();
                    self.last_error = Some(err_msg.clone());

                    if attempt + 1 < max_attempts {
                        warn!("Job attempt {} failed: {}. Will retry.", attempt + 1, err_msg);
                        // Generate new run_id for next attempt
                        self.run_id = uuid::Uuid::new_v4().to_string();
                        self.log_path = self.dirs.logs_dir().join(format!("{}.log", self.run_id));
                    } else {
                        // Last attempt — finalize with error
                        final_status = RunStatus::Error;
                        self.finalize_run(RunStatus::Error, Some(err_msg))?;
                        self.deliver_results().await?;
                    }
                }
            }
        }

        let _ = final_status;
        Ok(())
    }

    fn switch_profile_if_needed(&self) -> Result<()> {
        let Some(profile_name) = &self.schedule.profile else {
            return Ok(());
        };

        let profile_dir = self.dirs.profiles_dir().join(profile_name);
        if !profile_dir.exists() {
            return Err(ArcctlError::NotFound(format!(
                "Profile '{}' not found",
                profile_name
            )));
        }

        let home = dirs::home_dir().ok_or_else(|| {
            ArcctlError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "home directory not found",
            ))
        })?;

        // Restore credential files from profile
        let pm = crate::profile::ProfileManager::new(
            self.dirs.profiles_dir(),
            home.join(".claude"),
            home.clone(),
        );
        pm.restore_files(profile_name)?;

        Ok(())
    }

    fn build_claude_args(&self) -> Vec<String> {
        let mut args = Vec::new();

        // Prompt
        let prompt = self.schedule.prompt.as_deref().unwrap_or("");
        args.push("-p".to_string());
        args.push(prompt.to_string());

        // Output format for JSON streaming
        args.push("--output-format".to_string());
        args.push("stream-json".to_string());

        // Session mode
        match self.schedule.session_mode {
            SessionMode::Persistent => {
                if let Some(sid) = &self.schedule.last_session_id {
                    args.push("--resume".to_string());
                    args.push(sid.clone());
                }
            }
            SessionMode::Fresh => {
                // No session args for fresh mode
            }
        }

        // Agent
        if let Some(agent) = &self.schedule.agent {
            args.push("--agent".to_string());
            args.push(agent.clone());
        }

        args
    }

    async fn spawn_and_stream(&mut self) -> Result<RunStatus> {
        let args = self.build_claude_args();
        let directory = self.schedule.directory.clone();

        // Determine timeout
        let timeout_minutes = self.schedule.timeout_minutes.unwrap_or(60);
        let timeout_duration = std::time::Duration::from_secs(timeout_minutes as u64 * 60);

        // Build command
        let mut cmd = tokio::process::Command::new("claude");
        for arg in &args {
            cmd.arg(arg);
        }
        cmd.current_dir(&directory)
            .env_remove("CLAUDE_CODE_ENTRY_POINT")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let mut child = cmd.spawn().map_err(ArcctlError::Io)?;

        // Open log file
        std::fs::create_dir_all(self.log_path.parent().unwrap()).map_err(ArcctlError::Io)?;
        let mut log_file = tokio::fs::File::create(&self.log_path)
            .await
            .map_err(ArcctlError::Io)?;

        // Stream stdout, write to log, capture session_id
        let stdout = child.stdout.take();
        let captured_session_id = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
        let captured_clone = captured_session_id.clone();
        let reporter_clone = self.telegram_reporter.clone();

        let stream_task = async move {
            if let Some(stdout) = stdout {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    // Write to log
                    let _ = log_file
                        .write_all(format!("{}\n", line).as_bytes())
                        .await;

                    // Feed Telegram streaming reporter
                    if let Some(ref reporter) = reporter_clone {
                        reporter.on_jsonl_line(&line).await;
                    }

                    // Try to parse JSONL for session_id
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                        // Claude stream-json emits a "system" message with session_id
                        if val.get("type").and_then(|t| t.as_str()) == Some("system") {
                            if let Some(sid) = val
                                .get("session_id")
                                .and_then(|s| s.as_str())
                            {
                                let mut guard = captured_clone.lock().unwrap();
                                *guard = Some(sid.to_string());
                            }
                        }
                    }
                }
            }
        };

        // Race: process completion vs timeout
        let wait_result = tokio::time::timeout(timeout_duration, async {
            stream_task.await;
            child.wait().await
        })
        .await;

        // Recover captured session_id
        {
            let guard = captured_session_id.lock().unwrap();
            self.captured_session_id = guard.clone();
        }

        match wait_result {
            Ok(Ok(exit_status)) => {
                if exit_status.success() {
                    Ok(RunStatus::Complete)
                } else {
                    let code = exit_status.code().unwrap_or(-1);
                    self.last_error = Some(format!("claude exited with code {}", code));
                    Ok(RunStatus::Error)
                }
            }
            Ok(Err(e)) => {
                self.last_error = Some(format!("wait error: {}", e));
                Err(ArcctlError::Io(e))
            }
            Err(_timeout) => {
                // Timeout — kill the process
                warn!("Job timed out after {} minutes, sending SIGTERM", timeout_minutes);
                if let Some(pid) = child.id() {
                    unsafe {
                        libc::kill(pid as libc::pid_t, libc::SIGTERM);
                    }
                }
                let sigkill_result = tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    child.wait(),
                )
                .await;
                if sigkill_result.is_err() {
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                }
                self.last_error = Some(format!("timed out after {} minutes", timeout_minutes));
                Ok(RunStatus::Killed)
            }
        }
    }

    async fn deliver_results(&self) -> Result<()> {
        let status_str = match &self.last_error {
            Some(_) => "error",
            None => "complete",
        };

        // Check if this event should trigger delivery
        if !self.schedule.delivery.on_events.iter().any(|e| e == status_str || e == "complete") {
            return Ok(());
        }

        for channel in &self.schedule.delivery.channels {
            match channel.as_str() {
                "notification" => {
                    let title = format!("arcctl: {}", self.schedule.name);
                    let body = match &self.last_error {
                        Some(err) => format!("Job failed: {}", err),
                        None => "Job completed successfully".to_string(),
                    };

                    #[cfg(target_os = "macos")]
                    {
                        let _ = notify_rust::Notification::new()
                            .summary(&title)
                            .body(&body)
                            .show();
                    }
                    #[cfg(not(target_os = "macos"))]
                    {
                        info!("[notification] {}: {}", title, body);
                    }
                }
                "log" => {
                    // Already handled by log file
                }
                "telegram" => {
                    if self.telegram_delivered {
                        // Already handled by TelegramStreamReporter
                        continue;
                    }

                    #[cfg(target_os = "macos")]
                    {
                        let config = ArcctlConfig::load_or_default(&self.dirs.config_path());
                        if !config.telegram.enabled || config.telegram.paired_chat_ids.is_empty() {
                            info!("[telegram] not configured or no paired chats, skipping");
                            continue;
                        }
                        if let Ok(Some(token)) = get_telegram_token(&self.dirs) {
                            let client = Arc::new(crate::telegram::TelegramClient::new(token));
                            let status_emoji = if self.last_error.is_some() { "\u{274C}" } else { "\u{2705}" };
                            let text = match &self.last_error {
                                Some(err) => format!("{} *{}* failed\n\n`{}`", status_emoji, self.schedule.name, err),
                                None => format!("{} *{}* completed", status_emoji, self.schedule.name),
                            };
                            for chat_id_str in &config.telegram.paired_chat_ids {
                                if let Ok(chat_id) = chat_id_str.parse::<i64>() {
                                    let _ = client.send_with_retry(chat_id, &text, 3).await;
                                }
                            }
                        }
                    }
                }
                other => {
                    warn!("Unknown delivery channel: {}", other);
                }
            }
        }

        Ok(())
    }

    fn finalize_run(&self, status: RunStatus, error: Option<String>) -> Result<()> {
        self.store.complete_run(&self.run_id, status, error)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schedule::save_schedule;
    use crate::types::{DeliveryConfig, RetryConfig, Schedule, SessionMode};
    use tempfile::TempDir;

    fn make_schedule_for_executor(id: &str, session_mode: SessionMode, last_session_id: Option<String>, agent: Option<String>) -> Schedule {
        Schedule {
            id: id.to_string(),
            name: format!("Executor Test {}", id),
            schedule: "0 9 * * *".to_string(),
            timezone: None,
            agent,
            prompt: Some("Summarize the news".to_string()),
            directory: "/tmp".to_string(),
            profile: None,
            timeout_minutes: Some(5),
            session_mode,
            last_session_id,
            delivery: DeliveryConfig {
                channels: vec!["log".to_string()],
                on_events: vec!["complete".to_string(), "error".to_string()],
            },
            retry: RetryConfig {
                max_attempts: 1,
                backoff_seconds: vec![10],
            },
            enabled: true,
        }
    }

    fn make_dirs(tmp: &TempDir) -> ArcctlDirs {
        let dirs = ArcctlDirs::new(tmp.path().to_path_buf());
        dirs.ensure_all().unwrap();
        dirs
    }

    #[test]
    fn test_build_claude_args_fresh() {
        let tmp = TempDir::new().unwrap();
        let dirs = make_dirs(&tmp);

        let schedule = make_schedule_for_executor("fresh-001", SessionMode::Fresh, None, None);
        save_schedule(&dirs, &schedule).unwrap();

        let executor = JobExecutor {
            dirs,
            store: Store::open_in_memory().unwrap(),
            schedule,
            run_id: "test-run".to_string(),
            log_path: PathBuf::from("/tmp/test.log"),
            captured_session_id: None,
            last_error: None,
            telegram_reporter: None,
            telegram_delivered: false,
        };

        let args = executor.build_claude_args();

        assert!(args.contains(&"-p".to_string()), "should have -p flag");
        assert!(args.contains(&"Summarize the news".to_string()), "should contain prompt");
        assert!(args.contains(&"--output-format".to_string()), "should have --output-format");
        assert!(args.contains(&"stream-json".to_string()), "should have stream-json");

        // Fresh mode should NOT have --resume
        assert!(!args.contains(&"--resume".to_string()), "fresh mode should not have --resume");
    }

    #[test]
    fn test_build_claude_args_persistent_with_session_id() {
        let tmp = TempDir::new().unwrap();
        let dirs = make_dirs(&tmp);

        let schedule = make_schedule_for_executor(
            "persist-001",
            SessionMode::Persistent,
            Some("session-xyz-123".to_string()),
            None,
        );
        save_schedule(&dirs, &schedule).unwrap();

        let executor = JobExecutor {
            dirs,
            store: Store::open_in_memory().unwrap(),
            schedule,
            run_id: "test-run".to_string(),
            log_path: PathBuf::from("/tmp/test.log"),
            captured_session_id: None,
            last_error: None,
            telegram_reporter: None,
            telegram_delivered: false,
        };

        let args = executor.build_claude_args();

        assert!(args.contains(&"--resume".to_string()), "persistent mode with session_id should have --resume");
        let resume_idx = args.iter().position(|a| a == "--resume").unwrap();
        assert_eq!(args[resume_idx + 1], "session-xyz-123");
    }

    #[test]
    fn test_build_claude_args_with_agent() {
        let tmp = TempDir::new().unwrap();
        let dirs = make_dirs(&tmp);

        let schedule = make_schedule_for_executor(
            "agent-001",
            SessionMode::Fresh,
            None,
            Some("/path/to/agent.json".to_string()),
        );
        save_schedule(&dirs, &schedule).unwrap();

        let executor = JobExecutor {
            dirs,
            store: Store::open_in_memory().unwrap(),
            schedule,
            run_id: "test-run".to_string(),
            log_path: PathBuf::from("/tmp/test.log"),
            captured_session_id: None,
            last_error: None,
            telegram_reporter: None,
            telegram_delivered: false,
        };

        let args = executor.build_claude_args();

        assert!(args.contains(&"--agent".to_string()), "should have --agent flag");
        let agent_idx = args.iter().position(|a| a == "--agent").unwrap();
        assert_eq!(args[agent_idx + 1], "/path/to/agent.json");
    }
}
