use std::sync::Arc;
use tokio::sync::watch;
use crate::llm::client::LlmClient;
use crate::tools::registry::ToolRegistry;
use crate::types::*;
use crate::store::Store;
use crate::error::Result;
use crate::security::SecurityManager;
use super::context::ContextBuilder;
use super::hang::HangDetector;

const MAX_ITERATIONS: u32 = 50;

pub struct AgentLoop {
    llm: Arc<LlmClient>,
    tools: Arc<ToolRegistry>,
    store: Arc<tokio::sync::Mutex<Store>>,
    context: ContextBuilder,
    security: Arc<SecurityManager>,
    stop_rx: watch::Receiver<bool>,
}

impl AgentLoop {
    pub fn new(
        llm: Arc<LlmClient>,
        tools: Arc<ToolRegistry>,
        store: Arc<tokio::sync::Mutex<Store>>,
        context: ContextBuilder,
        security: Arc<SecurityManager>,
        stop_rx: watch::Receiver<bool>,
    ) -> Self {
        Self { llm, tools, store, context, security, stop_rx }
    }

    pub async fn run(&self, channel: &str, user_message: &str, trigger: Trigger) -> Result<String> {
        let run_id = uuid::Uuid::new_v4().to_string();
        let run = AgentRun {
            id: run_id.clone(),
            channel: channel.to_string(),
            trigger: trigger.clone(),
            status: RunStatus::Running,
            started_at: chrono::Utc::now(),
            finished_at: None,
            duration_ms: None,
            error_message: None,
            iterations: 0,
        };
        {
            let store = self.store.lock().await;
            store.insert_run(&run)?;
        }

        // Build context
        let system_prompt = self.context.build_system_prompt().await?;
        let history = {
            let store = self.store.lock().await;
            store.get_history(channel, 15)?
        };

        let mut messages: Vec<ChatMessage> = Vec::new();
        messages.push(ChatMessage {
            role: "system".to_string(),
            content: system_prompt,
            tool_call_id: None,
            tool_calls: None,
            timestamp: None,
        });

        // Add windowed history
        let windowed = self.context.window_history(&history);
        messages.extend(windowed);

        // Add user message
        let user_msg = ChatMessage {
            role: "user".to_string(),
            content: user_message.to_string(),
            tool_call_id: None,
            tool_calls: None,
            timestamp: Some(chrono::Utc::now()),
        };
        messages.push(user_msg.clone());

        // Save user message
        {
            let store = self.store.lock().await;
            store.save_message(channel, &user_msg)?;
        }

        // Get tool schemas (dynamically selected based on message)
        let all_schemas = self.tools.get_schemas();
        let tool_schemas = self.context.select_tools(user_message, &all_schemas);

        let mut iterations = 0u32;
        let mut hang_detector = HangDetector::new();
        let start_time = std::time::Instant::now();
        let permission_level = self.security.permission_level();

        let result = loop {
            // Check stop signal
            if *self.stop_rx.borrow() {
                break Err(crate::error::HomardError::Agent("Run stopped by user".to_string()));
            }

            // Hang detection
            if let Some(action) = hang_detector.check(iterations, start_time.elapsed(), &permission_level) {
                match action {
                    super::hang::HangAction::Pause(msg) => {
                        // In supervised mode, we'd need to wait for user response
                        // For now, just break with the message
                        break Ok(msg);
                    }
                    super::hang::HangAction::Alert(msg) => {
                        // In autonomous mode, just log and continue
                        tracing::warn!("{}", msg);
                    }
                }
            }

            // Call LLM
            let response = self.llm.chat(&messages, &tool_schemas).await?;

            // If response has content and no tool calls, we're done
            if !response.content.is_empty() && response.tool_calls.is_empty() {
                let assistant_msg = ChatMessage {
                    role: "assistant".to_string(),
                    content: response.content.clone(),
                    tool_call_id: None,
                    tool_calls: None,
                    timestamp: Some(chrono::Utc::now()),
                };
                messages.push(assistant_msg.clone());
                {
                    let store = self.store.lock().await;
                    store.save_message(channel, &assistant_msg)?;
                }
                break Ok(response.content);
            }

            // If tool calls, execute them
            if !response.tool_calls.is_empty() {
                // Add assistant message with tool calls
                let assistant_msg = ChatMessage {
                    role: "assistant".to_string(),
                    content: response.content.clone(),
                    tool_call_id: None,
                    tool_calls: Some(response.tool_calls.clone()),
                    timestamp: Some(chrono::Utc::now()),
                };
                messages.push(assistant_msg.clone());
                {
                    let store = self.store.lock().await;
                    store.save_message(channel, &assistant_msg)?;
                }

                // Execute tools in parallel
                let mut tool_futures = Vec::new();
                for tc in &response.tool_calls {
                    let tools = self.tools.clone();
                    let security = self.security.clone();
                    let tc_clone = tc.clone();
                    tool_futures.push(async move {
                        // Check security
                        let approved = security.check_tool(&tc_clone.name, &tc_clone.arguments).await;
                        let args_str = tc_clone.arguments.to_string();
                        if !approved {
                            return (tc_clone.id.clone(), tc_clone.name.clone(), args_str, "Tool execution denied by security policy".to_string(), false);
                        }
                        let result = tools.execute(&tc_clone.name, &tc_clone.arguments).await;
                        (tc_clone.id.clone(), tc_clone.name.clone(), args_str, result.unwrap_or_else(|e| format!("Error: {}", e)), true)
                    });
                }

                let results = futures::future::join_all(tool_futures).await;

                // Add tool result messages
                for (_tool_call_id, tool_name, tool_args, result, approved) in &results {
                    // Audit log
                    {
                        let store = self.store.lock().await;
                        let _ = store.log_audit(tool_name, Some(tool_args), Some(result), *approved);
                    }
                }
                for (tool_call_id, _tool_name, _tool_args, result, _approved) in results {
                    let tool_msg = ChatMessage {
                        role: "tool".to_string(),
                        content: result,
                        tool_call_id: Some(tool_call_id),
                        tool_calls: None,
                        timestamp: None,
                    };
                    messages.push(tool_msg.clone());
                    {
                        let store = self.store.lock().await;
                        store.save_message(channel, &tool_msg)?;
                    }
                }

                hang_detector.record_tool_calls(&response.tool_calls);
            } else {
                // No content and no tool calls -- unexpected
                break Err(crate::error::HomardError::Agent("LLM returned empty response".to_string()));
            }

            iterations += 1;
            if iterations >= MAX_ITERATIONS {
                tracing::error!("Agent loop hit maximum iteration cap ({})", MAX_ITERATIONS);
                break Err(crate::error::HomardError::Agent(
                    format!("Reached maximum {} iterations. The task may be too complex — try breaking it into smaller steps.", MAX_ITERATIONS)
                ));
            }
        };

        // Complete the run
        let (status, error_msg) = match &result {
            Ok(_) => (RunStatus::Complete, None),
            Err(e) => (RunStatus::Error, Some(e.to_string())),
        };

        {
            let store = self.store.lock().await;
            store.complete_run(&run_id, status, error_msg.as_deref(), iterations)?;
        }

        result
    }
}
