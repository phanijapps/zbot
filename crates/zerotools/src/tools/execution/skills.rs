// ============================================================================
// LOAD SKILL TOOL
// ============================================================================

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use agent_runtime::tools::{Tool, ToolExecError};
use agent_runtime::tools::context::ToolContext as BaseToolContext;
use agent_runtime::tools::error::ToolResult;
use agent_runtime::tools::builtin::FileSystemContext;

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

    async fn execute(&self, ctx: Arc<BaseToolContext>, args: Value) -> ToolResult<Value> {
        let skill_name = args.get("skill")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolExecError::InvalidArguments("Missing 'skill' parameter".to_string()))?;

        // Check if skill is available
        let available_skills = &ctx.available_skills;
        if !available_skills.is_empty() && !available_skills.contains(&skill_name.to_string()) {
            return Err(ToolExecError::NotFound(format!("Skill '{}' not available to this agent", skill_name)));
        }

        let skills_dir = self.fs.skills_dir()
            .ok_or_else(|| ToolExecError::ExecutionFailed("Skills directory not configured".to_string()))?;

        let skill_dir = skills_dir.join(skill_name);
        let skill_file = skill_dir.join("SKILL.md");

        if !skill_file.exists() {
            return Err(ToolExecError::NotFound(format!("Skill file not found: {}", skill_file.to_string_lossy())));
        }

        let content = std::fs::read_to_string(&skill_file)
            .map_err(|e| ToolExecError::ExecutionFailed(format!("Failed to read skill file: {}", e)))?;

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
    fn parse_skill_frontmatter(&self, content: &str) -> ToolResult<(Value, String)> {
        // Simple parser for YAML frontmatter between --- delimiters
        let parts: Vec<&str> = content.splitn(3, "---").collect();

        if parts.len() >= 3 {
            let yaml_content = parts[1].trim();
            let instructions = parts[2].trim().to_string();

            let metadata: Value = serde_yaml::from_str(yaml_content)
                .map_err(|e| ToolExecError::ParseError(format!("Failed to parse skill YAML: {}", e)))?;

            Ok((metadata, instructions))
        } else {
            // No frontmatter, return empty metadata
            Ok((json!({}), content.to_string()))
        }
    }
}
