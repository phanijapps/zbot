// ============================================================================
// VAULT TYPES
// Core data structures for vault management
// ============================================================================

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Represents a vault (profile/directory)
/// A vault is just a different configuration directory location
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vault {
    /// Unique identifier for the vault
    pub id: String,
    /// Display name for the vault
    pub name: String,
    /// Absolute path to the vault directory
    #[serde(rename = "path")]
    pub vault_path: String,
    /// Whether this is the default vault
    pub is_default: bool,
    /// When the vault was created
    #[serde(rename = "createdAt")]
    pub created_at: String,
    /// When the vault was last accessed
    #[serde(rename = "lastAccessed")]
    pub last_accessed: String,
}

impl Vault {
    /// Get the vault path as a PathBuf
    pub fn path(&self) -> PathBuf {
        PathBuf::from(&self.vault_path)
    }

    /// Create a new vault with generated timestamps
    pub fn new(id: String, name: String, path: String, is_default: bool) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id,
            name,
            vault_path: path,
            is_default,
            created_at: now.clone(),
            last_accessed: now,
        }
    }
}

/// Vault registry stored at ~/.config/agentzero/vaults_registry.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultRegistry {
    /// ID of the currently active vault
    #[serde(rename = "activeVaultId")]
    pub active_vault_id: String,
    /// List of all vaults
    pub vaults: Vec<Vault>,
}

impl VaultRegistry {
    /// Create a new vault registry
    pub fn new(active_vault_id: String, vaults: Vec<Vault>) -> Self {
        Self {
            active_vault_id,
            vaults,
        }
    }

    /// Get the active vault
    pub fn active_vault(&self) -> Option<&Vault> {
        self.vaults.iter().find(|v| v.id == self.active_vault_id)
    }

    /// Add a vault to the registry
    pub fn add_vault(&mut self, vault: Vault) {
        self.vaults.push(vault);
    }

    /// Remove a vault from the registry
    pub fn remove_vault(&mut self, vault_id: &str) -> Option<Vault> {
        let pos = self.vaults.iter().position(|v| v.id == vault_id)?;
        Some(self.vaults.remove(pos))
    }

    /// Set the active vault
    pub fn set_active(&mut self, vault_id: String) -> Result<(), String> {
        if self.vaults.iter().any(|v| v.id == vault_id) {
            self.active_vault_id = vault_id;
            Ok(())
        } else {
            Err(format!("Vault not found: {}", vault_id))
        }
    }

    /// Update the last_accessed timestamp for a vault
    pub fn update_access_time(&mut self, vault_id: &str) {
        if let Some(vault) = self.vaults.iter_mut().find(|v| v.id == vault_id) {
            vault.last_accessed = chrono::Utc::now().to_rfc3339();
        }
    }
}

/// Request to create a new vault
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateVaultRequest {
    /// Display name for the vault
    pub name: String,
    /// Optional custom path (if None, auto-generate in ~/Documents/)
    pub path: Option<String>,
}

/// Detailed information about a vault
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultInfo {
    /// The vault
    pub vault: Vault,
    /// Number of agents in the vault
    pub agent_count: usize,
    /// Number of skills in the vault
    pub skill_count: usize,
    /// Storage information
    pub storage_info: StorageInfo,
}

/// Storage information for a vault
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageInfo {
    /// Total used space in bytes
    pub total_used: u64,
    /// Database size in bytes
    pub database_size: u64,
    /// Agents directory size in bytes
    pub agents_size: u64,
    /// Skills directory size in bytes
    pub skills_size: u64,
}

impl Default for VaultRegistry {
    fn default() -> Self {
        Self {
            active_vault_id: String::new(),
            vaults: Vec::new(),
        }
    }
}
