use std::sync::Arc;
use crate::types::ToolSchema;
use crate::store::Store;
use crate::error::{HomardError, Result};

pub fn save_schema() -> ToolSchema {
    ToolSchema {
        name: "memory_save".to_string(),
        description: "Save an important fact or learned preference to long-term memory".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "fact": { "type": "string", "description": "The fact to remember" },
                "category": { "type": "string", "description": "Category: personal, work, preference, project, general" }
            },
            "required": ["fact"]
        }),
    }
}

pub fn search_schema() -> ToolSchema {
    ToolSchema {
        name: "memory_search".to_string(),
        description: "Search long-term memory for relevant facts".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search query" }
            },
            "required": ["query"]
        }),
    }
}

pub async fn save(args: serde_json::Value, store: Arc<tokio::sync::Mutex<Store>>) -> Result<String> {
    let fact = args.get("fact")
        .and_then(|f| f.as_str())
        .ok_or_else(|| HomardError::Tool("Missing 'fact' argument".to_string()))?;
    let category = args.get("category")
        .and_then(|c| c.as_str())
        .unwrap_or("general");

    let store = store.lock().await;
    store.save_memory(fact, category)?;
    Ok(format!("Saved to memory: {}", fact))
}

pub fn maintain_schema() -> ToolSchema {
    ToolSchema {
        name: "maintain_memory".to_string(),
        description: "Reorganize and prune MEMORY.md when it gets too long. Summarize old entries, remove duplicates.".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "updated_content": { "type": "string", "description": "The reorganized MEMORY.md content" }
            },
            "required": ["updated_content"]
        }),
    }
}

pub async fn maintain(args: serde_json::Value, homard_dir: std::path::PathBuf) -> Result<String> {
    let content = args.get("updated_content")
        .and_then(|c| c.as_str())
        .ok_or_else(|| HomardError::Tool("Missing 'updated_content' argument".to_string()))?;

    let path = homard_dir.join("MEMORY.md");
    tokio::fs::write(&path, content).await
        .map_err(|e| HomardError::Tool(format!("Failed to write MEMORY.md: {}", e)))?;

    Ok("MEMORY.md updated".to_string())
}

pub async fn search(args: serde_json::Value, store: Arc<tokio::sync::Mutex<Store>>) -> Result<String> {
    let query = args.get("query")
        .and_then(|q| q.as_str())
        .ok_or_else(|| HomardError::Tool("Missing 'query' argument".to_string()))?;

    let store = store.lock().await;
    let results = store.search_memories(query, 10)?;

    if results.is_empty() {
        Ok("No memories found.".to_string())
    } else {
        let formatted: Vec<String> = results.iter()
            .map(|(fact, cat)| format!("[{}] {}", cat, fact))
            .collect();
        Ok(formatted.join("\n"))
    }
}
