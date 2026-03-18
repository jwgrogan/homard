use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tracing::{debug, warn};

use crate::error::{ArcctlError, Result};
use crate::types::Trigger;

// ---------------------------------------------------------------------------
// SessionInfo
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub agent: Option<String>,
    pub profile: Option<String>,
    pub directory: String,
    pub trigger: Trigger,
    pub started_at: DateTime<Utc>,
    pub pid: Option<u32>,
}

// ---------------------------------------------------------------------------
// ProcessRegistry
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct ProcessRegistry {
    inner: Arc<Mutex<HashMap<String, SessionInfo>>>,
}

impl ProcessRegistry {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn register(&self, id: &str, info: SessionInfo) {
        let mut map = self.inner.lock().expect("registry lock poisoned");
        map.insert(id.to_string(), info);
    }

    pub fn remove(&self, id: &str) {
        let mut map = self.inner.lock().expect("registry lock poisoned");
        map.remove(id);
    }

    pub fn list(&self) -> Vec<SessionInfo> {
        let map = self.inner.lock().expect("registry lock poisoned");
        map.values().cloned().collect()
    }

    pub fn get(&self, id: &str) -> Option<SessionInfo> {
        let map = self.inner.lock().expect("registry lock poisoned");
        map.get(id).cloned()
    }
}

impl Default for ProcessRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Free functions
// ---------------------------------------------------------------------------

/// Parse a single JSONL line into a `serde_json::Value`.
pub fn parse_jsonl_line(line: &str) -> Result<serde_json::Value> {
    serde_json::from_str(line.trim()).map_err(ArcctlError::Json)
}

/// Spawn the `claude` CLI process.
///
/// Arguments:
/// - `-p <prompt>`
/// - `--output-format stream-json` (when `output_format` is `Some("stream-json")` or similar)
/// - `--agent <path>` (when `agent` is `Some`)
///
/// The `CLAUDE_CODE_ENTRY_POINT` environment variable is removed from the
/// child's environment so that the subprocess doesn't inherit it.
pub async fn spawn_claude(
    prompt: &str,
    directory: &str,
    agent: Option<&str>,
    output_format: Option<&str>,
) -> Result<Child> {
    let mut cmd = Command::new("claude");

    cmd.arg("-p").arg(prompt);

    if let Some(fmt) = output_format {
        cmd.arg("--output-format").arg(fmt);
    }

    if let Some(agent_path) = agent {
        cmd.arg("--agent").arg(agent_path);
    }

    cmd.current_dir(directory)
        .env_remove("CLAUDE_CODE_ENTRY_POINT")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    debug!("Spawning claude in directory: {}", directory);

    cmd.spawn().map_err(ArcctlError::Io)
}

/// Stream stdout lines from a child process, parse each as JSON, and call
/// `on_line` with the parsed value.  Lines that fail to parse are logged and
/// skipped.
pub async fn stream_jsonl<F>(mut child: Child, mut on_line: F)
where
    F: FnMut(serde_json::Value),
{
    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if line.trim().is_empty() {
                continue;
            }
            match parse_jsonl_line(&line) {
                Ok(value) => on_line(value),
                Err(e) => warn!("Failed to parse JSONL line: {} — {:?}", line, e),
            }
        }
    }

    // Wait for the process to exit so it is fully reaped.
    let _ = child.wait().await;
}

/// Attempt a graceful shutdown: send SIGTERM, wait up to 5 seconds, then
/// forcefully kill with `child.kill()` (which sends SIGKILL on Unix).
pub async fn kill_session(mut child: Child) {
    // Send SIGTERM to the child process.
    if let Some(pid) = child.id() {
        unsafe {
            libc::kill(pid as libc::pid_t, libc::SIGTERM);
        }
    }

    // Give the process up to 5 seconds to exit cleanly.
    let wait_result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        child.wait(),
    )
    .await;

    match wait_result {
        Ok(Ok(_status)) => {
            debug!("Process exited cleanly after SIGTERM");
        }
        _ => {
            // Timeout or wait error — force kill.
            warn!("Process did not exit after SIGTERM; sending SIGKILL");
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
    }
}

// ---------------------------------------------------------------------------
// PID liveness check
// ---------------------------------------------------------------------------

/// Check if a process is still alive by sending signal 0.
pub fn is_pid_alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_session(id: &str) -> SessionInfo {
        SessionInfo {
            id: id.to_string(),
            agent: None,
            profile: None,
            directory: "/tmp".to_string(),
            trigger: Trigger::Manual,
            started_at: Utc::now(),
            pid: None,
        }
    }

    #[test]
    fn test_parse_jsonl_line() {
        let line = r#"{"type":"text","content":"hello"}"#;
        let value = parse_jsonl_line(line).expect("should parse valid JSON");
        assert_eq!(value["type"], "text");
        assert_eq!(value["content"], "hello");
    }

    #[test]
    fn test_parse_jsonl_non_json() {
        let line = "this is not json at all";
        let result = parse_jsonl_line(line);
        assert!(result.is_err(), "invalid JSON should return an error");
    }

    #[test]
    fn test_registry_add_and_list() {
        let registry = ProcessRegistry::new();

        let session_a = make_session("sess-a");
        let session_b = make_session("sess-b");

        registry.register("sess-a", session_a.clone());
        registry.register("sess-b", session_b.clone());

        let list = registry.list();
        assert_eq!(list.len(), 2);

        // get() should also work
        let fetched = registry.get("sess-a").expect("sess-a should exist");
        assert_eq!(fetched.id, "sess-a");
    }

    #[test]
    fn test_registry_remove() {
        let registry = ProcessRegistry::new();

        registry.register("sess-x", make_session("sess-x"));
        assert_eq!(registry.list().len(), 1);

        registry.remove("sess-x");
        assert!(registry.list().is_empty(), "registry should be empty after remove");
        assert!(registry.get("sess-x").is_none());
    }

    #[test]
    fn test_is_pid_alive_current_process() {
        let pid = std::process::id();
        assert!(is_pid_alive(pid), "current process should be alive");
    }

    #[test]
    fn test_is_pid_alive_nonexistent() {
        // PID 999999 is very unlikely to exist
        assert!(!is_pid_alive(999999), "PID 999999 should not be alive");
    }
}
