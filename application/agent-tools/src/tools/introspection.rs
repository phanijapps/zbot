// ============================================================================
// INTROSPECTION TOOLS
// Tools for the agent to query its own capabilities
// ============================================================================

use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;
use zero_core::{FileSystemContext, Tool, ToolContext, Result};

// ============================================================================
// LIST SKILLS TOOL
// ============================================================================

/// Tool for listing available skills
///
/// This tool reads from cached skill data in the context state when available,
/// falling back to reading from disk if no cache is present.
pub struct ListSkillsTool {
    fs: Arc<dyn FileSystemContext>,
}

impl ListSkillsTool {
    pub fn new(fs: Arc<dyn FileSystemContext>) -> Self {
        Self { fs }
    }
}

#[async_trait]
impl Tool for ListSkillsTool {
    fn name(&self) -> &str {
        "list_skills"
    }

    fn description(&self) -> &str {
        "List all available skills that can be loaded with load_skill. Returns skill names and their descriptions."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {},
            "required": []
        }))
    }

    async fn execute(&self, ctx: Arc<dyn ToolContext>, _args: Value) -> Result<Value> {
        // Try to read cached skill list from context state first
        if let Some(cached_skills) = ctx.get_state("available_skills") {
            if let Some(skills_array) = cached_skills.as_array() {
                return Ok(json!({
                    "skills": skills_array,
                    "count": skills_array.len(),
                    "usage": "Use load_skill with the skill name to load a skill's instructions"
                }));
            }
        }

        // Fall back to reading from disk if no cache
        let skills_dir = match self.fs.skills_dir() {
            Some(dir) => dir,
            None => return Ok(json!({
                "error": "Skills directory not configured",
                "skills": []
            })),
        };

        if !skills_dir.exists() {
            return Ok(json!({
                "skills": [],
                "message": "No skills directory found"
            }));
        }

        let mut skills = Vec::new();

        // Read skills directory
        if let Ok(entries) = std::fs::read_dir(&skills_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let skill_name = path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string();

                    // Try to read SKILL.md for description
                    let skill_file = path.join("SKILL.md");
                    let description = if skill_file.exists() {
                        std::fs::read_to_string(&skill_file)
                            .ok()
                            .and_then(|content| extract_skill_description(&content))
                            .unwrap_or_else(|| "No description".to_string())
                    } else {
                        "No SKILL.md found".to_string()
                    };

                    skills.push(json!({
                        "name": skill_name,
                        "description": description,
                    }));
                }
            }
        }

        Ok(json!({
            "skills": skills,
            "count": skills.len(),
            "usage": "Use load_skill with the skill name to load a skill's instructions"
        }))
    }
}

/// Extract description from SKILL.md frontmatter or first paragraph
fn extract_skill_description(content: &str) -> Option<String> {
    // Try to extract from frontmatter
    if content.starts_with("---") {
        let parts: Vec<&str> = content.splitn(3, "---").collect();
        if parts.len() >= 2 {
            let frontmatter = parts[1];
            for line in frontmatter.lines() {
                if line.starts_with("description:") {
                    return Some(line.trim_start_matches("description:").trim().trim_matches('"').to_string());
                }
            }
        }
    }

    // Fall back to first non-empty, non-heading line
    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() && !trimmed.starts_with('#') && !trimmed.starts_with("---") {
            return Some(trimmed.chars().take(200).collect());
        }
    }

    None
}

// ============================================================================
// LIST TOOLS TOOL
// ============================================================================

/// Tool for listing available tools
///
/// This tool reads tool names from context state where they should be stored
/// by the executor when tools are registered.
pub struct ListToolsTool;

impl ListToolsTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ListToolsTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ListToolsTool {
    fn name(&self) -> &str {
        "list_tools"
    }

    fn description(&self) -> &str {
        "List all available tools that you can use. Returns tool names and descriptions."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {},
            "required": []
        }))
    }

    async fn execute(&self, ctx: Arc<dyn ToolContext>, _args: Value) -> Result<Value> {
        // Try to get tool list from context state
        let tools = ctx.get_state("app:available_tools")
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default();

        if tools.is_empty() {
            // Return a helpful message with common tools
            return Ok(json!({
                "tools": [
                    {"name": "read", "description": "Read file contents"},
                    {"name": "write", "description": "Write content to a file"},
                    {"name": "edit", "description": "Edit a file with search/replace"},
                    {"name": "grep", "description": "Search for patterns in files"},
                    {"name": "glob", "description": "Find files matching a pattern"},
                    {"name": "shell", "description": "Execute shell commands"},
                    {"name": "python", "description": "Execute Python code"},
                    {"name": "load_skill", "description": "Load a skill's instructions"},
                    {"name": "list_skills", "description": "List available skills"},
                    {"name": "list_tools", "description": "List available tools"},
                    {"name": "list_mcps", "description": "List available MCP servers"},
                    {"name": "memory", "description": "Store and retrieve memories"},
                    {"name": "todo", "description": "Manage TODO items"},
                    {"name": "web_fetch", "description": "Fetch content from URLs"},
                    {"name": "request_input", "description": "Request input from user"},
                    {"name": "show_content", "description": "Display content to user"}
                ],
                "note": "This is a default list. Some tools may not be available."
            }));
        }

        Ok(json!({
            "tools": tools,
            "count": tools.len()
        }))
    }
}

// ============================================================================
// LIST MCPS TOOL
// ============================================================================

/// Tool for listing available MCP servers
pub struct ListMcpsTool {
    fs: Arc<dyn FileSystemContext>,
}

impl ListMcpsTool {
    pub fn new(fs: Arc<dyn FileSystemContext>) -> Self {
        Self { fs }
    }
}

#[async_trait]
impl Tool for ListMcpsTool {
    fn name(&self) -> &str {
        "list_mcps"
    }

    fn description(&self) -> &str {
        "List all configured MCP (Model Context Protocol) servers. These provide additional tools and capabilities."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {},
            "required": []
        }))
    }

    async fn execute(&self, ctx: Arc<dyn ToolContext>, _args: Value) -> Result<Value> {
        // First try to get from context state (if executor populates it)
        if let Some(mcps) = ctx.get_state("app:available_mcps") {
            return Ok(json!({
                "mcps": mcps,
                "source": "runtime"
            }));
        }

        // Fall back to reading mcps.json from vault
        let vault_path = match self.fs.vault_path() {
            Some(p) => p,
            None => return Ok(json!({
                "mcps": [],
                "message": "Vault path not configured"
            })),
        };

        let mcps_file = vault_path.join("mcps.json");
        if !mcps_file.exists() {
            return Ok(json!({
                "mcps": [],
                "message": "No MCP servers configured. Add them in Settings > MCP Servers."
            }));
        }

        let content = match std::fs::read_to_string(&mcps_file) {
            Ok(c) => c,
            Err(e) => return Ok(json!({
                "mcps": [],
                "error": format!("Failed to read mcps.json: {}", e)
            })),
        };

        let mcps: Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(e) => return Ok(json!({
                "mcps": [],
                "error": format!("Failed to parse mcps.json: {}", e)
            })),
        };

        // Extract just the useful info
        let mcp_list: Vec<Value> = mcps.as_array()
            .map(|arr| {
                arr.iter().map(|mcp| {
                    json!({
                        "id": mcp.get("id").and_then(|v| v.as_str()).unwrap_or("unknown"),
                        "name": mcp.get("name").and_then(|v| v.as_str()).unwrap_or("unknown"),
                        "type": mcp.get("type").and_then(|v| v.as_str()).unwrap_or("unknown"),
                        "enabled": mcp.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false),
                        "description": mcp.get("description").and_then(|v| v.as_str())
                    })
                }).collect()
            })
            .unwrap_or_default();

        Ok(json!({
            "mcps": mcp_list,
            "count": mcp_list.len(),
            "source": "config"
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_skill_description_frontmatter() {
        let content = r#"---
name: test-skill
description: "This is a test skill"
---

# Test Skill

Some content here.
"#;
        let desc = extract_skill_description(content);
        assert_eq!(desc, Some("This is a test skill".to_string()));
    }

    #[test]
    fn test_extract_skill_description_no_frontmatter() {
        let content = r#"# Test Skill

This is a test skill that does things.
"#;
        let desc = extract_skill_description(content);
        assert_eq!(desc, Some("This is a test skill that does things.".to_string()));
    }
}
