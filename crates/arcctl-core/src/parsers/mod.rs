pub mod claude;
pub mod gemini;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentNode {
    pub id: String,
    pub name: String,
    pub agent_type: Option<String>,
    pub status: AgentStatus,
    pub current_activity: Option<String>,
    pub files_touched: Vec<String>,
    pub children: Vec<AgentNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Working,
    Waiting,
    Done,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTree {
    pub session_id: String,
    pub root: AgentNode,
}

pub trait SessionParser: Send + Sync {
    fn parse(&self, content: &str, session_id: &str) -> Option<SessionTree>;
}
