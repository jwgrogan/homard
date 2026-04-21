use crate::error::Result;
use crate::types::{ChatMessage, ToolSchema};
use std::path::PathBuf;

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
        let files = [
            "IDENTITY.md",
            "SOUL.md",
            "USER.md",
            "AGENTS.md",
            "TOOLS.md",
            "MEMORY.md",
        ];
        for filename in &files {
            let path = self.homard_dir.join(filename);
            if path.exists() {
                match tokio::fs::read_to_string(&path).await {
                    Ok(content) if !content.trim().is_empty() => {
                        parts.push(format!(
                            "# {}\n{}",
                            filename.trim_end_matches(".md"),
                            content.trim()
                        ));
                    }
                    _ => {}
                }
            }
        }

        // Add dynamic context
        let now = chrono::Local::now();
        parts.push(format!(
            "# Current Context\nDate: {}\nPlatform: macOS",
            now.format("%Y-%m-%d %H:%M %Z")
        ));

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

    /// Select relevant tools based on the user's message.
    /// Always includes core tools, adds extras based on keywords.
    pub fn select_tools(&self, message: &str, all_tools: &[ToolSchema]) -> Vec<ToolSchema> {
        let lower = message.to_lowercase();

        // Core tools always included
        let core_tools = [
            "shell_exec",
            "file_read",
            "file_write",
            "memory_save",
            "memory_search",
        ];

        // Keyword -> tool name mappings
        let keyword_tools: &[(&[&str], &str)] = &[
            (
                &["search", "find", "look up", "google", "what is"],
                "web_search",
            ),
            (
                &["url", "http", "fetch", "website", "page", "link"],
                "web_fetch",
            ),
            (
                &[
                    "deploy",
                    "build",
                    "run",
                    "code",
                    "fix",
                    "debug",
                    "implement",
                    "refactor",
                    "test",
                    "claude",
                    "codex",
                ],
                "spawn_session",
            ),
            (
                &["session", "running", "status", "check on"],
                "list_sessions",
            ),
            (&["kill", "stop", "cancel", "abort"], "kill_session"),
            (
                &["remember", "note", "save", "memory", "learn"],
                "memory_save",
            ),
            (&["recall", "memory", "did i", "what was"], "memory_search"),
            (
                &["profile", "my name", "about me", "i am", "i work"],
                "update_user_profile",
            ),
            (
                &["memory.md", "reorganize", "clean up memory", "prune"],
                "maintain_memory",
            ),
        ];

        let mut selected_names: std::collections::HashSet<&str> =
            core_tools.iter().copied().collect();

        for (keywords, tool_name) in keyword_tools {
            if keywords.iter().any(|kw| lower.contains(kw)) {
                selected_names.insert(tool_name);
            }
        }

        // If message is long or complex, include more tools
        if lower.len() > 200 || lower.contains("and") || lower.contains("then") {
            for tool in all_tools {
                selected_names.insert(&tool.name);
            }
        }

        all_tools
            .iter()
            .filter(|t| selected_names.contains(t.name.as_str()))
            .cloned()
            .collect()
    }
}
