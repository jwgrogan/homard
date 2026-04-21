use super::anthropic::AnthropicProvider;
use super::cli_backend::CliBackend;
use super::codex_server::CodexServer;
use super::oauth::OAuthManager;
use super::openai::OpenAiProvider;
use crate::config::HomardConfig;
use crate::error::{HomardError, Result};
use crate::types::*;
use std::sync::Arc;

pub struct LlmResponse {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
}

pub struct LlmClient {
    http: reqwest::Client,
    oauth: Arc<OAuthManager>,
    shared_config: Arc<tokio::sync::RwLock<HomardConfig>>,
    codex_server: Arc<CodexServer>,
}

impl LlmClient {
    pub fn new(
        shared_config: Arc<tokio::sync::RwLock<HomardConfig>>,
        oauth: Arc<OAuthManager>,
    ) -> Self {
        let http = reqwest::Client::builder()
            .pool_max_idle_per_host(5)
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .expect("failed to build HTTP client");

        Self {
            http,
            oauth,
            shared_config,
            codex_server: Arc::new(CodexServer::new()),
        }
    }

    pub async fn chat(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolSchema],
    ) -> Result<LlmResponse> {
        let config = self.shared_config.read().await;
        let provider_name = config.active_provider.clone();
        let provider_config = config
            .providers
            .get(&provider_name)
            .ok_or_else(|| {
                HomardError::Llm(format!(
                    "Provider '{}' not configured. Sign in via Settings.",
                    provider_name
                ))
            })?
            .clone();
        drop(config); // Release lock before making LLM call

        match provider_config.kind {
            // Codex: use persistent app-server (sub-second after first call)
            ProviderKind::CodexCli => {
                let user_msg = messages
                    .iter()
                    .rev()
                    .find(|m| m.role == "user")
                    .map(|m| m.content.as_str())
                    .unwrap_or("");
                self.codex_server.chat(user_msg).await
            }
            ProviderKind::ClaudeCli => CliBackend::claude_chat(messages, tools).await,
            // Direct API backends -- use HTTP with our own auth
            ProviderKind::Openai | ProviderKind::Openrouter => {
                let token = self.get_token(&provider_name, &provider_config).await?;
                let base_url = provider_config
                    .base_url
                    .as_deref()
                    .unwrap_or(match provider_config.kind {
                        ProviderKind::Openai => "https://api.openai.com/v1",
                        ProviderKind::Openrouter => "https://openrouter.ai/api/v1",
                        _ => unreachable!(),
                    });
                OpenAiProvider::chat(
                    &self.http,
                    base_url,
                    &token,
                    &provider_config.model,
                    messages,
                    tools,
                )
                .await
            }
            ProviderKind::Anthropic => {
                let token = self.get_token(&provider_name, &provider_config).await?;
                let base_url = provider_config
                    .base_url
                    .as_deref()
                    .unwrap_or("https://api.anthropic.com/v1");
                AnthropicProvider::chat(
                    &self.http,
                    base_url,
                    &token,
                    &provider_config.model,
                    messages,
                    tools,
                )
                .await
            }
        }
    }

    async fn get_token(&self, provider_name: &str, config: &ProviderConfig) -> Result<String> {
        match config.auth_type.as_str() {
            "oauth_pkce" => self.oauth.get_valid_token(provider_name).await,
            "api_key" => {
                let keychain_ref = config
                    .api_key_keychain_ref
                    .as_deref()
                    .ok_or_else(|| HomardError::Llm("No API key configured".to_string()))?;
                // Parse service/account from ref like "homard.openrouter.api_key"
                #[cfg(target_os = "macos")]
                {
                    let parts: Vec<&str> = keychain_ref.split('.').collect();
                    let service = if parts.len() >= 2 {
                        &parts[..parts.len() - 1].join(".")
                    } else {
                        keychain_ref
                    };
                    let account = parts.last().unwrap_or(&"api_key");
                    crate::keychain::read_secret(service, account)?.ok_or_else(|| {
                        HomardError::Llm("API key not found in Keychain".to_string())
                    })
                }
                #[cfg(not(target_os = "macos"))]
                {
                    let _ = keychain_ref;
                    Err(HomardError::Llm(
                        "Keychain only supported on macOS".to_string(),
                    ))
                }
            }
            _ => Err(HomardError::Llm(format!(
                "Unknown auth type: {}",
                config.auth_type
            ))),
        }
    }

    /// Pre-warm the codex app-server at startup
    pub async fn warmup_codex(&self) {
        self.codex_server.warmup().await;
    }

    pub async fn set_active_provider(&self, provider: String) {
        let mut config = self.shared_config.write().await;
        config.active_provider = provider;
    }
}
