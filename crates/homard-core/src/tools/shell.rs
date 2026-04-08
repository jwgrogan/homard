use crate::types::ToolSchema;
use crate::error::{HomardError, Result};

pub fn schema() -> ToolSchema {
    ToolSchema {
        name: "shell_exec".to_string(),
        description: "Execute a shell command and return its output".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "command": { "type": "string", "description": "The shell command to execute" }
            },
            "required": ["command"]
        }),
    }
}

pub async fn execute(args: serde_json::Value) -> Result<String> {
    let command = args.get("command")
        .and_then(|c| c.as_str())
        .ok_or_else(|| HomardError::Tool("Missing 'command' argument".to_string()))?;

    let output = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .output()
        .await
        .map_err(|e| HomardError::Tool(e.to_string()))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    Ok(format!("Exit code: {}\n{}{}", output.status.code().unwrap_or(-1), stdout, stderr))
}
