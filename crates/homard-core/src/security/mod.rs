pub mod prompt_guard;
pub mod sandbox;

use crate::types::{PermissionLevel, ToolPolicy};
use tokio::sync::RwLock;

pub enum ToolAuthorization {
    Allow,
    Deny(String),
}

pub struct SecurityManager {
    level: RwLock<PermissionLevel>,
}

impl SecurityManager {
    pub fn new(level: PermissionLevel) -> Self {
        Self {
            level: RwLock::new(level),
        }
    }

    pub fn permission_level(&self) -> PermissionLevel {
        // Use try_read to avoid async in non-async context
        self.level
            .try_read()
            .map(|l| l.clone())
            .unwrap_or(PermissionLevel::Supervised)
    }

    pub async fn set_permission_level(&self, level: PermissionLevel) {
        *self.level.write().await = level;
    }

    pub async fn check_tool(
        &self,
        tool_name: &str,
        policy: ToolPolicy,
        arguments: &serde_json::Value,
    ) -> ToolAuthorization {
        let level = self.level.read().await;
        match *level {
            PermissionLevel::Locked => {
                if policy == ToolPolicy::ReadOnly {
                    ToolAuthorization::Allow
                } else {
                    ToolAuthorization::Deny(format!(
                        "`{}` is unavailable in locked mode",
                        tool_name
                    ))
                }
            }
            PermissionLevel::Autonomous => {
                // Autonomous auto-approves everything EXCEPT categorically dangerous commands
                if policy == ToolPolicy::ShellCommand && sandbox::is_blocked(arguments) {
                    tracing::warn!("Blocked dangerous command even in autonomous mode: {:?}", arguments);
                    return ToolAuthorization::Deny(
                        "Dangerous shell command blocked by security policy".to_string(),
                    );
                }
                ToolAuthorization::Allow
            }
            PermissionLevel::Supervised => {
                match policy {
                    ToolPolicy::ShellCommand => {
                        if sandbox::is_blocked(arguments) {
                            return ToolAuthorization::Deny(
                                "Dangerous shell command blocked by security policy".to_string(),
                            );
                        }
                        if sandbox::needs_confirmation(arguments) {
                            tracing::warn!(
                                "Shell command requires approval but supervised mode has no approval flow yet: {:?}",
                                arguments
                            );
                            return ToolAuthorization::Deny(
                                "Shell command requires approval. Supervised mode currently blocks these commands until interactive approvals are implemented.".to_string(),
                            );
                        }
                        if arguments.get("command").and_then(|c| c.as_str()).is_none() {
                            return ToolAuthorization::Deny(
                                "Shell-backed tools require interactive approval. Switch to autonomous mode to run them.".to_string(),
                            );
                        }
                        ToolAuthorization::Allow
                    }
                    ToolPolicy::DelegatedSession | ToolPolicy::ProcessControl => ToolAuthorization::Deny(
                        format!(
                            "`{}` requires interactive approval. Supervised mode blocks delegated agents and process control until the approval flow exists.",
                            tool_name
                        ),
                    ),
                    ToolPolicy::ReadOnly | ToolPolicy::StatefulWrite => ToolAuthorization::Allow,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{SecurityManager, ToolAuthorization};
    use crate::types::{PermissionLevel, ToolPolicy};

    fn shell_args(command: &str) -> serde_json::Value {
        serde_json::json!({ "command": command })
    }

    #[tokio::test]
    async fn supervised_denies_confirmation_required_shell_commands() {
        let security = SecurityManager::new(PermissionLevel::Supervised);
        let decision = security
            .check_tool(
                "shell_exec",
                ToolPolicy::ShellCommand,
                &shell_args("git push origin main"),
            )
            .await;

        assert!(matches!(decision, ToolAuthorization::Deny(_)));
    }

    #[tokio::test]
    async fn supervised_denies_delegated_sessions() {
        let security = SecurityManager::new(PermissionLevel::Supervised);
        let decision = security
            .check_tool(
                "spawn_session",
                ToolPolicy::DelegatedSession,
                &serde_json::json!({ "cli": "codex" }),
            )
            .await;

        assert!(matches!(decision, ToolAuthorization::Deny(_)));
    }

    #[tokio::test]
    async fn autonomous_allows_delegated_sessions() {
        let security = SecurityManager::new(PermissionLevel::Autonomous);
        let decision = security
            .check_tool(
                "spawn_session",
                ToolPolicy::DelegatedSession,
                &serde_json::json!({ "cli": "codex" }),
            )
            .await;

        assert!(matches!(decision, ToolAuthorization::Allow));
    }

    #[tokio::test]
    async fn locked_allows_only_read_only_tools() {
        let security = SecurityManager::new(PermissionLevel::Locked);
        let read_only = security
            .check_tool(
                "web_search",
                ToolPolicy::ReadOnly,
                &serde_json::json!({ "query": "weather" }),
            )
            .await;
        let write = security
            .check_tool(
                "memory_save",
                ToolPolicy::StatefulWrite,
                &serde_json::json!({ "text": "hi" }),
            )
            .await;

        assert!(matches!(read_only, ToolAuthorization::Allow));
        assert!(matches!(write, ToolAuthorization::Deny(_)));
    }
}
