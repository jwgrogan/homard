use std::collections::HashMap;
use std::sync::Arc;
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
    tokens: Arc<RwLock<HashMap<String, OAuthTokens>>>,
    pending_verifiers: Arc<RwLock<HashMap<String, String>>>,
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
            authorize_url: "https://platform.claude.com/oauth/authorize".to_string(),
            token_url: "https://platform.claude.com/v1/oauth/token".to_string(),
            client_id: "https://claude.ai/oauth/claude-code-client-metadata".to_string(),
            scopes: "org:create_api_key user:profile user:inference".to_string(),
        });

        Self {
            tokens: Arc::new(RwLock::new(HashMap::new())),
            pending_verifiers: Arc::new(RwLock::new(HashMap::new())),
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

    /// Start OAuth flow: returns (auth_url, port). Spawns a temp callback server.
    pub async fn start_auth(&self, provider_name: &str) -> Result<(String, u16)> {
        let provider = self.providers.get(provider_name)
            .ok_or_else(|| HomardError::OAuth(format!("Unknown provider: {}", provider_name)))?;

        let (verifier, challenge) = Self::generate_pkce();

        // OpenAI requires port 1455 with localhost (not 127.0.0.1)
        // Anthropic uses ephemeral port with localhost
        let (listener, redirect_uri) = if provider_name == "openai" {
            let l = tokio::net::TcpListener::bind("127.0.0.1:1455").await
                .map_err(|e| HomardError::OAuth(format!("Port 1455 busy (is Codex running?): {}", e)))?;
            (l, "http://localhost:1455/auth/callback".to_string())
        } else {
            let l = tokio::net::TcpListener::bind("127.0.0.1:0").await
                .map_err(|e| HomardError::OAuth(format!("Failed to bind: {}", e)))?;
            let port = l.local_addr().map_err(|e| HomardError::OAuth(e.to_string()))?.port();
            let uri = format!("http://localhost:{}/callback", port);
            (l, uri)
        };

        let port = listener.local_addr()
            .map_err(|e| HomardError::OAuth(e.to_string()))?.port();

        // Generate random state
        use rand::Rng;
        let state: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();

        // Build auth URL with provider-specific params
        let mut auth_url = format!(
            "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&code_challenge={}&code_challenge_method=S256&state={}",
            provider.authorize_url,
            urlencoding::encode(&provider.client_id),
            urlencoding::encode(&redirect_uri),
            urlencoding::encode(&provider.scopes),
            challenge,
            state,
        );

        // OpenAI-specific params
        if provider_name == "openai" {
            auth_url.push_str("&codex_cli_simplified_flow=true&originator=homard&id_token_add_organizations=true");
        }

        // Store verifier + redirect_uri for later exchange
        self.pending_verifiers.write().await.insert(
            provider_name.to_string(),
            format!("{}||{}", verifier, redirect_uri),
        );

        // Spawn temp callback server
        let provider_name_owned = provider_name.to_string();
        let token_url = provider.token_url.clone();
        let client_id = provider.client_id.clone();
        let http = self.http.clone();
        let tokens_store = self.tokens.clone();
        let verifiers_store = self.pending_verifiers.clone();

        tokio::spawn(async move {
            // Accept one connection (the OAuth callback)
            let timeout = tokio::time::timeout(
                std::time::Duration::from_secs(300), // 5 min timeout
                listener.accept(),
            ).await;

            let (stream, _) = match timeout {
                Ok(Ok(v)) => v,
                _ => {
                    tracing::warn!("OAuth callback server timed out");
                    return;
                }
            };

            use tokio::io::{AsyncReadExt, AsyncWriteExt};

            // May need to accept multiple connections (browser sends favicon, etc.)
            // Keep accepting until we get one with a code parameter or timeout
            let mut stream = stream;
            let mut buf = [0u8; 8192];
            let n = stream.read(&mut buf).await.unwrap_or(0);
            let request = String::from_utf8_lossy(&buf[..n]);

            // Extract code from the HTTP request
            // Request line looks like: GET /auth/callback?code=xxx&state=yyy HTTP/1.1
            let first_line = request.lines().next().unwrap_or("");
            tracing::info!("OAuth callback received: {}", &first_line[..first_line.len().min(200)]);

            // Extract query string from the request path
            let code = first_line.split_whitespace().nth(1) // Get the path: /auth/callback?code=xxx
                .and_then(|path| path.split('?').nth(1)) // Get query: code=xxx&state=yyy
                .and_then(|qs| {
                    // Parse query params properly
                    for param in qs.split('&') {
                        if let Some(value) = param.strip_prefix("code=") {
                            let decoded = urlencoding::decode(value).unwrap_or(std::borrow::Cow::Borrowed(value));
                            return Some(decoded.to_string());
                        }
                    }
                    None
                });

            let code = match code {
                Some(c) if !c.is_empty() => c,
                _ => {
                    tracing::error!("OAuth callback: no code parameter found in: {}", first_line);
                    let response = format!(
                        "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\n\r\n<html><body><h1>Missing code parameter</h1><p>Request: {}</p></body></html>",
                        first_line.chars().take(200).collect::<String>()
                    );
                    let _ = stream.write_all(response.as_bytes()).await;
                    return;
                }
            };

            // Get stored verifier + redirect_uri
            let stored = verifiers_store.write().await.remove(&provider_name_owned);
            let (verifier, redirect_uri) = match stored {
                Some(s) => {
                    let parts: Vec<&str> = s.splitn(2, "||").collect();
                    if parts.len() == 2 {
                        (parts[0].to_string(), parts[1].to_string())
                    } else {
                        let response = "HTTP/1.1 500 Internal Server Error\r\nContent-Type: text/html\r\n\r\n<html><body><h1>Invalid state</h1></body></html>";
                        let _ = stream.write_all(response.as_bytes()).await;
                        return;
                    }
                }
                None => {
                    let response = "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\n\r\n<html><body><h1>No pending auth flow</h1></body></html>";
                    let _ = stream.write_all(response.as_bytes()).await;
                    return;
                }
            };

            // Exchange code for tokens
            let resp = http.post(&token_url)
                .form(&[
                    ("grant_type", "authorization_code"),
                    ("code", &code),
                    ("redirect_uri", &redirect_uri),
                    ("client_id", &client_id),
                    ("code_verifier", &verifier),
                ])
                .send()
                .await;

            let (success, message) = match resp {
                Ok(r) if r.status().is_success() => {
                    match r.json::<serde_json::Value>().await {
                        Ok(data) => {
                            let mut access_token = data.get("access_token").and_then(|t| t.as_str()).unwrap_or("").to_string();
                            let refresh_token = data.get("refresh_token").and_then(|t| t.as_str()).map(|s| s.to_string());
                            let id_token = data.get("id_token").and_then(|t| t.as_str()).map(|s| s.to_string());

                            // OpenAI: exchange id_token for API key (bills to subscription)
                            if provider_name_owned == "openai" {
                                tracing::info!("OpenAI: id_token present: {}", id_token.is_some());
                                if let Some(ref idt) = id_token {
                                    // Decode id_token to extract organization info
                                    let org_id = {
                                        let parts: Vec<&str> = idt.split('.').collect();
                                        if parts.len() >= 2 {
                                            use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
                                            let mut payload = parts[1].to_string();
                                            // Pad base64 if needed
                                            while payload.len() % 4 != 0 { payload.push('='); }
                                            URL_SAFE_NO_PAD.decode(&payload).ok()
                                                .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                                                .and_then(|claims| {
                                                    tracing::info!("OpenAI id_token claims keys: {:?}", claims.as_object().map(|o| o.keys().collect::<Vec<_>>()));
                                                    // Try organization_id directly or from organizations array
                                                    claims.get("organization_id").and_then(|o| o.as_str()).map(|s| s.to_string())
                                                        .or_else(|| {
                                                            claims.get("organizations").and_then(|o| o.as_array())
                                                                .and_then(|orgs| orgs.first())
                                                                .and_then(|org| org.get("id").or_else(|| org.get("organization_id")))
                                                                .and_then(|id| id.as_str())
                                                                .map(|s| s.to_string())
                                                        })
                                                })
                                        } else { None }
                                    };

                                    tracing::info!("OpenAI: org_id from id_token: {:?}", org_id);

                                    // Build token exchange request
                                    let mut form_params = vec![
                                        ("grant_type".to_string(), "urn:ietf:params:oauth:grant-type:token-exchange".to_string()),
                                        ("client_id".to_string(), client_id.clone()),
                                        ("requested_token".to_string(), "openai-api-key".to_string()),
                                        ("subject_token".to_string(), idt.clone()),
                                        ("subject_token_type".to_string(), "urn:ietf:params:oauth:token-type:id_token".to_string()),
                                    ];
                                    if let Some(ref oid) = org_id {
                                        form_params.push(("organization_id".to_string(), oid.clone()));
                                    }

                                    match http.post(&token_url)
                                        .form(&form_params)
                                        .send()
                                        .await
                                    {
                                        Ok(r) => {
                                            let status = r.status();
                                            let body = r.text().await.unwrap_or_default();
                                            tracing::info!("OpenAI token exchange: {} {}", status, &body[..body.len().min(300)]);
                                            if status.is_success() {
                                                if let Ok(key_data) = serde_json::from_str::<serde_json::Value>(&body) {
                                                    if let Some(key) = key_data.get("access_token").and_then(|t| t.as_str()) {
                                                        access_token = key.to_string();
                                                        tracing::info!("OpenAI: got API key (prefix: {}...)", &access_token[..access_token.len().min(12)]);
                                                    }
                                                }
                                            }
                                        }
                                        Err(e) => tracing::error!("OpenAI token exchange failed: {}", e),
                                    }
                                } else {
                                    tracing::warn!("OpenAI: no id_token, cannot exchange for API key");
                                }
                            }

                            let expires_in = data.get("expires_in").and_then(|e| e.as_u64());
                            let expires_at = expires_in.map(|secs| chrono::Utc::now() + chrono::Duration::seconds(secs as i64));

                            let tokens = OAuthTokens { access_token, refresh_token, expires_at };
                            tokens_store.write().await.insert(provider_name_owned.clone(), tokens.clone());

                            // Save to Keychain
                            #[cfg(target_os = "macos")]
                            {
                                let token_json = serde_json::json!({
                                    "access_token": tokens.access_token,
                                    "refresh_token": tokens.refresh_token,
                                    "expires_at": tokens.expires_at,
                                });
                                let service = format!("homard.{}", provider_name_owned);
                                let _ = crate::keychain::store_secret(&service, "oauth_tokens", &token_json.to_string());
                            }

                            // Save provider to config.json
                            let dirs = crate::config::HomardDirs::default_path();
                            let mut config = crate::config::HomardConfig::load_or_default(&dirs.config_path());
                            let provider_config = crate::types::ProviderConfig {
                                kind: match provider_name_owned.as_str() {
                                    "openai" => crate::types::ProviderKind::Openai,
                                    "anthropic" => crate::types::ProviderKind::Anthropic,
                                    _ => crate::types::ProviderKind::Openai,
                                },
                                auth_type: "oauth_pkce".to_string(),
                                client_id: None,
                                token_keychain_ref: Some(format!("homard.{}.oauth_tokens", provider_name_owned)),
                                api_key_keychain_ref: None,
                                model: match provider_name_owned.as_str() {
                                    "openai" => "gpt-5.4".to_string(),
                                    "anthropic" => "claude-sonnet-4-6".to_string(),
                                    _ => "gpt-5.4".to_string(),
                                },
                                base_url: None,
                            };
                            config.providers.insert(provider_name_owned.clone(), provider_config);
                            if config.providers.len() == 1 || config.active_provider == "anthropic" && provider_name_owned == "openai" {
                                config.active_provider = provider_name_owned.clone();
                            }
                            let _ = config.save(&dirs.config_path());

                            tracing::info!("OAuth flow complete for {}", provider_name_owned);
                            (true, format!("Connected to {}!", provider_name_owned))
                        }
                        Err(e) => (false, format!("Failed to parse token response: {}", e)),
                    }
                }
                Ok(r) => {
                    let body = r.text().await.unwrap_or_default();
                    (false, format!("Token exchange failed: {}", body))
                }
                Err(e) => (false, format!("Token exchange error: {}", e)),
            };

            // Send HTML response to browser
            let html = if success {
                format!(
                    r#"<html><head><style>body{{font-family:-apple-system,sans-serif;display:flex;align-items:center;justify-content:center;height:100vh;margin:0;background:#FAF5ED;color:#1B2D4F}}.c{{text-align:center;padding:2rem;border-radius:1rem;background:#FDF8F0;border:1px solid #C2D1C8}}h1{{color:#E85D4A}}</style></head><body><div class="c"><h1>🦞 {}</h1><p>You can close this tab.</p></div></body></html>"#,
                    message
                )
            } else {
                format!(
                    r#"<html><head><style>body{{font-family:-apple-system,sans-serif;display:flex;align-items:center;justify-content:center;height:100vh;margin:0;background:#FAF5ED;color:#1B2D4F}}.c{{text-align:center;padding:2rem;border-radius:1rem;background:#FDF8F0;border:1px solid #C2D1C8}}h1{{color:#E85D4A}}</style></head><body><div class="c"><h1>Authentication Failed</h1><p>{}</p></div></body></html>"#,
                    message
                )
            };

            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                html.len(),
                html
            );
            let _ = stream.write_all(response.as_bytes()).await;
        });

        Ok((auth_url, port))
    }

    /// Retrieve and consume a pending PKCE verifier for a provider
    pub async fn take_verifier(&self, provider_name: &str) -> Option<String> {
        self.pending_verifiers.write().await.remove(provider_name)
            .map(|s| s.split("||").next().unwrap_or("").to_string())
    }

    /// Exchange authorization code for tokens (used by API callback route as fallback)
    pub async fn exchange_code(
        &self,
        provider_name: &str,
        code: &str,
        code_verifier: &str,
        redirect_uri: &str,
    ) -> Result<OAuthTokens> {
        let provider = self.providers.get(provider_name)
            .ok_or_else(|| HomardError::OAuth(format!("Unknown provider: {}", provider_name)))?;

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
