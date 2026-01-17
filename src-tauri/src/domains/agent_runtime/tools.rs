// ============================================================================
// AGENT RUNTIME TOOLS
// Port of existing tools to our own Tool trait
// ============================================================================

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};
use async_trait::async_trait;

use crate::settings::AppDirs;

// ============================================================================
// TOOL TRAIT (Simplified)
// ============================================================================

/// Error type for tool execution
#[derive(Debug, Clone)]
pub struct ToolError(pub String);

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Tool error: {}", self.0)
    }
}

impl std::error::Error for ToolError {}

/// Result type for tool operations
pub type ToolResult<T> = Result<T, ToolError>;

/// Simple tool context for tool execution
pub struct ToolContext {
    /// Optional conversation ID for scoping file operations
    pub conversation_id: Option<String>,
    /// Skills available to the current agent (for load_skill tool)
    pub available_skills: Vec<String>,
}

impl ToolContext {
    pub fn new() -> Self {
        Self {
            conversation_id: None,
            available_skills: Vec::new(),
        }
    }

    pub fn with_conversation(conversation_id: String) -> Self {
        Self {
            conversation_id: Some(conversation_id),
            available_skills: Vec::new(),
        }
    }

    pub fn with_skills(available_skills: Vec<String>) -> Self {
        Self {
            conversation_id: None,
            available_skills,
        }
    }

    pub fn with_conversation_and_skills(conversation_id: String, available_skills: Vec<String>) -> Self {
        Self {
            conversation_id: Some(conversation_id),
            available_skills,
        }
    }

    /// Get the conversation directory if conversation_id is set
    pub fn conversation_dir(&self) -> Option<PathBuf> {
        let dirs = AppDirs::get().ok()?;
        let conv_id = self.conversation_id.as_ref()?;
        Some(dirs.conversation_dir(conv_id))
    }
}

/// Tool trait that all tools must implement
#[async_trait]
pub trait Tool: Send + Sync {
    /// Returns the name of the tool
    fn name(&self) -> &str;

    /// Returns a description of what the tool does
    fn description(&self) -> &str;

    /// Returns the JSON schema for the tool's parameters (optional)
    fn parameters_schema(&self) -> Option<Value> {
        None
    }

    /// Executes the tool with the given arguments
    async fn execute(
        &self,
        ctx: Arc<ToolContext>,
        args: Value,
    ) -> ToolResult<Value>;

    /// Returns whether this is a long-running operation (default: false)
    fn is_long_running(&self) -> bool {
        false
    }
}

// ============================================================================
// TOOL REGISTRY
// ============================================================================

/// Registry of all available tools
pub struct ToolRegistry {
    tools: Vec<Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: Vec::new(),
        }
    }

    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.push(tool);
    }

    pub fn register_all(&mut self, tools: Vec<Arc<dyn Tool>>) {
        for tool in tools {
            self.register(tool);
        }
    }

    pub fn get_all(&self) -> &[Arc<dyn Tool>] {
        &self.tools
    }

    pub fn find(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.iter().find(|t| t.name() == name).cloned()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        let mut registry = Self::new();
        registry.register_all(builtin_tools());
        registry
    }
}

/// Get all built-in tools
pub fn builtin_tools() -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(ReadTool::new()),
        Arc::new(WriteTool::new()),
        Arc::new(EditTool::new()),
        Arc::new(GrepTool::new()),
        Arc::new(GlobTool::new()),
        Arc::new(PythonTool::new()),
        Arc::new(LoadSkillTool::new()),
        Arc::new(RequestInputTool::new()),
        Arc::new(ShowContentTool::new()),
    ]
}

// ============================================================================
// BUILT-IN TOOLS
// ============================================================================

/// Read file tool
pub struct ReadTool;

impl ReadTool {
    pub fn new() -> Self {
        Self
    }
}

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
                    "description": "Number of lines to read (-1 for all lines)",
                    "default": -1
                }
            },
            "required": ["path"]
        }))
    }

    async fn execute(
        &self,
        _ctx: Arc<ToolContext>,
        args: Value,
    ) -> ToolResult<Value> {
        let path = args.get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError("Missing 'path' parameter".to_string()))?;

        let offset = args.get("offset")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        let limit = args.get("limit")
            .and_then(|v| v.as_i64())
            .unwrap_or(-1);

        let path_buf = PathBuf::from(path);

        if !path_buf.exists() {
            return Err(ToolError(format!("File not found: {}", path)));
        }

        let content = fs::read_to_string(&path_buf)
            .map_err(|e| ToolError(format!("Failed to read file: {}", e)))?;

        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        let start = offset.min(total_lines);
        let end = if limit < 0 {
            total_lines
        } else {
            (offset + limit as usize).min(total_lines)
        };

        if start >= total_lines {
            return Ok(json!(""));
        }

        let selected_lines: Vec<&str> = lines[start..end].to_vec();
        let result = selected_lines.join("\n");

        Ok(json!({
            "content": result,
            "total_lines": total_lines,
            "lines_read": end - start
        }))
    }
}

/// Write file tool
pub struct WriteTool;

impl WriteTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for WriteTool {
    fn name(&self) -> &str {
        "write"
    }

    fn description(&self) -> &str {
        "Write content to a file. Use relative paths like 'attachments/report.md', 'outputs/report.html', or 'scratchpad/temp.txt'. Parent directories are created automatically. When in a conversation, files are written to the conversation's scoped directory (attachments/, scratchpad/). Use 'outputs/' prefix to save to ~/Documents/ZeroAgent/outputs/ for easy browser access."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to write"
                },
                "content": {
                    "type": "string",
                    "description": "Content to write to the file"
                }
            },
            "required": ["path", "content"]
        }))
    }

    async fn execute(
        &self,
        ctx: Arc<ToolContext>,
        args: Value,
    ) -> ToolResult<Value> {
        let path = args.get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError("Missing 'path' parameter".to_string()))?;

        let content = args.get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError("Missing 'content' parameter".to_string()))?;

        // Check if this is an outputs/ path - save to accessible outputs directory
        let path_buf = if path.starts_with("outputs/") {
            let dirs = crate::settings::AppDirs::get()
                .map_err(|e| ToolError(format!("Failed to get app directories: {}", e)))?;

            // Extract the path after "outputs/"
            let relative_path = path.trim_start_matches("outputs/");
            dirs.outputs_dir.join(relative_path)
        } else if let Some(conv_dir) = ctx.conversation_dir() {
            // Write operations are scoped to conversation directory
            conv_dir.join(path)
        } else {
            // No conversation context, use path as-is
            PathBuf::from(path)
        };

        // Create parent directories if needed
        if let Some(parent) = path_buf.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)
                    .map_err(|e| ToolError(format!("Failed to create directories: {}", e)))?;
            }
        }

        // Write the file
        fs::write(&path_buf, content)
            .map_err(|e| ToolError(format!("Failed to write file: {}", e)))?;

        // Set permissions to 644 for outputs directory files so browser can access them
        if path.starts_with("outputs/") {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&path_buf)
                    .map_err(|e| ToolError(format!("Failed to get file metadata: {}", e)))?
                    .permissions();
                perms.set_mode(0o644);
                fs::set_permissions(&path_buf, perms)
                    .map_err(|e| ToolError(format!("Failed to set file permissions: {}", e)))?;
            }
        }

        Ok(json!({
            "success": true,
            "path": path_buf.display().to_string(),
            "bytes_written": content.len()
        }))
    }
}

/// Edit file tool (search and replace)
pub struct EditTool;

impl EditTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str {
        "edit"
    }

    fn description(&self) -> &str {
        "Edit a file by replacing text. Use relative paths like 'attachments/report.md' or 'memory.md'. Supports multiple replacements. When in a conversation, files are scoped to the conversation's directory."
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
                    "description": "List of replacements to make",
                    "items": {
                        "type": "object",
                        "properties": {
                            "old": {
                                "type": "string",
                                "description": "Text to replace"
                            },
                            "new": {
                                "type": "string",
                                "description": "Replacement text"
                            }
                        },
                        "required": ["old", "new"]
                    }
                }
            },
            "required": ["path", "replacements"]
        }))
    }

    async fn execute(
        &self,
        ctx: Arc<ToolContext>,
        args: Value,
    ) -> ToolResult<Value> {
        let path = args.get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError("Missing 'path' parameter".to_string()))?;

        let replacements = args.get("replacements")
            .and_then(|v| v.as_array())
            .ok_or_else(|| ToolError("Missing 'replacements' parameter".to_string()))?;

        // Resolve the final path - use conversation directory if available
        let path_buf = if let Some(conv_dir) = ctx.conversation_dir() {
            // Edit operations are scoped to conversation directory
            conv_dir.join(path)
        } else {
            // No conversation context, use path as-is
            PathBuf::from(path)
        };

        if !path_buf.exists() {
            return Err(ToolError(format!("File not found: {}", path_buf.display())));
        }

        let mut content = fs::read_to_string(&path_buf)
            .map_err(|e| ToolError(format!("Failed to read file: {}", e)))?;

        let mut replacements_made = 0;

        for replacement in replacements {
            let old_text = replacement.get("old")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError("Missing 'old' in replacement".to_string()))?;

            let new_text = replacement.get("new")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError("Missing 'new' in replacement".to_string()))?;

            if content.contains(old_text) {
                content = content.replace(old_text, new_text);
                replacements_made += 1;
            }
        }

        // Write back the modified content
        fs::write(&path_buf, content)
            .map_err(|e| ToolError(format!("Failed to write file: {}", e)))?;

        Ok(json!({
            "success": true,
            "path": path_buf.display().to_string(),
            "replacements_made": replacements_made
        }))
    }
}

/// Grep tool for searching files
pub struct GrepTool;

impl GrepTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Search for a pattern in files using regex. Supports recursive search and context lines."
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
                    "description": "Path to search in"
                },
                "recursive": {
                    "type": "boolean",
                    "description": "Search recursively in subdirectories",
                    "default": true
                },
                "case_insensitive": {
                    "type": "boolean",
                    "description": "Case-insensitive search",
                    "default": false
                },
                "context_before": {
                    "type": "integer",
                    "description": "Number of context lines before match",
                    "default": 2
                },
                "context_after": {
                    "type": "integer",
                    "description": "Number of context lines after match",
                    "default": 2
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of matches to return",
                    "default": 100
                }
            },
            "required": ["pattern", "path"]
        }))
    }

    async fn execute(
        &self,
        _ctx: Arc<ToolContext>,
        args: Value,
    ) -> ToolResult<Value> {
        use regex::Regex;

        let pattern = args.get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError("Missing 'pattern' parameter".to_string()))?;

        let path = args.get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError("Missing 'path' parameter".to_string()))?;

        let recursive = args.get("recursive")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let case_insensitive = args.get("case_insensitive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let context_before = args.get("context_before")
            .and_then(|v| v.as_u64())
            .unwrap_or(2) as usize;

        let context_after = args.get("context_after")
            .and_then(|v| v.as_u64())
            .unwrap_or(2) as usize;

        let max_results = args.get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(100) as usize;

        let path_buf = PathBuf::from(path);

        if !path_buf.exists() {
            return Err(ToolError(format!("Path not found: {}", path)));
        }

        // Build regex
        let regex = Regex::new(&format!(
            "(?{}){}",
            if case_insensitive { "i" } else { "" },
            pattern
        )).map_err(|e| ToolError(format!("Invalid regex pattern: {}", e)))?;

        let mut results = Vec::new();
        let mut match_count = 0;

        // Collect files to search
        let mut files_to_search: Vec<PathBuf> = Vec::new();

        if path_buf.is_file() {
            files_to_search.push(path_buf);
        } else {
            Self::collect_files(&path_buf, &mut files_to_search, recursive);
        }

        // Search each file
        for file_path in files_to_search {
            if match_count >= max_results {
                break;
            }

            match fs::read_to_string(&file_path) {
                Ok(content) => {
                    for (line_num, line) in content.lines().enumerate() {
                        if regex.is_match(line) {
                            match_count += 1;

                            // Add context lines
                            let content_lines: Vec<&str> = content.lines().collect();
                            let start = line_num.saturating_sub(context_before);
                            let end = (line_num + context_after + 1).min(content_lines.len());

                            for ctx_line in start..end {
                                let marker = if ctx_line == line_num { ">>>" } else { "   " };
                                results.push(json!({
                                    "marker": marker,
                                    "line": ctx_line + 1,
                                    "file": file_path.display().to_string(),
                                    "content": content_lines[ctx_line]
                                }));
                            }

                            if match_count >= max_results {
                                break;
                            }
                        }
                    }
                }
                Err(_) => continue,
            }
        }

        Ok(json!({
            "matches": match_count,
            "results": results
        }))
    }
}

impl GrepTool {
    fn collect_files(dir: &PathBuf, files: &mut Vec<PathBuf>, recursive: bool) {
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            // Skip hidden files
            if name.starts_with('.') {
                continue;
            }

            if path.is_file() && Self::is_text_file(&path) {
                files.push(path);
            } else if recursive && path.is_dir() {
                Self::collect_files(&path, files, recursive);
            }
        }
    }

    fn is_text_file(path: &PathBuf) -> bool {
        if let Some(ext) = path.extension() {
            let ext_str = ext.to_string_lossy().to_lowercase();
            matches!(
                ext_str.as_str(),
                "txt" | "md" | "js" | "ts" | "tsx" | "jsx" | "rs"
                    | "toml" | "yaml" | "yml" | "json" | "xml"
                    | "html" | "css" | "scss" | "py" | "sh"
            )
        } else {
            Self::is_likely_text(path)
        }
    }

    fn is_likely_text(path: &PathBuf) -> bool {
        match fs::read(path) {
            Ok(contents) => {
                let check_bytes = contents.iter().take(512);
                for byte in check_bytes {
                    if *byte < 9 || (*byte > 13 && *byte < 32) || *byte == 127 {
                        return false;
                    }
                }
                true
            }
            Err(_) => false,
        }
    }
}

/// Glob tool for finding files
pub struct GlobTool;

impl GlobTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }

    fn description(&self) -> &str {
        "Find files matching a pattern using glob syntax."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern (e.g., '*.rs', '**/*.txt')"
                },
                "path": {
                    "type": "string",
                    "description": "Base path to search from"
                },
                "include_hidden": {
                    "type": "boolean",
                    "description": "Include hidden files (starting with .)",
                    "default": false
                }
            },
            "required": ["pattern", "path"]
        }))
    }

    async fn execute(
        &self,
        _ctx: Arc<ToolContext>,
        args: Value,
    ) -> ToolResult<Value> {
        use glob::glob;

        let pattern = args.get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError("Missing 'pattern' parameter".to_string()))?;

        let path = args.get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError("Missing 'path' parameter".to_string()))?;

        let include_hidden = args.get("include_hidden")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let search_path = PathBuf::from(path);

        if !search_path.exists() {
            return Err(ToolError(format!("Path not found: {}", path)));
        }

        // Build the full glob pattern
        let full_pattern = if search_path.is_absolute() {
            format!("{}/{}", search_path.display(), pattern)
        } else {
            format!("{}/{}", std::env::current_dir().unwrap().display(), pattern)
        };

        let mut results = Vec::new();

        match glob(&full_pattern) {
            Ok(paths) => {
                for entry in paths.flatten() {
                    let file_name = entry.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("");

                    // Skip hidden files unless requested
                    if !include_hidden && file_name.starts_with('.') {
                        continue;
                    }

                    results.push(entry.display().to_string());
                }
            }
            Err(e) => {
                return Err(ToolError(format!("Invalid glob pattern: {}", e)));
            }
        }

        results.sort();

        Ok(json!({
            "matches": results.len(),
            "files": results
        }))
    }
}

/// Python execution tool
pub struct PythonTool;

impl PythonTool {
    pub fn new() -> Self {
        Self
    }

    fn get_python_venv() -> Result<PathBuf, String> {
        let dirs = AppDirs::get().map_err(|e| e.to_string())?;
        let venv_path = dirs.config_dir.join("venv");

        #[cfg(target_os = "windows")]
        let python_path = venv_path.join("Scripts").join("python.exe");

        #[cfg(not(target_os = "windows"))]
        let python_path = venv_path.join("bin").join("python");

        if !python_path.exists() {
            return Err(format!(
                "Python venv not found at {}. Please create it first.",
                venv_path.display()
            ));
        }

        Ok(python_path)
    }
}

#[async_trait]
impl Tool for PythonTool {
    fn name(&self) -> &str {
        "python"
    }

    fn description(&self) -> &str {
        "Execute Python code in the configured virtual environment."
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

    async fn execute(
        &self,
        _ctx: Arc<ToolContext>,
        args: Value,
    ) -> ToolResult<Value> {
        let code = args.get("code")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError("Missing 'code' parameter".to_string()))?;

        let python_path = Self::get_python_venv()
            .map_err(|e| ToolError(e))?;

        // Create a temporary Python script
        let temp_script = format!(
            r#"
import sys
from io import StringIO

# Capture stdout
old_stdout = sys.stdout
sys.stdout = captured = StringIO()

try:
{}

finally:
    # Restore stdout
    sys.stdout = old_stdout
    # Get captured output
    output = captured.getvalue()
    print(output, end='')
"#,
            code
        );

        let output = tokio::process::Command::new(&python_path)
            .arg("-c")
            .arg(&temp_script)
            .output()
            .await
            .map_err(|e| ToolError(format!("Failed to execute Python: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() {
            return Err(ToolError(format!("Python execution failed: {}", stderr)));
        }

        Ok(json!({
            "output": stdout
        }))
    }
}

/// Load skill tool - loads a skill from the skill registry
pub struct LoadSkillTool;

impl LoadSkillTool {
    pub fn new() -> Self {
        Self
    }

    /// Parse SKILL.md file with YAML frontmatter
    fn parse_skill_file(content: &str) -> Result<(Value, String), String> {
        // Look for YAML frontmatter between --- markers
        // Use [\s\S]+? instead of .+? to match across newlines
        let frontmatter_regex = regex::Regex::new(r"^---\n([\s\S]+?)\n---\n([\s\S]+)$")
            .map_err(|e| format!("Failed to create regex: {}", e))?;

        let captures = frontmatter_regex.captures(content)
            .ok_or_else(|| "Invalid SKILL.md format. Expected YAML frontmatter between --- markers.".to_string())?;

        let frontmatter = captures.get(1)
            .map(|m| m.as_str())
            .unwrap_or("");

        let body = captures.get(2)
            .map(|m| m.as_str())
            .unwrap_or("")
            .to_string();

        // Parse YAML frontmatter
        let metadata: Value = serde_yaml::from_str(frontmatter)
            .map_err(|e| format!("Failed to parse skill metadata: {}", e))?;

        Ok((metadata, body))
    }
}

#[async_trait]
impl Tool for LoadSkillTool {
    fn name(&self) -> &str {
        "load_skill"
    }

    fn description(&self) -> &str {
        "Load a skill from the skill registry. Returns the skill's instructions and metadata. Only skills associated with the current agent can be loaded."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "skill_name": {
                    "type": "string",
                    "description": "Name of the skill to load (e.g., 'code-review', 'agile-planning')"
                }
            },
            "required": ["skill_name"]
        }))
    }

    async fn execute(
        &self,
        ctx: Arc<ToolContext>,
        args: Value,
    ) -> ToolResult<Value> {
        let skill_name = args.get("skill_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError("Missing 'skill_name' parameter".to_string()))?;

        // Check if the skill is available to this agent
        if !ctx.available_skills.contains(&skill_name.to_string()) {
            return Err(ToolError(format!(
                "Skill '{}' is not associated with this agent. Available skills: {}",
                skill_name,
                ctx.available_skills.join(", ")
            )));
        }

        // Get the skills directory
        let dirs = AppDirs::get().map_err(|e| ToolError(e.to_string()))?;
        let skill_dir = dirs.skills_dir.join(skill_name);
        let skill_file = skill_dir.join("SKILL.md");

        if !skill_dir.exists() {
            return Err(ToolError(format!(
                "Skill directory not found: {}",
                skill_dir.display()
            )));
        }

        if !skill_file.exists() {
            return Err(ToolError(format!(
                "SKILL.md not found in skill directory: {}",
                skill_file.display()
            )));
        }

        // Read the skill file
        let content = fs::read_to_string(&skill_file)
            .map_err(|e| ToolError(format!("Failed to read skill file: {}", e)))?;

        // Parse the skill file
        let (metadata, body) = Self::parse_skill_file(&content)
            .map_err(|e| ToolError(format!("Failed to parse skill file: {}", e)))?;

        // Extract useful metadata
        let name = metadata.get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(skill_name);

        let display_name = metadata.get("displayName")
            .and_then(|v| v.as_str())
            .unwrap_or(name);

        let description = metadata.get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let category = metadata.get("category")
            .and_then(|v| v.as_str())
            .unwrap_or("general");

        // Return the skill content
        Ok(json!({
            "name": name,
            "displayName": display_name,
            "description": description,
            "category": category,
            "instructions": body.trim(),
            "loaded": true
        }))
    }
}

/// Request input tool - requests user input via JSON Schema form
pub struct RequestInputTool;

impl RequestInputTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for RequestInputTool {
    fn name(&self) -> &str {
        "request_input"
    }

    fn description(&self) -> &str {
        r#"IMPORTANT: Use this tool PROACTIVELY whenever you need to collect structured information from the user.

You MUST use request_input instead of asking questions in plain text when:
- You need to collect 2 or more related pieces of information
- The user needs to provide specific details (names, dates, options, etc.)
- You need structured data rather than freeform text
- Gathering information would benefit from a form interface

The user will see a form based on your schema and their response will be sent as JSON. This provides a MUCH better user experience than asking multiple questions in chat.

Example: If you need a project name, description, and deadline - use request_input ONCE with a schema containing those 3 fields, instead of asking 3 separate questions in chat."#
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Title for the form"
                },
                "description": {
                    "type": "string",
                    "description": "Optional description or instructions for the user"
                },
                "schema": {
                    "type": "object",
                    "description": "JSON Schema for the form. Should define the structure of data to collect."
                },
                "submit_button": {
                    "type": "string",
                    "description": "Optional custom text for the submit button (default: 'Submit')"
                }
            },
            "required": ["title", "schema"]
        }))
    }

    async fn execute(
        &self,
        _ctx: Arc<ToolContext>,
        args: Value,
    ) -> ToolResult<Value> {
        let title = args.get("title")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError("Missing 'title' parameter".to_string()))?;

        let description = args.get("description").and_then(|v| v.as_str());

        let schema = args.get("schema")
            .and_then(|v| v.as_object())
            .ok_or_else(|| ToolError("Missing 'schema' parameter".to_string()))?
            .clone();

        let submit_button = args.get("submit_button").and_then(|v| v.as_str());

        // Generate a unique form ID
        let form_id = format!("form_{}", chrono::Utc::now().timestamp_millis());

        Ok(json!({
            "__request_input": true,
            "form_id": form_id,
            "form_type": "json_schema",
            "title": title,
            "description": description,
            "schema": schema,
            "submit_button": submit_button
        }))
    }
}

/// Show content tool - displays content in the generative UI canvas
pub struct ShowContentTool;

impl ShowContentTool {
    pub fn new() -> Self {
        Self
    }

    /// Read file and encode to base64
    fn read_and_encode(path: &PathBuf) -> Result<String, String> {
        let content = fs::read(path)
            .map_err(|e| format!("Failed to read file: {}", e))?;

        // Check if it's binary or text
        let is_text = content.iter().take(1024).all(|&b| b >= 32 && b <= 126 || b == b'\n' || b == b'\r' || b == b'\t');

        if is_text {
            // Return as plain text
            String::from_utf8(content)
                .map_err(|e| format!("File contains invalid UTF-8: {}", e))
        } else {
            // Return as base64
            use base64::prelude::*;
            Ok(BASE64_STANDARD.encode(&content))
        }
    }
}

#[async_trait]
impl Tool for ShowContentTool {
    fn name(&self) -> &str {
        "show_content"
    }

    fn description(&self) -> &str {
        r#"CRITICAL: ALWAYS use this tool AFTER saving a file with write_file.

## MANDATORY TWO-STEP WORKFLOW:

When you generate HTML, PDF, reports, or any structured content, you MUST:

**STEP 1: First save the file**
write_file({ path: "attachments/report.html", content: "<html>...</html>" })

**STEP 2: Then display it**
show_content({
  content_type: "html",
  title: "Monthly Report",
  content: { path: "attachments/report.html" }
})

## NEVER skip step 1 - ALWAYS save files before displaying!

For attachments: Use content: { path: "attachments/filename.ext" }
For simple inline content only: Use content: "string"

SUPPORTED TYPES: pdf, ppt, html, image, text, markdown"#
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "content_type": {
                    "type": "string",
                    "enum": ["pdf", "ppt", "html", "image", "text", "markdown"],
                    "description": "Type of content to display"
                },
                "title": {
                    "type": "string",
                    "description": "Title for the content viewer"
                },
                "content": {
                    "oneOf": [
                        {
                            "type": "string",
                            "description": "Raw content (for HTML, text, markdown)"
                        },
                        {
                            "type": "object",
                            "properties": {
                                "path": {
                                    "type": "string",
                                    "description": "Path to the file to read"
                                },
                                "base64": {
                                    "type": "boolean",
                                    "description": "Whether content is base64 encoded (default: false for file paths)"
                                }
                            },
                            "required": ["path"]
                        }
                    ],
                    "description": "Content to display - either raw string or file path object"
                },
                "metadata": {
                    "type": "object",
                    "description": "Optional metadata (page numbers for PDF, slide info for PPT, etc.)"
                }
            },
            "required": ["content_type", "title", "content"]
        }))
    }

    async fn execute(
        &self,
        ctx: Arc<ToolContext>,
        args: Value,
    ) -> ToolResult<Value> {
        let content_type = args.get("content_type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError("Missing 'content_type' parameter".to_string()))?;

        let title = args.get("title")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError("Missing 'title' parameter".to_string()))?;

        let content_arg = args.get("content")
            .ok_or_else(|| ToolError("Missing 'content' parameter".to_string()))?;

        let metadata = args.get("metadata").cloned();

        // Get conversation ID
        let conversation_id = ctx.conversation_id.clone()
            .ok_or_else(|| ToolError("This tool requires a conversation context".to_string()))?;

        // Resolve content based on type
        let (content_value, file_path, is_attachment) = if let Some(content_obj) = content_arg.as_object() {
            // Content is a file path reference
            let path_str = content_obj.get("path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError("Missing 'path' in content object".to_string()))?;

            let mut path_buf = PathBuf::from(path_str);

            // If path is relative, resolve against conversation directory
            if path_buf.is_relative() {
                if let Some(conv_dir) = ctx.conversation_dir() {
                    path_buf = conv_dir.join(path_str);
                }
            }

            // Check if this is an attachment file (in the attachments directory)
            let is_attachment = path_buf.to_string_lossy().contains("/attachments/") ||
                               path_buf.to_string_lossy().contains("\\attachments\\");

            // For attachment files, we don't read the content here - just return the filename
            // The frontend will load it via read_attachment_file command
            if is_attachment {
                let filename = path_buf.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("file");

                let relative_path = format!("{}/attachments/{}", conversation_id, filename);

                // Return filename as content (not the actual file content)
                (filename.to_string(), Some(relative_path), Some(true))
            } else {
                // For non-attachment files, read and encode the content
                let data = Self::read_and_encode(&path_buf)
                    .map_err(|e| ToolError(format!("Failed to read file: {}", e)))?;

                // Auto-detect if we need to base64 encode binary files
                let should_base64 = matches!(content_type, "pdf" | "ppt" | "image");

                (data, None, if should_base64 { Some(true) } else { Some(false) })
            }
        } else {
            // Content is raw string - for backwards compatibility
            let raw_content = content_arg.as_str()
                .ok_or_else(|| ToolError("Content must be string or object".to_string()))?
                .to_string();

            // For binary content types, base64 encode
            let final_content = if matches!(content_type, "pdf" | "ppt" | "image") {
                use base64::prelude::*;
                BASE64_STANDARD.encode(raw_content.as_bytes())
            } else {
                raw_content
            };

            (final_content, None, Some(matches!(content_type, "pdf" | "ppt" | "image")))
        };

        Ok(json!({
            "__show_content": true,
            "content_type": content_type,
            "title": title,
            "content": content_value,
            "metadata": metadata,
            "file_path": file_path,
            "is_attachment": is_attachment,
            "base64": is_attachment.unwrap_or(false) && matches!(content_type, "pdf" | "ppt" | "image")
        }))
    }
}
