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
