pub mod sandbox;
pub mod prompt_guard;

use tokio::sync::RwLock;
use crate::types::PermissionLevel;

pub struct SecurityManager {
    level: RwLock<PermissionLevel>,
}

impl SecurityManager {
    pub fn new(level: PermissionLevel) -> Self {
        Self { level: RwLock::new(level) }
    }

    pub fn permission_level(&self) -> PermissionLevel {
        // Use try_read to avoid async in non-async context
        self.level.try_read().map(|l| l.clone()).unwrap_or(PermissionLevel::Supervised)
    }

    pub async fn set_permission_level(&self, level: PermissionLevel) {
        *self.level.write().await = level;
    }

    pub async fn check_tool(&self, tool_name: &str, arguments: &serde_json::Value) -> bool {
        let level = self.level.read().await;
        match *level {
            PermissionLevel::Locked => {
                matches!(tool_name, "memory_search" | "web_search" | "web_fetch")
            }
            PermissionLevel::Autonomous => {
                // Autonomous auto-approves everything EXCEPT categorically dangerous commands
                if tool_name == "shell_exec" && sandbox::is_blocked(arguments) {
                    tracing::warn!("Blocked dangerous command even in autonomous mode: {:?}", arguments);
                    return false;
                }
                true
            }
            PermissionLevel::Supervised => {
                // In supervised mode, we'd ideally prompt for dangerous operations.
                // For v1, auto-approve everything except shell_exec with dangerous patterns.
                // Full approval flow (Telegram inline keyboard) is a v2 feature.
                if tool_name == "shell_exec" {
                    if sandbox::is_blocked(arguments) {
                        return false;
                    }
                    // In supervised mode, confirm patterns need approval
                    // For v1, we auto-approve (full approval flow is v2)
                    // but we log them for audit
                    if sandbox::needs_confirmation(arguments) {
                        tracing::warn!("Shell command needs confirmation (auto-approving in v1): {:?}", arguments);
                    }
                    return true;
                }
                true
            }
        }
    }
}
