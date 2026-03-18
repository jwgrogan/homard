use super::{AgentNode, AgentStatus, SessionParser, SessionTree};

pub struct GeminiParser;

impl SessionParser for GeminiParser {
    fn parse(&self, content: &str, session_id: &str) -> Option<SessionTree> {
        let session: serde_json::Value = serde_json::from_str(content).ok()?;
        let messages = session.get("messages")?.as_array()?;

        let mut children = Vec::new();
        let mut root_activity = None;

        for msg in messages {
            if msg.get("type").and_then(|t| t.as_str()) != Some("gemini") {
                continue;
            }

            // Update root activity from content
            if let Some(text) = msg.get("content").and_then(|c| c.as_str()) {
                let short = if text.len() > 100 { &text[..100] } else { text };
                root_activity = Some(short.to_string());
            }

            // Extract tool calls as agent children
            if let Some(tool_calls) = msg.get("toolCalls").and_then(|t| t.as_array()) {
                for tc in tool_calls {
                    let id = tc.get("id").and_then(|i| i.as_str()).unwrap_or("unknown");
                    let name = tc
                        .get("displayName")
                        .or(tc.get("name"))
                        .and_then(|n| n.as_str())
                        .unwrap_or("Tool Call");
                    let status = match tc.get("status").and_then(|s| s.as_str()) {
                        Some("success") => AgentStatus::Done,
                        Some("error") => AgentStatus::Error,
                        _ => AgentStatus::Working,
                    };

                    children.push(AgentNode {
                        id: id.to_string(),
                        name: name.to_string(),
                        agent_type: tc
                            .get("description")
                            .and_then(|d| d.as_str())
                            .map(String::from),
                        status,
                        current_activity: None,
                        files_touched: Vec::new(),
                        children: Vec::new(),
                    });
                }
            }
        }

        let root = AgentNode {
            id: "root".to_string(),
            name: "Main Agent".to_string(),
            agent_type: None,
            status: if children.iter().any(|c| c.status == AgentStatus::Working) {
                AgentStatus::Working
            } else {
                AgentStatus::Done
            },
            current_activity: root_activity,
            files_touched: Vec::new(),
            children,
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

    fn sample_gemini_session() -> &'static str {
        r#"{
  "messages": [
    {
      "type": "user",
      "content": "Please analyze this repository"
    },
    {
      "type": "gemini",
      "content": "I'll analyze the repository by running several tool calls.",
      "toolCalls": [
        {
          "id": "tc-001",
          "name": "read_file",
          "displayName": "Read Source File",
          "description": "Reads a source code file",
          "status": "success"
        },
        {
          "id": "tc-002",
          "name": "search_code",
          "displayName": "Search Codebase",
          "description": "Searches for patterns in code",
          "status": "success"
        }
      ]
    },
    {
      "type": "gemini",
      "content": "Now I'll generate a summary report.",
      "toolCalls": [
        {
          "id": "tc-003",
          "name": "write_file",
          "displayName": "Write Report",
          "status": "working"
        }
      ]
    }
  ]
}"#
    }

    #[test]
    fn test_parse_basic_gemini_session() {
        let parser = GeminiParser;
        let result = parser.parse(sample_gemini_session(), "gemini-session-001");
        assert!(result.is_some());

        let tree = result.unwrap();
        assert_eq!(tree.session_id, "gemini-session-001");
        assert_eq!(tree.root.id, "root");
        assert_eq!(tree.root.name, "Main Agent");
    }

    #[test]
    fn test_gemini_root_activity() {
        let parser = GeminiParser;
        let tree = parser.parse(sample_gemini_session(), "gemini-session-001").unwrap();
        // Last gemini message content should be the activity
        let activity = tree.root.current_activity.unwrap();
        assert!(activity.contains("summary report") || activity.contains("Now I'll generate"));
    }

    #[test]
    fn test_gemini_tool_calls_as_children() {
        let parser = GeminiParser;
        let tree = parser.parse(sample_gemini_session(), "gemini-session-001").unwrap();
        // Should have 3 tool calls as children (2 from first message + 1 from second)
        assert_eq!(tree.root.children.len(), 3);
    }

    #[test]
    fn test_gemini_tool_call_status_done() {
        let parser = GeminiParser;
        let tree = parser.parse(sample_gemini_session(), "gemini-session-001").unwrap();
        let done_children: Vec<_> = tree
            .root
            .children
            .iter()
            .filter(|c| c.status == AgentStatus::Done)
            .collect();
        assert_eq!(done_children.len(), 2);
    }

    #[test]
    fn test_gemini_root_status_working_when_any_child_working() {
        let parser = GeminiParser;
        let tree = parser.parse(sample_gemini_session(), "gemini-session-001").unwrap();
        // tc-003 has status "working", so root should be Working
        assert_eq!(tree.root.status, AgentStatus::Working);
    }

    #[test]
    fn test_gemini_display_name_preferred_over_name() {
        let parser = GeminiParser;
        let tree = parser.parse(sample_gemini_session(), "gemini-session-001").unwrap();
        let first = &tree.root.children[0];
        assert_eq!(first.name, "Read Source File");
    }

    #[test]
    fn test_gemini_tool_call_description() {
        let parser = GeminiParser;
        let tree = parser.parse(sample_gemini_session(), "gemini-session-001").unwrap();
        let first = &tree.root.children[0];
        assert_eq!(first.agent_type, Some("Reads a source code file".to_string()));
    }

    #[test]
    fn test_gemini_non_gemini_messages_skipped() {
        let content = r#"{"messages":[{"type":"user","content":"Hello","toolCalls":[{"id":"t1","name":"tool1","status":"success"}]}]}"#;
        let parser = GeminiParser;
        let tree = parser.parse(content, "session-x").unwrap();
        // User messages should be skipped
        assert_eq!(tree.root.children.len(), 0);
    }

    #[test]
    fn test_gemini_invalid_json_returns_none() {
        let parser = GeminiParser;
        let result = parser.parse("not valid json", "session-invalid");
        assert!(result.is_none());
    }

    #[test]
    fn test_gemini_missing_messages_field_returns_none() {
        let parser = GeminiParser;
        let result = parser.parse(r#"{"other": "data"}"#, "session-no-messages");
        assert!(result.is_none());
    }

    #[test]
    fn test_gemini_error_status() {
        let content = r#"{"messages":[{"type":"gemini","content":"Running","toolCalls":[{"id":"t1","name":"FailingTool","status":"error"}]}]}"#;
        let parser = GeminiParser;
        let tree = parser.parse(content, "session-err").unwrap();
        let child = &tree.root.children[0];
        assert_eq!(child.status, AgentStatus::Error);
    }

    #[test]
    fn test_gemini_all_done_root_status_done() {
        let content = r#"{"messages":[{"type":"gemini","content":"Done","toolCalls":[{"id":"t1","name":"Tool1","status":"success"},{"id":"t2","name":"Tool2","status":"success"}]}]}"#;
        let parser = GeminiParser;
        let tree = parser.parse(content, "session-done").unwrap();
        assert_eq!(tree.root.status, AgentStatus::Done);
    }
}
