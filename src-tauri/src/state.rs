use arcctl_core::config::{ArcctlConfig, ArcctlDirs};
use arcctl_core::process::ProcessRegistry;
use arcctl_core::store::Store;
use std::collections::HashMap;
use std::sync::Mutex;

pub struct AppState {
    pub store: Mutex<Store>,
    pub registry: ProcessRegistry,
    pub config: Mutex<ArcctlConfig>,
    pub dirs: ArcctlDirs,
    pub children: Mutex<HashMap<String, tokio::process::Child>>,
    /// Handle to the background Telegram polling task.
    pub telegram_poll_handle: std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// Cancellation token for stopping the Telegram poller.
    pub telegram_cancel: std::sync::Mutex<Option<tokio_util::sync::CancellationToken>>,
}
