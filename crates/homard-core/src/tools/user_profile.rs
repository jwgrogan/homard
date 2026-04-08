use std::path::PathBuf;
use crate::types::ToolSchema;
use crate::error::{HomardError, Result};

pub fn schema() -> ToolSchema {
    ToolSchema {
        name: "update_user_profile".to_string(),
        description: "Update the user's profile (USER.md) with newly learned information".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "content": { "type": "string", "description": "Updated USER.md content" }
            },
            "required": ["content"]
        }),
    }
}

pub async fn execute(args: serde_json::Value, homard_dir: PathBuf) -> Result<String> {
    let content = args.get("content")
        .and_then(|c| c.as_str())
        .ok_or_else(|| HomardError::Tool("Missing 'content' argument".to_string()))?;

    let path = homard_dir.join("USER.md");
    tokio::fs::write(&path, content).await
        .map_err(|e| HomardError::Tool(format!("Failed to write USER.md: {}", e)))?;

    Ok("Updated USER.md".to_string())
}
