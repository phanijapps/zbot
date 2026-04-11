// ============================================================================
// LOAD SKILL TOOL
// ============================================================================

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use zero_core::FileSystemContext;
use zero_core::{Result, Tool, ToolContext};

use crate::tools::guards::has_placeholder_specs;

// ============================================================================
// SKILL STATE TYPES
// ============================================================================

/// Entry for a loaded skill in the skill graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillEntry {
    /// Tool call ID when the skill was loaded
    pub tool_call_id: String,
    /// Timestamp when the skill was loaded (millis since epoch)
    pub loaded_at: i64,
    /// Resources (files) loaded within this skill
    pub resources: Vec<ResourceEntry>,
}

/// Entry for a resource file loaded within a skill
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceEntry {
    /// Relative path within the skill directory
    pub path: String,
    /// Tool call ID when this resource was loaded
    pub tool_call_id: String,
}

/// Skill graph: maps skill name -> skill entry
pub type SkillGraph = HashMap<String, SkillEntry>;

// ============================================================================
// SKILL STATE HELPERS
// ============================================================================

/// Track a skill being loaded in the context state
fn track_skill_load(ctx: &Arc<dyn ToolContext>, skill_name: &str, tool_call_id: &str) {
    // Get or create the skill graph
    let mut graph: SkillGraph = ctx
        .get_state("skill:graph")
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    // Add/update this skill entry
    graph.insert(
        skill_name.to_string(),
        SkillEntry {
            tool_call_id: tool_call_id.to_string(),
            loaded_at: chrono::Utc::now().timestamp_millis(),
            resources: vec![],
        },
    );

    ctx.set_state("skill:graph".to_string(), json!(graph));

    // Update the loaded_skills list
    let mut loaded: Vec<String> = ctx
        .get_state("skill:loaded_skills")
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    if !loaded.contains(&skill_name.to_string()) {
        loaded.push(skill_name.to_string());
        ctx.set_state("skill:loaded_skills".to_string(), json!(loaded));
    }
}

/// Track a resource file being loaded within a skill
fn track_resource_load(
    ctx: &Arc<dyn ToolContext>,
    skill_name: &str,
    resource_path: &str,
    tool_call_id: &str,
) {
    let mut graph: SkillGraph = ctx
        .get_state("skill:graph")
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    if let Some(entry) = graph.get_mut(skill_name) {
        // Add resource to existing skill entry
        entry.resources.push(ResourceEntry {
            path: resource_path.to_string(),
            tool_call_id: tool_call_id.to_string(),
        });
        ctx.set_state("skill:graph".to_string(), json!(graph));
    } else {
        // Skill not in graph yet (unusual case - resource loaded before main skill)
        // Create a placeholder entry with just this resource
        graph.insert(
            skill_name.to_string(),
            SkillEntry {
                tool_call_id: String::new(), // Unknown - skill wasn't loaded via load_skill
                loaded_at: chrono::Utc::now().timestamp_millis(),
                resources: vec![ResourceEntry {
                    path: resource_path.to_string(),
                    tool_call_id: tool_call_id.to_string(),
                }],
            },
        );
        ctx.set_state("skill:graph".to_string(), json!(graph));

        // Also add to loaded_skills list
        let mut loaded: Vec<String> = ctx
            .get_state("skill:loaded_skills")
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();

        if !loaded.contains(&skill_name.to_string()) {
            loaded.push(skill_name.to_string());
            ctx.set_state("skill:loaded_skills".to_string(), json!(loaded));
        }
    }
}

// ============================================================================
// LOAD SKILL TOOL
// ============================================================================

/// Tool for loading skills and skill files
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

    /// Parse skill file path
    /// Returns (skill_name, relative_path, is_explicit)
    /// Supports formats:
    /// - "@skill:skill-name/path/to/file" -> ("skill-name", "path/to/file", true)
    /// - "@skill:skill-name" -> ("skill-name", "SKILL.md", true) - loads SKILL.md explicitly
    /// - "@skill:FILENAME.md" -> ("", "FILENAME.md", false) (uses current skill)
    /// - "path/to/file" -> ("", "path/to/file", false) (uses current skill)
    fn parse_skill_path(&self, file_path: &str) -> (String, String, bool) {
        const FILE_EXTENSIONS: &[&str] = &[
            ".md", ".txt", ".json", ".yaml", ".yml", ".toml", ".html", ".css", ".js", ".ts", ".py",
            ".rs", ".pdf", ".png", ".jpg", ".jpeg", ".gif",
        ];

        if file_path.starts_with("@skill:") {
            let path = &file_path[7..]; // Skip "@skill:"
            if path.contains('/') {
                let parts: Vec<&str> = path.splitn(2, '/').collect();
                return (parts[0].to_string(), parts[1].to_string(), true);
            }
            // @skill:skill-name or @skill:FILENAME.md
            // Check if it has a file extension
            let has_file_extension = FILE_EXTENSIONS.iter().any(|ext| path.ends_with(ext));
            if has_file_extension {
                // It's a filename, use current skill
                return (String::new(), path.to_string(), false);
            }
            // It's a skill name, load SKILL.md
            return (path.to_string(), "SKILL.md".to_string(), true);
        }
        // Relative path - will use current skill from context
        (String::new(), file_path.to_string(), false)
    }
}

#[async_trait]
impl Tool for LoadSkillTool {
    fn name(&self) -> &str {
        "load_skill"
    }

    fn description(&self) -> &str {
        "Load a skill or files from a skill's directory. Use 'skill' parameter to load SKILL.md, or 'file' parameter with @skill: prefix (e.g., '@skill:rust-development' loads that skill's SKILL.md, '@skill:rust-development/REFERENCE.md' loads a specific file)."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "skill": {
                    "type": "string",
                    "description": "Name of the skill to load (loads SKILL.md)"
                },
                "file": {
                    "type": "string",
                    "description": "Path to file within skill directory. Use '@skill:' prefix. Examples: '@skill:rust-development' (loads SKILL.md), '@skill:rust-development/REFERENCE.md', '@skill:assets/config.json' (after loading skill)"
                }
            },
            "required": []
        }))
    }

    async fn execute(&self, ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
        if has_placeholder_specs(ctx.as_ref()) {
            return Ok(json!({
                "status": "redirect",
                "message": "Placeholder specs exist in your ward's specs/ folder. Delegate to a planning subagent to fill them first. Skills needed are listed in each spec file."
            }));
        }

        // Check if loading main skill file or specific file
        let has_skill = args.get("skill").and_then(|v| v.as_str()).is_some();
        let has_file = args.get("file").and_then(|v| v.as_str()).is_some();

        if has_skill && !has_file {
            // Load main SKILL.md
            self.load_main_skill(ctx, args).await
        } else if has_file {
            // Load specific file from skill directory
            self.load_skill_file(ctx, args).await
        } else {
            Err(zero_core::ZeroError::Tool(
                "Either 'skill' or 'file' parameter must be provided".to_string(),
            ))
        }
    }
}

impl LoadSkillTool {
    /// Load the main SKILL.md file for a skill
    async fn load_main_skill(&self, ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
        let skill_name = args
            .get("skill")
            .and_then(|v| v.as_str())
            .ok_or_else(|| zero_core::ZeroError::Tool("Missing 'skill' parameter".to_string()))?;

        let skills_dir = self.fs.skills_dir().ok_or_else(|| {
            zero_core::ZeroError::Tool("Skills directory not configured".to_string())
        })?;

        let skill_dir = skills_dir.join(skill_name);
        let skill_file = skill_dir.join("SKILL.md");

        if !skill_file.exists() {
            return Err(zero_core::ZeroError::Tool(format!(
                "Skill file not found: {}",
                skill_file.to_string_lossy()
            )));
        }

        let content = std::fs::read_to_string(&skill_file)
            .map_err(|e| zero_core::ZeroError::Tool(format!("Failed to read skill file: {}", e)))?;

        // Parse YAML frontmatter
        let (metadata, instructions) = self.parse_skill_frontmatter(&content)?;

        // Store current skill in state for subsequent convenience file loads
        ctx.set_state("skill:current_skill".to_string(), json!(skill_name));

        // Track this skill load in the skill graph
        let tool_call_id = ctx.function_call_id();
        track_skill_load(&ctx, skill_name, &tool_call_id);

        // List available resource files in the skill directory
        let resources = list_skill_resources(&skill_dir, skill_name);

        Ok(json!({
            "name": skill_name,
            "metadata": metadata,
            "instructions": instructions,
            "resources": resources,
        }))
    }

    /// Load a specific file from a skill's directory
    async fn load_skill_file(&self, ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
        let file_path = args
            .get("file")
            .and_then(|v| v.as_str())
            .ok_or_else(|| zero_core::ZeroError::Tool("Missing 'file' parameter".to_string()))?;

        let skills_dir = self.fs.skills_dir().ok_or_else(|| {
            zero_core::ZeroError::Tool("Skills directory not configured".to_string())
        })?;

        // Parse path and get skill name
        let (skill_name_from_path, relative_path, is_explicit) = self.parse_skill_path(file_path);

        // Get skill name - use explicit skill from path, or fall back to context
        let skill_name = if !skill_name_from_path.is_empty() {
            skill_name_from_path
        } else {
            ctx.get_state("skill:current_skill")
                .ok_or_else(|| zero_core::ZeroError::Tool(
                    "No skill context. Either use @skill:skill-name/path format or load a skill first using the 'skill' parameter.".to_string()
                ))?
                .as_str()
                .ok_or_else(|| zero_core::ZeroError::Tool("Invalid skill state".to_string()))?
                .to_string()
        };

        let skill_dir = skills_dir.join(&skill_name);
        let full_path = skill_dir.join(&relative_path);

        // Security: Ensure path doesn't escape skill directory
        if !full_path.starts_with(&skill_dir) {
            return Err(zero_core::ZeroError::Tool(
                "Invalid path: cannot access files outside skill directory".to_string(),
            ));
        }

        if !full_path.exists() {
            return Err(zero_core::ZeroError::Tool(format!(
                "Skill file not found: {} (searched in skill: {})",
                relative_path, skill_name
            )));
        }

        // Check for binary file
        if is_binary_file(&relative_path) {
            return Ok(json!({
                "skill": skill_name,
                "path": relative_path,
                "content": null,
                "is_binary": true,
                "message": "Binary file - content not displayed"
            }));
        }

        // Read file content
        let content = std::fs::read_to_string(&full_path)
            .map_err(|e| zero_core::ZeroError::Tool(format!("Failed to read skill file: {}", e)))?;

        // Get the tool call ID for tracking
        let tool_call_id = ctx.function_call_id();

        // Update current skill in state if we explicitly loaded a different skill's SKILL.md
        if is_explicit && relative_path == "SKILL.md" {
            ctx.set_state("skill:current_skill".to_string(), json!(skill_name));
            // Track as a skill load
            track_skill_load(&ctx, &skill_name, &tool_call_id);
        } else {
            // Track as a resource load under the parent skill
            track_resource_load(&ctx, &skill_name, &relative_path, &tool_call_id);
        }

        Ok(json!({
            "skill": skill_name,
            "path": relative_path,
            "content": content,
            "is_binary": false
        }))
    }

    fn parse_skill_frontmatter(&self, content: &str) -> Result<(Value, String)> {
        // Simple parser for YAML frontmatter between --- delimiters
        let parts: Vec<&str> = content.splitn(3, "---").collect();

        if parts.len() >= 3 {
            let yaml_content = parts[1].trim();
            let instructions = parts[2].trim().to_string();

            let metadata: Value = serde_yaml::from_str(yaml_content).map_err(|e| {
                zero_core::ZeroError::Tool(format!("Failed to parse skill YAML: {}", e))
            })?;

            Ok((metadata, instructions))
        } else {
            // No frontmatter, return empty metadata
            Ok((json!({}), content.to_string()))
        }
    }
}

/// List resource files in a skill directory (excluding SKILL.md).
///
/// Returns a list of objects with filename and the load_skill command to use.
fn list_skill_resources(skill_dir: &std::path::Path, skill_name: &str) -> Vec<Value> {
    let mut resources = Vec::new();
    if let Ok(entries) = std::fs::read_dir(skill_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    // Skip SKILL.md itself
                    if name.eq_ignore_ascii_case("SKILL.md") {
                        continue;
                    }
                    resources.push(json!({
                        "file": name,
                        "load_with": format!("load_skill(file=\"{}\")", name),
                    }));
                }
            }
        }
    }
    // Also check subdirectories one level deep
    if let Ok(entries) = std::fs::read_dir(skill_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Ok(sub_entries) = std::fs::read_dir(&path) {
                    for sub_entry in sub_entries.flatten() {
                        let sub_path = sub_entry.path();
                        if sub_path.is_file() {
                            if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
                                if let Some(file_name) =
                                    sub_path.file_name().and_then(|n| n.to_str())
                                {
                                    let rel_path = format!("{}/{}", dir_name, file_name);
                                    resources.push(json!({
                                        "file": rel_path.clone(),
                                        "load_with": format!("load_skill(file=\"@skill:{}/{}\")", skill_name, rel_path),
                                    }));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    resources
}

/// Check if a file is binary based on its extension
fn is_binary_file(filename: &str) -> bool {
    const BINARY_EXTENSIONS: &[&str] = &[
        "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "zip", "tar", "gz", "rar", "7z", "exe",
        "dll", "so", "dylib", "png", "jpg", "jpeg", "gif", "bmp", "ico", "webp", "mp3", "mp4",
        "wav", "avi", "mov", "mkv", "ttf", "otf", "woff", "woff2",
    ];

    if let Some(ext) = filename.rsplit('.').next() {
        BINARY_EXTENSIONS.contains(&ext.to_lowercase().as_str())
    } else {
        false
    }
}
