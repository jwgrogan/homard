use crate::types::*;
use crate::error::{HomardError, Result};
use super::client::LlmResponse;

pub struct AnthropicProvider;

impl AnthropicProvider {
    pub async fn chat(
        http: &reqwest::Client,
        base_url: &str,
        token: &str,
        model: &str,
        messages: &[ChatMessage],
        tools: &[ToolSchema],
    ) -> Result<LlmResponse> {
        let url = format!("{}/messages", base_url);

        // Extract system message
        let system = messages.iter()
            .find(|m| m.role == "system")
            .map(|m| m.content.clone())
            .unwrap_or_default();

        // Convert messages to Anthropic format
        let msgs: Vec<serde_json::Value> = messages.iter()
            .filter(|m| m.role != "system")
            .map(|m| Self::adapt_message(m))
            .collect();

        // Convert tools to Anthropic format
        let tool_defs: Vec<serde_json::Value> = tools.iter().map(|t| {
            serde_json::json!({
                "name": t.name,
                "description": t.description,
                "input_schema": t.parameters,
            })
        }).collect();

        let mut body = serde_json::json!({
            "model": model,
            "max_tokens": 4096,
            "messages": msgs,
        });
        if !system.is_empty() {
            body["system"] = serde_json::json!(system);
        }
        if !tool_defs.is_empty() {
            body["tools"] = serde_json::json!(tool_defs);
        }

        // Determine auth header
        let is_oauth = token.starts_with("sk-ant-oat");
        let mut req = http.post(&url)
            .header("Content-Type", "application/json")
            .header("anthropic-version", "2023-06-01");

        if is_oauth {
            req = req.header("Authorization", format!("Bearer {}", token))
                .header("anthropic-beta", "oauth-2025-04-20");
        } else {
            req = req.header("x-api-key", token);
        }

        let resp = req.json(&body).send().await
            .map_err(|e| HomardError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();
            return Err(HomardError::Llm(format!("Anthropic HTTP {}: {}", status, body_text)));
        }

        let data: serde_json::Value = resp.json().await
            .map_err(|e| HomardError::Http(e.to_string()))?;

        Self::parse_response(&data)
    }

    fn adapt_message(msg: &ChatMessage) -> serde_json::Value {
        match msg.role.as_str() {
            "tool" => {
                serde_json::json!({
                    "role": "user",
                    "content": [{
                        "type": "tool_result",
                        "tool_use_id": msg.tool_call_id.as_deref().unwrap_or(""),
                        "content": msg.content,
                    }]
                })
            }
            "assistant" if msg.tool_calls.is_some() => {
                let mut content = Vec::new();
                if !msg.content.is_empty() {
                    content.push(serde_json::json!({"type": "text", "text": msg.content}));
                }
                if let Some(ref tcs) = msg.tool_calls {
                    for tc in tcs {
                        content.push(serde_json::json!({
                            "type": "tool_use",
                            "id": tc.id,
                            "name": tc.name,
                            "input": tc.arguments,
                        }));
                    }
                }
                serde_json::json!({"role": "assistant", "content": content})
            }
            _ => {
                serde_json::json!({"role": msg.role, "content": msg.content})
            }
        }
    }

    fn parse_response(data: &serde_json::Value) -> Result<LlmResponse> {
        let content_blocks = data.get("content")
            .and_then(|c| c.as_array())
            .ok_or_else(|| HomardError::Llm("Invalid Anthropic response".to_string()))?;

        let mut content = String::new();
        let mut tool_calls = Vec::new();

        for block in content_blocks {
            match block.get("type").and_then(|t| t.as_str()) {
                Some("text") => {
                    if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                        content.push_str(text);
                    }
                }
                Some("tool_use") => {
                    let id = block.get("id").and_then(|i| i.as_str()).unwrap_or("").to_string();
                    let name = block.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
                    let input = block.get("input").cloned().unwrap_or(serde_json::json!({}));
                    tool_calls.push(ToolCall { id, name, arguments: input });
                }
                _ => {}
            }
        }

        Ok(LlmResponse { content, tool_calls })
    }
}
