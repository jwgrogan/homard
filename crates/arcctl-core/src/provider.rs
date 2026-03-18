use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderId {
    Claude,
    Gemini,
}

impl ProviderId {
    pub fn cli_command(&self) -> &'static str {
        match self {
            ProviderId::Claude => "claude",
            ProviderId::Gemini => "gemini",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            ProviderId::Claude => "Claude Code",
            ProviderId::Gemini => "Gemini CLI",
        }
    }

    pub fn supports_session_id_flag(&self) -> bool {
        matches!(self, ProviderId::Claude)
    }

    pub fn supports_resume(&self) -> bool {
        true // both support --resume
    }

    pub fn resume_flag(&self) -> &'static str {
        "--resume"
    }

    pub fn session_dir(&self) -> Option<PathBuf> {
        let home = dirs::home_dir()?;
        match self {
            ProviderId::Claude => Some(home.join(".claude").join("projects")),
            ProviderId::Gemini => Some(home.join(".gemini").join("tmp")),
        }
    }
}

impl FromStr for ProviderId {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "claude" => Ok(ProviderId::Claude),
            "gemini" => Ok(ProviderId::Gemini),
            _ => Err(format!("Unknown provider: {}", s)),
        }
    }
}

impl std::fmt::Display for ProviderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_cli_commands() {
        assert_eq!(ProviderId::Claude.cli_command(), "claude");
        assert_eq!(ProviderId::Gemini.cli_command(), "gemini");
    }

    #[test]
    fn test_provider_serialization() {
        let json = serde_json::to_string(&ProviderId::Claude).unwrap();
        assert_eq!(json, r#""claude""#);
        let gemini: ProviderId = serde_json::from_str(r#""gemini""#).unwrap();
        assert_eq!(gemini, ProviderId::Gemini);
    }

    #[test]
    fn test_from_str() {
        assert_eq!(ProviderId::from_str("claude").unwrap(), ProviderId::Claude);
        assert_eq!(ProviderId::from_str("gemini").unwrap(), ProviderId::Gemini);
        assert!(ProviderId::from_str("unknown").is_err());
    }

    #[test]
    fn test_session_dir() {
        assert!(ProviderId::Claude.session_dir().is_some());
        assert!(ProviderId::Gemini.session_dir().is_some());
    }
}
