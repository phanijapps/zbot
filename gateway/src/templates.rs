//! Embedded templates for the gateway.

use rust_embed::RustEmbed;

/// Embedded template files.
#[derive(RustEmbed)]
#[folder = "templates/"]
pub struct Templates;

/// Get the default system prompt for agents.
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

    #[test]
    fn test_default_system_prompt() {
        let prompt = default_system_prompt();
        assert!(prompt.contains("Tool Call Guidelines"));
        assert!(prompt.contains("Memory Usage"));
    }
}
