use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::parsers::{SessionParser, SessionTree};
use crate::parsers::claude::ClaudeParser;
use crate::parsers::gemini::GeminiParser;
use crate::provider::ProviderId;

/// Monitors session files and maintains parsed agent trees.
pub struct SessionMonitor {
    trees: Arc<Mutex<HashMap<String, SessionTree>>>,
    watchers: Arc<Mutex<HashMap<String, RecommendedWatcher>>>,
}

impl SessionMonitor {
    pub fn new() -> Self {
        Self {
            trees: Arc::new(Mutex::new(HashMap::new())),
            watchers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Start monitoring a session file.
    pub fn start_monitoring(
        &self,
        session_id: &str,
        file_path: PathBuf,
        provider: &ProviderId,
    ) -> Result<(), String> {
        let trees = self.trees.clone();
        let sid = session_id.to_string();
        let path = file_path.clone();
        let provider = provider.clone();

        // Initial parse
        self.parse_and_update(&sid, &path, &provider);

        // Set up file watcher
        let trees_watcher = trees.clone();
        let sid_watcher = sid.clone();
        let path_watcher = path.clone();
        let provider_watcher = provider.clone();

        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                    if let Ok(content) = std::fs::read_to_string(&path_watcher) {
                        let parser = get_parser(&provider_watcher);
                        if let Some(tree) = parser.parse(&content, &sid_watcher) {
                            if let Ok(mut map) = trees_watcher.lock() {
                                map.insert(sid_watcher.clone(), tree);
                            }
                        }
                    }
                }
            }
        })
        .map_err(|e| e.to_string())?;

        // Watch the parent directory (file may not exist yet)
        let watch_path = file_path.parent().unwrap_or(Path::new("."));
        watcher
            .watch(watch_path, RecursiveMode::NonRecursive)
            .map_err(|e| e.to_string())?;

        if let Ok(mut w) = self.watchers.lock() {
            w.insert(sid.clone(), watcher);
        }

        Ok(())
    }

    /// Stop monitoring a session.
    pub fn stop_monitoring(&self, session_id: &str) {
        if let Ok(mut w) = self.watchers.lock() {
            w.remove(session_id);
        }
        if let Ok(mut t) = self.trees.lock() {
            t.remove(session_id);
        }
    }

    /// Get the current agent tree for a session.
    pub fn get_tree(&self, session_id: &str) -> Option<SessionTree> {
        self.trees.lock().ok()?.get(session_id).cloned()
    }

    fn parse_and_update(&self, session_id: &str, file_path: &Path, provider: &ProviderId) {
        if let Ok(content) = std::fs::read_to_string(file_path) {
            let parser = get_parser(provider);
            if let Some(tree) = parser.parse(&content, session_id) {
                if let Ok(mut map) = self.trees.lock() {
                    map.insert(session_id.to_string(), tree);
                }
            }
        }
    }
}

impl Default for SessionMonitor {
    fn default() -> Self {
        Self::new()
    }
}

fn get_parser(provider: &ProviderId) -> Box<dyn SessionParser> {
    match provider {
        ProviderId::Claude => Box::new(ClaudeParser),
        ProviderId::Gemini => Box::new(GeminiParser),
    }
}
