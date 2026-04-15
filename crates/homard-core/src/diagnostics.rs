use std::collections::HashMap;
use std::path::Path;

use serde::Serialize;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

use crate::api::AppState;
use crate::config::{HomardConfig, HomardDirs};
use crate::types::{AgentRun, PermissionLevel, ServerMode};

const SETTINGS_FILES: [&str; 8] = [
    "BOOTSTRAP.md",
    "IDENTITY.md",
    "SOUL.md",
    "USER.md",
    "AGENTS.md",
    "TOOLS.md",
    "HEARTBEAT.md",
    "MEMORY.md",
];

#[derive(Debug, Clone, Serialize)]
pub struct SettingsSnapshot {
    pub overview: SettingsOverview,
    pub providers: HashMap<String, ProviderDiagnostics>,
    pub telegram: TelegramDiagnostics,
    pub daemon: DaemonDiagnostics,
    pub identity: IdentityDiagnostics,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SettingsOverview {
    pub active_provider: Option<String>,
    pub active_model: Option<String>,
    pub configured_provider_count: usize,
    pub ready_provider_count: usize,
    pub permission_level: PermissionLevel,
    pub telegram_connected: bool,
    pub telegram_label: String,
    pub current_run: Option<String>,
    pub running_sessions: usize,
    pub assistant_name: Option<String>,
    pub user_name: Option<String>,
    pub server_mode: ServerMode,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderDiagnostics {
    pub key: String,
    pub label: String,
    pub configured: bool,
    pub active: bool,
    pub connected: bool,
    pub installed: Option<bool>,
    pub binary_path: Option<String>,
    pub version: Option<String>,
    pub model: Option<String>,
    pub auth_status: String,
    pub auth_detail: Option<String>,
    pub mcp_servers: Vec<McpServerDiagnostics>,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpServerDiagnostics {
    pub name: String,
    pub target: String,
    pub status: String,
    pub auth: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TelegramDiagnostics {
    pub connected: bool,
    pub token_configured: bool,
    pub bot_name: Option<String>,
    pub paired_chat_ids: Vec<String>,
    pub allowed_usernames: Vec<String>,
    pub status_label: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DaemonDiagnostics {
    pub server_mode: ServerMode,
    pub launchd_installed: bool,
    pub current_run: Option<String>,
    pub current_run_id: Option<String>,
    pub running_sessions: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct IdentityDiagnostics {
    pub assistant_name: Option<String>,
    pub assistant_emoji: Option<String>,
    pub assistant_tagline: Option<String>,
    pub user_name: Option<String>,
    pub user_role: Option<String>,
}

pub async fn build_settings_snapshot(state: &AppState) -> SettingsSnapshot {
    let homard_dirs = HomardDirs::default_path();
    let fresh = HomardConfig::load_or_default(&homard_dirs.config_path());
    {
        let mut config = state.config.write().await;
        *config = fresh.clone();
    }

    let home_dir = dirs::home_dir().unwrap_or_else(|| state.homard_dir.clone());
    let (codex_cli, claude_cli, openai, anthropic, openrouter, telegram, identity) = tokio::join!(
        inspect_codex_cli(&fresh, &home_dir),
        inspect_claude_cli(&fresh, &home_dir),
        inspect_oauth_provider("openai", "OpenAI", &fresh, state),
        inspect_oauth_provider("anthropic", "Anthropic", &fresh, state),
        inspect_api_key_provider("openrouter", "OpenRouter", &fresh),
        inspect_telegram(&fresh),
        inspect_identity(&state.homard_dir),
    );

    let (current_run, current_run_id, running_sessions) = {
        let store = state.store.lock().await;
        let running_run = store.get_running_run().ok().flatten();
        let running_sessions = store
            .get_running_sessions()
            .map(|sessions| sessions.len())
            .unwrap_or(0);
        (
            running_run.as_ref().map(format_run_label),
            running_run.as_ref().map(|run| run.id.clone()),
            running_sessions,
        )
    };

    let launchd_installed = home_dir
        .join("Library/LaunchAgents/com.homard.daemon.plist")
        .exists();

    let providers = HashMap::from([
        ("codex_cli".to_string(), codex_cli),
        ("claude_cli".to_string(), claude_cli),
        ("openai".to_string(), openai),
        ("anthropic".to_string(), anthropic),
        ("openrouter".to_string(), openrouter),
    ]);

    let configured_provider_count = providers
        .values()
        .filter(|provider| provider.configured)
        .count();
    let ready_provider_count = providers
        .values()
        .filter(|provider| provider.connected)
        .count();

    SettingsSnapshot {
        overview: SettingsOverview {
            active_provider: if fresh.providers.is_empty() {
                None
            } else {
                Some(fresh.active_provider.clone())
            },
            active_model: fresh
                .providers
                .get(&fresh.active_provider)
                .map(|provider| provider.model.clone()),
            configured_provider_count,
            ready_provider_count,
            permission_level: fresh.permission_level.clone(),
            telegram_connected: telegram.connected,
            telegram_label: telegram.status_label.clone(),
            current_run: current_run.clone(),
            running_sessions,
            assistant_name: identity.assistant_name.clone(),
            user_name: identity.user_name.clone(),
            server_mode: fresh.server_mode.clone(),
        },
        providers,
        telegram,
        daemon: DaemonDiagnostics {
            server_mode: fresh.server_mode.clone(),
            launchd_installed,
            current_run,
            current_run_id,
            running_sessions,
        },
        identity,
        files: SETTINGS_FILES
            .iter()
            .map(|name| (*name).to_string())
            .collect(),
    }
}

async fn inspect_codex_cli(config: &HomardConfig, cwd: &Path) -> ProviderDiagnostics {
    let installed = which("codex").await;
    let configured = config.providers.get("codex_cli");
    let active = config.active_provider == "codex_cli" && configured.is_some();

    if let Some(binary_path) = installed {
        let version =
            command_text("codex", &["--version"], Some(cwd), Duration::from_secs(3)).await;
        let login_status = command_text(
            "codex",
            &["login", "status"],
            Some(cwd),
            Duration::from_secs(3),
        )
        .await;
        let mcp_output =
            command_text("codex", &["mcp", "list"], Some(cwd), Duration::from_secs(5)).await;
        let connected = login_status
            .as_deref()
            .map(|text| text.to_lowercase().contains("logged in"))
            .unwrap_or(false);

        ProviderDiagnostics {
            key: "codex_cli".to_string(),
            label: "Codex CLI".to_string(),
            configured: configured.is_some(),
            active,
            connected,
            installed: Some(true),
            binary_path: Some(binary_path),
            version,
            model: configured.map(|provider| provider.model.clone()),
            auth_status: if connected {
                "Ready".to_string()
            } else {
                "Needs login".to_string()
            },
            auth_detail: login_status,
            mcp_servers: mcp_output
                .as_deref()
                .map(parse_codex_mcp_list)
                .unwrap_or_default(),
        }
    } else {
        ProviderDiagnostics {
            key: "codex_cli".to_string(),
            label: "Codex CLI".to_string(),
            configured: configured.is_some(),
            active,
            connected: false,
            installed: Some(false),
            binary_path: None,
            version: None,
            model: configured.map(|provider| provider.model.clone()),
            auth_status: "Not installed".to_string(),
            auth_detail: Some("Install the Codex CLI to enable this provider.".to_string()),
            mcp_servers: Vec::new(),
        }
    }
}

async fn inspect_claude_cli(config: &HomardConfig, cwd: &Path) -> ProviderDiagnostics {
    let installed = which("claude").await;
    let configured = config.providers.get("claude_cli");
    let active = config.active_provider == "claude_cli" && configured.is_some();

    if let Some(binary_path) = installed {
        let version =
            command_text("claude", &["--version"], Some(cwd), Duration::from_secs(3)).await;
        let auth_output = command_text(
            "claude",
            &["auth", "status"],
            Some(cwd),
            Duration::from_secs(3),
        )
        .await;
        let mcp_output = command_text(
            "claude",
            &["mcp", "list"],
            Some(cwd),
            Duration::from_secs(8),
        )
        .await;
        let auth_json = auth_output
            .as_deref()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok());
        let connected = auth_json
            .as_ref()
            .and_then(|json| json.get("loggedIn"))
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        let auth_detail = auth_json
            .as_ref()
            .map(format_claude_auth_detail)
            .or(auth_output.clone());

        ProviderDiagnostics {
            key: "claude_cli".to_string(),
            label: "Claude CLI".to_string(),
            configured: configured.is_some(),
            active,
            connected,
            installed: Some(true),
            binary_path: Some(binary_path),
            version,
            model: configured.map(|provider| provider.model.clone()),
            auth_status: if connected {
                "Ready".to_string()
            } else {
                "Needs login".to_string()
            },
            auth_detail,
            mcp_servers: mcp_output
                .as_deref()
                .map(parse_claude_mcp_list)
                .unwrap_or_default(),
        }
    } else {
        ProviderDiagnostics {
            key: "claude_cli".to_string(),
            label: "Claude CLI".to_string(),
            configured: configured.is_some(),
            active,
            connected: false,
            installed: Some(false),
            binary_path: None,
            version: None,
            model: configured.map(|provider| provider.model.clone()),
            auth_status: "Not installed".to_string(),
            auth_detail: Some("Install Claude Code to enable this provider.".to_string()),
            mcp_servers: Vec::new(),
        }
    }
}

async fn inspect_oauth_provider(
    key: &str,
    label: &str,
    config: &HomardConfig,
    state: &AppState,
) -> ProviderDiagnostics {
    let configured = config.providers.get(key);
    let active = config.active_provider == key && configured.is_some();
    let connected = if configured
        .and_then(|provider| provider.token_keychain_ref.as_deref())
        .is_some()
    {
        state.oauth.load_from_keychain(key).await.unwrap_or(false)
    } else {
        false
    };

    ProviderDiagnostics {
        key: key.to_string(),
        label: label.to_string(),
        configured: configured.is_some(),
        active,
        connected,
        installed: None,
        binary_path: None,
        version: None,
        model: configured.map(|provider| provider.model.clone()),
        auth_status: if connected {
            "Connected".to_string()
        } else if configured.is_some() {
            "Token missing".to_string()
        } else {
            "Not connected".to_string()
        },
        auth_detail: if connected {
            Some("OAuth token found in secure storage.".to_string())
        } else if configured.is_some() {
            Some("Provider is configured, but no valid OAuth token was found.".to_string())
        } else {
            Some("Connect in the browser to add this provider.".to_string())
        },
        mcp_servers: Vec::new(),
    }
}

async fn inspect_api_key_provider(
    key: &str,
    label: &str,
    config: &HomardConfig,
) -> ProviderDiagnostics {
    let configured = config.providers.get(key);
    let active = config.active_provider == key && configured.is_some();
    let connected = configured
        .and_then(|provider| provider.api_key_keychain_ref.as_deref())
        .map(keychain_ref_exists)
        .unwrap_or(false);

    ProviderDiagnostics {
        key: key.to_string(),
        label: label.to_string(),
        configured: configured.is_some(),
        active,
        connected,
        installed: None,
        binary_path: None,
        version: None,
        model: configured.map(|provider| provider.model.clone()),
        auth_status: if connected {
            "Connected".to_string()
        } else if configured.is_some() {
            "Key missing".to_string()
        } else {
            "Not connected".to_string()
        },
        auth_detail: if connected {
            Some("API key found in secure storage.".to_string())
        } else if configured.is_some() {
            Some("Provider is configured, but the API key was not found.".to_string())
        } else {
            Some("Save an API key to enable this provider.".to_string())
        },
        mcp_servers: Vec::new(),
    }
}

async fn inspect_telegram(config: &HomardConfig) -> TelegramDiagnostics {
    let token_configured = config.telegram.token_keychain_ref.is_some();
    let mut bot_name = None;
    let mut connected = false;

    if token_configured {
        let dirs = HomardDirs::default_path();
        #[cfg(target_os = "macos")]
        if let Ok(Some(token)) = crate::config::get_telegram_token(&dirs) {
            let client = crate::telegram::TelegramClient::new(&token);
            if let Ok(name) = client.verify().await {
                connected = true;
                bot_name = Some(name);
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = dirs;
        }
    }

    let status_label = if connected {
        match bot_name.as_deref() {
            Some(name) => format!("Connected to @{}", name),
            None => "Connected".to_string(),
        }
    } else if token_configured {
        "Token saved, but verification failed".to_string()
    } else {
        "Not connected".to_string()
    };

    TelegramDiagnostics {
        connected,
        token_configured,
        bot_name,
        paired_chat_ids: config.telegram.paired_chat_ids.clone(),
        allowed_usernames: config.telegram.allowed_usernames.clone(),
        status_label,
    }
}

async fn inspect_identity(homard_dir: &Path) -> IdentityDiagnostics {
    let identity_text = tokio::fs::read_to_string(homard_dir.join("IDENTITY.md"))
        .await
        .unwrap_or_default();
    let user_text = tokio::fs::read_to_string(homard_dir.join("USER.md"))
        .await
        .unwrap_or_default();

    IdentityDiagnostics {
        assistant_name: find_kv(&identity_text, "name"),
        assistant_emoji: find_kv(&identity_text, "emoji"),
        assistant_tagline: find_kv(&identity_text, "tagline"),
        user_name: find_named_field(&user_text, "Name"),
        user_role: find_named_field(&user_text, "Role"),
    }
}

fn format_run_label(run: &AgentRun) -> String {
    let trigger = match run.trigger {
        crate::types::Trigger::Chat => "Chat",
        crate::types::Trigger::Telegram => "Telegram",
        crate::types::Trigger::Cron => "Schedule",
        crate::types::Trigger::Cli => "CLI",
    };
    let channel = if run.channel == "chat" {
        "Local thread".to_string()
    } else {
        run.channel.clone()
    };
    format!("{} via {}", channel, trigger)
}

fn format_claude_auth_detail(value: &serde_json::Value) -> String {
    let method = value
        .get("authMethod")
        .and_then(|item| item.as_str())
        .unwrap_or("unknown");
    let email = value.get("email").and_then(|item| item.as_str());
    let subscription = value.get("subscriptionType").and_then(|item| item.as_str());

    match (email, subscription) {
        (Some(email), Some(subscription)) => {
            format!("Logged in via {} as {} ({})", method, email, subscription)
        }
        (Some(email), None) => format!("Logged in via {} as {}", method, email),
        _ => format!("Logged in via {}", method),
    }
}

fn find_kv(content: &str, key: &str) -> Option<String> {
    content.lines().find_map(|line| {
        let (field, value) = line.split_once(':')?;
        if field.trim().eq_ignore_ascii_case(key) {
            let value = value.trim();
            if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            }
        } else {
            None
        }
    })
}

fn find_named_field(content: &str, field_name: &str) -> Option<String> {
    content.lines().find_map(|line| {
        let (field, value) = line.split_once(':')?;
        if field.trim().eq_ignore_ascii_case(field_name) {
            let value = value.trim();
            if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            }
        } else {
            None
        }
    })
}

fn keychain_ref_exists(reference: &str) -> bool {
    parse_secret_reference(reference)
        .and_then(|(service, account)| {
            crate::secrets::read_secret(&service, &account)
                .ok()
                .flatten()
        })
        .is_some()
}

fn parse_secret_reference(reference: &str) -> Option<(String, String)> {
    if let Some((service, account)) = reference.split_once('/') {
        return Some((service.to_string(), account.to_string()));
    }

    let mut parts = reference.split('.').collect::<Vec<_>>();
    if parts.len() >= 2 {
        let account = parts.pop()?.to_string();
        let service = parts.join(".");
        return Some((service, account));
    }

    None
}

async fn which(binary: &str) -> Option<String> {
    let mut command = Command::new("which");
    command.arg(binary);
    let output = match timeout(Duration::from_secs(2), command.output()).await {
        Ok(Ok(output)) if output.status.success() => output,
        _ => return None,
    };
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        None
    } else {
        Some(stdout)
    }
}

async fn command_text(
    program: &str,
    args: &[&str],
    cwd: Option<&Path>,
    timeout_after: Duration,
) -> Option<String> {
    let mut command = Command::new(program);
    command.args(args);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    command.env("NO_COLOR", "1");

    let output = match timeout(timeout_after, command.output()).await {
        Ok(Ok(output)) => output,
        _ => return None,
    };

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !stdout.is_empty() {
        Some(stdout)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            None
        } else {
            Some(stderr)
        }
    }
}

fn parse_codex_mcp_list(output: &str) -> Vec<McpServerDiagnostics> {
    output
        .lines()
        .skip_while(|line| !line.trim_start().starts_with("Name"))
        .skip(1)
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return None;
            }

            let columns = trimmed
                .split("  ")
                .filter(|part| !part.trim().is_empty())
                .map(str::trim)
                .collect::<Vec<_>>();

            if columns.len() < 4 {
                return None;
            }

            let (name, target, status, auth) = if columns.len() >= 5 {
                (
                    columns[0].to_string(),
                    columns[1].to_string(),
                    columns[3].to_string(),
                    Some(columns[4].to_string()),
                )
            } else {
                (
                    columns[0].to_string(),
                    columns[1].to_string(),
                    columns[2].to_string(),
                    Some(columns[3].to_string()),
                )
            };

            Some(McpServerDiagnostics {
                name,
                target,
                status: normalize_mcp_status(&status, auth.as_deref()),
                auth,
            })
        })
        .collect()
}

fn parse_claude_mcp_list(output: &str) -> Vec<McpServerDiagnostics> {
    output
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("Checking MCP server health") {
                return None;
            }

            let (left, right) = trimmed.rsplit_once(" - ")?;
            let (name, target) = left.split_once(": ")?;
            Some(McpServerDiagnostics {
                name: name.to_string(),
                target: target.to_string(),
                status: normalize_mcp_status(right, Some(right)),
                auth: Some(right.to_string()),
            })
        })
        .collect()
}

fn normalize_mcp_status(status: &str, auth: Option<&str>) -> String {
    let haystack = format!(
        "{} {}",
        status.to_lowercase(),
        auth.unwrap_or_default().to_lowercase()
    );

    if haystack.contains("connected") {
        "connected".to_string()
    } else if haystack.contains("needs authentication") || haystack.contains("not logged in") {
        "needs_auth".to_string()
    } else if haystack.contains("failed") {
        "failed".to_string()
    } else if haystack.contains("enabled") {
        "enabled".to_string()
    } else {
        "unknown".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_codex_mcp_output() {
        let output = "Name            Url                             Bearer Token Env Var  Status   Auth\ncloudflare-api  https://mcp.cloudflare.com/mcp  -                     enabled  Not logged in\n";
        let servers = parse_codex_mcp_list(output);
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "cloudflare-api");
        assert_eq!(servers[0].target, "https://mcp.cloudflare.com/mcp");
        assert_eq!(servers[0].status, "needs_auth");
    }

    #[test]
    fn parses_claude_mcp_output() {
        let output = "Checking MCP server health…\n\nclaude.ai Vercel: https://mcp.vercel.com - ✓ Connected\nplugin:playwright:playwright: npx @playwright/mcp@latest - ✓ Connected\n";
        let servers = parse_claude_mcp_list(output);
        assert_eq!(servers.len(), 2);
        assert_eq!(servers[0].name, "claude.ai Vercel");
        assert_eq!(servers[1].name, "plugin:playwright:playwright");
        assert_eq!(servers[1].status, "connected");
    }

    #[test]
    fn parses_secret_reference() {
        assert_eq!(
            parse_secret_reference("homard.openrouter.api_key"),
            Some(("homard.openrouter".to_string(), "api_key".to_string()))
        );
        assert_eq!(
            parse_secret_reference("homard-telegram/bot-token"),
            Some(("homard-telegram".to_string(), "bot-token".to_string()))
        );
    }
}
