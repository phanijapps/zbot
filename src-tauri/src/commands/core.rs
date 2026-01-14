// ============================================================================
// CORE COMMANDS
// Core application commands
// ============================================================================

/// Greets the user (example command)
#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! Welcome to Agent Zero!", name)
}

/// Gets application version
#[tauri::command]
pub fn get_app_info() -> AppInfo {
    AppInfo {
        name: "Agent Zero".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

#[derive(serde::Serialize)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
}
