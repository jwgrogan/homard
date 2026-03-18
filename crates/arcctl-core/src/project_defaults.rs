use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectDefaults {
    #[serde(flatten)]
    pub mappings: HashMap<String, String>,
}

impl ProjectDefaults {
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(contents) => {
                if contents.trim().is_empty() {
                    return Ok(Self::default());
                }
                Ok(serde_json::from_str(&contents)?)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&self.mappings)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn get_profile(&self, directory: &str) -> Option<&str> {
        self.mappings.get(directory).map(|s| s.as_str())
    }

    pub fn set_profile(&mut self, directory: String, profile: String) {
        self.mappings.insert(directory, profile);
    }

    pub fn remove(&mut self, directory: &str) {
        self.mappings.remove(directory);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_round_trip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("project-defaults.json");

        let mut defaults = ProjectDefaults::default();
        defaults.set_profile("/Users/test/repo".to_string(), "Work Claude".to_string());
        defaults.save(&path).unwrap();

        let loaded = ProjectDefaults::load(&path).unwrap();
        assert_eq!(loaded.get_profile("/Users/test/repo"), Some("Work Claude"));
    }

    #[test]
    fn test_missing_file_returns_empty() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nonexistent.json");
        let defaults = ProjectDefaults::load(&path).unwrap();
        assert!(defaults.mappings.is_empty());
    }

    #[test]
    fn test_set_and_remove() {
        let mut defaults = ProjectDefaults::default();
        defaults.set_profile("/foo".to_string(), "bar".to_string());
        assert_eq!(defaults.get_profile("/foo"), Some("bar"));
        defaults.remove("/foo");
        assert!(defaults.get_profile("/foo").is_none());
    }
}
