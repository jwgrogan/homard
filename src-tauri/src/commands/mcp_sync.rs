use arcctl_core::mcp_sync::{ManagedMcpConfig, McpSyncManager};
use arcctl_core::settings::{claude_global_settings_path, McpServerConfig};

fn mcp_sync_manager() -> McpSyncManager {
    let path = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".arcctl")
        .join("mcp-servers.json");
    McpSyncManager::new(path)
}

#[tauri::command]
pub fn list_managed_mcps() -> Result<ManagedMcpConfig, String> {
    let manager = mcp_sync_manager();
    manager.list_servers().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_managed_mcp(name: String, config: McpServerConfig) -> Result<(), String> {
    let manager = mcp_sync_manager();
    manager
        .add_server(name, config)
        .map_err(|e| e.to_string())?;
    let claude_path = claude_global_settings_path();
    manager
        .sync_to_claude(&claude_path)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_managed_mcp(name: String) -> Result<(), String> {
    let manager = mcp_sync_manager();
    manager
        .remove_server(&name)
        .map_err(|e| e.to_string())?;
    let claude_path = claude_global_settings_path();
    manager
        .sync_to_claude(&claude_path)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn sync_all_mcps() -> Result<(), String> {
    let manager = mcp_sync_manager();
    let claude_path = claude_global_settings_path();
    manager
        .sync_to_claude(&claude_path)
        .map_err(|e| e.to_string())
}
