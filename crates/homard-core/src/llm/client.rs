use std::sync::Arc;
use crate::types::*;
use crate::error::{HomardError, Result};
use super::openai::OpenAiProvider;
use super::anthropic::AnthropicProvider;
use super::oauth::OAuthManager;

pub struct LlmResponse {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
}

pub struct LlmClient {
    http: reqwest::Client,
    oauth: Arc<OAuthManager>,
    provider_configs: tokio::sync::RwLock<std::collections::HashMap<String, ProviderConfig>>,
    active_provider: tokio::sync::RwLock<String>,
}

impl LlmClient {
    pub fn new(
        provider_configs: std::collections::HashMap<String, ProviderConfig>,
        active_provider: String,
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
            provider_configs: tokio::sync::RwLock::new(provider_configs),
            active_provider: tokio::sync::RwLock::new(active_provider),
        }
    }

    /// Reload provider configs and active provider from disk
    pub async fn reload_config(&self) {
        let dirs = crate::config::HomardDirs::default_path();
        let fresh = crate::config::HomardConfig::load_or_default(&dirs.config_path());
        *self.provider_configs.write().await = fresh.providers;
        *self.active_provider.write().await = fresh.active_provider;
    }

    pub async fn chat(&self, messages: &[ChatMessage], tools: &[ToolSchema]) -> Result<LlmResponse> {
        let provider_name = self.active_provider.read().await.clone();
        let configs = self.provider_configs.read().await;

        // If provider not found, try reloading config from disk (OAuth may have added it)
        let config = match configs.get(&provider_name) {
            Some(c) => c.clone(),
            None => {
                drop(configs);
                self.reload_config().await;
                let configs = self.provider_configs.read().await;
                let provider_name = self.active_provider.read().await.clone();
                configs.get(&provider_name)
                    .ok_or_else(|| HomardError::Llm(format!("Provider '{}' not configured. Sign in via Settings.", provider_name)))?
                    .clone()
            }
        };

        // Get auth token
        let token = self.get_token(&provider_name, &config).await?;

        match config.kind {
            ProviderKind::Openai | ProviderKind::Openrouter => {
                let base_url = config.base_url.as_deref().unwrap_or(match config.kind {
                    ProviderKind::Openai => "https://api.openai.com/v1",
                    ProviderKind::Openrouter => "https://openrouter.ai/api/v1",
                    _ => unreachable!(),
                });
                OpenAiProvider::chat(&self.http, base_url, &token, &config.model, messages, tools).await
            }
            ProviderKind::Anthropic => {
                let base_url = config.base_url.as_deref().unwrap_or("https://api.anthropic.com/v1");
                AnthropicProvider::chat(&self.http, base_url, &token, &config.model, messages, tools).await
            }
        }
    }

    async fn get_token(&self, provider_name: &str, config: &ProviderConfig) -> Result<String> {
        match config.auth_type.as_str() {
            "oauth_pkce" => {
                self.oauth.get_valid_token(provider_name).await
            }
            "api_key" => {
                let keychain_ref = config.api_key_keychain_ref.as_deref()
                    .ok_or_else(|| HomardError::Llm("No API key configured".to_string()))?;
                // Parse service/account from ref like "homard.openrouter.api_key"
                #[cfg(target_os = "macos")]
                {
                    let parts: Vec<&str> = keychain_ref.split('.').collect();
                    let service = if parts.len() >= 2 { &parts[..parts.len()-1].join(".") } else { keychain_ref };
                    let account = parts.last().unwrap_or(&"api_key");
                    crate::keychain::read_secret(service, account)?
                        .ok_or_else(|| HomardError::Llm("API key not found in Keychain".to_string()))
                }
                #[cfg(not(target_os = "macos"))]
                {
                    let _ = keychain_ref;
                    Err(HomardError::Llm("Keychain only supported on macOS".to_string()))
                }
            }
            _ => Err(HomardError::Llm(format!("Unknown auth type: {}", config.auth_type))),
        }
    }

    pub async fn set_active_provider(&self, provider: String) {
        *self.active_provider.write().await = provider;
    }
}
