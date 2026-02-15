//! # Vault Paths
//!
//! Centralized path management for the AgentZero vault.
//!
//! Provides XDG-inspired directory structure:
//! - Config files go in `config/` subdirectory
//! - Data files go in `data/` subdirectory
//! - Agent/session data goes in `wards/` subdirectory (each agent/session is a ward)
//! - Other directories (logs, agents, skills) remain at root level

use std::io;
use std::path::PathBuf;
use std::sync::Arc;

/// Centralized path management for the AgentZero vault.
///
/// Provides a consistent interface for accessing all vault paths,
/// following XDG-inspired conventions:
/// - Config files → `config/` subdirectory
/// - Data files → `data/` subdirectory
/// - Agent/session data → `wards/{id}/` subdirectory
#[derive(Debug, Clone)]
pub struct VaultPaths {
    /// Base vault directory (e.g., ~/Documents/agentzero)
    vault_dir: PathBuf,
}

impl VaultPaths {
    /// Create a new VaultPaths instance.
    pub fn new(vault_dir: PathBuf) -> Self {
        Self { vault_dir }
    }

    // =========================================================================
    // Config paths (config/ subdirectory)
    // =========================================================================

    /// Path to `config/settings.json`
    pub fn settings(&self) -> PathBuf {
        self.vault_dir.join("config").join("settings.json")
    }

    /// Path to `config/providers.json`
    pub fn providers(&self) -> PathBuf {
        self.vault_dir.join("config").join("providers.json")
    }

    /// Path to `config/mcps.json`
    pub fn mcps(&self) -> PathBuf {
        self.vault_dir.join("config").join("mcps.json")
    }

    /// Path to `config/connectors.json`
    pub fn connectors(&self) -> PathBuf {
        self.vault_dir.join("config").join("connectors.json")
    }

    /// Path to `config/cron_jobs.json`
    pub fn cron_jobs(&self) -> PathBuf {
        self.vault_dir.join("config").join("cron_jobs.json")
    }

    /// Path to `config/INSTRUCTIONS.md`
    pub fn instructions(&self) -> PathBuf {
        self.vault_dir.join("config").join("INSTRUCTIONS.md")
    }

    /// Path to the config directory
    pub fn config_dir(&self) -> PathBuf {
        self.vault_dir.join("config")
    }

    // =========================================================================
    // Data paths (data/ subdirectory)
    // =========================================================================

    /// Path to `data/conversations.db`
    pub fn conversations_db(&self) -> PathBuf {
        self.vault_dir.join("data").join("conversations.db")
    }

    /// Path to the data directory
    pub fn data_dir(&self) -> PathBuf {
        self.vault_dir.join("data")
    }

    // =========================================================================
    // Root-level directories
    // =========================================================================

    /// Path to logs directory
    pub fn logs_dir(&self) -> PathBuf {
        self.vault_dir.join("logs")
    }

    /// Path to agents directory (agent definitions)
    pub fn agents_dir(&self) -> PathBuf {
        self.vault_dir.join("agents")
    }

    /// Path to skills directory
    pub fn skills_dir(&self) -> PathBuf {
        self.vault_dir.join("skills")
    }

    /// Path to wards directory (contains agent data, session data, and scratch ward)
    pub fn wards_dir(&self) -> PathBuf {
        self.vault_dir.join("wards")
    }

    /// Path to a specific ward (also used for agent data and session data).
    /// Returns `wards/{ward_id}/`
    pub fn ward_dir(&self, ward_id: &str) -> PathBuf {
        self.vault_dir.join("wards").join(ward_id)
    }

    /// Get the base vault directory
    pub fn vault_dir(&self) -> &PathBuf {
        &self.vault_dir
    }

    // =========================================================================
    // Directory initialization
    // =========================================================================

    /// Ensure all required directories exist.
    ///
    /// Creates:
    /// - config/
    /// - data/
    /// - logs/
    /// - agents/
    /// - skills/
    /// - wards/ (with scratch subdirectory)
    pub fn ensure_dirs_exist(&self) -> io::Result<()> {
        let dirs = [
            self.config_dir(),
            self.data_dir(),
            self.logs_dir(),
            self.agents_dir(),
            self.skills_dir(),
            self.wards_dir(),
        ];

        for dir in &dirs {
            if !dir.exists() {
                std::fs::create_dir_all(dir)?;
                tracing::debug!("Created directory: {:?}", dir);
            }
        }

        // Ensure scratch ward exists
        let scratch_dir = self.ward_dir("scratch");
        if !scratch_dir.exists() {
            std::fs::create_dir_all(&scratch_dir)?;
            tracing::debug!("Created scratch ward: {:?}", scratch_dir);
        }

        Ok(())
    }
}

/// Reference-counted VaultPaths for sharing across services.
pub type SharedVaultPaths = Arc<VaultPaths>;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_config_paths() {
        let dir = tempdir().unwrap();
        let paths = VaultPaths::new(dir.path().to_path_buf());

        assert_eq!(paths.settings(), dir.path().join("config").join("settings.json"));
        assert_eq!(paths.providers(), dir.path().join("config").join("providers.json"));
        assert_eq!(paths.mcps(), dir.path().join("config").join("mcps.json"));
        assert_eq!(paths.connectors(), dir.path().join("config").join("connectors.json"));
        assert_eq!(paths.cron_jobs(), dir.path().join("config").join("cron_jobs.json"));
        assert_eq!(paths.instructions(), dir.path().join("config").join("INSTRUCTIONS.md"));
    }

    #[test]
    fn test_data_paths() {
        let dir = tempdir().unwrap();
        let paths = VaultPaths::new(dir.path().to_path_buf());

        assert_eq!(paths.conversations_db(), dir.path().join("data").join("conversations.db"));
    }

    #[test]
    fn test_directory_paths() {
        let dir = tempdir().unwrap();
        let paths = VaultPaths::new(dir.path().to_path_buf());

        assert_eq!(paths.logs_dir(), dir.path().join("logs"));
        assert_eq!(paths.agents_dir(), dir.path().join("agents"));
        assert_eq!(paths.skills_dir(), dir.path().join("skills"));
        assert_eq!(paths.wards_dir(), dir.path().join("wards"));
    }

    #[test]
    fn test_ward_dir() {
        let dir = tempdir().unwrap();
        let paths = VaultPaths::new(dir.path().to_path_buf());

        assert_eq!(paths.ward_dir("scratch"), dir.path().join("wards").join("scratch"));
        assert_eq!(paths.ward_dir("root"), dir.path().join("wards").join("root"));
    }

    #[test]
    fn test_ensure_dirs_exist() {
        let dir = tempdir().unwrap();
        let paths = VaultPaths::new(dir.path().to_path_buf());

        paths.ensure_dirs_exist().unwrap();

        assert!(dir.path().join("config").exists());
        assert!(dir.path().join("data").exists());
        assert!(dir.path().join("logs").exists());
        assert!(dir.path().join("agents").exists());
        assert!(dir.path().join("skills").exists());
        assert!(dir.path().join("wards").exists());
        assert!(dir.path().join("wards").join("scratch").exists());
    }
}
