//! Persistent codex app-server backend.
//! Keeps a single `codex app-server` process running and communicates via JSON-RPC over stdio.
//! First turn ~2s, subsequent turns <1s (vs 15s with codex exec subprocess).

use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::sync::Mutex;
use crate::error::{HomardError, Result};
use super::client::LlmResponse;

struct CodexProcess {
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    _child: Child,
    thread_id: Option<String>,
    next_id: u64,
}

pub struct CodexServer {
    process: Mutex<Option<CodexProcess>>,
}

impl CodexServer {
    pub fn new() -> Self {
        Self {
            process: Mutex::new(None),
        }
    }

    /// Ensure the codex app-server process is running and initialized
    async fn ensure_running(&self) -> Result<()> {
        let mut proc = self.process.lock().await;
        if proc.is_some() {
            return Ok(());
        }

        tracing::info!("Starting codex app-server...");

        let mut child = tokio::process::Command::new("codex")
            .arg("app-server")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| HomardError::Llm(format!("Failed to start codex app-server: {}", e)))?;

        let stdin = child.stdin.take()
            .ok_or_else(|| HomardError::Llm("No stdin for codex app-server".to_string()))?;
        let stdout = child.stdout.take()
            .ok_or_else(|| HomardError::Llm("No stdout for codex app-server".to_string()))?;

        let mut cp = CodexProcess {
            stdin,
            stdout: BufReader::new(stdout),
            _child: child,
            thread_id: None,
            next_id: 1,
        };

        // Initialize
        let init_msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": cp.next_id,
            "method": "initialize",
            "params": {"clientInfo": {"name": "homard", "version": "0.1.0"}}
        });
        cp.next_id += 1;
        send_msg(&mut cp.stdin, &init_msg).await?;
        recv_msg(&mut cp.stdout).await?; // init response

        // Start ephemeral thread
        let thread_msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": cp.next_id,
            "method": "thread/start",
            "params": {
                "approvalPolicy": "never",
                "sandbox": "danger-full-access",
                "ephemeral": true,
            }
        });
        cp.next_id += 1;
        send_msg(&mut cp.stdin, &thread_msg).await?;
        let r = recv_msg(&mut cp.stdout).await?;
        let tid = r.get("result")
            .and_then(|r| r.get("thread"))
            .and_then(|t| t.get("id"))
            .and_then(|id| id.as_str())
            .ok_or_else(|| HomardError::Llm("No thread ID in response".to_string()))?
            .to_string();

        cp.thread_id = Some(tid.clone());
        tracing::info!("Codex app-server ready, thread: {}", &tid[..8]);

        // Drain notifications until MCP server is ready
        for _ in 0..20 {
            let n = recv_msg(&mut cp.stdout).await?;
            if n.get("params").and_then(|p| p.get("status")).and_then(|s| s.as_str()) == Some("ready") {
                break;
            }
        }

        *proc = Some(cp);
        Ok(())
    }

    /// Send a chat message and return the response
    pub async fn chat(&self, user_message: &str) -> Result<LlmResponse> {
        self.ensure_running().await?;

        let mut proc = self.process.lock().await;
        let cp = proc.as_mut()
            .ok_or_else(|| HomardError::Llm("Codex server not running".to_string()))?;

        let tid = cp.thread_id.clone()
            .ok_or_else(|| HomardError::Llm("No thread ID".to_string()))?;

        let turn_msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": cp.next_id,
            "method": "turn/start",
            "params": {
                "threadId": tid,
                "input": [{"type": "text", "text": user_message}]
            }
        });
        cp.next_id += 1;
        send_msg(&mut cp.stdin, &turn_msg).await?;

        // Collect streaming deltas until turn/completed
        let mut text = String::new();
        let timeout = tokio::time::Duration::from_secs(120);
        let deadline = tokio::time::Instant::now() + timeout;

        loop {
            let msg = tokio::time::timeout_at(deadline, recv_msg(&mut cp.stdout)).await
                .map_err(|_| HomardError::Llm("Codex response timed out after 120s".to_string()))?
                .map_err(|e| HomardError::Llm(format!("Codex read error: {}", e)))?;

            let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");

            match method {
                "item/agentMessage/delta" => {
                    if let Some(delta) = msg.get("params").and_then(|p| p.get("delta")).and_then(|d| d.as_str()) {
                        text.push_str(delta);
                    }
                }
                "turn/completed" => {
                    break;
                }
                _ => {
                    // Skip other notifications (thread/status/changed, item/started, etc.)
                }
            }
        }

        Ok(LlmResponse {
            content: text.trim().to_string(),
            tool_calls: Vec::new(),
        })
    }

    /// Pre-warm the server at startup (call from daemon init)
    pub async fn warmup(&self) {
        if let Err(e) = self.ensure_running().await {
            tracing::warn!("Codex app-server warmup failed (will retry on first chat): {}", e);
        }
    }

    /// Shutdown the codex process
    pub async fn shutdown(&self) {
        let mut proc = self.process.lock().await;
        if let Some(mut cp) = proc.take() {
            let _ = cp._child.kill().await;
        }
    }
}

async fn send_msg(stdin: &mut ChildStdin, msg: &serde_json::Value) -> Result<()> {
    let data = serde_json::to_string(msg).map_err(|e| HomardError::Llm(e.to_string()))?;
    stdin.write_all(data.as_bytes()).await.map_err(|e| HomardError::Llm(e.to_string()))?;
    stdin.write_all(b"\n").await.map_err(|e| HomardError::Llm(e.to_string()))?;
    stdin.flush().await.map_err(|e| HomardError::Llm(e.to_string()))?;
    Ok(())
}

async fn recv_msg(stdout: &mut BufReader<ChildStdout>) -> Result<serde_json::Value> {
    let mut line = String::new();
    stdout.read_line(&mut line).await.map_err(|e| HomardError::Llm(e.to_string()))?;
    if line.is_empty() {
        return Err(HomardError::Llm("Codex app-server process exited".to_string()));
    }
    serde_json::from_str(&line).map_err(|e| HomardError::Llm(format!("Invalid JSON from codex: {}", e)))
}
