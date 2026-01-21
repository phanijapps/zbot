// ============================================================================
// VAULT COMMANDS
// Tauri commands for vault (profile) management
// ============================================================================

use crate::domains::vault::{
    manager::{self, create_default_vault_configs, expand_tilde, generate_vault_id, validate_vault_path, set_active_vault_path},
    registry::{self, initialize_default_vault, vault_registry_exists},
    types::{CreateVaultRequest, StorageInfo, Vault, VaultInfo, VaultRegistry, VaultStatus},
};
use crate::settings::AppDirs;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};

/// List all vaults
#[tauri::command]
pub async fn list_vaults() -> Result<Vec<Vault>, String> {
    let registry = registry::load_vault_registry()
        .map_err(|e| format!("Failed to load vault registry: {}", e))?;
    Ok(registry.vaults)
}

/// Get the active vault
#[tauri::command]
pub async fn get_active_vault() -> Result<Vault, String> {
    let registry = registry::load_vault_registry()
        .map_err(|e| format!("Failed to load vault registry: {}", e))?;

    registry
        .active_vault()
        .cloned()
        .ok_or_else(|| "No active vault found".to_string())
}

/// Create a new vault
#[tauri::command]
pub async fn create_vault(request: CreateVaultRequest) -> Result<Vault, String> {
    // Generate vault ID from name
    let vault_id = generate_vault_id(&request.name);

    // Check if vault with this ID already exists
    let mut registry = registry::load_vault_registry()
        .unwrap_or_else(|_| VaultRegistry::default());

    if registry.vaults.iter().any(|v| v.id == vault_id) {
        return Err(format!("Vault with ID '{}' already exists", vault_id));
    }

    // Determine vault path
    let vault_path = if let Some(path) = &request.path {
        expand_tilde(path)
    } else {
        // Auto-generate in ~/Documents/{vault-name}
        let docs = dirs::document_dir()
            .ok_or("Failed to get Documents directory")?;
        docs.join(&vault_id)
    };

    // Validate vault path
    validate_vault_path(&vault_path)?;

    // Create vault directory structure
    let _app_dirs = AppDirs::for_vault(&vault_path)
        .map_err(|e| format!("Failed to create vault directories: {}", e))?;

    // Create default configuration files
    create_default_vault_configs(&vault_path)?;

    // Create vault object
    let vault = Vault::new(
        vault_id.clone(),
        request.name.clone(),
        vault_path.to_string_lossy().to_string(),
        false,
    );

    // Add to registry
    registry.add_vault(vault.clone());

    // Save registry
    registry::save_vault_registry(&registry)
        .map_err(|e| format!("Failed to save vault registry: {}", e))?;

    Ok(vault)
}

/// Switch to a different vault
#[tauri::command]
pub async fn switch_vault(vault_id: String, app: AppHandle) -> Result<Vault, String> {
    let mut registry = registry::load_vault_registry()
        .map_err(|e| format!("Failed to load vault registry: {}", e))?;

    // Find vault
    let vault = registry
        .vaults
        .iter()
        .find(|v| v.id == vault_id)
        .ok_or_else(|| format!("Vault not found: {}", vault_id))?
        .clone();

    // Update active vault
    registry
        .set_active(vault_id.clone())
        .map_err(|e| e.to_string())?;

    // Update last accessed time
    registry.update_access_time(&vault_id);

    // Save registry
    registry::save_vault_registry(&registry)
        .map_err(|e| format!("Failed to save vault registry: {}", e))?;

    // Update global state
    set_active_vault_path(vault.path()).await;

    // Emit event for frontend to reload
    app.emit("vault-changed", &vault)
        .map_err(|e| format!("Failed to emit vault-changed event: {}", e))?;

    Ok(vault)
}

/// Delete a vault
#[tauri::command]
pub async fn delete_vault(vault_id: String) -> Result<(), String> {
    let mut registry = registry::load_vault_registry()
        .map_err(|e| format!("Failed to load vault registry: {}", e))?;

    // Don't allow deleting active vault
    if registry.active_vault_id == vault_id {
        return Err("Cannot delete active vault. Switch to another vault first.".to_string());
    }

    // Don't allow deleting default vault
    if let Some(vault) = registry.vaults.iter().find(|v| v.id == vault_id) {
        if vault.is_default {
            return Err("Cannot delete default vault".to_string());
        }
    }

    // Find vault and remove its directory
    if let Some(vault) = registry.remove_vault(&vault_id) {
        let vault_path = vault.path();
        if vault_path.exists() {
            std::fs::remove_dir_all(&vault_path)
                .map_err(|e| format!("Failed to remove vault directory: {}", e))?;
        }
    }

    // Save registry
    registry::save_vault_registry(&registry)
        .map_err(|e| format!("Failed to save vault registry: {}", e))?;

    Ok(())
}

/// Get detailed info about a vault
#[tauri::command]
pub async fn get_vault_info(vault_id: String) -> Result<VaultInfo, String> {
    let registry = registry::load_vault_registry()
        .map_err(|e| format!("Failed to load vault registry: {}", e))?;

    let vault = registry
        .vaults
        .iter()
        .find(|v| v.id == vault_id)
        .ok_or_else(|| format!("Vault not found: {}", vault_id))?;

    // Get AppDirs for this vault
    let app_dirs = AppDirs::for_vault(&vault.path())
        .map_err(|e| format!("Failed to get vault directories: {}", e))?;

    // Count agents
    let agent_count = if app_dirs.agents_dir.exists() {
        std::fs::read_dir(&app_dirs.agents_dir)
            .map(|entries| entries.filter_map(|e| e.ok()).count())
            .unwrap_or(0)
    } else {
        0
    };

    // Count skills
    let skill_count = if app_dirs.skills_dir.exists() {
        std::fs::read_dir(&app_dirs.skills_dir)
            .map(|entries| entries.filter_map(|e| e.ok()).count())
            .unwrap_or(0)
    } else {
        0
    };

    // Get storage info
    let storage_info = app_dirs
        .get_storage_info()
        .map_err(|e| format!("Failed to get storage info: {}", e))?;

    Ok(VaultInfo {
        vault: vault.clone(),
        agent_count,
        skill_count,
        storage_info: StorageInfo {
            total_used: storage_info.total_used,
            database_size: storage_info.database_size,
            agents_size: storage_info.agents_size,
            skills_size: storage_info.skills_size,
        },
    })
}

/// Initialize the vault system
/// This should be called on app startup to ensure vault registry exists
#[tauri::command]
pub async fn initialize_vault_system() -> Result<Vault, String> {
    // Initialize default vault if registry doesn't exist
    let registry = initialize_default_vault()
        .map_err(|e| format!("Failed to initialize vault system: {}", e))?;

    // Set the active vault path in global state
    if let Some(vault) = registry.active_vault() {
        set_active_vault_path(vault.path()).await;
        Ok(vault.clone())
    } else {
        Err("No active vault after initialization".to_string())
    }
}

/// Get the current status of the vault system
/// Returns information about vault registry, available vaults, and active vault
#[tauri::command]
pub async fn get_vault_status() -> Result<VaultStatus, String> {
    let registry_exists = vault_registry_exists();
    let registry = registry::load_vault_registry()
        .unwrap_or_else(|_| VaultRegistry::default());

    let has_vaults = !registry.vaults.is_empty();

    // Get active vault (validating it exists in the vaults list)
    let active_vault = if !registry.active_vault_id.is_empty() {
        registry.vaults.iter()
            .find(|v| v.id == registry.active_vault_id)
            .cloned()
    } else {
        None
    };

    let has_active_vault = active_vault.is_some();

    Ok(VaultStatus {
        registry_exists,
        has_vaults,
        has_active_vault,
        active_vault,
        vaults: registry.vaults,
    })
}

/// Set a vault as the default (active) vault
/// This updates both the is_default flag and the active_vault_id
#[tauri::command]
pub async fn set_default_vault(vault_id: String, app: AppHandle) -> Result<Vault, String> {
    let mut registry = registry::load_vault_registry()
        .map_err(|e| format!("Failed to load vault registry: {}", e))?;

    // Find the vault
    let vault = registry.vaults.iter()
        .find(|v| v.id == vault_id)
        .ok_or_else(|| format!("Vault not found: {}", vault_id))?
        .clone();

    // Update active_vault_id
    registry.set_active(vault_id.clone())
        .map_err(|e| e.to_string())?;

    // Update last_accessed time
    registry.update_access_time(&vault_id);

    // Save registry
    registry::save_vault_registry(&registry)
        .map_err(|e| format!("Failed to save vault registry: {}", e))?;

    // Update global state
    set_active_vault_path(vault.path()).await;

    // Emit event for frontend to reload
    app.emit("vault-changed", &vault)
        .map_err(|e| format!("Failed to emit vault-changed event: {}", e))?;

    Ok(vault)
}
