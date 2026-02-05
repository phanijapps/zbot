//! Embedded templates for the gateway.

use rust_embed::RustEmbed;
use std::path::Path;

/// Embedded template files.
#[derive(RustEmbed)]
#[folder = "templates/"]
pub struct Templates;

/// Load system prompt from filesystem, falling back to embedded.
///
/// Checks for `INSTRUCTIONS.md` in the data directory first.
/// If not found or empty, returns the embedded default template.
pub fn load_system_prompt(data_dir: &Path) -> String {
    let instructions_path = data_dir.join("INSTRUCTIONS.md");

    if instructions_path.exists() {
        match std::fs::read_to_string(&instructions_path) {
            Ok(content) if !content.trim().is_empty() => {
                tracing::info!("Loaded system prompt from {:?}", instructions_path);
                return content;
            }
            Ok(_) => {
                tracing::warn!("INSTRUCTIONS.md is empty, using embedded default");
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to read INSTRUCTIONS.md: {}, using embedded default",
                    e
                );
            }
        }
    }

    default_system_prompt()
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
    fn test_load_system_prompt_from_filesystem() {
        let dir = TempDir::new().unwrap();
        let instructions_path = dir.path().join("INSTRUCTIONS.md");
        std::fs::write(&instructions_path, "Custom system prompt content").unwrap();

        let prompt = load_system_prompt(dir.path());
        assert_eq!(prompt, "Custom system prompt content");
    }

    #[test]
    fn test_load_system_prompt_falls_back_when_missing() {
        let dir = TempDir::new().unwrap();
        // No INSTRUCTIONS.md file

        let prompt = load_system_prompt(dir.path());
        // Should return embedded default
        assert!(!prompt.is_empty());
        assert!(prompt.contains("Jaffa"));
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
}
