use std::path::PathBuf;
use crate::error::Result;
use crate::types::ChatMessage;

pub struct ContextBuilder {
    homard_dir: PathBuf,
}

impl ContextBuilder {
    pub fn new(homard_dir: PathBuf) -> Self {
        Self { homard_dir }
    }

    pub async fn build_system_prompt(&self) -> Result<String> {
        let mut parts = Vec::new();

        // Load identity files in order
        let files = ["IDENTITY.md", "SOUL.md", "USER.md", "AGENTS.md", "TOOLS.md", "MEMORY.md"];
        for filename in &files {
            let path = self.homard_dir.join(filename);
            if path.exists() {
                match tokio::fs::read_to_string(&path).await {
                    Ok(content) if !content.trim().is_empty() => {
                        parts.push(format!("# {}\n{}", filename.trim_end_matches(".md"), content.trim()));
                    }
                    _ => {}
                }
            }
        }

        // Add dynamic context
        let now = chrono::Local::now();
        parts.push(format!("# Current Context\nDate: {}\nPlatform: macOS", now.format("%Y-%m-%d %H:%M %Z")));

        Ok(parts.join("\n\n"))
    }

    /// Window conversation history: last 4 always, 5-15 if substantive
    pub fn window_history(&self, history: &[ChatMessage]) -> Vec<ChatMessage> {
        let len = history.len();
        if len <= 4 {
            return history.to_vec();
        }

        let mut windowed = Vec::new();

        // Messages 5-15 (from end): only if substantive
        let older_start = if len > 15 { len - 15 } else { 0 };
        let older_end = len - 4;
        for msg in &history[older_start..older_end] {
            if msg.content.len() > 100 || msg.role == "tool" {
                windowed.push(msg.clone());
            }
        }

        // Last 4 always included
        windowed.extend_from_slice(&history[len - 4..]);
        windowed
    }
}
