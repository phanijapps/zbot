//! Embedded templates for the gateway.

use rust_embed::RustEmbed;
use std::path::Path;

/// Embedded template files.
#[derive(RustEmbed)]
#[folder = "templates/"]
pub struct Templates;

/// Shards that are always appended to custom instructions.
/// These provide core functionality documentation that users shouldn't have to maintain.
const REQUIRED_SHARDS: &[&str] = &["memory_learning"];

/// Load system prompt from filesystem, falling back to embedded.
///
/// When loading from filesystem:
/// - Loads custom `INSTRUCTIONS.md` from data directory
/// - Appends required shards (memory, tools, etc.) automatically
///
/// When no custom file exists:
/// - Returns the full embedded default template
pub fn load_system_prompt(data_dir: &Path) -> String {
    let instructions_path = data_dir.join("INSTRUCTIONS.md");

    if instructions_path.exists() {
        match std::fs::read_to_string(&instructions_path) {
            Ok(content) if !content.trim().is_empty() => {
                tracing::info!("Loaded system prompt from {:?}", instructions_path);
                // Append required shards to custom instructions
                return append_shards(content);
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

/// Append required shards to custom instructions.
fn append_shards(mut content: String) -> String {
    let shards = load_required_shards();
    if !shards.is_empty() {
        content.push_str("\n\n# --- SYSTEM INJECTED ---\n\n");
        content.push_str(&shards);
    }
    content
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

    #[test]
    fn test_load_shard_returns_content() {
        let shard = load_shard("memory_learning");
        assert!(shard.is_some());
        assert!(shard.unwrap().contains("MEMORY & LEARNING"));
    }

    #[test]
    fn test_load_shard_returns_none_for_missing() {
        let shard = load_shard("nonexistent_shard");
        assert!(shard.is_none());
    }

    #[test]
    fn test_append_shards_adds_separator() {
        let content = "My custom instructions".to_string();
        let result = append_shards(content);

        assert!(result.starts_with("My custom instructions"));
        assert!(result.contains("# --- SYSTEM INJECTED ---"));
    }
}
