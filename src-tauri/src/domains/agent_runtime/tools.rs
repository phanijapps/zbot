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
}

impl ToolContext {
    pub fn new() -> Self {
        Self {
            conversation_id: None,
        }
    }

    pub fn with_conversation(conversation_id: String) -> Self {
        Self {
            conversation_id: Some(conversation_id),
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
        "Write content to a file. Use relative paths like 'attachments/report.md' or 'scratchpad/temp.txt'. Parent directories are created automatically. When in a conversation, files are written to the conversation's scoped directory (attachments/, scratchpad/, or memory.md)."
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

        // Resolve the final path - use conversation directory if available
        let path_buf = if let Some(conv_dir) = ctx.conversation_dir() {
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
