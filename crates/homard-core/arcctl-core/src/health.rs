use chrono::Utc;
use regex_lite::Regex;
use tokio::process::Command;

use crate::types::HealthStatus;

pub async fn check_claude_cli() -> Option<String> {
    let output = Command::new("claude").arg("--version").output().await.ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);
    parse_version_output(&combined)
}

pub fn parse_version_output(output: &str) -> Option<String> {
    let re = Regex::new(r"v?(\d+\.\d+\.\d+)").ok()?;
    let caps = re.captures(output)?;
    Some(caps[1].to_string())
}

pub async fn run_health_check() -> HealthStatus {
    let version = check_claude_cli().await;
    let arcctl_dir = dirs::home_dir()
        .map(|h| h.join(".arcctl").exists())
        .unwrap_or(false);

    HealthStatus {
        claude_cli_installed: version.is_some(),
        claude_cli_version: version,
        active_profile: None,
        telegram_connected: false,
        email_configured: false,
        arcctl_dir_exists: arcctl_dir,
        checked_at: Utc::now(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_claude_version() {
        assert_eq!(
            parse_version_output("claude v1.2.3\n"),
            Some("1.2.3".to_string())
        );
        assert_eq!(
            parse_version_output("Claude Code v2.0.1\n"),
            Some("2.0.1".to_string())
        );
        assert_eq!(parse_version_output("garbage"), None);
    }
}
