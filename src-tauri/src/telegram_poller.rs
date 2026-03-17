use arcctl_core::config::ArcctlDirs;
use tokio_util::sync::CancellationToken;

/// Run the Telegram update polling loop.
/// Polls for inbound messages and handles commands.
/// This is a stub — full implementation in Task 5.
pub async fn run_poller(
    _dirs: ArcctlDirs,
    _app_handle: tauri::AppHandle,
    _cancel: CancellationToken,
) -> Result<(), String> {
    Ok(())
}
