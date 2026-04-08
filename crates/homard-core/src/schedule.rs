use std::path::PathBuf;

use crate::config::HomardDirs;
use crate::error::{HomardError, Result};
use crate::launchd::{install_plist, uninstall_plist};
use crate::types::Schedule;

// ---------------------------------------------------------------------------
// Binary resolution
// ---------------------------------------------------------------------------

/// Find the homard binary path. Checks:
/// 1. Current executable path
/// 2. /usr/local/bin/homard
/// 3. ~/.cargo/bin/homard
pub fn resolve_homard_bin() -> Result<String> {
    // Try current executable first
    if let Ok(exe) = std::env::current_exe() {
        if exe.exists() {
            return Ok(exe.to_string_lossy().to_string());
        }
    }

    // Try /usr/local/bin/homard
    let usr_local = std::path::Path::new("/usr/local/bin/homard");
    if usr_local.exists() {
        return Ok(usr_local.to_string_lossy().to_string());
    }

    // Try ~/.cargo/bin/homard
    if let Some(home) = dirs::home_dir() {
        let cargo_bin = home.join(".cargo").join("bin").join("homard");
        if cargo_bin.exists() {
            return Ok(cargo_bin.to_string_lossy().to_string());
        }
    }

    Err(HomardError::NotFound("homard binary not found".to_string()))
}

// ---------------------------------------------------------------------------
// File-based CRUD
// ---------------------------------------------------------------------------

/// Write a schedule as JSON to `schedules_dir/<id>.json`.
pub fn save_schedule(dirs: &HomardDirs, schedule: &Schedule) -> Result<PathBuf> {
    let path = dirs.schedules_dir().join(format!("{}.json", schedule.id));
    let json = serde_json::to_string_pretty(schedule).map_err(HomardError::Json)?;
    std::fs::write(&path, json).map_err(HomardError::Io)?;
    Ok(path)
}

/// Read a schedule from `schedules_dir/<id>.json`.
pub fn load_schedule(dirs: &HomardDirs, id: &str) -> Result<Schedule> {
    let path = dirs.schedules_dir().join(format!("{}.json", id));
    let contents = std::fs::read_to_string(&path).map_err(HomardError::Io)?;
    let schedule: Schedule = serde_json::from_str(&contents).map_err(HomardError::Json)?;
    Ok(schedule)
}

/// List all schedules in `schedules_dir`, sorted by name.
pub fn list_schedules(dirs: &HomardDirs) -> Result<Vec<Schedule>> {
    let dir = dirs.schedules_dir();
    if !dir.exists() {
        return Ok(vec![]);
    }

    let entries = std::fs::read_dir(&dir).map_err(HomardError::Io)?;
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
pub fn delete_schedule_file(dirs: &HomardDirs, id: &str) -> Result<()> {
    let path = dirs.schedules_dir().join(format!("{}.json", id));
    if path.exists() {
        std::fs::remove_file(&path).map_err(HomardError::Io)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Higher-level operations (file + launchd)
// ---------------------------------------------------------------------------

/// Save JSON + install launchd plist.
pub fn create_schedule(dirs: &HomardDirs, schedule: &Schedule) -> Result<()> {
    save_schedule(dirs, schedule)?;
    let bin = resolve_homard_bin().unwrap_or_else(|_| "homard".to_string());
    install_plist(schedule, &bin)?;
    Ok(())
}

/// Uninstall old plist, save JSON, install new plist.
pub fn update_schedule(dirs: &HomardDirs, schedule: &Schedule) -> Result<()> {
    uninstall_plist(&schedule.id)?;
    save_schedule(dirs, schedule)?;
    let bin = resolve_homard_bin().unwrap_or_else(|_| "homard".to_string());
    install_plist(schedule, &bin)?;
    Ok(())
}

/// Uninstall plist + delete file.
pub fn delete_schedule(dirs: &HomardDirs, id: &str) -> Result<()> {
    uninstall_plist(id)?;
    delete_schedule_file(dirs, id)?;
    Ok(())
}

/// Set enabled=false and `launchctl unload` the plist.
pub fn pause_schedule(dirs: &HomardDirs, id: &str) -> Result<()> {
    let mut schedule = load_schedule(dirs, id)?;
    schedule.enabled = false;
    save_schedule(dirs, &schedule)?;

    let home = dirs::home_dir().ok_or_else(|| {
        HomardError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "home directory not found",
        ))
    })?;
    let label = format!("com.homard.job.{}", id);
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
pub fn resume_schedule(dirs: &HomardDirs, id: &str) -> Result<()> {
    let mut schedule = load_schedule(dirs, id)?;
    schedule.enabled = true;
    save_schedule(dirs, &schedule)?;

    let home = dirs::home_dir().ok_or_else(|| {
        HomardError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "home directory not found",
        ))
    })?;
    let label = format!("com.homard.job.{}", id);
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
            message: "Do a task".to_string(),
            schedule: "0 9 * * *".to_string(),
            enabled: true,
            deliver_to: vec!["notification".to_string()],
        }
    }

    fn make_dirs(tmp: &TempDir) -> HomardDirs {
        let dirs = HomardDirs::new(tmp.path().to_path_buf());
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
        assert_eq!(loaded.message, "Do a task");
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
}
