use arcctl_core::config::read_claude_settings;

#[tauri::command]
pub fn read_claude_settings_cmd(path: String) -> Result<serde_json::Value, String> {
    read_claude_settings(std::path::Path::new(&path)).map_err(|e| e.to_string())
}
