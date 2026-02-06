//! # Gateway Templates
//!
//! Embedded templates and system prompt assembly for AgentZero agents.
//!
//! Provides:
//! - `Templates` struct with embedded template files
//! - `load_system_prompt()` for loading and assembling the agent system prompt
//! - Shard injection (safety, tooling, memory) into custom instructions

use rust_embed::RustEmbed;
use std::path::Path;

/// Embedded template files.
#[derive(RustEmbed)]
#[folder = "../templates/"]
pub struct Templates;

/// Shards that are always appended to custom instructions.
/// These provide core functionality documentation that users shouldn't have to maintain.
const REQUIRED_SHARDS: &[&str] = &["safety", "tooling_skills", "memory_learning"];

/// Load system prompt from filesystem, creating starter if missing.
///
/// Behavior:
/// 1. If `INSTRUCTIONS.md` doesn't exist, creates it from starter template
/// 2. Loads `INSTRUCTIONS.md` from data directory
/// 3. Appends required shards (memory, tools, etc.) automatically
///
/// Falls back to embedded default only if file operations fail.
pub fn load_system_prompt(data_dir: &Path) -> String {
    let instructions_path = data_dir.join("INSTRUCTIONS.md");

    // Create starter INSTRUCTIONS.md if it doesn't exist
    if !instructions_path.exists() {
        if let Err(e) = create_starter_instructions(&instructions_path) {
            tracing::warn!(
                "Failed to create starter INSTRUCTIONS.md: {}, using embedded default",
                e
            );
            return default_system_prompt();
        }
    }

    // Load from filesystem
    match std::fs::read_to_string(&instructions_path) {
        Ok(content) if !content.trim().is_empty() => {
            tracing::info!("Loaded system prompt from {:?}", instructions_path);
            // Append required shards to custom instructions
            append_shards(content, data_dir)
        }
        Ok(_) => {
            tracing::warn!("INSTRUCTIONS.md is empty, using embedded default");
            default_system_prompt()
        }
        Err(e) => {
            tracing::warn!(
                "Failed to read INSTRUCTIONS.md: {}, using embedded default",
                e
            );
            default_system_prompt()
        }
    }
}

/// Create starter INSTRUCTIONS.md from embedded template.
fn create_starter_instructions(path: &Path) -> std::io::Result<()> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let starter = Templates::get("instructions_starter.md")
        .map(|file| String::from_utf8_lossy(&file.data).to_string())
        .unwrap_or_else(|| default_system_prompt());

    std::fs::write(path, starter)?;
    tracing::info!("Created starter INSTRUCTIONS.md at {:?}", path);
    Ok(())
}

/// Append required shards and environment info to custom instructions.
fn append_shards(mut content: String, data_dir: &Path) -> String {
    content.push_str("\n\n# --- SYSTEM INJECTED ---\n\n");

    // Add environment info first
    content.push_str(&environment_section(data_dir));
    content.push_str("\n\n");

    // Add shards
    let shards = load_required_shards();
    if !shards.is_empty() {
        content.push_str(&shards);
    }
    content
}

/// Generate environment section with OS and runtime info.
fn environment_section(data_dir: &Path) -> String {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    let shell_hint = match os {
        "windows" => "Use PowerShell or cmd syntax (dir, type, copy, etc.)",
        "macos" => "Use Unix shell syntax (ls, cat, cp, etc.)",
        "linux" => "Use Unix shell syntax (ls, cat, cp, etc.)",
        _ => "Detect shell syntax from context",
    };

    let mut lines = vec![
        "ENVIRONMENT".to_string(),
        format!("- OS: {} ({})", os, arch),
        format!("- Shell: {}", shell_hint),
        format!("- Vault: {}", data_dir.display()),
    ];

    // Python venv - always show canonical location with status
    let venv_dir = data_dir.join("venv");
    let python_path = if cfg!(windows) {
        venv_dir.join("Scripts").join("python.exe")
    } else {
        venv_dir.join("bin").join("python")
    };
    let pip_path = if cfg!(windows) {
        venv_dir.join("Scripts").join("pip.exe")
    } else {
        venv_dir.join("bin").join("pip")
    };
    if python_path.exists() {
        lines.push(format!("- Python: {} (ready)", python_path.display()));
        lines.push(format!("- Pip: {}", pip_path.display()));
    } else {
        lines.push(format!("- Python: {} (not configured)", python_path.display()));
    }

    // Node environment - always show canonical location with status
    let node_env_dir = data_dir.join("node_env");
    let node_modules = node_env_dir.join("node_modules");
    if node_modules.exists() {
        lines.push(format!("- NodeModules: {} (ready)", node_modules.display()));
        lines.push("- Node: NODE_PATH is auto-set to NodeModules; `npm install <pkg>` installs there".to_string());
    } else {
        lines.push(format!("- NodeModules: {} (not configured)", node_modules.display()));
    }

    // Working directory info
    let code_dir = data_dir.join("code");
    lines.push(format!("- CodeDir: {} (shell commands run in CodeDir/{{session_id}}/)", code_dir.display()));
    lines.push("- Attachments: use the write tool to save output files (images, docs) to agent_data".to_string());

    lines.join("\n")
}

/// Load all required shards from embedded templates.
fn load_required_shards() -> String {
    REQUIRED_SHARDS
        .iter()
        .filter_map(|name| load_shard(name))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Load a single shard by name from embedded templates.
fn load_shard(name: &str) -> Option<String> {
    let path = format!("shards/{}.md", name);
    Templates::get(&path).map(|file| String::from_utf8_lossy(&file.data).to_string())
}

/// Get the embedded default system prompt for agents.
///
/// This is the fallback when no filesystem override exists.
pub fn default_system_prompt() -> String {
    Templates::get("system_prompt.md")
        .map(|file| String::from_utf8_lossy(&file.data).to_string())
        .unwrap_or_else(|| {
            // Fallback if template not found
            "You are a helpful AI assistant.".to_string()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default_system_prompt_contains_expected_content() {
        let prompt = default_system_prompt();
        assert!(prompt.contains("Jaffa"));
        assert!(prompt.contains("SAFETY & PERMISSIONS"));
    }

    #[test]
    fn test_load_system_prompt_from_filesystem_appends_shards() {
        let dir = TempDir::new().unwrap();
        let instructions_path = dir.path().join("INSTRUCTIONS.md");
        std::fs::write(&instructions_path, "Custom system prompt content").unwrap();

        let prompt = load_system_prompt(dir.path());

        // Should contain custom content
        assert!(prompt.contains("Custom system prompt content"));

        // Should have injected shards
        assert!(prompt.contains("# --- SYSTEM INJECTED ---"));
        assert!(prompt.contains("MEMORY & LEARNING"));
    }

    #[test]
    fn test_load_system_prompt_creates_starter_when_missing() {
        let dir = TempDir::new().unwrap();
        let instructions_path = dir.path().join("INSTRUCTIONS.md");

        // File should not exist initially
        assert!(!instructions_path.exists());

        let prompt = load_system_prompt(dir.path());

        // File should now exist
        assert!(instructions_path.exists());

        // Should contain Jaffa content with injected shards
        assert!(prompt.contains("Jaffa"));
        assert!(prompt.contains("MEMORY & LEARNING"));
        assert!(prompt.contains("# --- SYSTEM INJECTED ---"));
    }

    #[test]
    fn test_load_system_prompt_falls_back_when_empty() {
        let dir = TempDir::new().unwrap();
        let instructions_path = dir.path().join("INSTRUCTIONS.md");
        std::fs::write(&instructions_path, "   \n  ").unwrap(); // whitespace only

        let prompt = load_system_prompt(dir.path());
        // Should return embedded default, not empty string
        assert!(!prompt.trim().is_empty());
        assert!(prompt.contains("Jaffa"));
    }

    #[test]
    fn test_load_shard_returns_content() {
        let shard = load_shard("memory_learning");
        assert!(shard.is_some());
        assert!(shard.unwrap().contains("MEMORY & LEARNING"));
    }

    #[test]
    fn test_load_tooling_skills_shard() {
        let shard = load_shard("tooling_skills");
        assert!(shard.is_some());
        let content = shard.unwrap();
        assert!(content.contains("TOOLING & SKILLS"));
        assert!(content.contains("list_skills"));
        assert!(content.contains("delegate_to_agent"));
    }

    #[test]
    fn test_load_shard_returns_none_for_missing() {
        let shard = load_shard("nonexistent_shard");
        assert!(shard.is_none());
    }

    #[test]
    fn test_append_shards_adds_separator_and_environment() {
        let dir = TempDir::new().unwrap();
        let content = "My custom instructions".to_string();
        let result = append_shards(content, dir.path());

        assert!(result.starts_with("My custom instructions"));
        assert!(result.contains("# --- SYSTEM INJECTED ---"));
        assert!(result.contains("ENVIRONMENT"));
        assert!(result.contains("OS:"));
        assert!(result.contains("Shell:"));
        assert!(result.contains("Vault:"));
    }

    #[test]
    fn test_create_starter_instructions_creates_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("INSTRUCTIONS.md");

        let result = create_starter_instructions(&path);
        assert!(result.is_ok());
        assert!(path.exists());

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("Jaffa"));
        // Starter should NOT contain MEMORY & LEARNING (that's injected)
        assert!(!content.contains("MEMORY & LEARNING"));
    }

    #[test]
    fn test_create_starter_instructions_creates_parent_dirs() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nested").join("dir").join("INSTRUCTIONS.md");

        let result = create_starter_instructions(&path);
        assert!(result.is_ok());
        assert!(path.exists());
    }
}
