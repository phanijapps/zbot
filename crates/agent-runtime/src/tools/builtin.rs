// ============================================================================
// BUILT-IN TOOLS
// Default tools provided by the framework
// ============================================================================

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use regex::Regex;

use super::super::tools::{Tool, ToolExecError};
use super::super::tools::context::ToolContext as BaseToolContext;
use super::super::tools::error::ToolResult;


// ============================================================================
// FILE SYSTEM CONTEXT TRAIT
// ============================================================================

/// Trait for providing file system context to tools
///
/// This allows the framework to be used with different directory structures
/// without depending on application-specific code like AppDirs.
pub trait FileSystemContext: Send + Sync {
    /// Get the conversation directory for a given conversation ID
    fn conversation_dir(&self, conversation_id: &str) -> Option<PathBuf>;

    /// Get the outputs directory
    fn outputs_dir(&self) -> Option<PathBuf>;

    /// Get the skills directory
    fn skills_dir(&self) -> Option<PathBuf>;

    /// Get the Python executable path
    fn python_executable(&self) -> Option<PathBuf>;
}

/// Default file system context that returns None for all paths
/// (for library-only usage without application integration)
#[derive(Debug, Clone, Default)]
pub struct NoFileSystemContext;

impl FileSystemContext for NoFileSystemContext {
    fn conversation_dir(&self, _conversation_id: &str) -> Option<PathBuf> {
        None
    }

    fn outputs_dir(&self) -> Option<PathBuf> {
        None
    }

    fn skills_dir(&self) -> Option<PathBuf> {
        None
    }

    fn python_executable(&self) -> Option<PathBuf> {
        None
    }
}

// ============================================================================
// TOOL CONTEXT WITH FILE SYSTEM
// ============================================================================

/// Extended tool context with file system access
pub struct ToolContextWithFs {
    /// Base tool context
    pub base: BaseToolContext,

    /// File system context
    pub fs: Arc<dyn FileSystemContext>,
}

impl ToolContextWithFs {
    /// Create a new tool context with file system
    #[must_use]
    pub fn new(fs: Arc<dyn FileSystemContext>) -> Self {
        Self {
            base: BaseToolContext::new(),
            fs,
        }
    }

    /// Create with conversation ID
    #[must_use]
    pub fn with_conversation(fs: Arc<dyn FileSystemContext>, conversation_id: String) -> Self {
        Self {
            base: BaseToolContext::with_conversation(conversation_id),
            fs,
        }
    }
}

// ============================================================================
// READ TOOL
// ============================================================================

/// Tool for reading file contents
pub struct ReadTool;

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str {
        "read"
    }

    fn description(&self) -> &str {
        "Read contents of a file. Supports optional offset and limit for line-by-line reading."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to read"
                },
                "offset": {
                    "type": "integer",
                    "description": "Starting line number (0-indexed)",
                    "default": 0
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of lines to read"
                }
            },
            "required": ["path"]
        }))
    }

    async fn execute(&self, _ctx: Arc<BaseToolContext>, args: Value) -> ToolResult<Value> {
        let path = args.get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolExecError::InvalidArguments("Missing 'path' parameter".to_string()))?;

        let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let limit = args.get("limit").and_then(|v| v.as_u64());

        tracing::debug!("Reading file: {} (offset: {}, limit: {:?})", path, offset, limit);

        let content = std::fs::read_to_string(path)
            .map_err(|e| ToolExecError::ExecutionFailed(format!("Failed to read file: {}", e)))?;

        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        let start = offset.min(total_lines);
        let end = if let Some(lim) = limit {
            (start + lim as usize).min(total_lines)
        } else {
            total_lines
        };

        let selected_lines = lines[start..end].join("\n");

        Ok(json!({
            "content": selected_lines,
            "total_lines": total_lines,
            "lines_read": end - start,
            "offset": start,
        }))
    }
}

// ============================================================================
// WRITE TOOL
// ============================================================================

/// Tool for writing content to files
pub struct WriteTool {
    /// File system context
    fs: Arc<dyn FileSystemContext>,
}

impl WriteTool {
    /// Create a new write tool with file system context
    #[must_use]
    pub fn new(fs: Arc<dyn FileSystemContext>) -> Self {
        Self { fs }
    }
}

#[async_trait]
impl Tool for WriteTool {
    fn name(&self) -> &str {
        "write"
    }

    fn description(&self) -> &str {
        "Write content to a file. Creates parent directories automatically. Use 'outputs/' prefix for files that should be accessible via browser."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to write. Use 'outputs/' prefix for browser-accessible files."
                },
                "content": {
                    "type": "string",
                    "description": "Content to write to the file"
                }
            },
            "required": ["path", "content"]
        }))
    }

    async fn execute(&self, ctx: Arc<BaseToolContext>, args: Value) -> ToolResult<Value> {
        let path = args.get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolExecError::InvalidArguments("Missing 'path' parameter".to_string()))?;

        let content = args.get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolExecError::InvalidArguments("Missing 'content' parameter".to_string()))?;

        tracing::debug!("Writing file: {} ({} bytes)", path, content.len());

        // Handle outputs/ prefix
        let final_path = if path.starts_with("outputs/") {
            if let Some(outputs_dir) = self.fs.outputs_dir() {
                outputs_dir.join(&path[8..])
            } else {
                PathBuf::from(path)
            }
        } else {
            // Check if we're in a conversation context
            if let Some(conv_dir) = ctx.conversation_dir() {
                conv_dir.join(path)
            } else {
                PathBuf::from(path)
            }
        };

        // Create parent directories
        if let Some(parent) = final_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ToolExecError::ExecutionFailed(format!("Failed to create directories: {}", e)))?;
        }

        // Write the file
        std::fs::write(&final_path, content)
            .map_err(|e| ToolExecError::ExecutionFailed(format!("Failed to write file: {}", e)))?;

        // Set permissions for output files
        if path.starts_with("outputs/") {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(&final_path)?.permissions();
                perms.set_mode(0o644);
                std::fs::set_permissions(&final_path, perms)?;
            }
        }

        Ok(json!({
            "path": final_path.to_string_lossy(),
            "bytes_written": content.len(),
        }))
    }
}

// ============================================================================
// EDIT TOOL
// ============================================================================

/// Tool for editing files with search and replace
pub struct EditTool;

#[async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str {
        "edit"
    }

    fn description(&self) -> &str {
        "Edit a file by performing search and replace operations."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to edit"
                },
                "replacements": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "old": {"type": "string"},
                            "new": {"type": "string"}
                        },
                        "required": ["old", "new"]
                    },
                    "description": "List of search/replace operations"
                }
            },
            "required": ["path", "replacements"]
        }))
    }

    async fn execute(&self, ctx: Arc<BaseToolContext>, args: Value) -> ToolResult<Value> {
        let path = args.get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolExecError::InvalidArguments("Missing 'path' parameter".to_string()))?;

        let replacements = args.get("replacements")
            .and_then(|v| v.as_array())
            .ok_or_else(|| ToolExecError::InvalidArguments("Missing 'replacements' parameter".to_string()))?;

        // Resolve path in conversation context if available
        let final_path = if let Some(conv_dir) = ctx.conversation_dir() {
            conv_dir.join(path)
        } else {
            PathBuf::from(path)
        };

        let mut content = std::fs::read_to_string(&final_path)
            .map_err(|e| ToolExecError::ExecutionFailed(format!("Failed to read file: {}", e)))?;

        let mut count = 0;
        for repl in replacements {
            let old = repl.get("old")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolExecError::InvalidArguments("Missing 'old' in replacement".to_string()))?;

            let new = repl.get("new")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolExecError::InvalidArguments("Missing 'new' in replacement".to_string()))?;

            count += content.matches(old).count();
            content = content.replace(old, new);
        }

        std::fs::write(&final_path, content)
            .map_err(|e| ToolExecError::ExecutionFailed(format!("Failed to write file: {}", e)))?;

        Ok(json!({
            "replacements_made": count,
        }))
    }
}

// ============================================================================
// GREP TOOL
// ============================================================================

/// Tool for searching files with regex
pub struct GrepTool;

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Search files for a pattern using regex."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regex pattern to search for"
                },
                "path": {
                    "type": "string",
                    "description": "Path to search in (defaults to current directory)"
                },
                "case_insensitive": {
                    "type": "boolean",
                    "default": false
                },
                "max_results": {
                    "type": "integer",
                    "default": 100
                }
            },
            "required": ["pattern"]
        }))
    }

    async fn execute(&self, _ctx: Arc<BaseToolContext>, args: Value) -> ToolResult<Value> {
        let pattern = args.get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolExecError::InvalidArguments("Missing 'pattern' parameter".to_string()))?;

        let path = args.get("path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");

        let case_insensitive = args.get("case_insensitive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let max_results = args.get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(100) as usize;

        tracing::debug!("Grep: pattern={}, path={}, case_insensitive={}", pattern, path, case_insensitive);

        let regex = Regex::new(&format!("(?{}){}", if case_insensitive { "i" } else { "" }, pattern))
            .map_err(|e| ToolExecError::InvalidArguments(format!("Invalid regex: {}", e)))?;

        let mut results = Vec::new();

        // Simple recursive search
        let search_path = PathBuf::from(path);
        if search_path.is_file() {
            self.search_file(&search_path, &regex, &mut results)?;
        } else {
            self.search_directory(&search_path, &regex, &mut results, max_results)?;
        }

        Ok(json!({
            "matches": results,
            "total_matches": results.len(),
        }))
    }
}

impl GrepTool {
    const TEXT_EXTENSIONS: [&'static str; 12] = [
        "txt", "md", "rs", "js", "ts", "jsx", "tsx", "py", "json", "yaml", "yml", "toml"
    ];

    fn search_directory(
        &self,
        dir: &PathBuf,
        regex: &Regex,
        results: &mut Vec<Value>,
        max_results: usize,
    ) -> ToolResult<()> {
        if results.len() >= max_results {
            return Ok(());
        }

        let entries = std::fs::read_dir(dir)
            .map_err(|e| ToolExecError::ExecutionFailed(format!("Failed to read directory: {}", e)))?;

        for entry in entries.flatten() {
            let path = entry.path();

            if path.is_dir() {
                // Skip hidden directories and common exclusions
                if let Some(name) = path.file_name() {
                    let name_str = name.to_string_lossy();
                    if name_str.starts_with('.') || name_str == "node_modules" || name_str == "target" {
                        continue;
                    }
                }
                self.search_directory(&path, regex, results, max_results)?;
            } else if let Some(ext) = path.extension() {
                let ext_str = ext.to_string_lossy();
                if Self::TEXT_EXTENSIONS.contains(&ext_str.as_ref()) {
                    self.search_file(&path, regex, results)?;
                    if results.len() >= max_results {
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    fn search_file(&self, path: &PathBuf, regex: &Regex, results: &mut Vec<Value>) -> ToolResult<()> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ToolExecError::ExecutionFailed(format!("Failed to read file: {}", e)))?;

        for (line_num, line) in content.lines().enumerate() {
            if regex.is_match(line) {
                results.push(json!({
                    "file": path.to_string_lossy(),
                    "line": line_num + 1,
                    "content": line,
                }));
            }
        }

        Ok(())
    }
}

// ============================================================================
// GLOB TOOL
// ============================================================================

/// Tool for finding files with glob patterns
pub struct GlobTool;

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }

    fn description(&self) -> &str {
        "Find files using glob patterns like '*.rs' or '**/*.txt'."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern"
                },
                "include_hidden": {
                    "type": "boolean",
                    "default": false
                }
            },
            "required": ["pattern"]
        }))
    }

    async fn execute(&self, _ctx: Arc<BaseToolContext>, args: Value) -> ToolResult<Value> {
        let pattern = args.get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolExecError::InvalidArguments("Missing 'pattern' parameter".to_string()))?;

        let _include_hidden = args.get("include_hidden")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        tracing::debug!("Glob: pattern={}", pattern);

        let matches = glob::glob(pattern)
            .map_err(|e| ToolExecError::InvalidArguments(format!("Invalid glob pattern: {}", e)))?
            .filter_map(|entry| entry.ok())
            .filter(|path| path.is_file())
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>();

        Ok(json!({
            "matches": matches,
            "count": matches.len(),
        }))
    }
}

// ============================================================================
// PYTHON TOOL
// ============================================================================

/// Tool for executing Python code
pub struct PythonTool {
    /// File system context
    fs: Arc<dyn FileSystemContext>,
}

impl PythonTool {
    /// Create a new Python tool with file system context
    #[must_use]
    pub fn new(fs: Arc<dyn FileSystemContext>) -> Self {
        Self { fs }
    }
}

#[async_trait]
impl Tool for PythonTool {
    fn name(&self) -> &str {
        "python"
    }

    fn description(&self) -> &str {
        "Execute Python code in a virtual environment."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "code": {
                    "type": "string",
                    "description": "Python code to execute"
                }
            },
            "required": ["code"]
        }))
    }

    async fn execute(&self, _ctx: Arc<BaseToolContext>, args: Value) -> ToolResult<Value> {
        let code = args.get("code")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolExecError::InvalidArguments("Missing 'code' parameter".to_string()))?;

        let python = self.fs.python_executable()
            .ok_or_else(|| ToolExecError::ExecutionFailed("Python executable not configured".to_string()))?;

        tracing::debug!("Executing Python code ({} bytes)", code.len());

        // Create temp file for code
        let temp_dir = std::env::temp_dir();
        let script_path = temp_dir.join(format!("agent_{}.py", uuid::Uuid::new_v4()));

        std::fs::write(&script_path, code)
            .map_err(|e| ToolExecError::ExecutionFailed(format!("Failed to write script: {}", e)))?;

        // Execute Python
        let output = tokio::process::Command::new(&python)
            .arg(&script_path)
            .output()
            .await
            .map_err(|e| ToolExecError::ExecutionFailed(format!("Failed to execute Python: {}", e)))?;

        // Clean up temp file
        let _ = std::fs::remove_file(&script_path);

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ToolExecError::ExecutionFailed(format!("Python error: {}", stderr)));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        Ok(json!({
            "output": stdout,
        }))
    }
}

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
        if !ctx.available_skills.is_empty() && !ctx.available_skills.contains(&skill_name.to_string()) {
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

// ============================================================================
// REQUEST INPUT TOOL
// ============================================================================

/// Tool for requesting structured user input
pub struct RequestInputTool;

#[async_trait]
impl Tool for RequestInputTool {
    fn name(&self) -> &str {
        "request_input"
    }

    fn description(&self) -> &str {
        "Request structured input from the user via a form."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "form_id": {
                    "type": "string",
                    "description": "Unique identifier for this form"
                },
                "title": {
                    "type": "string",
                    "description": "Form title"
                },
                "description": {
                    "type": "string",
                    "description": "Form description"
                },
                "schema": {
                    "type": "object",
                    "description": "JSON Schema for the form"
                }
            },
            "required": ["form_id", "title", "schema"]
        }))
    }

    async fn execute(&self, _ctx: Arc<BaseToolContext>, args: Value) -> ToolResult<Value> {
        let form_id = args.get("form_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolExecError::InvalidArguments("Missing 'form_id' parameter".to_string()))?;

        let title = args.get("title")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolExecError::InvalidArguments("Missing 'title' parameter".to_string()))?;

        let schema = args.get("schema")
            .ok_or_else(|| ToolExecError::InvalidArguments("Missing 'schema' parameter".to_string()))?;

        tracing::debug!("Requesting input form: {}", form_id);

        // Return the form request - the application layer handles the actual UI
        Err(ToolExecError::ExecutionFailed(format!(
            "Form request created: {} ({}). The application should handle displaying this form.",
            form_id, title
        )))
    }
}

// ============================================================================
// SHOW CONTENT TOOL
// ============================================================================

/// Tool for displaying content in the UI
pub struct ShowContentTool;

#[async_trait]
impl Tool for ShowContentTool {
    fn name(&self) -> &str {
        "show_content"
    }

    fn description(&self) -> &str {
        "Display content to the user in a specialized viewer. Supports PDF, images, HTML, etc."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "content_type": {
                    "type": "string",
                    "description": "Type of content (pdf, image, html, text, etc.)"
                },
                "title": {
                    "type": "string",
                    "description": "Title for the content"
                },
                "content": {
                    "type": "string",
                    "description": "The content to display (or file path)"
                },
                "file_path": {
                    "type": "string",
                    "description": "Path to a previously saved file"
                }
            },
            "required": ["content_type", "title"]
        }))
    }

    async fn execute(&self, _ctx: Arc<BaseToolContext>, args: Value) -> ToolResult<Value> {
        let content_type = args.get("content_type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolExecError::InvalidArguments("Missing 'content_type' parameter".to_string()))?;

        let title = args.get("title")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolExecError::InvalidArguments("Missing 'title' parameter".to_string()))?;

        tracing::debug!("Showing content: type={}, title={}", content_type, title);

        // Return the content display request - the application layer handles the actual UI
        Err(ToolExecError::ExecutionFailed(format!(
            "Content display request: {} ({}). The application should handle displaying this content.",
            content_type, title
        )))
    }
}

// ============================================================================
// BUILT-IN TOOLS FACTORY
// ============================================================================

/// Get all built-in tools with a file system context
///
/// This function creates all the built-in tools with the provided
/// file system context. Tools that don't need file system access
/// are created without context.
#[must_use]
pub fn builtin_tools_with_fs(fs: Arc<dyn FileSystemContext>) -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(ReadTool),
        Arc::new(WriteTool::new(fs.clone())),
        Arc::new(EditTool),
        Arc::new(GrepTool),
        Arc::new(GlobTool),
        Arc::new(PythonTool::new(fs.clone())),
        Arc::new(LoadSkillTool::new(fs.clone())),
        Arc::new(RequestInputTool),
        Arc::new(ShowContentTool),
    ]
}
