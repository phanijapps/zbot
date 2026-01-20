// ============================================================================
// VAULT REGISTRY
// Load/save vault registry from ~/.config/agentzero/vaults_registry.json
// ============================================================================

use super::types::VaultRegistry;
use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

/// Get the path to the vault registry file
/// Located at ~/.config/agentzero/vaults_registry.json
pub fn get_vault_registry_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .context("Failed to get config directory")?
        .join("agentzero"); // Note: agentzero, not zeroagent

    // Ensure directory exists
    fs::create_dir_all(&config_dir)
        .context("Failed to create agentzero config directory")?;

    Ok(config_dir.join("vaults_registry.json"))
}

/// Check if vault registry exists
pub fn vault_registry_exists() -> bool {
    get_vault_registry_path()
        .map(|p| p.exists())
        .unwrap_or(false)
}

/// Load vault registry from disk
pub fn load_vault_registry() -> Result<VaultRegistry> {
    let path = get_vault_registry_path()?;

    if !path.exists() {
        // Return empty registry if file doesn't exist
        return Ok(VaultRegistry::default());
    }

    let content = fs::read_to_string(&path)
        .context("Failed to read vault registry file")?;

    let registry: VaultRegistry = serde_json::from_str(&content)
        .context("Failed to parse vault registry JSON")?;

    Ok(registry)
}

/// Save vault registry to disk
pub fn save_vault_registry(registry: &VaultRegistry) -> Result<()> {
    let path = get_vault_registry_path()?;

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .context("Failed to create vault registry directory")?;
    }

    let json = serde_json::to_string_pretty(registry)
        .context("Failed to serialize vault registry")?;

    fs::write(&path, json)
        .context("Failed to write vault registry file")?;

    Ok(())
}

/// Initialize vault registry with default vault if it doesn't exist
/// This is used for migrating existing users to the vault system
pub fn initialize_default_vault() -> Result<VaultRegistry> {
    if vault_registry_exists() {
        return load_vault_registry();
    }

    // Check if existing installation at ~/.config/zeroagent
    let old_config_path = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("zeroagent");

    let default_vault = if old_config_path.exists() {
        // Migrate existing installation
        super::types::Vault::new(
            "default".to_string(),
            "Default Vault".to_string(),
            old_config_path.to_string_lossy().to_string(),
            true,
        )
    } else {
        // New installation - create at ~/.config/agentzero
        let new_path = dirs::config_dir()
            .context("Failed to get config directory")?
            .join("agentzero");

        super::types::Vault::new(
            "default".to_string(),
            "Default Vault".to_string(),
            new_path.to_string_lossy().to_string(),
            true,
        )
    };

    let registry = VaultRegistry::new("default".to_string(), vec![default_vault]);
    save_vault_registry(&registry)?;

    Ok(registry)
}
