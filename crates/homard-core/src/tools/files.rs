use crate::types::ToolSchema;
use crate::error::{HomardError, Result};

const BLOCKED_READ_PATHS: &[&str] = &[
    ".ssh/",
    ".gnupg/",
    ".aws/credentials",
    ".env",
    "Keychain-2.db",
    "/etc/shadow",
    "/etc/master.passwd",
];

const ALLOWED_WRITE_PREFIXES: &[&str] = &[
    ".homard/",
];

fn is_read_blocked(path: &str) -> bool {
    BLOCKED_READ_PATHS.iter().any(|p| path.contains(p))
}

fn is_write_allowed(path: &str) -> bool {
    // Allow writes to ~/.homard/ and relative paths (workspace)
    let expanded = if path.starts_with("~/") {
        path.to_string()
    } else if path.starts_with('/') {
        path.to_string()
    } else {
        return true; // relative paths are OK (workspace)
    };

    // Check allowed prefixes (expand ~ to home dir)
    if let Some(home) = dirs::home_dir() {
        let home_str = home.to_string_lossy();
        for prefix in ALLOWED_WRITE_PREFIXES {
            let full_prefix = format!("{}/{}", home_str, prefix);
            if expanded.starts_with(&full_prefix) || expanded.starts_with(&format!("~/{}", prefix)) {
                return true;
            }
        }
    }

    false
}

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

    if is_read_blocked(path) {
        return Err(HomardError::Tool(format!("Access denied: '{}' is a sensitive file", path)));
    }

    tokio::fs::read_to_string(path).await
        .map_err(|e| HomardError::Tool(format!("Failed to read '{}': {}", path, e)))
}

pub async fn write(args: serde_json::Value) -> Result<String> {
    let path = args.get("path")
        .and_then(|p| p.as_str())
        .ok_or_else(|| HomardError::Tool("Missing 'path' argument".to_string()))?;

    if !is_write_allowed(path) {
        return Err(HomardError::Tool(format!("Write denied: '{}' is outside allowed directories. Writes are restricted to ~/.homard/", path)));
    }

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
