use std::collections::HashMap;
use tokio::sync::RwLock;
use crate::error::{HomardError, Result};

#[derive(Debug, Clone)]
pub struct OAuthTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone)]
pub struct OAuthProviderConfig {
    pub authorize_url: String,
    pub token_url: String,
    pub client_id: String,
    pub scopes: String,
}

pub struct OAuthManager {
    tokens: RwLock<HashMap<String, OAuthTokens>>,
    providers: HashMap<String, OAuthProviderConfig>,
    http: reqwest::Client,
}

impl OAuthManager {
    pub fn new() -> Self {
        let mut providers = HashMap::new();
        providers.insert("openai".to_string(), OAuthProviderConfig {
            authorize_url: "https://auth.openai.com/oauth/authorize".to_string(),
            token_url: "https://auth.openai.com/oauth/token".to_string(),
            client_id: "app_EMoamEEZ73f0CkXaXp7hrann".to_string(),
            scopes: "openid profile email offline_access".to_string(),
        });
        providers.insert("anthropic".to_string(), OAuthProviderConfig {
            authorize_url: "https://claude.ai/oauth/authorize".to_string(),
            token_url: "https://console.anthropic.com/v1/oauth/token".to_string(),
            client_id: "9d1c250a-e61b-44d9-88ed-5944d1962f5e".to_string(),
            scopes: "org:create_api_key user:profile user:inference".to_string(),
        });

        Self {
            tokens: RwLock::new(HashMap::new()),
            providers,
            http: reqwest::Client::new(),
        }
    }

    /// Generate PKCE code verifier and challenge
    fn generate_pkce() -> (String, String) {
        use rand::Rng;
        use sha2::{Sha256, Digest};
        use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

        let verifier: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(64)
            .map(char::from)
            .collect();

        let mut hasher = Sha256::new();
        hasher.update(verifier.as_bytes());
        let challenge = URL_SAFE_NO_PAD.encode(hasher.finalize());

        (verifier, challenge)
    }

    /// Start OAuth flow: returns (auth_url, code_verifier, local_port)
    pub async fn start_auth(&self, provider_name: &str) -> Result<(String, String, u16)> {
        let provider = self.providers.get(provider_name)
            .ok_or_else(|| HomardError::OAuth(format!("Unknown provider: {}", provider_name)))?;

        let (verifier, challenge) = Self::generate_pkce();

        // Bind to an ephemeral port
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await
            .map_err(|e| HomardError::OAuth(e.to_string()))?;
        let port = listener.local_addr()
            .map_err(|e| HomardError::OAuth(e.to_string()))?.port();

        let redirect_uri = format!("http://127.0.0.1:{}/callback", port);

        let auth_url = format!(
            "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&code_challenge={}&code_challenge_method=S256",
            provider.authorize_url,
            provider.client_id,
            urlencoding::encode(&redirect_uri),
            urlencoding::encode(&provider.scopes),
            challenge,
        );

        // Drop the listener so the port is free for the callback server
        drop(listener);

        Ok((auth_url, verifier, port))
    }

    /// Exchange authorization code for tokens
    pub async fn exchange_code(
        &self,
        provider_name: &str,
        code: &str,
        code_verifier: &str,
        port: u16,
    ) -> Result<OAuthTokens> {
        let provider = self.providers.get(provider_name)
            .ok_or_else(|| HomardError::OAuth(format!("Unknown provider: {}", provider_name)))?;

        let redirect_uri = format!("http://127.0.0.1:{}/callback", port);

        let resp = self.http.post(&provider.token_url)
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", &redirect_uri),
                ("client_id", &provider.client_id),
                ("code_verifier", code_verifier),
            ])
            .send()
            .await
            .map_err(|e| HomardError::OAuth(e.to_string()))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(HomardError::OAuth(format!("Token exchange failed: {}", body)));
        }

        let data: serde_json::Value = resp.json().await
            .map_err(|e| HomardError::OAuth(e.to_string()))?;

        let access_token = data.get("access_token")
            .and_then(|t| t.as_str())
            .ok_or_else(|| HomardError::OAuth("No access_token in response".to_string()))?
            .to_string();

        let refresh_token = data.get("refresh_token")
            .and_then(|t| t.as_str())
            .map(|s| s.to_string());

        let expires_in = data.get("expires_in")
            .and_then(|e| e.as_u64());

        let expires_at = expires_in.map(|secs| chrono::Utc::now() + chrono::Duration::seconds(secs as i64));

        let tokens = OAuthTokens { access_token, refresh_token, expires_at };

        // Store in memory
        self.tokens.write().await.insert(provider_name.to_string(), tokens.clone());

        // Store in Keychain
        #[cfg(target_os = "macos")]
        {
            let token_json = serde_json::json!({
                "access_token": tokens.access_token,
                "refresh_token": tokens.refresh_token,
                "expires_at": tokens.expires_at,
            });
            let service = format!("homard.{}", provider_name);
            crate::keychain::store_secret(&service, "oauth_tokens", &token_json.to_string())?;
        }

        Ok(tokens)
    }

    /// Get a valid token, refreshing if needed
    pub async fn get_valid_token(&self, provider_name: &str) -> Result<String> {
        // Check memory cache
        {
            let tokens = self.tokens.read().await;
            if let Some(t) = tokens.get(provider_name) {
                if let Some(exp) = t.expires_at {
                    if exp > chrono::Utc::now() + chrono::Duration::minutes(5) {
                        return Ok(t.access_token.clone());
                    }
                } else {
                    return Ok(t.access_token.clone());
                }
            }
        }

        // Try loading from Keychain
        #[cfg(target_os = "macos")]
        {
            let service = format!("homard.{}", provider_name);
            if let Some(json_str) = crate::keychain::read_secret(&service, "oauth_tokens")? {
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(&json_str) {
                    let access_token = data.get("access_token").and_then(|t| t.as_str()).unwrap_or("").to_string();
                    let refresh_token = data.get("refresh_token").and_then(|t| t.as_str()).map(|s| s.to_string());
                    let expires_at = data.get("expires_at").and_then(|e| e.as_str())
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                        .map(|d| d.with_timezone(&chrono::Utc));

                    let tokens = OAuthTokens { access_token, refresh_token, expires_at };

                    // Check if expired
                    if let Some(exp) = tokens.expires_at {
                        if exp > chrono::Utc::now() + chrono::Duration::minutes(5) {
                            self.tokens.write().await.insert(provider_name.to_string(), tokens.clone());
                            return Ok(tokens.access_token);
                        }
                    } else {
                        self.tokens.write().await.insert(provider_name.to_string(), tokens.clone());
                        return Ok(tokens.access_token);
                    }

                    // Try refresh
                    if let Some(ref rt) = tokens.refresh_token {
                        if let Ok(new_tokens) = self.refresh_token(provider_name, rt).await {
                            return Ok(new_tokens.access_token);
                        }
                    }
                }
            }
        }

        Err(HomardError::OAuth(format!("No valid token for '{}'. Please sign in.", provider_name)))
    }

    async fn refresh_token(&self, provider_name: &str, refresh_token: &str) -> Result<OAuthTokens> {
        let provider = self.providers.get(provider_name)
            .ok_or_else(|| HomardError::OAuth(format!("Unknown provider: {}", provider_name)))?;

        let resp = self.http.post(&provider.token_url)
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
                ("client_id", &provider.client_id),
            ])
            .send()
            .await
            .map_err(|e| HomardError::OAuth(e.to_string()))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(HomardError::OAuth(format!("Token refresh failed: {}", body)));
        }

        let data: serde_json::Value = resp.json().await
            .map_err(|e| HomardError::OAuth(e.to_string()))?;

        let access_token = data.get("access_token").and_then(|t| t.as_str()).unwrap_or("").to_string();
        let new_refresh = data.get("refresh_token").and_then(|t| t.as_str()).map(|s| s.to_string())
            .or_else(|| Some(refresh_token.to_string()));
        let expires_in = data.get("expires_in").and_then(|e| e.as_u64());
        let expires_at = expires_in.map(|secs| chrono::Utc::now() + chrono::Duration::seconds(secs as i64));

        let tokens = OAuthTokens { access_token, refresh_token: new_refresh, expires_at };
        self.tokens.write().await.insert(provider_name.to_string(), tokens.clone());

        #[cfg(target_os = "macos")]
        {
            let token_json = serde_json::json!({
                "access_token": tokens.access_token,
                "refresh_token": tokens.refresh_token,
                "expires_at": tokens.expires_at,
            });
            let service = format!("homard.{}", provider_name);
            crate::keychain::store_secret(&service, "oauth_tokens", &token_json.to_string())?;
        }

        Ok(tokens)
    }

    /// Load tokens from Keychain for a provider (called at startup)
    pub async fn load_from_keychain(&self, provider_name: &str) -> Result<bool> {
        #[cfg(target_os = "macos")]
        {
            let service = format!("homard.{}", provider_name);
            if let Some(json_str) = crate::keychain::read_secret(&service, "oauth_tokens")? {
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(&json_str) {
                    let tokens = OAuthTokens {
                        access_token: data.get("access_token").and_then(|t| t.as_str()).unwrap_or("").to_string(),
                        refresh_token: data.get("refresh_token").and_then(|t| t.as_str()).map(|s| s.to_string()),
                        expires_at: data.get("expires_at").and_then(|e| e.as_str())
                            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                            .map(|d| d.with_timezone(&chrono::Utc)),
                    };
                    self.tokens.write().await.insert(provider_name.to_string(), tokens);
                    return Ok(true);
                }
            }
            Ok(false)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = provider_name;
            Ok(false)
        }
    }
}
