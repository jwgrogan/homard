use crate::types::DaemonStatus;
use crate::types::PermissionLevel;

/// Check if the homard daemon is running by probing the local API.
pub async fn check_daemon(port: u16) -> DaemonStatus {
    let url = format!("http://127.0.0.1:{}/status", port);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .unwrap_or_default();

    match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => match resp.json::<DaemonStatus>().await {
            Ok(status) => status,
            Err(_) => DaemonStatus {
                running: true,
                uptime_secs: None,
                active_provider: None,
                active_model: None,
                permission_level: PermissionLevel::Supervised,
                telegram_connected: false,
                current_run: None,
            },
        },
        _ => DaemonStatus {
            running: false,
            uptime_secs: None,
            active_provider: None,
            active_model: None,
            permission_level: PermissionLevel::Supervised,
            telegram_connected: false,
            current_run: None,
        },
    }
}
