use crate::error::{HomardError, Result};
use crate::types::ToolSchema;
use std::path::{Component, Path, PathBuf};

const BLOCKED_READ_PATHS: &[&str] = &[
    ".ssh/",
    ".gnupg/",
    ".aws/credentials",
    ".env",
    "Keychain-2.db",
    "/etc/shadow",
    "/etc/master.passwd",
];

fn is_read_blocked(path: &str) -> bool {
    BLOCKED_READ_PATHS.iter().any(|p| path.contains(p))
}

fn is_write_allowed(path: &str) -> bool {
    resolve_write_path(path).is_some()
}

fn has_parent_dir(path: &Path) -> bool {
    path.components()
        .any(|component| matches!(component, Component::ParentDir))
}

fn resolve_write_path(path: &str) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let homard_root = home.join(".homard");

    let input = Path::new(path);
    if has_parent_dir(input) {
        return None;
    }

    let resolved = if let Some(stripped) = path.strip_prefix("~/") {
        home.join(stripped)
    } else if input.is_absolute() {
        input.to_path_buf()
    } else {
        let workspace = homard_root.join("workspace");
        let _ = std::fs::create_dir_all(&workspace);
        workspace.join(input)
    };

    if resolved.starts_with(&homard_root) {
        Some(resolved)
    } else {
        None
    }
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
    let path = args
        .get("path")
        .and_then(|p| p.as_str())
        .ok_or_else(|| HomardError::Tool("Missing 'path' argument".to_string()))?;

    if is_read_blocked(path) {
        return Err(HomardError::Tool(format!(
            "Access denied: '{}' is a sensitive file",
            path
        )));
    }

    tokio::fs::read_to_string(path)
        .await
        .map_err(|e| HomardError::Tool(format!("Failed to read '{}': {}", path, e)))
}

pub async fn write(args: serde_json::Value) -> Result<String> {
    let path = args
        .get("path")
        .and_then(|p| p.as_str())
        .ok_or_else(|| HomardError::Tool("Missing 'path' argument".to_string()))?;

    let resolved = resolve_write_path(path).ok_or_else(|| {
        HomardError::Tool(format!(
            "Write denied: '{}' is outside allowed directories. Writes are restricted to ~/.homard/",
            path
        ))
    })?;

    if !is_write_allowed(path) {
        return Err(HomardError::Tool(format!("Write denied: '{}' is outside allowed directories. Writes are restricted to ~/.homard/", path)));
    }

    let content = args
        .get("content")
        .and_then(|c| c.as_str())
        .ok_or_else(|| HomardError::Tool("Missing 'content' argument".to_string()))?;

    if let Some(parent) = resolved.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| HomardError::Tool(e.to_string()))?;
    }

    tokio::fs::write(&resolved, content).await.map_err(|e| {
        HomardError::Tool(format!("Failed to write '{}': {}", resolved.display(), e))
    })?;

    Ok(format!(
        "Wrote {} bytes to {}",
        content.len(),
        resolved.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::resolve_write_path;

    #[test]
    fn relative_write_paths_resolve_into_homard_workspace() {
        let resolved = resolve_write_path("notes/todo.txt").expect("resolved path");
        let home = dirs::home_dir().expect("home dir");
        assert_eq!(resolved, home.join(".homard/workspace/notes/todo.txt"));
    }

    #[test]
    fn parent_dir_components_are_rejected() {
        assert!(resolve_write_path("../escape.txt").is_none());
        assert!(resolve_write_path("~/../escape.txt").is_none());
    }
}
