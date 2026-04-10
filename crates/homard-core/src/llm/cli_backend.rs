use std::sync::OnceLock;
use crate::types::*;
use crate::error::{HomardError, Result};
use super::client::LlmResponse;

static CODEX_PATH: OnceLock<Option<String>> = OnceLock::new();
static CLAUDE_PATH: OnceLock<Option<String>> = OnceLock::new();

/// Track whether we've had a first message (for --continue optimization)
static CLAUDE_HAS_SESSION: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn find_cli(binary: &str) -> Option<&'static str> {
    let cell = match binary {
        "codex" => &CODEX_PATH,
        "claude" => &CLAUDE_PATH,
        _ => return None,
    };
    cell.get_or_init(|| {
        std::process::Command::new("which")
            .arg(binary)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    }).as_deref()
}

/// Strip ANSI escape codes and control characters from CLI output
fn strip_ansi(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip escape sequence: ESC [ ... final_byte
            if chars.peek() == Some(&'[') {
                chars.next(); // consume '['
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if next.is_ascii_alphabetic() || next == '~' {
                        break;
                    }
                }
            }
        } else if c == '\r' {
            // Skip carriage returns (progress bars)
            continue;
        } else if c.is_control() && c != '\n' && c != '\t' {
            // Skip other control characters
            continue;
        } else {
            result.push(c);
        }
    }
    result
}

pub fn shell_escape_pub(s: &str) -> String {
    shell_escape(s)
}

fn shell_escape(s: &str) -> String {
    // Wrap in single quotes, escaping any single quotes in the string
    format!("'{}'", s.replace('\'', "'\\''"))
}

pub struct CliBackend;

impl CliBackend {
    /// Run a prompt through the Codex CLI (uses user's ChatGPT subscription)
    pub async fn codex_chat(messages: &[ChatMessage], tools: &[ToolSchema]) -> Result<LlmResponse> {
        let prompt = Self::build_prompt(messages, tools, false);
        let output = Self::run_cli("codex", &prompt, false).await?;
        Ok(LlmResponse {
            content: output,
            tool_calls: Vec::new(),
        })
    }

    /// Run a prompt through the Claude CLI (uses user's Anthropic auth)
    pub async fn claude_chat(messages: &[ChatMessage], tools: &[ToolSchema]) -> Result<LlmResponse> {
        let is_cont = CLAUDE_HAS_SESSION.load(std::sync::atomic::Ordering::Relaxed);
        let prompt = Self::build_prompt(messages, tools, is_cont);
        let output = Self::run_cli("claude", &prompt, is_cont).await?;
        CLAUDE_HAS_SESSION.store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(LlmResponse {
            content: output,
            tool_calls: Vec::new(),
        })
    }

    /// Build prompt — first call includes identity, subsequent calls just the user message
    fn build_prompt(messages: &[ChatMessage], _tools: &[ToolSchema], is_continuation: bool) -> String {
        // For continuations, just send the user's message — CLI already has context
        if is_continuation {
            return messages.iter().rev()
                .find(|m| m.role == "user")
                .map(|m| m.content.clone())
                .unwrap_or_default();
        }

        let mut parts = Vec::new();

        // First message: include identity context (truncated)
        if let Some(sys) = messages.iter().find(|m| m.role == "system") {
            let truncated = if sys.content.len() > 500 {
                format!("{}...", &sys.content[..500])
            } else {
                sys.content.clone()
            };
            parts.push(truncated);
        }

        if let Some(user_msg) = messages.iter().rev().find(|m| m.role == "user") {
            parts.push(user_msg.content.clone());
        }

        parts.join("\n\n")
    }

    async fn run_cli(binary: &str, prompt: &str, continue_session: bool) -> Result<String> {
        // Check CLI is available (cached after first lookup)
        let cli_path = find_cli(binary)
            .ok_or_else(|| HomardError::Llm(format!(
                "{} CLI not installed. Run `{}` to install it.",
                binary,
                if binary == "codex" { "npm i -g @openai/codex" } else { "npm i -g @anthropic-ai/claude-code" }
            )))?;

        // Run CLI via sh -c with echo piped to stdin (codex needs stdin closed)
        let shell_cmd = if binary == "claude" {
            let cont_flag = if continue_session { " --continue" } else { "" };
            format!("echo '' | {} -p {}{} --output-format text", cli_path, shell_escape(prompt), cont_flag)
        } else {
            format!("echo '' | {} exec {}", cli_path, shell_escape(prompt))
        };

        let mut cmd = tokio::process::Command::new("sh");
        cmd.arg("-c").arg(&shell_cmd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let child = cmd.spawn()
            .map_err(|e| HomardError::Llm(format!("Failed to start {}: {}", binary, e)))?;

        let output = tokio::time::timeout(
            std::time::Duration::from_secs(300), // 5 min timeout
            child.wait_with_output(),
        ).await
            .map_err(|_| HomardError::Llm(format!("{} timed out after 5 minutes", binary)))?
            .map_err(|e| HomardError::Llm(format!("{} failed: {}", binary, e)))?;

        let stdout = strip_ansi(&String::from_utf8_lossy(&output.stdout));
        let stderr = strip_ansi(&String::from_utf8_lossy(&output.stderr));

        if output.status.success() {
            let text = if stdout.trim().is_empty() {
                if stderr.trim().is_empty() { "(no output)".to_string() } else { stderr.trim().to_string() }
            } else {
                stdout.trim().to_string()
            };
            Ok(text)
        } else {
            Err(HomardError::Llm(format!("{} exited with error: {}", binary, stderr)))
        }
    }
}
