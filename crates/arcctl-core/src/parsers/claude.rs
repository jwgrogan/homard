use super::{AgentNode, AgentStatus, SessionParser, SessionTree};
use std::collections::HashMap;

pub struct ClaudeParser;

impl SessionParser for ClaudeParser {
    fn parse(&self, content: &str, session_id: &str) -> Option<SessionTree> {
        let mut agents: HashMap<String, AgentNode> = HashMap::new();
        let mut parent_map: HashMap<String, String> = HashMap::new(); // child_id -> parent_id
        let mut root_activity: Option<String> = None;
        let mut root_files: Vec<String> = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let value: serde_json::Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let tool_use_id = value
                .get("toolUseID")
                .and_then(|v| v.as_str())
                .map(String::from);
            let parent_id = value
                .get("parentToolUseID")
                .and_then(|v| v.as_str())
                .map(String::from);

            // Check for assistant messages with tool calls
            if let Some(message) = value.get("message") {
                if let Some(content_arr) = message.get("content").and_then(|c| c.as_array()) {
                    for block in content_arr {
                        let block_type = block.get("type").and_then(|t| t.as_str());

                        match block_type {
                            Some("tool_use") => {
                                let tool_name =
                                    block.get("name").and_then(|n| n.as_str()).unwrap_or("");
                                let tool_id =
                                    block.get("id").and_then(|i| i.as_str()).unwrap_or("");

                                if tool_name == "Agent" {
                                    // Extract agent info from input
                                    let input = block.get("input");
                                    let description = input
                                        .and_then(|i| i.get("description"))
                                        .and_then(|d| d.as_str())
                                        .unwrap_or("Subagent");
                                    let agent_type = input
                                        .and_then(|i| i.get("subagent_type"))
                                        .and_then(|t| t.as_str())
                                        .map(String::from);
                                    let name = input
                                        .and_then(|i| i.get("name"))
                                        .and_then(|n| n.as_str())
                                        .unwrap_or(description);

                                    let node = AgentNode {
                                        id: tool_id.to_string(),
                                        name: name.to_string(),
                                        agent_type,
                                        status: AgentStatus::Working,
                                        current_activity: Some(description.to_string()),
                                        files_touched: Vec::new(),
                                        children: Vec::new(),
                                    };
                                    agents.insert(tool_id.to_string(), node);

                                    if let Some(ref pid) = tool_use_id {
                                        parent_map.insert(tool_id.to_string(), pid.clone());
                                    }
                                } else if matches!(
                                    tool_name,
                                    "Read" | "Write" | "Edit" | "Glob" | "Grep"
                                ) {
                                    // Extract file path
                                    if let Some(path) = block
                                        .get("input")
                                        .and_then(|i| {
                                            i.get("file_path")
                                                .or(i.get("path"))
                                                .or(i.get("pattern"))
                                        })
                                        .and_then(|p| p.as_str())
                                    {
                                        // Find which agent this belongs to
                                        let mut added = false;
                                        if let Some(ref pid) = parent_id {
                                            if let Some(agent) = agents.get_mut(pid) {
                                                if !agent.files_touched.contains(&path.to_string())
                                                {
                                                    agent.files_touched.push(path.to_string());
                                                }
                                                added = true;
                                            }
                                        }
                                        if !added && !root_files.contains(&path.to_string()) {
                                            root_files.push(path.to_string());
                                        }
                                    }
                                }
                            }
                            Some("text") => {
                                let text =
                                    block.get("text").and_then(|t| t.as_str()).unwrap_or("");
                                if !text.is_empty() {
                                    // Update activity for the parent agent or root
                                    let short = if text.len() > 100 {
                                        &text[..100]
                                    } else {
                                        text
                                    };
                                    if let Some(ref pid) = parent_id {
                                        if let Some(agent) = agents.get_mut(pid) {
                                            agent.current_activity = Some(short.to_string());
                                        }
                                    } else {
                                        root_activity = Some(short.to_string());
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }

            // Check for tool results (marks agents as done)
            if value.get("type").and_then(|t| t.as_str()) == Some("tool_result") {
                if let Some(tid) = value.get("toolUseID").and_then(|t| t.as_str()) {
                    if let Some(agent) = agents.get_mut(tid) {
                        agent.status = AgentStatus::Done;
                    }
                }
            }
        }

        // Build tree: nest children under parents
        let agent_ids: Vec<String> = agents.keys().cloned().collect();
        let mut children_map: HashMap<String, Vec<AgentNode>> = HashMap::new();

        for id in &agent_ids {
            if let Some(parent_id) = parent_map.get(id) {
                if agents.contains_key(parent_id) {
                    if let Some(node) = agents.remove(id) {
                        children_map.entry(parent_id.clone()).or_default().push(node);
                    }
                }
            }
        }

        // Assign children
        for (parent_id, children) in children_map {
            if let Some(parent) = agents.get_mut(&parent_id) {
                parent.children = children;
            }
        }

        // Remaining top-level agents become children of root
        let top_level: Vec<AgentNode> = agents.into_values().collect();

        let root = AgentNode {
            id: "root".to_string(),
            name: "Main Agent".to_string(),
            agent_type: None,
            status: if top_level.iter().any(|a| a.status == AgentStatus::Working) {
                AgentStatus::Working
            } else {
                AgentStatus::Done
            },
            current_activity: root_activity,
            files_touched: root_files,
            children: top_level,
        };

        Some(SessionTree {
            session_id: session_id.to_string(),
            root,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::SessionParser;

    fn sample_jsonl() -> &'static str {
        r#"{"type":"assistant","toolUseID":"tool-1","message":{"content":[{"type":"text","text":"I'll help you analyze this codebase by spawning a subagent to read the files."},{"type":"tool_use","id":"agent-abc","name":"Agent","input":{"description":"Analyze source files and report findings","name":"CodeAnalyzer","subagent_type":"general-coding"}}]}}
{"type":"assistant","toolUseID":"tool-2","parentToolUseID":"agent-abc","message":{"content":[{"type":"tool_use","id":"read-001","name":"Read","input":{"file_path":"/src/main.rs"}},{"type":"tool_use","id":"read-002","name":"Write","input":{"file_path":"/src/output.txt"}}]}}
{"type":"tool_result","toolUseID":"agent-abc"}
"#
    }

    #[test]
    fn test_parse_basic_session() {
        let parser = ClaudeParser;
        let result = parser.parse(sample_jsonl(), "session-001");
        assert!(result.is_some());

        let tree = result.unwrap();
        assert_eq!(tree.session_id, "session-001");
        assert_eq!(tree.root.id, "root");
        assert_eq!(tree.root.name, "Main Agent");
    }

    #[test]
    fn test_root_activity_extracted() {
        let parser = ClaudeParser;
        let tree = parser.parse(sample_jsonl(), "session-001").unwrap();
        assert!(tree.root.current_activity.is_some());
        let activity = tree.root.current_activity.unwrap();
        assert!(activity.contains("subagent"));
    }

    #[test]
    fn test_subagent_spawned() {
        let parser = ClaudeParser;
        let tree = parser.parse(sample_jsonl(), "session-001").unwrap();
        assert_eq!(tree.root.children.len(), 1);

        let subagent = &tree.root.children[0];
        assert_eq!(subagent.id, "agent-abc");
        assert_eq!(subagent.name, "CodeAnalyzer");
        assert_eq!(subagent.agent_type, Some("general-coding".to_string()));
    }

    #[test]
    fn test_subagent_marked_done() {
        let parser = ClaudeParser;
        let tree = parser.parse(sample_jsonl(), "session-001").unwrap();
        let subagent = &tree.root.children[0];
        assert_eq!(subagent.status, AgentStatus::Done);
    }

    #[test]
    fn test_root_files_collected() {
        let parser = ClaudeParser;
        let tree = parser.parse(sample_jsonl(), "session-001").unwrap();
        // Files touched by the tool calls under parentToolUseID=agent-abc go to that agent,
        // not root, because agent-abc is in agents map at time of processing
        // The root_files should be empty since all file ops are under agent-abc
        // (they have parentToolUseID set to agent-abc)
        assert!(tree.root.files_touched.is_empty());
    }

    #[test]
    fn test_empty_content() {
        let parser = ClaudeParser;
        let result = parser.parse("", "empty-session");
        assert!(result.is_some());
        let tree = result.unwrap();
        assert_eq!(tree.root.children.len(), 0);
        assert_eq!(tree.root.status, AgentStatus::Done);
    }

    #[test]
    fn test_invalid_json_lines_skipped() {
        let content = "not json\n{\"valid\": true}\nalso not json\n";
        let parser = ClaudeParser;
        let result = parser.parse(content, "session-002");
        assert!(result.is_some());
    }

    #[test]
    fn test_root_file_ops_without_parent() {
        let content = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"r1","name":"Read","input":{"file_path":"/some/file.rs"}}]}}"#;
        let parser = ClaudeParser;
        let tree = parser.parse(content, "session-003").unwrap();
        // No parentToolUseID, so should go to root_files
        assert!(tree.root.files_touched.contains(&"/some/file.rs".to_string()));
    }

    #[test]
    fn test_long_text_truncated_to_100_chars() {
        let long_text = "a".repeat(200);
        let content = format!(
            r#"{{"type":"assistant","message":{{"content":[{{"type":"text","text":"{}"}}]}}}}"#,
            long_text
        );
        let parser = ClaudeParser;
        let tree = parser.parse(&content, "session-004").unwrap();
        let activity = tree.root.current_activity.unwrap();
        assert_eq!(activity.len(), 100);
    }
}
