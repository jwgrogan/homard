use crate::types::*;
use crate::error::{HomardError, Result};
use super::client::LlmResponse;

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

    /// Build a single prompt string from the message history
    fn build_prompt(messages: &[ChatMessage], tools: &[ToolSchema]) -> String {
        let mut parts = Vec::new();

        for msg in messages {
            match msg.role.as_str() {
                "system" => parts.push(msg.content.clone()),
                "user" => parts.push(format!("User: {}", msg.content)),
                "assistant" => {
                    if !msg.content.is_empty() {
                        parts.push(format!("Assistant: {}", msg.content));
                    }
                }
                "tool" => {
                    parts.push(format!("Tool result: {}", msg.content));
                }
                _ => {}
            }
        }

        // Add tool descriptions so the CLI knows what's available
        if !tools.is_empty() {
            parts.push("\nAvailable tools (respond with JSON tool calls if needed):".to_string());
            for tool in tools {
                parts.push(format!("- {}: {}", tool.name, tool.description));
            }
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

        let mut cmd = tokio::process::Command::new(binary);

        if binary == "claude" {
            // claude -p "prompt" --output-format text
            cmd.arg("-p").arg(prompt)
                .arg("--output-format").arg("text");
        } else if binary == "codex" {
            // codex exec "prompt"
            cmd.arg("exec").arg(prompt);
        }

        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let child = cmd.spawn()
            .map_err(|e| HomardError::Llm(format!("Failed to start {}: {}", binary, e)))?;

        let output = tokio::time::timeout(
            std::time::Duration::from_secs(300), // 5 min timeout
            child.wait_with_output(),
        ).await
            .map_err(|_| HomardError::Llm(format!("{} timed out after 5 minutes", binary)))?
            .map_err(|e| HomardError::Llm(format!("{} failed: {}", binary, e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            if stdout.trim().is_empty() {
                // Some CLIs output to stderr
                Ok(if stderr.trim().is_empty() { "(no output)".to_string() } else { stderr })
            } else {
                Ok(stdout)
            }
        } else {
            Err(HomardError::Llm(format!("{} exited with error: {}", binary, stderr)))
        }
    }
}
