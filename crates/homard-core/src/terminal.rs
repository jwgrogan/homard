use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::error::{HomardError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TerminalApp {
    Iterm,
    Ghostty,
    Warp,
    Kitty,
    AppleTerminal,
}

impl TerminalApp {
    /// Detect installed terminals in preference order.
    pub fn detect_installed() -> Vec<TerminalApp> {
        let mut found = Vec::new();
        if Path::new("/Applications/iTerm.app").exists() {
            found.push(TerminalApp::Iterm);
        }
        if Path::new("/Applications/Ghostty.app").exists() {
            found.push(TerminalApp::Ghostty);
        }
        if Path::new("/Applications/Warp.app").exists() {
            found.push(TerminalApp::Warp);
        }
        if Path::new("/Applications/kitty.app").exists() {
            found.push(TerminalApp::Kitty);
        }
        // Terminal.app is always available on macOS
        found.push(TerminalApp::AppleTerminal);
        found
    }

    /// Launch a shell command in a new terminal window.
    /// Returns the PID of the launched process (best effort -- AppleScript launches may return None).
    pub fn launch(&self, shell_command: &str) -> Result<Option<u32>> {
        let escaped = shell_command.replace('\\', "\\\\").replace('"', "\\\"");
        match self {
            TerminalApp::AppleTerminal => {
                let script = format!(
                    r#"tell application "Terminal"
                        activate
                        do script "{}"
                    end tell"#,
                    escaped
                );
                run_osascript(&script)?;
                Ok(None)
            }
            TerminalApp::Iterm => {
                let script = format!(
                    r#"tell application "iTerm"
                        activate
                        set newWindow to (create window with default profile command "{}")
                    end tell"#,
                    escaped
                );
                run_osascript(&script)?;
                Ok(None)
            }
            TerminalApp::Ghostty => {
                let child = Command::new("ghostty")
                    .arg("-e")
                    .arg(shell_command)
                    .spawn()
                    .map_err(HomardError::Io)?;
                Ok(Some(child.id()))
            }
            TerminalApp::Kitty => {
                let child = Command::new("kitty")
                    .arg("sh")
                    .arg("-c")
                    .arg(shell_command)
                    .spawn()
                    .map_err(HomardError::Io)?;
                Ok(Some(child.id()))
            }
            TerminalApp::Warp => {
                // Warp doesn't have great programmatic launch support yet
                let child = Command::new("open")
                    .args(["-a", "Warp"])
                    .spawn()
                    .map_err(HomardError::Io)?;
                Ok(Some(child.id()))
            }
        }
    }
}

fn run_osascript(script: &str) -> Result<()> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(HomardError::Io)?;
    if !output.status.success() {
        return Err(HomardError::Terminal(
            String::from_utf8_lossy(&output.stderr).trim().to_string()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_installed_always_includes_apple_terminal() {
        let found = TerminalApp::detect_installed();
        assert!(!found.is_empty());
        assert_eq!(*found.last().unwrap(), TerminalApp::AppleTerminal);
    }

    #[test]
    fn test_serialization() {
        let t = TerminalApp::Iterm;
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(json, r#""iterm""#);

        let at: TerminalApp = serde_json::from_str(r#""apple_terminal""#).unwrap();
        assert_eq!(at, TerminalApp::AppleTerminal);
    }
}
