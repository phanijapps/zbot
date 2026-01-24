// ============================================================================
// VAULT MANAGER
// Vault switching logic and global state management
// ============================================================================

use super::types::Vault;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
extern crate dirs;

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

    // Create default mcps.json with common MCP servers (no sensitive data)
    let mcps_path = vault_path.join("mcps.json");
    if !mcps_path.exists() {
        let default_mcps = serde_json::json!([
          {
            "type": "stdio",
            "id": "filesystem",
            "name": "Filesystem",
            "description": "Access and manipulate files in specified directories",
            "command": "npx",
            "args": ["-y", "@modelcontextprotocol/server-filesystem", "{HOME}/Downloads"],
            "enabled": false,
            "validated": false
          },
          {
            "type": "stdio",
            "id": "time",
            "name": "Time",
            "description": "Get current time and perform time calculations",
            "command": "uvx",
            "args": ["mcp-server-time"],
            "enabled": false,
            "validated": false
          }
        ]);
        // Replace {HOME} placeholder with actual home directory
        let mcps_content = serde_json::to_string_pretty(&default_mcps)
            .map_err(|e| format!("Failed to serialize mcps.json: {}", e))?;
        let mcps_content = mcps_content.replace("{HOME}", &dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .to_string_lossy());
        fs::write(&mcps_path, mcps_content)
            .map_err(|e| format!("Failed to create mcps.json: {}", e))?;
    }

    // Create empty providers.json (users will add their own providers with API keys)
    let providers_path = vault_path.join("providers.json");
    if !providers_path.exists() {
        let default_providers = serde_json::json!([]);
        fs::write(&providers_path, serde_json::to_string_pretty(&default_providers).unwrap())
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

    // Deploy default agents (e.g., agent-creator)
    deploy_default_agents(vault_path)?;

    Ok(())
}

/// Deploy default agents from templates to the vault
fn deploy_default_agents(vault_path: &PathBuf) -> Result<(), String> {
    use std::fs;

    let agents_dir = vault_path.join("agents");
    let agent_creator_dir = agents_dir.join("agent-creator");

    // Skip if agent-creator already exists
    if agent_creator_dir.exists() {
        tracing::debug!("agent-creator already exists in vault");
        return Ok(());
    }

    // Get the path to the templates directory (relative to crate root)
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|_| ".".to_string());
    let templates_dir = std::path::PathBuf::from(crate_dir)
        .join("templates")
        .join("default-agents")
        .join("agent-creator");
    
    // If templates directory doesn't exist, skip deployment
    if !templates_dir.exists() {
        tracing::warn!("Agent templates directory not found at {:?}, skipping agent deployment", templates_dir);
        return Ok(());
    }

    // Create agent directory
    fs::create_dir_all(&agent_creator_dir)
        .map_err(|e| format!("Failed to create agent directory: {}", e))?;

    // Copy config.yaml
    let config_src = templates_dir.join("config.yaml");
    let config_dst = agent_creator_dir.join("config.yaml");
    fs::copy(&config_src, &config_dst)
        .map_err(|e| format!("Failed to copy config.yaml: {}", e))?;

    // Copy AGENTS.md
    let agents_md_src = templates_dir.join("AGENTS.md");
    let agents_md_dst = agent_creator_dir.join("AGENTS.md");
    fs::copy(&agents_md_src, &agents_md_dst)
        .map_err(|e| format!("Failed to copy AGENTS.md: {}", e))?;

    tracing::info!("Deployed agent-creator to vault");

    Ok(())
}

/// Builtin skills embedded in the binary
/// Each skill has its content embedded at compile time
pub struct BuiltinSkill {
    pub id: &'static str,
    pub skill_md: &'static str,
}

/// List of builtin skills that should be available in every vault
/// The skill content is embedded directly into the binary using include_str!
pub fn get_builtin_skills() -> Vec<BuiltinSkill> {
    vec![
        BuiltinSkill {
            id: "zero-entity-extract",
            skill_md: include_str!("../../../templates/default-skills/zero-entity-extract/SKILL.md"),
        },
        BuiltinSkill {
            id: "zero-agent-creator",
            skill_md: include_str!("../../../templates/default-skills/zero-agent-creator/SKILL.md"),
        },
    ]
}

/// Copy builtin skills from embedded content to the vault's skills directory
pub fn copy_builtin_skills(vault_path: &PathBuf) -> Result<(), String> {
    use std::fs;

    let skills_dir = vault_path.join("skills");
    
    for skill in get_builtin_skills() {
        let target_dir = skills_dir.join(skill.id);
        let target_file = target_dir.join("SKILL.md");

        // Skip if target already exists (don't overwrite user customizations)
        if target_file.exists() {
            tracing::debug!("Builtin skill '{}' already exists in vault, skipping", skill.id);
            continue;
        }

        // Create target directory
        fs::create_dir_all(&target_dir)
            .map_err(|e| format!("Failed to create skill directory: {}", e))?;

        // Write the embedded skill content
        fs::write(&target_file, skill.skill_md)
            .map_err(|e| format!("Failed to write skill file: {}", e))?;
        
        tracing::info!("Installed builtin skill '{}' to vault", skill.id);
    }

    Ok(())
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
