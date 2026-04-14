use super::anthropic::AnthropicProvider;
use super::cli_backend::CliBackend;
use super::oauth::OAuthManager;
use super::openai::OpenAiProvider;
use crate::config::HomardConfig;
use crate::error::{HomardError, Result};
use crate::security::SecurityManager;
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
    security: Arc<SecurityManager>,
}

impl LlmClient {
    pub fn new(
        shared_config: Arc<tokio::sync::RwLock<HomardConfig>>,
        oauth: Arc<OAuthManager>,
        security: Arc<SecurityManager>,
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
            security,
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

        let permission_level = self.security.permission_level();
        if matches!(
            provider_config.kind,
            ProviderKind::CodexCli | ProviderKind::ClaudeCli
        ) && permission_level != PermissionLevel::Autonomous
        {
            return Err(HomardError::Llm(
                "CLI-backed providers are only available in autonomous mode because they manage tools outside Homard's approval system. Use an API-backed provider for supervised or locked mode.".to_string(),
            ));
        }

        match provider_config.kind {
            // Codex CLI: run through the regular CLI backend so Homard's
            // permission model remains the source of truth.
            ProviderKind::CodexCli => CliBackend::codex_chat(messages, tools).await,
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
    pub async fn warmup_codex(&self) {}

    pub async fn set_active_provider(&self, provider: String) {
        let mut config = self.shared_config.write().await;
        config.active_provider = provider;
    }
}

#[cfg(test)]
mod tests {
    use super::LlmClient;
    use crate::config::HomardConfig;
    use crate::llm::oauth::OAuthManager;
    use crate::security::SecurityManager;
    use crate::types::{ChatMessage, PermissionLevel, ProviderConfig, ProviderKind};
    use std::collections::HashMap;
    use std::sync::Arc;

    #[tokio::test]
    async fn supervised_mode_rejects_cli_backed_provider() {
        let mut providers = HashMap::new();
        providers.insert(
            "codex_cli".to_string(),
            ProviderConfig {
                kind: ProviderKind::CodexCli,
                auth_type: "oauth_pkce".to_string(),
                client_id: None,
                token_keychain_ref: None,
                api_key_keychain_ref: None,
                model: "gpt-5.4".to_string(),
                base_url: None,
            },
        );

        let config = HomardConfig {
            providers,
            active_provider: "codex_cli".to_string(),
            ..HomardConfig::default()
        };

        let client = LlmClient::new(
            Arc::new(tokio::sync::RwLock::new(config)),
            Arc::new(OAuthManager::new()),
            Arc::new(SecurityManager::new(PermissionLevel::Supervised)),
        );

        let err = match client
            .chat(
                &[ChatMessage {
                    role: "user".to_string(),
                    content: "hello".to_string(),
                    tool_call_id: None,
                    tool_calls: None,
                    timestamp: None,
                }],
                &[],
            )
            .await
        {
            Ok(_) => panic!("cli provider should be rejected"),
            Err(err) => err,
        };

        assert!(err
            .to_string()
            .contains("CLI-backed providers are only available in autonomous mode"));
    }
}
