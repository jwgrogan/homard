use crate::types::{PermissionLevel, ToolCall};
use std::time::Duration;

pub enum HangAction {
    Pause(String), // Supervised: pause and ask
    Alert(String), // Autonomous: alert but continue
}

pub struct HangDetector {
    recent_tool_calls: Vec<String>,
    alerted: bool,
}

impl HangDetector {
    pub fn new() -> Self {
        Self {
            recent_tool_calls: Vec::new(),
            alerted: false,
        }
    }

    pub fn check(
        &mut self,
        iterations: u32,
        elapsed: Duration,
        permission_level: &PermissionLevel,
    ) -> Option<HangAction> {
        if self.alerted {
            return None; // Only alert once
        }

        let hanging = iterations >= 10 || elapsed >= Duration::from_secs(300);
        if !hanging {
            return None;
        }

        // Check if we're making progress (not just repeating)
        if self.is_making_progress() {
            return None;
        }

        self.alerted = true;

        match permission_level {
            PermissionLevel::Supervised => Some(HangAction::Pause(
                "I've been working on this for a while without making progress. Should I continue?"
                    .to_string(),
            )),
            PermissionLevel::Autonomous => Some(HangAction::Alert(
                "Heads up -- this run has been looping for a while. /stop to end run.".to_string(),
            )),
            PermissionLevel::Locked => None,
        }
    }

    pub fn record_tool_calls(&mut self, calls: &[ToolCall]) {
        for tc in calls {
            let sig = format!("{}:{}", tc.name, tc.arguments);
            self.recent_tool_calls.push(sig);
        }
        // Keep last 20
        if self.recent_tool_calls.len() > 20 {
            self.recent_tool_calls
                .drain(..self.recent_tool_calls.len() - 20);
        }
    }

    fn is_making_progress(&self) -> bool {
        if self.recent_tool_calls.len() < 6 {
            return true; // Not enough data
        }
        // Check if last 6 calls are all the same
        let last_6 = &self.recent_tool_calls[self.recent_tool_calls.len() - 6..];
        let unique: std::collections::HashSet<&String> = last_6.iter().collect();
        unique.len() > 2 // If more than 2 unique calls in last 6, we're making progress
    }
}
