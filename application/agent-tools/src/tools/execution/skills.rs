// ============================================================================
// LOAD SKILL TOOL
// ============================================================================

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use zero_core::{Tool, ToolContext, Result};
use zero_core::FileSystemContext;

// ============================================================================
// LOAD SKILL TOOL
// ============================================================================

/// Tool for loading skills
pub struct LoadSkillTool {
    /// File system context
    fs: Arc<dyn FileSystemContext>,
}

impl LoadSkillTool {
    /// Create a new load skill tool with file system context
    #[must_use]
    pub fn new(fs: Arc<dyn FileSystemContext>) -> Self {
        Self { fs }
    }
}

#[async_trait]
impl Tool for LoadSkillTool {
    fn name(&self) -> &str {
        "load_skill"
    }

    fn description(&self) -> &str {
        "Load a skill from the skill registry."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "skill": {
                    "type": "string",
                    "description": "Name of the skill to load"
                }
            },
            "required": ["skill"]
        }))
    }

    async fn execute(&self, _ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
        let skill_name = args.get("skill")
            .and_then(|v| v.as_str())
            .ok_or_else(|| zero_core::ZeroError::Tool("Missing 'skill' parameter".to_string()))?;

        // Note: available_skills checking removed since ToolContext doesn't expose it
        // in the new trait. This should be handled at the tool registry level.

        let skills_dir = self.fs.skills_dir()
            .ok_or_else(|| zero_core::ZeroError::Tool("Skills directory not configured".to_string()))?;

        let skill_dir = skills_dir.join(skill_name);
        let skill_file = skill_dir.join("SKILL.md");

        if !skill_file.exists() {
            return Err(zero_core::ZeroError::Tool(format!("Skill file not found: {}", skill_file.to_string_lossy())));
        }

        let content = std::fs::read_to_string(&skill_file)
            .map_err(|e| zero_core::ZeroError::Tool(format!("Failed to read skill file: {}", e)))?;

        // Parse YAML frontmatter
        let (metadata, instructions) = self.parse_skill_frontmatter(&content)?;

        Ok(json!({
            "name": skill_name,
            "metadata": metadata,
            "instructions": instructions,
        }))
    }
}

impl LoadSkillTool {
    fn parse_skill_frontmatter(&self, content: &str) -> Result<(Value, String)> {
        // Simple parser for YAML frontmatter between --- delimiters
        let parts: Vec<&str> = content.splitn(3, "---").collect();

        if parts.len() >= 3 {
            let yaml_content = parts[1].trim();
            let instructions = parts[2].trim().to_string();

            let metadata: Value = serde_yaml::from_str(yaml_content)
                .map_err(|e| zero_core::ZeroError::Tool(format!("Failed to parse skill YAML: {}", e)))?;

            Ok((metadata, instructions))
        } else {
            // No frontmatter, return empty metadata
            Ok((json!({}), content.to_string()))
        }
    }
}
