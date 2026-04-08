use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::types::Schedule;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredPlist {
    pub label: String,
    pub path: PathBuf,
    pub program_args: Vec<String>,
    pub hour: Option<u32>,
    pub minute: Option<u32>,
}

pub fn generate_plist(schedule: &Schedule, arcctl_bin: &str) -> String {
    let label = format!("com.arcctl.job.{}", schedule.id);
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    let log_dir = home.join(".arcctl").join("logs");
    let stdout_log = log_dir.join(format!("{}.stdout.log", schedule.id));
    let stderr_log = log_dir.join(format!("{}.stderr.log", schedule.id));

    let (hour, minute) = parse_simple_cron(&schedule.schedule).unwrap_or((None, None));

    let calendar_interval = match (hour, minute) {
        (Some(h), Some(m)) => format!(
            "\t<key>StartCalendarInterval</key>\n\t<dict>\n\t\t<key>Hour</key>\n\t\t<integer>{}</integer>\n\t\t<key>Minute</key>\n\t\t<integer>{}</integer>\n\t</dict>",
            h, m
        ),
        (Some(h), None) => format!(
            "\t<key>StartCalendarInterval</key>\n\t<dict>\n\t\t<key>Hour</key>\n\t\t<integer>{}</integer>\n\t</dict>",
            h
        ),
        (None, Some(m)) => format!(
            "\t<key>StartCalendarInterval</key>\n\t<dict>\n\t\t<key>Minute</key>\n\t\t<integer>{}</integer>\n\t</dict>",
            m
        ),
        (None, None) => String::new(),
    };

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Label</key>
	<string>{label}</string>
	<key>ProgramArguments</key>
	<array>
		<string>{arcctl_bin}</string>
		<string>run-job</string>
		<string>{id}</string>
	</array>
{calendar_interval}
	<key>StandardOutPath</key>
	<string>{stdout}</string>
	<key>StandardErrorPath</key>
	<string>{stderr}</string>
</dict>
</plist>
"#,
        label = label,
        arcctl_bin = arcctl_bin,
        id = schedule.id,
        calendar_interval = calendar_interval,
        stdout = stdout_log.display(),
        stderr = stderr_log.display(),
    )
}

pub fn install_plist(schedule: &Schedule, arcctl_bin: &str) -> crate::Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| {
        crate::ArcctlError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "home directory not found",
        ))
    })?;
    let agents_dir = home.join("Library").join("LaunchAgents");
    std::fs::create_dir_all(&agents_dir).map_err(crate::ArcctlError::Io)?;

    let label = format!("com.arcctl.job.{}", schedule.id);
    let plist_path = agents_dir.join(format!("{}.plist", label));
    let content = generate_plist(schedule, arcctl_bin);
    std::fs::write(&plist_path, &content).map_err(crate::ArcctlError::Io)?;

    let _ = std::process::Command::new("launchctl")
        .arg("load")
        .arg(&plist_path)
        .output();

    Ok(plist_path)
}

pub fn uninstall_plist(schedule_id: &str) -> crate::Result<()> {
    let home = dirs::home_dir().ok_or_else(|| {
        crate::ArcctlError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "home directory not found",
        ))
    })?;
    let label = format!("com.arcctl.job.{}", schedule_id);
    let plist_path = home
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{}.plist", label));

    if plist_path.exists() {
        let _ = std::process::Command::new("launchctl")
            .arg("unload")
            .arg(&plist_path)
            .output();
        std::fs::remove_file(&plist_path).map_err(crate::ArcctlError::Io)?;
    }

    Ok(())
}

pub fn scan_claude_plists(dir: &Path) -> crate::Result<Vec<DiscoveredPlist>> {
    let mut results = Vec::new();

    let entries = std::fs::read_dir(dir).map_err(crate::ArcctlError::Io)?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("plist") {
            continue;
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        if !content.to_lowercase().contains("claude") {
            continue;
        }

        let dict: plist::Dictionary =
            match plist::from_reader_xml(std::io::Cursor::new(content.as_bytes())) {
                Ok(plist::Value::Dictionary(d)) => d,
                _ => continue,
            };

        let label = match dict.get("Label") {
            Some(plist::Value::String(s)) => s.clone(),
            _ => continue,
        };

        let program_args: Vec<String> = match dict.get("ProgramArguments") {
            Some(plist::Value::Array(arr)) => arr
                .iter()
                .filter_map(|v| {
                    if let plist::Value::String(s) = v {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
                .collect(),
            _ => vec![],
        };

        let (hour, minute) = match dict.get("StartCalendarInterval") {
            Some(plist::Value::Dictionary(ci)) => {
                let h = if let Some(plist::Value::Integer(v)) = ci.get("Hour") {
                    v.as_unsigned().map(|n| n as u32)
                } else {
                    None
                };
                let m = if let Some(plist::Value::Integer(v)) = ci.get("Minute") {
                    v.as_unsigned().map(|n| n as u32)
                } else {
                    None
                };
                (h, m)
            }
            _ => (None, None),
        };

        results.push(DiscoveredPlist {
            label,
            path,
            program_args,
            hour,
            minute,
        });
    }

    Ok(results)
}

pub fn parse_simple_cron(cron: &str) -> anyhow::Result<(Option<u32>, Option<u32>)> {
    let parts: Vec<&str> = cron.split_whitespace().collect();
    if parts.len() < 2 {
        anyhow::bail!("invalid cron expression: {}", cron);
    }

    let minute = if parts[0] == "*" {
        None
    } else {
        Some(
            parts[0]
                .parse::<u32>()
                .with_context(|| format!("invalid minute: {}", parts[0]))?,
        )
    };

    let hour = if parts[1] == "*" {
        None
    } else {
        Some(
            parts[1]
                .parse::<u32>()
                .with_context(|| format!("invalid hour: {}", parts[1]))?,
        )
    };

    Ok((hour, minute))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{DeliveryConfig, RetryConfig, SessionMode};
    use tempfile::TempDir;

    fn make_schedule(id: &str, cron: &str) -> Schedule {
        Schedule {
            id: id.to_string(),
            name: "Test Schedule".to_string(),
            schedule: cron.to_string(),
            timezone: None,
            agent: None,
            prompt: None,
            directory: "/tmp".to_string(),
            profile: None,
            timeout_minutes: None,
            session_mode: SessionMode::Fresh,
            last_session_id: None,
            delivery: DeliveryConfig {
                channels: vec![],
                on_events: vec![],
            },
            retry: RetryConfig {
                max_attempts: 3,
                backoff_seconds: vec![60, 120, 300],
            },
            enabled: true,
        }
    }

    #[test]
    fn test_generate_plist() {
        let schedule = make_schedule("job-123", "0 7 * * *");
        let plist = generate_plist(&schedule, "/usr/local/bin/arcctl");

        assert!(plist.contains("com.arcctl.job.job-123"));
        assert!(plist.contains("run-job"));
        assert!(plist.contains("<integer>7</integer>"));
    }

    #[test]
    fn test_scan_claude_plists() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();

        // Write a plist that mentions "claude"
        let claude_plist = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Label</key>
	<string>com.claude.test</string>
	<key>ProgramArguments</key>
	<array>
		<string>/usr/bin/claude</string>
		<string>run-job</string>
		<string>abc</string>
	</array>
	<key>StartCalendarInterval</key>
	<dict>
		<key>Hour</key>
		<integer>8</integer>
		<key>Minute</key>
		<integer>30</integer>
	</dict>
</dict>
</plist>
"#;

        // Write a plist that does NOT mention "claude"
        let other_plist = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Label</key>
	<string>com.other.service</string>
	<key>ProgramArguments</key>
	<array>
		<string>/usr/bin/other</string>
	</array>
</dict>
</plist>
"#;

        std::fs::write(dir.join("com.claude.test.plist"), claude_plist).unwrap();
        std::fs::write(dir.join("com.other.service.plist"), other_plist).unwrap();

        let found = scan_claude_plists(dir).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].label, "com.claude.test");
        assert_eq!(found[0].hour, Some(8));
        assert_eq!(found[0].minute, Some(30));
    }

    #[test]
    fn test_parse_cron_to_calendar_interval() {
        let (hour, minute) = parse_simple_cron("0 7 * * *").unwrap();
        assert_eq!(hour, Some(7));
        assert_eq!(minute, Some(0));
    }
}
