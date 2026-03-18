use arcctl_core::config::{ArcctlConfig, ArcctlDirs};
use arcctl_core::session_monitor::SessionMonitor;
use arcctl_core::store::Store;
use arcctl_core::terminal::TerminalApp;
use std::sync::Mutex;

pub struct AppState {
    pub store: Mutex<Store>,
    pub config: Mutex<ArcctlConfig>,
    pub dirs: ArcctlDirs,
    pub preferred_terminal: Mutex<Option<TerminalApp>>,
    /// Handle to the background Telegram polling task.
    pub telegram_poll_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// Cancellation token for stopping the Telegram poller.
    pub telegram_cancel: Mutex<Option<tokio_util::sync::CancellationToken>>,
    pub session_monitor: SessionMonitor,
}
