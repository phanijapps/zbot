// ============================================================================
// SETTINGS COMMANDS
// Tauri commands for settings management
// ============================================================================

use crate::settings::{AppDirs, Settings, StorageInfo};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
extern crate dirs;

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

/// Clear conversation data only (preserves agents and skills)
#[tauri::command]
pub async fn clear_conversations() -> Result<(), String> {
    let dirs = AppDirs::get().map_err(|e| e.to_string())?;
    dirs.clear_conversations().map_err(|e| e.to_string())
}

/// Clear all application data (except settings) - WARNING: Also deletes agents and skills!
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

// ============================================================================
// PYTHON VENV REQUIREMENTS COMMANDS
// ============================================================================

/// Get the shared venv path at ~/.config/zeroagent/venv
/// All vaults use this shared Python environment
fn get_shared_venv_path() -> Result<PathBuf, String> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| "Failed to get config directory".to_string())?
        .join("zeroagent");
    Ok(config_dir.join("venv"))
}

/// Get the Python interpreter path from the venv
fn get_python_venv() -> Result<PathBuf, String> {
    let venv_path = get_shared_venv_path()?;

    #[cfg(target_os = "windows")]
    let python_path = venv_path.join("Scripts").join("python.exe");

    #[cfg(not(target_os = "windows"))]
    let python_path = venv_path.join("bin").join("python");

    if !python_path.exists() {
        return Err(format!(
            "Python venv not found at {}. Please create it first.",
            venv_path.display()
        ));
    }

    Ok(python_path)
}

/// Get venv information (path, requirements.txt exists, installed packages)
#[tauri::command]
pub async fn get_venv_info() -> Result<VenvInfo, String> {
    let venv_path = get_shared_venv_path()?;
    let requirements_path = venv_path.join("requirements.txt");

    let requirements_exists = requirements_path.exists();
    let venv_exists = venv_path.exists();

    // Get installed packages list
    let installed_packages = if venv_exists {
        match get_python_venv() {
            Ok(python_path) => {
                let output = Command::new(&python_path)
                    .args(["-m", "pip", "list", "--format=json"])
                    .output();

                match output {
                    Ok(result) if result.status.success() => {
                        let stdout = String::from_utf8_lossy(&result.stdout).to_string();
                        Some(stdout)
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    } else {
        None
    };

    Ok(VenvInfo {
        venv_path: venv_path.to_string_lossy().to_string(),
        venv_exists,
        requirements_exists,
        installed_packages,
    })
}

/// Information about the Python venv
#[derive(serde::Serialize)]
pub struct VenvInfo {
    pub venv_path: String,
    pub venv_exists: bool,
    pub requirements_exists: bool,
    pub installed_packages: Option<String>,  // JSON array of {name, version}
}

/// Read requirements.txt content
#[tauri::command]
pub async fn read_requirements() -> Result<String, String> {
    let venv_path = get_shared_venv_path()?;
    let requirements_path = venv_path.join("requirements.txt");

    // Ensure venv directory exists
    if !venv_path.exists() {
        fs::create_dir_all(&venv_path)
            .map_err(|e| format!("Failed to create venv directory: {}", e))?;
    }

    if !requirements_path.exists() {
        // Return empty content with a comment
        return Ok(String::from("# Add your Python requirements here (one per line)\n# Example: numpy==1.24.0\n"));
    }

    fs::read_to_string(&requirements_path)
        .map_err(|e| format!("Failed to read requirements.txt: {}", e))
}

/// Save requirements.txt content
#[tauri::command]
pub async fn save_requirements(content: String) -> Result<(), String> {
    let venv_path = get_shared_venv_path()?;
    let requirements_path = venv_path.join("requirements.txt");

    // Ensure venv directory exists before writing
    if !venv_path.exists() {
        fs::create_dir_all(&venv_path)
            .map_err(|e| format!("Failed to create venv directory: {}", e))?;
    }

    fs::write(&requirements_path, content)
        .map_err(|e| format!("Failed to write requirements.txt: {}", e))?;

    Ok(())
}

/// Install requirements from requirements.txt
#[tauri::command]
pub async fn install_requirements() -> Result<String, String> {
    let python_path = get_python_venv()?;
    let venv_path = get_shared_venv_path()?;
    let requirements_path = venv_path.join("requirements.txt");

    if !requirements_path.exists() {
        return Err("requirements.txt not found. Please create it first.".to_string());
    }

    // Run pip install -r requirements.txt
    let output = Command::new(&python_path)
        .args(["-m", "pip", "install", "-r", requirements_path.to_str().unwrap()])
        .output()
        .map_err(|e| format!("Failed to run pip install: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        return Err(format!("Installation failed:\n{}", stderr));
    }

    Ok(format!("Successfully installed requirements.\n\n{}", stdout))
}

/// List installed packages in the venv
#[tauri::command]
pub async fn list_installed_packages() -> Result<Vec<PackageInfo>, String> {
    let python_path = get_python_venv()?;

    let output = Command::new(&python_path)
        .args(["-m", "pip", "list", "--format=json"])
        .output()
        .map_err(|e| format!("Failed to list packages: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(format!("Failed to list packages: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

    // Parse JSON output
    serde_json::from_str(&stdout)
        .map_err(|e| format!("Failed to parse package list: {}", e))
}

/// Information about an installed package
#[derive(serde::Serialize, serde::Deserialize)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
}

// ============================================================================
// NODE.JS ENVIRONMENT COMMANDS
// ============================================================================

