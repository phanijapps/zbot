// ============================================================================
// VAULT MANAGER
// Vault switching logic and global state management
// ============================================================================

use super::types::Vault;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Thread-safe global vault path
/// This stores the currently active vault's path
lazy_static::lazy_static! {
    pub static ref CURRENT_VAULT_PATH: Arc<RwLock<Option<PathBuf>>> =
        Arc::new(RwLock::new(None));
}

/// Set the active vault path
pub async fn set_active_vault_path(path: PathBuf) {
    let mut vault_path = CURRENT_VAULT_PATH.write().await;
    *vault_path = Some(path);
}

/// Get the active vault path
/// Returns error if no vault is set
pub async fn get_active_vault_path() -> Result<PathBuf, String> {
    let vault_path = CURRENT_VAULT_PATH.read().await;
    vault_path
        .clone()
        .ok_or_else(|| "No active vault set".to_string())
}

/// Clear the active vault path
pub async fn clear_active_vault_path() {
    let mut vault_path = CURRENT_VAULT_PATH.write().await;
    *vault_path = None;
}

/// Expand tilde in path (e.g., ~/Documents -> /home/user/Documents)
pub fn expand_tilde(path: &str) -> PathBuf {
    if path.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(&path[2..]);
        }
    }
    PathBuf::from(path)
}

/// Generate a vault ID from a name
/// Converts "My Vault" to "my-vault"
pub fn generate_vault_id(name: &str) -> String {
    name.to_lowercase()
        .replace(' ', "-")
        .replace('_', "-")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .collect()
}

/// Validate vault path
/// Ensures the path is valid and accessible
pub fn validate_vault_path(path: &PathBuf) -> Result<(), String> {
    // Check if path is absolute
    if !path.is_absolute() {
        return Err("Vault path must be absolute".to_string());
    }

    // Check if parent directory exists (we'll create the vault itself)
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            return Err(format!(
                "Parent directory does not exist: {}",
                parent.display()
            ));
        }
    }

    Ok(())
}

/// Create default configuration files for a new vault
pub fn create_default_vault_configs(vault_path: &PathBuf) -> Result<(), String> {
    use std::fs;

    // Create empty mcps.json if it doesn't exist
    let mcps_path = vault_path.join("mcps.json");
    if !mcps_path.exists() {
        let empty_mcps = serde_json::json!([]);
        fs::write(&mcps_path, serde_json::to_string_pretty(&empty_mcps).unwrap())
            .map_err(|e| format!("Failed to create mcps.json: {}", e))?;
    }

    // Create empty providers.json if it doesn't exist
    let providers_path = vault_path.join("providers.json");
    if !providers_path.exists() {
        let empty_providers = serde_json::json!({});
        fs::write(&providers_path, serde_json::to_string_pretty(&empty_providers).unwrap())
            .map_err(|e| format!("Failed to create providers.json: {}", e))?;
    }

    // Create default settings.yaml if it doesn't exist
    let settings_path = vault_path.join("settings.yaml");
    if !settings_path.exists() {
        let default_settings = crate::settings::Settings::default();
        crate::settings::AppDirs {
            settings_file: settings_path,
            ..crate::settings::AppDirs::for_vault(vault_path)
                .map_err(|e| format!("Failed to create AppDirs: {}", e))?
        }
        .save_settings(&default_settings)
        .map_err(|e| format!("Failed to save settings: {}", e))?;
    }

    // Copy builtin skills to the vault
    copy_builtin_skills(vault_path)?;

    Ok(())
}

/// List of builtin skills that should be available in every vault
pub const BUILTIN_SKILLS: &[&str] = &[
    "zero-entity-extract",  // Entity extraction from transcripts
];

/// Copy builtin skills from templates to the vault's skills directory
pub fn copy_builtin_skills(vault_path: &PathBuf) -> Result<(), String> {
    use std::fs;

    let skills_dir = vault_path.join("skills");
    
    // Try multiple approaches to find the templates directory
    let templates_dir = find_templates_dir()?;
    
    for skill_id in BUILTIN_SKILLS {
        let source_dir = templates_dir.join(skill_id);
        let target_dir = skills_dir.join(skill_id);

        // Skip if source doesn't exist (e.g., running from a different location)
        if !source_dir.exists() {
            tracing::warn!("Builtin skill not found in templates: {}", skill_id);
            continue;
        }

        // Skip if target already exists (don't overwrite user customizations)
        if target_dir.exists() {
            continue;
        }

        // Create target directory
        fs::create_dir_all(&target_dir)
            .map_err(|e| format!("Failed to create skill directory: {}", e))?;

        // Copy all files from source to target
        copy_dir_recursive(&source_dir, &target_dir)?;
        
        tracing::info!("Copied builtin skill '{}' to vault", skill_id);
    }

    Ok(())
}

/// Find the templates directory using multiple fallback approaches
fn find_templates_dir() -> Result<std::path::PathBuf, String> {
    use std::fs;
    
    // Approach 1: Try CARGO_MANIFEST_DIR (set during build)
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let path = std::path::PathBuf::from(manifest_dir)
            .join("templates")
            .join("default-skills");
        if path.exists() {
            tracing::info!("Found templates via CARGO_MANIFEST_DIR: {:?}", path);
            return Ok(path);
        }
    }
    
    // Approach 2: Try relative to current exe (dev: target/debug/ -> ../../templates/)
    if let Ok(exe_path) = std::env::current_exe() {
        // In development: exe is in src-tauri/target/debug/
        // Templates are in src-tauri/templates/
        if let Some(parent) = exe_path.parent() {
            // target/debug -> target -> src-tauri -> templates
            if let Some(target_dir) = parent.parent() {
                let src_tauri = target_dir.join("src-tauri");
                let path = src_tauri.join("templates").join("default-skills");
                if path.exists() {
                    tracing::info!("Found templates via exe path: {:?}", path);
                    return Ok(path);
                }
            }
        }
    }
    
    // Approach 3: Try current directory (for production installs)
    let current_dir = std::env::current_dir()
        .map_err(|e| format!("Failed to get current directory: {}", e))?;
    let path = current_dir.join("templates").join("default-skills");
    if path.exists() {
        tracing::info!("Found templates via current directory: {:?}", path);
        return Ok(path);
    }
    
    // Approach 4: Try src-tauri subdirectory (common development setup)
    let path = current_dir.join("src-tauri").join("templates").join("default-skills");
    if path.exists() {
        tracing::info!("Found templates via src-tauri subdirectory: {:?}", path);
        return Ok(path);
    }
    
    Err(format!(
        "Could not find templates directory. Searched in:\n\
         - CARGO_MANIFEST_DIR/templates\n\
         - Exe path/../../src-tauri/templates\n\
         - ./templates\n\
         - ./src-tauri/templates\n\
         \n\
         Current directory: {:?}\n\
         Exe path: {:?}",
         current_dir,
         std::env::current_exe()
    ))
}

/// Recursively copy a directory's contents
fn copy_dir_recursive(source: &std::path::Path, target: &std::path::Path) -> Result<(), String> {
    use std::fs;

    for entry in fs::read_dir(source)
        .map_err(|e| format!("Failed to read source directory: {}", e))?
    {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());

        if source_path.is_dir() {
            fs::create_dir_all(&target_path)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
            copy_dir_recursive(&source_path, &target_path)?;
        } else {
            fs::copy(&source_path, &target_path)
                .map_err(|e| format!("Failed to copy file: {}", e))?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_vault_id() {
        assert_eq!(generate_vault_id("My Vault"), "my-vault");
        assert_eq!(generate_vault_id("Project Alpha"), "project-alpha");
        assert_eq!(generate_vault_id("test_vault"), "test-vault");
        assert_eq!(generate_vault_id("Vault123"), "vault123");
    }

    #[test]
    fn test_expand_tilde() {
        let result = expand_tilde("~/Documents");
        assert!(result.is_absolute());
        assert!(result.ends_with("Documents"));
    }
}
