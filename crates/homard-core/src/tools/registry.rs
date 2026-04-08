use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use crate::types::ToolSchema;
use crate::error::{HomardError, Result};

type ToolHandler = Arc<dyn Fn(serde_json::Value) -> Pin<Box<dyn Future<Output = Result<String>> + Send>> + Send + Sync>;

pub struct ToolRegistry {
    tools: HashMap<String, (ToolSchema, ToolHandler)>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: HashMap::new() }
    }

    pub fn register<F, Fut>(&mut self, schema: ToolSchema, handler: F)
    where
        F: Fn(serde_json::Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<String>> + Send + 'static,
    {
        let name = schema.name.clone();
        let handler: ToolHandler = Arc::new(move |args| Box::pin(handler(args)));
        self.tools.insert(name, (schema, handler));
    }

    pub fn get_schemas(&self) -> Vec<ToolSchema> {
        self.tools.values().map(|(schema, _)| schema.clone()).collect()
    }

    pub async fn execute(&self, name: &str, arguments: &serde_json::Value) -> Result<String> {
        let (_, handler) = self.tools.get(name)
            .ok_or_else(|| HomardError::Tool(format!("Unknown tool: {}", name)))?;
        let result = handler(arguments.clone()).await?;
        Ok(Self::truncate_output(name, &result))
    }

    fn truncate_output(tool_name: &str, output: &str) -> String {
        let max = match tool_name {
            "web_fetch" => 4000,
            "shell_exec" => 2000,
            _ => 1000,
        };
        if output.len() <= max {
            output.to_string()
        } else {
            format!("{}...[truncated]", &output[..max])
        }
    }

    /// Register shell tools from config
    pub fn register_shell_tools(&mut self, shell_tools: &[crate::types::ShellTool]) {
        for tool in shell_tools {
            let command = tool.command.clone();
            let schema = ToolSchema {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": [],
                }),
            };
            self.register(schema, move |_args| {
                let cmd = command.clone();
                async move {
                    let output = tokio::process::Command::new("sh")
                        .arg("-c")
                        .arg(&cmd)
                        .output()
                        .await
                        .map_err(|e| HomardError::Tool(e.to_string()))?;
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    if output.status.success() {
                        Ok(stdout.to_string())
                    } else {
                        Ok(format!("Exit code: {}\nStdout: {}\nStderr: {}", output.status.code().unwrap_or(-1), stdout, stderr))
                    }
                }
            });
        }
    }
}
