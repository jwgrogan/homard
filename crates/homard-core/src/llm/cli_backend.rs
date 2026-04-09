use crate::types::*;
use crate::error::{HomardError, Result};
use super::client::LlmResponse;

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

fn shell_escape(s: &str) -> String {
    // Wrap in single quotes, escaping any single quotes in the string
    format!("'{}'", s.replace('\'', "'\\''"))
}

pub struct CliBackend;

impl CliBackend {
    /// Run a prompt through the Codex CLI (uses user's ChatGPT subscription)
    pub async fn codex_chat(messages: &[ChatMessage], tools: &[ToolSchema]) -> Result<LlmResponse> {
        let prompt = Self::build_prompt(messages, tools);
        let output = Self::run_cli("codex", &prompt).await?;
        Ok(LlmResponse {
            content: output,
            tool_calls: Vec::new(), // CLI doesn't return structured tool calls
        })
    }

    /// Run a prompt through the Claude CLI (uses user's Anthropic auth)
    pub async fn claude_chat(messages: &[ChatMessage], tools: &[ToolSchema]) -> Result<LlmResponse> {
        let prompt = Self::build_prompt(messages, tools);
        let output = Self::run_cli("claude", &prompt).await?;
        Ok(LlmResponse {
            content: output,
            tool_calls: Vec::new(),
        })
    }

    /// Build a prompt for the CLI backend.
    /// Only sends the system prompt (identity) and the last user message.
    /// The CLI has its own context management — don't overwhelm it.
    fn build_prompt(messages: &[ChatMessage], _tools: &[ToolSchema]) -> String {
        let mut parts = Vec::new();

        // Include system prompt (identity files) but truncated
        if let Some(sys) = messages.iter().find(|m| m.role == "system") {
            // Take first 500 chars of system prompt to set identity
            let truncated = if sys.content.len() > 500 {
                format!("{}...", &sys.content[..500])
            } else {
                sys.content.clone()
            };
            parts.push(truncated);
        }

        // Include last user message only
        if let Some(user_msg) = messages.iter().rev().find(|m| m.role == "user") {
            parts.push(user_msg.content.clone());
        }

        parts.join("\n\n")
    }

    async fn run_cli(binary: &str, prompt: &str) -> Result<String> {
        // Check CLI is available
        let which = tokio::process::Command::new("which")
            .arg(binary)
            .output()
            .await
            .map_err(|e| HomardError::Llm(format!("{} not found: {}", binary, e)))?;

        if !which.status.success() {
            return Err(HomardError::Llm(format!(
                "{} CLI not installed. Run `{}` to install it.",
                binary,
                if binary == "codex" { "npm i -g @openai/codex" } else { "npm i -g @anthropic-ai/claude-code" }
            )));
        }

        // Run CLI via sh -c with echo piped to stdin (codex needs stdin closed)
        let shell_cmd = if binary == "claude" {
            format!("echo '' | claude -p {} --output-format text", shell_escape(prompt))
        } else {
            format!("echo '' | codex exec {}", shell_escape(prompt))
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
