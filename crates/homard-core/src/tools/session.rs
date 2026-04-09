use std::sync::Arc;
use crate::types::*;
use crate::store::Store;
use crate::error::{HomardError, Result};

pub fn spawn_schema() -> ToolSchema {
    ToolSchema {
        name: "spawn_session".to_string(),
        description: "Spawn a Claude Code or Codex CLI session to handle complex coding tasks. Use this for tasks that need a full coding agent (file editing, debugging, multi-step development). The session runs in a specified directory and returns the output when complete.".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "cli": {
                    "type": "string",
                    "enum": ["claude", "codex"],
                    "description": "Which CLI to use: 'claude' for Claude Code, 'codex' for OpenAI Codex"
                },
                "prompt": {
                    "type": "string",
                    "description": "The task/prompt to send to the CLI agent"
                },
                "directory": {
                    "type": "string",
                    "description": "Working directory for the session (e.g., ~/GitHub/my-project)"
                }
            },
            "required": ["cli", "prompt", "directory"]
        }),
    }
}

pub fn list_sessions_schema() -> ToolSchema {
    ToolSchema {
        name: "list_sessions".to_string(),
        description: "List recent CLI sessions (Claude Code / Codex) with their status and output summary".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "limit": {
                    "type": "integer",
                    "description": "Max sessions to return (default 10)"
                }
            }
        }),
    }
}

pub fn kill_session_schema() -> ToolSchema {
    ToolSchema {
        name: "kill_session".to_string(),
        description: "Kill a running CLI session by ID".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "session_id": {
                    "type": "string",
                    "description": "The session ID to kill"
                }
            },
            "required": ["session_id"]
        }),
    }
}

pub async fn spawn(args: serde_json::Value, store: Arc<tokio::sync::Mutex<Store>>) -> Result<String> {
    let cli_str = args.get("cli").and_then(|c| c.as_str()).unwrap_or("claude");
    let prompt = args.get("prompt").and_then(|p| p.as_str())
        .ok_or_else(|| HomardError::Tool("Missing 'prompt' argument".to_string()))?;
    let directory = args.get("directory").and_then(|d| d.as_str())
        .ok_or_else(|| HomardError::Tool("Missing 'directory' argument".to_string()))?;

    let cli = match cli_str {
        "codex" => CliType::Codex,
        _ => CliType::Claude,
    };

    // Expand ~ in directory
    let expanded_dir = if directory.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            home.join(&directory[2..]).to_string_lossy().to_string()
        } else {
            directory.to_string()
        }
    } else {
        directory.to_string()
    };

    // Verify directory exists
    if !std::path::Path::new(&expanded_dir).is_dir() {
        return Err(HomardError::Tool(format!("Directory does not exist: {}", expanded_dir)));
    }

    // Check that the CLI is available
    let cli_binary = match cli {
        CliType::Claude => "claude",
        CliType::Codex => "codex",
    };

    let which = tokio::process::Command::new("which")
        .arg(cli_binary)
        .output()
        .await;

    if which.is_err() || !which.unwrap().status.success() {
        return Err(HomardError::Tool(format!("{} CLI not found. Install it first.", cli_binary)));
    }

    let session_id = uuid::Uuid::new_v4().to_string();

    // Build command
    let mut cmd = tokio::process::Command::new(cli_binary);
    cmd.current_dir(&expanded_dir);

    match cli {
        CliType::Claude => {
            cmd.arg("-p").arg(prompt)
                .arg("--output-format").arg("text")
                .arg("--verbose");
        }
        CliType::Codex => {
            cmd.arg("--prompt").arg(prompt)
                .arg("--auto-edit");
        }
    }

    // Capture output
    cmd.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let child = cmd.spawn()
        .map_err(|e| HomardError::Tool(format!("Failed to spawn {}: {}", cli_binary, e)))?;

    let pid = child.id();

    // Track session
    let session = CliSession {
        id: session_id.clone(),
        cli: cli.clone(),
        prompt: prompt.to_string(),
        directory: expanded_dir.clone(),
        status: SessionStatus::Running,
        output: None,
        error: None,
        pid,
        started_at: chrono::Utc::now(),
        finished_at: None,
        duration_ms: None,
    };

    {
        let store = store.lock().await;
        store.insert_session(&session)?;
    }

    tracing::info!("Spawned {} session {} in {}: {}", cli_binary, session_id, expanded_dir, prompt);

    // Wait for completion
    let output = child.wait_with_output().await
        .map_err(|e| HomardError::Tool(format!("Session failed: {}", e)))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    let (status, error) = if output.status.success() {
        (SessionStatus::Complete, None)
    } else {
        (SessionStatus::Error, Some(stderr.clone()))
    };

    // Update session in store
    {
        let store = store.lock().await;
        store.complete_session(
            &session_id,
            status.clone(),
            Some(&stdout),
            error.as_deref(),
        )?;
    }

    // Return result
    if status == SessionStatus::Complete {
        let summary = if stdout.len() > 3000 {
            format!("{}...\n[truncated, {} total chars]", &stdout[..3000], stdout.len())
        } else {
            stdout
        };
        Ok(format!("Session {} complete.\n\n{}", session_id, summary))
    } else {
        Ok(format!("Session {} failed.\nStdout: {}\nStderr: {}", session_id, stdout, stderr))
    }
}

pub async fn list(args: serde_json::Value, store: Arc<tokio::sync::Mutex<Store>>) -> Result<String> {
    let limit = args.get("limit").and_then(|l| l.as_u64()).unwrap_or(10) as usize;

    let store = store.lock().await;
    let sessions = store.list_sessions(limit)?;

    if sessions.is_empty() {
        return Ok("No CLI sessions found.".to_string());
    }

    let mut output = String::from("Recent CLI sessions:\n\n");
    for s in sessions {
        let status_label = match s.status {
            SessionStatus::Running => "RUNNING",
            SessionStatus::Complete => "COMPLETE",
            SessionStatus::Error => "ERROR",
            SessionStatus::Killed => "KILLED",
        };
        let duration = s.duration_ms.map(|ms| format!(" ({}s)", ms / 1000)).unwrap_or_default();
        let cli_name = match s.cli {
            CliType::Claude => "Claude",
            CliType::Codex => "Codex",
        };
        output.push_str(&format!(
            "[{}] {} — {}{}\n  dir: {}\n  prompt: {}\n\n",
            &s.id[..8],
            cli_name,
            match s.status {
                SessionStatus::Running => "running".to_string(),
                SessionStatus::Complete => "complete".to_string(),
                SessionStatus::Error => format!("error: {}", s.error.as_deref().unwrap_or("unknown")),
                SessionStatus::Killed => "killed".to_string(),
            },
            duration,
            s.directory,
            if s.prompt.len() > 80 { format!("{}...", &s.prompt[..80]) } else { s.prompt.clone() },
        ));
        let _ = status_label; // used for potential future formatting
    }

    Ok(output)
}

pub async fn kill(args: serde_json::Value, store: Arc<tokio::sync::Mutex<Store>>) -> Result<String> {
    let session_id = args.get("session_id").and_then(|s| s.as_str())
        .ok_or_else(|| HomardError::Tool("Missing 'session_id' argument".to_string()))?;

    let store_locked = store.lock().await;
    let sessions = store_locked.get_running_sessions()?;
    drop(store_locked);

    let session = sessions.iter().find(|s| s.id.starts_with(session_id))
        .ok_or_else(|| HomardError::Tool(format!("No running session matching '{}'", session_id)))?;

    if let Some(pid) = session.pid {
        // Send SIGTERM
        unsafe {
            libc::kill(pid as i32, libc::SIGTERM);
        }

        // Update store
        let store_locked = store.lock().await;
        store_locked.complete_session(&session.id, SessionStatus::Killed, None, Some("Killed by user"))?;

        Ok(format!("Killed session {} (PID {})", &session.id[..8], pid))
    } else {
        Err(HomardError::Tool("Session has no PID".to_string()))
    }
}
