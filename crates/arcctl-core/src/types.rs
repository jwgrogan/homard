use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::provider::ProviderId;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CredentialHealth {
    Valid,
    Expiring,
    Expired,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Running,
    Complete,
    Error,
    Killed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    Manual,
    Cron,
    Telegram,
    Email,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SessionMode {
    Fresh,
    Persistent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    pub id: String,
    pub schedule_id: Option<String>,
    pub agent: Option<String>,
    pub profile: Option<String>,
    pub directory: Option<String>,
    pub trigger: Trigger,
    pub status: RunStatus,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<i64>,
    pub error_message: Option<String>,
    pub delivery_status: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub provider: ProviderId,
    pub email: Option<String>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub claude_cli_installed: bool,
    pub claude_cli_version: Option<String>,
    pub active_profile: Option<Profile>,
    pub telegram_connected: bool,
    pub email_configured: bool,
    pub arcctl_dir_exists: bool,
    pub checked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schedule {
    pub id: String,
    pub name: String,
    pub schedule: String,
    pub timezone: Option<String>,
    pub agent: Option<String>,
    pub prompt: Option<String>,
    pub directory: String,
    pub profile: Option<String>,
    pub timeout_minutes: Option<u32>,
    pub session_mode: SessionMode,
    pub last_session_id: Option<String>,
    pub delivery: DeliveryConfig,
    pub retry: RetryConfig,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryConfig {
    pub channels: Vec<String>,
    #[serde(rename = "on")]
    pub on_events: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub backoff_seconds: Vec<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Running,
    Stopped,
    Error,
    Killed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub cli_session_id: Option<String>,
    pub profile_name: Option<String>,
    pub provider: String,
    pub directory: Option<String>,
    pub terminal_pid: Option<u32>,
    pub trigger: Trigger,
    pub status: SessionStatus,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<i64>,
    pub error_message: Option<String>,
    pub agent: Option<String>,
    pub parent_session_id: Option<String>,
    pub forked_from: Option<String>,
}
