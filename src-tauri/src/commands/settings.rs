use std::path::Path;

use arcctl_core::agents::{discover_agents, discover_commands, AgentInfo, CommandInfo};
use arcctl_core::settings::{
    claude_global_settings_path, claude_project_settings_path, ClaudeSettings, McpServerConfig,
};

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn resolve_settings_path(
    scope: &str,
    project_dir: &Option<String>,
) -> Result<std::path::PathBuf, String> {
    match scope {
        "global" => Ok(claude_global_settings_path()),
        "project" => {
            let dir = project_dir
                .as_ref()
                .ok_or("project_dir required for project scope")?;
            Ok(claude_project_settings_path(Path::new(dir)))
        }
        _ => Err(format!("Invalid scope: {scope}")),
    }
}

fn load(scope: &str, project_dir: &Option<String>) -> Result<(ClaudeSettings, std::path::PathBuf), String> {
    let path = resolve_settings_path(scope, project_dir)?;
    let settings = ClaudeSettings::load(&path).map_err(|e| e.to_string())?;
    Ok((settings, path))
}

// ---------------------------------------------------------------------------
// IPC commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_claude_settings(
    scope: String,
    project_dir: Option<String>,
) -> Result<ClaudeSettings, String> {
    let (settings, _) = load(&scope, &project_dir)?;
    Ok(settings)
}

#[tauri::command]
pub fn add_permission(
    scope: String,
    list: String,
    pattern: String,
    project_dir: Option<String>,
) -> Result<(), String> {
    let (mut settings, path) = load(&scope, &project_dir)?;
    match list.as_str() {
        "allow" => settings.add_permission_allow(pattern),
        "deny" => settings.add_permission_deny(pattern),
        _ => return Err(format!("Invalid list: {list}; expected 'allow' or 'deny'")),
    }
    settings.save(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_permission(
    scope: String,
    list: String,
    pattern: String,
    project_dir: Option<String>,
) -> Result<(), String> {
    let (mut settings, path) = load(&scope, &project_dir)?;
    match list.as_str() {
        "allow" => settings.remove_permission_allow(&pattern),
        "deny" => settings.remove_permission_deny(&pattern),
        _ => return Err(format!("Invalid list: {list}; expected 'allow' or 'deny'")),
    }
    settings.save(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_default_mode(
    scope: String,
    mode: Option<String>,
    project_dir: Option<String>,
) -> Result<(), String> {
    let (mut settings, path) = load(&scope, &project_dir)?;
    settings.set_default_mode(mode);
    settings.save(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_mcp_server(
    scope: String,
    name: String,
    config: McpServerConfig,
    project_dir: Option<String>,
) -> Result<(), String> {
    let (mut settings, path) = load(&scope, &project_dir)?;
    settings.add_mcp_server(name, config);
    settings.save(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_mcp_server(
    scope: String,
    name: String,
    project_dir: Option<String>,
) -> Result<(), String> {
    let (mut settings, path) = load(&scope, &project_dir)?;
    settings.remove_mcp_server(&name);
    settings.save(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_env_var(
    scope: String,
    key: String,
    value: String,
    project_dir: Option<String>,
) -> Result<(), String> {
    let (mut settings, path) = load(&scope, &project_dir)?;
    settings.set_env_var(key, value);
    settings.save(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_env_var(
    scope: String,
    key: String,
    project_dir: Option<String>,
) -> Result<(), String> {
    let (mut settings, path) = load(&scope, &project_dir)?;
    settings.remove_env_var(&key);
    settings.save(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_agents(project_dir: Option<String>) -> Result<Vec<AgentInfo>, String> {
    let global_dir = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".claude")
        .join("agents");

    let proj: Option<std::path::PathBuf> =
        project_dir.map(|d| std::path::PathBuf::from(d).join(".claude").join("agents"));

    discover_agents(&global_dir, proj.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_commands(project_dir: Option<String>) -> Result<Vec<CommandInfo>, String> {
    let global_dir = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".claude")
        .join("commands");

    let proj: Option<std::path::PathBuf> =
        project_dir.map(|d| std::path::PathBuf::from(d).join(".claude").join("commands"));

    discover_commands(&global_dir, proj.as_deref()).map_err(|e| e.to_string())
}
