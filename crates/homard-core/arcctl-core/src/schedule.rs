use std::path::PathBuf;

use crate::config::ArcctlDirs;
use crate::error::{ArcctlError, Result};
use crate::launchd::{install_plist, scan_claude_plists, uninstall_plist, DiscoveredPlist};
use crate::types::{DeliveryConfig, RetryConfig, Schedule, SessionMode};

// ---------------------------------------------------------------------------
// Binary resolution
// ---------------------------------------------------------------------------

/// Find the arcctl binary path. Checks:
/// 1. Current executable path
/// 2. /usr/local/bin/arcctl
/// 3. ~/.cargo/bin/arcctl
pub fn resolve_arcctl_bin() -> Result<String> {
    // Try current executable first
    if let Ok(exe) = std::env::current_exe() {
        if exe.exists() {
            return Ok(exe.to_string_lossy().to_string());
        }
    }

    // Try /usr/local/bin/arcctl
    let usr_local = std::path::Path::new("/usr/local/bin/arcctl");
    if usr_local.exists() {
        return Ok(usr_local.to_string_lossy().to_string());
    }

    // Try ~/.cargo/bin/arcctl
    if let Some(home) = dirs::home_dir() {
        let cargo_bin = home.join(".cargo").join("bin").join("arcctl");
        if cargo_bin.exists() {
            return Ok(cargo_bin.to_string_lossy().to_string());
        }
    }

    Err(ArcctlError::NotFound("arcctl binary not found".to_string()))
}

// ---------------------------------------------------------------------------
// File-based CRUD
// ---------------------------------------------------------------------------

/// Write a schedule as JSON to `schedules_dir/<id>.json`.
pub fn save_schedule(dirs: &ArcctlDirs, schedule: &Schedule) -> Result<PathBuf> {
    let path = dirs.schedules_dir().join(format!("{}.json", schedule.id));
    let json = serde_json::to_string_pretty(schedule).map_err(ArcctlError::Json)?;
    std::fs::write(&path, json).map_err(ArcctlError::Io)?;
    Ok(path)
}

/// Read a schedule from `schedules_dir/<id>.json`.
pub fn load_schedule(dirs: &ArcctlDirs, id: &str) -> Result<Schedule> {
    let path = dirs.schedules_dir().join(format!("{}.json", id));
    let contents = std::fs::read_to_string(&path).map_err(ArcctlError::Io)?;
    let schedule: Schedule = serde_json::from_str(&contents).map_err(ArcctlError::Json)?;
    Ok(schedule)
}

/// List all schedules in `schedules_dir`, sorted by name.
pub fn list_schedules(dirs: &ArcctlDirs) -> Result<Vec<Schedule>> {
    let dir = dirs.schedules_dir();
    if !dir.exists() {
        return Ok(vec![]);
    }

    let entries = std::fs::read_dir(&dir).map_err(ArcctlError::Io)?;
    let mut schedules = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let contents = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        if let Ok(s) = serde_json::from_str::<Schedule>(&contents) {
            schedules.push(s);
        }
    }

    schedules.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(schedules)
}

/// Remove `schedules_dir/<id>.json`.
pub fn delete_schedule_file(dirs: &ArcctlDirs, id: &str) -> Result<()> {
    let path = dirs.schedules_dir().join(format!("{}.json", id));
    if path.exists() {
        std::fs::remove_file(&path).map_err(ArcctlError::Io)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Higher-level operations (file + launchd)
// ---------------------------------------------------------------------------

/// Save JSON + install launchd plist.
pub fn create_schedule(dirs: &ArcctlDirs, schedule: &Schedule) -> Result<()> {
    save_schedule(dirs, schedule)?;
    let bin = resolve_arcctl_bin().unwrap_or_else(|_| "arcctl".to_string());
    install_plist(schedule, &bin)?;
    Ok(())
}

/// Uninstall old plist, save JSON, install new plist.
pub fn update_schedule(dirs: &ArcctlDirs, schedule: &Schedule) -> Result<()> {
    uninstall_plist(&schedule.id)?;
    save_schedule(dirs, schedule)?;
    let bin = resolve_arcctl_bin().unwrap_or_else(|_| "arcctl".to_string());
    install_plist(schedule, &bin)?;
    Ok(())
}

/// Uninstall plist + delete file.
pub fn delete_schedule(dirs: &ArcctlDirs, id: &str) -> Result<()> {
    uninstall_plist(id)?;
    delete_schedule_file(dirs, id)?;
    Ok(())
}

/// Set enabled=false and `launchctl unload` the plist.
pub fn pause_schedule(dirs: &ArcctlDirs, id: &str) -> Result<()> {
    let mut schedule = load_schedule(dirs, id)?;
    schedule.enabled = false;
    save_schedule(dirs, &schedule)?;

    let home = dirs::home_dir().ok_or_else(|| {
        ArcctlError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "home directory not found",
        ))
    })?;
    let label = format!("com.arcctl.job.{}", id);
    let plist_path = home
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{}.plist", label));

    if plist_path.exists() {
        let _ = std::process::Command::new("launchctl")
            .arg("unload")
            .arg(&plist_path)
            .output();
    }

    Ok(())
}

/// Set enabled=true and `launchctl load` the plist.
pub fn resume_schedule(dirs: &ArcctlDirs, id: &str) -> Result<()> {
    let mut schedule = load_schedule(dirs, id)?;
    schedule.enabled = true;
    save_schedule(dirs, &schedule)?;

    let home = dirs::home_dir().ok_or_else(|| {
        ArcctlError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "home directory not found",
        ))
    })?;
    let label = format!("com.arcctl.job.{}", id);
    let plist_path = home
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{}.plist", label));

    if plist_path.exists() {
        let _ = std::process::Command::new("launchctl")
            .arg("load")
            .arg(&plist_path)
            .output();
    }

    Ok(())
}

/// Update the `last_session_id` field for a schedule.
pub fn update_last_session_id(dirs: &ArcctlDirs, schedule_id: &str, session_id: &str) -> Result<()> {
    let mut schedule = load_schedule(dirs, schedule_id)?;
    schedule.last_session_id = Some(session_id.to_string());
    save_schedule(dirs, &schedule)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Launchd import
// ---------------------------------------------------------------------------

/// Convert a discovered launchd plist to an arcctl Schedule.
/// Backs up the original plist, then creates the new schedule.
pub fn import_launchd_job(dirs: &ArcctlDirs, discovered: &DiscoveredPlist) -> Result<Schedule> {
    // Build a cron string from hour/minute
    let cron = match (discovered.hour, discovered.minute) {
        (Some(h), Some(m)) => format!("{} {} * * *", m, h),
        (Some(h), None) => format!("0 {} * * *", h),
        (None, Some(m)) => format!("{} * * * *", m),
        (None, None) => "0 * * * *".to_string(),
    };

    // Extract prompt and directory from program_args if possible
    let prompt = extract_arg_value(&discovered.program_args, "-p")
        .or_else(|| extract_arg_value(&discovered.program_args, "--prompt"));

    let directory = extract_arg_value(&discovered.program_args, "--cwd")
        .unwrap_or_else(|| "/tmp".to_string());

    let id = uuid::Uuid::new_v4().to_string();
    let name = discovered.label.clone();

    let schedule = Schedule {
        id: id.clone(),
        name,
        schedule: cron,
        timezone: None,
        agent: None,
        prompt,
        directory,
        profile: None,
        timeout_minutes: None,
        session_mode: SessionMode::Fresh,
        last_session_id: None,
        delivery: DeliveryConfig {
            channels: vec!["notification".to_string()],
            on_events: vec!["complete".to_string(), "error".to_string()],
        },
        retry: RetryConfig {
            max_attempts: 3,
            backoff_seconds: vec![60, 120, 300],
        },
        enabled: true,
    };

    // Backup original plist
    let backups_dir = dirs.backups_dir();
    std::fs::create_dir_all(&backups_dir).map_err(ArcctlError::Io)?;
    let backup_name = format!(
        "{}.plist.bak",
        discovered.path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
    );
    let backup_path = backups_dir.join(&backup_name);
    let _ = std::fs::copy(&discovered.path, &backup_path);

    // Create the new schedule
    create_schedule(dirs, &schedule)?;

    Ok(schedule)
}

/// Scan LaunchAgents and filter out com.arcctl.job.* (already managed by arcctl).
pub fn discover_importable_jobs(_dirs: &ArcctlDirs) -> Result<Vec<DiscoveredPlist>> {
    let home = dirs::home_dir().ok_or_else(|| {
        ArcctlError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "home directory not found",
        ))
    })?;
    let agents_dir = home.join("Library").join("LaunchAgents");

    if !agents_dir.exists() {
        return Ok(vec![]);
    }

    let all = scan_claude_plists(&agents_dir)?;
    let filtered: Vec<DiscoveredPlist> = all
        .into_iter()
        .filter(|p| !p.label.starts_with("com.arcctl.job."))
        .collect();

    Ok(filtered)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn extract_arg_value(args: &[String], flag: &str) -> Option<String> {
    for (i, arg) in args.iter().enumerate() {
        if arg == flag {
            return args.get(i + 1).cloned();
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_schedule(id: &str) -> Schedule {
        Schedule {
            id: id.to_string(),
            name: format!("Test Schedule {}", id),
            schedule: "0 9 * * *".to_string(),
            timezone: None,
            agent: None,
            prompt: Some("Do a task".to_string()),
            directory: "/tmp".to_string(),
            profile: None,
            timeout_minutes: Some(30),
            session_mode: SessionMode::Fresh,
            last_session_id: None,
            delivery: DeliveryConfig {
                channels: vec!["notification".to_string()],
                on_events: vec!["complete".to_string()],
            },
            retry: RetryConfig {
                max_attempts: 3,
                backoff_seconds: vec![60, 120, 300],
            },
            enabled: true,
        }
    }

    fn make_dirs(tmp: &TempDir) -> ArcctlDirs {
        let dirs = ArcctlDirs::new(tmp.path().to_path_buf());
        dirs.ensure_all().unwrap();
        dirs
    }

    #[test]
    fn test_save_and_load_schedule() {
        let tmp = TempDir::new().unwrap();
        let dirs = make_dirs(&tmp);

        let schedule = make_schedule("sched-001");
        let path = save_schedule(&dirs, &schedule).unwrap();

        assert!(path.exists(), "schedule file should be written");

        let loaded = load_schedule(&dirs, "sched-001").unwrap();
        assert_eq!(loaded.id, "sched-001");
        assert_eq!(loaded.name, "Test Schedule sched-001");
        assert_eq!(loaded.schedule, "0 9 * * *");
        assert_eq!(loaded.prompt, Some("Do a task".to_string()));
        assert_eq!(loaded.timeout_minutes, Some(30));
    }

    #[test]
    fn test_list_schedules() {
        let tmp = TempDir::new().unwrap();
        let dirs = make_dirs(&tmp);

        save_schedule(&dirs, &make_schedule("beta")).unwrap();
        save_schedule(&dirs, &make_schedule("alpha")).unwrap();
        save_schedule(&dirs, &make_schedule("gamma")).unwrap();

        let schedules = list_schedules(&dirs).unwrap();
        assert_eq!(schedules.len(), 3, "should list 3 schedules");

        // Sorted by name
        assert_eq!(schedules[0].id, "alpha");
        assert_eq!(schedules[1].id, "beta");
        assert_eq!(schedules[2].id, "gamma");
    }

    #[test]
    fn test_delete_schedule_file() {
        let tmp = TempDir::new().unwrap();
        let dirs = make_dirs(&tmp);

        let schedule = make_schedule("del-001");
        save_schedule(&dirs, &schedule).unwrap();

        let path = dirs.schedules_dir().join("del-001.json");
        assert!(path.exists(), "file should exist before delete");

        delete_schedule_file(&dirs, "del-001").unwrap();
        assert!(!path.exists(), "file should be gone after delete");

        // Deleting again should not error
        delete_schedule_file(&dirs, "del-001").unwrap();
    }

    #[test]
    fn test_update_last_session_id() {
        let tmp = TempDir::new().unwrap();
        let dirs = make_dirs(&tmp);

        let schedule = make_schedule("sess-001");
        save_schedule(&dirs, &schedule).unwrap();

        assert!(schedule.last_session_id.is_none());

        update_last_session_id(&dirs, "sess-001", "session-abc-123").unwrap();

        let updated = load_schedule(&dirs, "sess-001").unwrap();
        assert_eq!(updated.last_session_id, Some("session-abc-123".to_string()));

        // Update again
        update_last_session_id(&dirs, "sess-001", "session-xyz-999").unwrap();
        let updated2 = load_schedule(&dirs, "sess-001").unwrap();
        assert_eq!(updated2.last_session_id, Some("session-xyz-999".to_string()));
    }
}
