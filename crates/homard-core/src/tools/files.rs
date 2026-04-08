use crate::types::ToolSchema;
use crate::error::{HomardError, Result};

pub fn read_schema() -> ToolSchema {
    ToolSchema {
        name: "file_read".to_string(),
        description: "Read content from a file".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "File path to read" }
            },
            "required": ["path"]
        }),
    }
}

pub fn write_schema() -> ToolSchema {
    ToolSchema {
        name: "file_write".to_string(),
        description: "Write content to a file".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "File path to write" },
                "content": { "type": "string", "description": "Content to write" }
            },
            "required": ["path", "content"]
        }),
    }
}

pub async fn read(args: serde_json::Value) -> Result<String> {
    let path = args.get("path")
        .and_then(|p| p.as_str())
        .ok_or_else(|| HomardError::Tool("Missing 'path' argument".to_string()))?;

    tokio::fs::read_to_string(path).await
        .map_err(|e| HomardError::Tool(format!("Failed to read '{}': {}", path, e)))
}

pub async fn write(args: serde_json::Value) -> Result<String> {
    let path = args.get("path")
        .and_then(|p| p.as_str())
        .ok_or_else(|| HomardError::Tool("Missing 'path' argument".to_string()))?;
    let content = args.get("content")
        .and_then(|c| c.as_str())
        .ok_or_else(|| HomardError::Tool("Missing 'content' argument".to_string()))?;

    if let Some(parent) = std::path::Path::new(path).parent() {
        tokio::fs::create_dir_all(parent).await
            .map_err(|e| HomardError::Tool(e.to_string()))?;
    }

    tokio::fs::write(path, content).await
        .map_err(|e| HomardError::Tool(format!("Failed to write '{}': {}", path, e)))?;

    Ok(format!("Wrote {} bytes to {}", content.len(), path))
}
