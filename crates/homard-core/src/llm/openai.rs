use super::client::LlmResponse;
use crate::error::{HomardError, Result};
use crate::types::*;

pub struct OpenAiProvider;

impl OpenAiProvider {
    pub async fn chat(
        http: &reqwest::Client,
        base_url: &str,
        token: &str,
        model: &str,
        messages: &[ChatMessage],
        tools: &[ToolSchema],
    ) -> Result<LlmResponse> {
        let url = format!("{}/chat/completions", base_url);

        // Build messages array
        let msgs: Vec<serde_json::Value> = messages
            .iter()
            .map(|m| {
                let mut msg = serde_json::json!({
                    "role": m.role,
                    "content": m.content,
                });
                if let Some(ref tc_id) = m.tool_call_id {
                    msg["tool_call_id"] = serde_json::json!(tc_id);
                }
                if let Some(ref tcs) = m.tool_calls {
                    let tool_calls: Vec<serde_json::Value> = tcs
                        .iter()
                        .map(|tc| {
                            serde_json::json!({
                                "id": tc.id,
                                "type": "function",
                                "function": {
                                    "name": tc.name,
                                    "arguments": tc.arguments.to_string(),
                                }
                            })
                        })
                        .collect();
                    msg["tool_calls"] = serde_json::json!(tool_calls);
                }
                msg
            })
            .collect();

        // Build tools array
        let tool_defs: Vec<serde_json::Value> = tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters,
                    }
                })
            })
            .collect();

        let mut body = serde_json::json!({
            "model": model,
            "messages": msgs,
            "max_tokens": 4096,
        });
        if !tool_defs.is_empty() {
            body["tools"] = serde_json::json!(tool_defs);
            body["tool_choice"] = serde_json::json!("auto");
        }

        let response = Self::send_with_retry(http, &url, token, &body, 3).await?;
        Self::parse_response(&response)
    }

    async fn send_with_retry(
        http: &reqwest::Client,
        url: &str,
        token: &str,
        body: &serde_json::Value,
        max_retries: u32,
    ) -> Result<serde_json::Value> {
        let backoff = [1u64, 2, 4];
        let mut last_err = HomardError::Llm("no attempts".to_string());

        for attempt in 0..=max_retries {
            let resp = http
                .post(url)
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .json(body)
                .send()
                .await
                .map_err(|e| HomardError::Http(e.to_string()))?;

            let status = resp.status();
            if status.is_success() {
                let data: serde_json::Value = resp
                    .json()
                    .await
                    .map_err(|e| HomardError::Http(e.to_string()))?;
                return Ok(data);
            }

            let body_text = resp.text().await.unwrap_or_default();

            if (status.as_u16() == 429 || status.as_u16() == 529) && attempt < max_retries {
                let delay = backoff.get(attempt as usize).copied().unwrap_or(4);
                tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                last_err = HomardError::Llm(format!("HTTP {}: {}", status, body_text));
                continue;
            }

            return Err(HomardError::Llm(format!("HTTP {}: {}", status, body_text)));
        }

        Err(last_err)
    }

    fn parse_response(data: &serde_json::Value) -> Result<LlmResponse> {
        let choice = data
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .ok_or_else(|| HomardError::Llm("Invalid response: no choices".to_string()))?;

        let content = choice
            .get("content")
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string();

        let mut tool_calls = Vec::new();
        if let Some(tcs) = choice.get("tool_calls").and_then(|t| t.as_array()) {
            for tc in tcs {
                let id = tc
                    .get("id")
                    .and_then(|i| i.as_str())
                    .unwrap_or("")
                    .to_string();
                let func = tc.get("function").unwrap_or(&serde_json::Value::Null);
                let name = func
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("")
                    .to_string();
                let args_str = func
                    .get("arguments")
                    .and_then(|a| a.as_str())
                    .unwrap_or("{}");
                let arguments: serde_json::Value =
                    serde_json::from_str(args_str).unwrap_or(serde_json::json!({}));
                tool_calls.push(ToolCall {
                    id,
                    name,
                    arguments,
                });
            }
        }

        Ok(LlmResponse {
            content,
            tool_calls,
        })
    }

    /// Use the OpenAI Responses API (for OAuth/subscription billing)
    pub async fn responses(
        http: &reqwest::Client,
        base_url: &str,
        token: &str,
        model: &str,
        messages: &[ChatMessage],
        tools: &[ToolSchema],
    ) -> Result<LlmResponse> {
        let url = format!("{}/responses", base_url);

        // Build input — combine system prompt and conversation into a single input
        // The Responses API takes "input" as either a string or array of messages
        let input: Vec<serde_json::Value> = messages
            .iter()
            .map(|m| {
                let mut msg = serde_json::json!({
                    "role": m.role,
                    "content": m.content,
                });
                if let Some(ref tc_id) = m.tool_call_id {
                    msg["tool_call_id"] = serde_json::json!(tc_id);
                }
                if let Some(ref tcs) = m.tool_calls {
                    let tool_calls: Vec<serde_json::Value> = tcs
                        .iter()
                        .map(|tc| {
                            serde_json::json!({
                                "id": tc.id,
                                "type": "function",
                                "function": {
                                    "name": tc.name,
                                    "arguments": tc.arguments.to_string(),
                                }
                            })
                        })
                        .collect();
                    msg["tool_calls"] = serde_json::json!(tool_calls);
                }
                msg
            })
            .collect();

        // Build tools array (same format as chat/completions)
        let tool_defs: Vec<serde_json::Value> = tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters,
                    }
                })
            })
            .collect();

        let mut body = serde_json::json!({
            "model": model,
            "input": input,
        });
        if !tool_defs.is_empty() {
            body["tools"] = serde_json::json!(tool_defs);
        }

        let response = Self::send_with_retry(http, &url, token, &body, 3).await?;
        Self::parse_responses_response(&response)
    }

    fn parse_responses_response(data: &serde_json::Value) -> Result<LlmResponse> {
        // Responses API returns { output: [ { type: "message", content: [...] } ] }
        let output = data
            .get("output")
            .and_then(|o| o.as_array())
            .ok_or_else(|| {
                HomardError::Llm(format!(
                    "Invalid responses API response: {}",
                    serde_json::to_string(data)
                        .unwrap_or_default()
                        .chars()
                        .take(200)
                        .collect::<String>()
                ))
            })?;

        let mut content = String::new();
        let mut tool_calls = Vec::new();

        for item in output {
            match item.get("type").and_then(|t| t.as_str()) {
                Some("message") => {
                    if let Some(parts) = item.get("content").and_then(|c| c.as_array()) {
                        for part in parts {
                            if part.get("type").and_then(|t| t.as_str()) == Some("output_text") {
                                if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                                    content.push_str(text);
                                }
                            }
                        }
                    }
                }
                Some("function_call") => {
                    let id = item
                        .get("call_id")
                        .and_then(|i| i.as_str())
                        .unwrap_or("")
                        .to_string();
                    let name = item
                        .get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("")
                        .to_string();
                    let args_str = item
                        .get("arguments")
                        .and_then(|a| a.as_str())
                        .unwrap_or("{}");
                    let arguments: serde_json::Value =
                        serde_json::from_str(args_str).unwrap_or(serde_json::json!({}));
                    tool_calls.push(ToolCall {
                        id,
                        name,
                        arguments,
                    });
                }
                _ => {}
            }
        }

        Ok(LlmResponse {
            content,
            tool_calls,
        })
    }
}
