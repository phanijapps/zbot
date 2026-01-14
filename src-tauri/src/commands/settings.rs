// ============================================================================
// SETTINGS COMMANDS
// Tauri commands for settings management
// ============================================================================

use crate::settings::{AppDirs, Settings, StorageInfo};

/// Get all application settings
#[tauri::command]
pub async fn get_settings() -> Result<Settings, String> {
    let dirs = AppDirs::get().map_err(|e| e.to_string())?;
    dirs.load_settings().map_err(|e| e.to_string())
}

/// Save application settings
#[tauri::command]
pub async fn save_settings(settings: Settings) -> Result<(), String> {
    let dirs = AppDirs::get().map_err(|e| e.to_string())?;
    dirs.save_settings(&settings).map_err(|e| e.to_string())
}

/// Reset settings to defaults
#[tauri::command]
pub async fn reset_settings() -> Result<Settings, String> {
    let dirs = AppDirs::get().map_err(|e| e.to_string())?;
    dirs.reset_settings().map_err(|e| e.to_string())?;
    dirs.load_settings().map_err(|e| e.to_string())
}

/// Get storage information
#[tauri::command]
pub async fn get_storage_info() -> Result<StorageInfo, String> {
    let dirs = AppDirs::get().map_err(|e| e.to_string())?;
    dirs.get_storage_info().map_err(|e| e.to_string())
}

/// Clear all application data (except settings)
#[tauri::command]
pub async fn clear_all_data() -> Result<(), String> {
    let dirs = AppDirs::get().map_err(|e| e.to_string())?;
    dirs.clear_all_data().map_err(|e| e.to_string())
}

/// Get the config directory path
#[tauri::command]
pub async fn get_config_path() -> Result<String, String> {
    let dirs = AppDirs::get().map_err(|e| e.to_string())?;
    Ok(dirs.config_dir.to_string_lossy().to_string())
}

/// Initialize application directories
#[tauri::command]
pub async fn initialize_directories() -> Result<DirectoriesInfo, String> {
    let dirs = AppDirs::get().map_err(|e| e.to_string())?;
    dirs.initialize().map_err(|e| e.to_string())?;

    Ok(DirectoriesInfo {
        config_dir: dirs.config_dir.to_string_lossy().to_string(),
        settings_file: dirs.settings_file.to_string_lossy().to_string(),
        database_path: dirs.database_path.to_string_lossy().to_string(),
        agents_dir: dirs.agents_dir.to_string_lossy().to_string(),
        skills_dir: dirs.skills_dir.to_string_lossy().to_string(),
        venv_dir: dirs.venv_dir.to_string_lossy().to_string(),
    })
}

/// Information about application directories
#[derive(serde::Serialize)]
pub struct DirectoriesInfo {
    pub config_dir: String,
    pub settings_file: String,
    pub database_path: String,
    pub agents_dir: String,
    pub skills_dir: String,
    pub venv_dir: String,
}
