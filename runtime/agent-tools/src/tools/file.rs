// ============================================================================
// FILE TOOLS
// Read, Write, and Edit tools
// ============================================================================

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use zero_core::{Tool, ToolContext, ToolPermissions, Result};
use zero_core::FileSystemContext;

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

    fn permissions(&self) -> ToolPermissions {
        ToolPermissions::safe()
    }

    async fn execute(&self, _ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
        let path = args.get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| zero_core::ZeroError::Tool("Missing 'path' parameter".to_string()))?;

        let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let limit = args.get("limit").and_then(|v| v.as_u64());

        tracing::debug!(
            file = %file!(),
            line = %line!(),
            "Reading file: {} (offset: {}, limit: {:?})",
            path, offset, limit
        );

        let content = std::fs::read_to_string(path)
            .map_err(|e| zero_core::ZeroError::Tool(format!("Failed to read file: {}", e)))?;

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
        "Write or append content to a file. Path routing:\n\
         Default → current ward directory (code files, visible to shell)\n\
         'attachments/' → agent_data/{session}/attachments/ (final outputs: .docx, .pptx)\n\
         'scratchpad/' → agent_data/{session}/scratchpad/ (intermediate work)\n\
         For large files (200+ lines), write the skeleton first, then use append calls."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Relative path. Default goes to code dir. Prefix 'attachments/' or 'scratchpad/' routes to agent_data."
                },
                "content": {
                    "type": "string",
                    "description": "Content to write to the file"
                },
                "mode": {
                    "type": "string",
                    "enum": ["write", "append"],
                    "default": "write",
                    "description": "write: Create/overwrite file. append: Add to end of existing file. Use append for large content that must be split across multiple calls."
                }
            },
            "required": ["path", "content"]
        }))
    }

    fn permissions(&self) -> ToolPermissions {
        ToolPermissions::moderate(vec!["filesystem:write".into()])
    }

    async fn execute(&self, ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
        // Check for error markers from truncated tool calls
        if let Some(error_type) = args.get("__error__").and_then(|v| v.as_str()) {
            let message = args.get("__message__").and_then(|v| v.as_str()).unwrap_or("Unknown error");
            let _truncated = args.get("__truncated__").and_then(|v| v.as_bool()).unwrap_or(false);
            return Err(zero_core::ZeroError::Tool(format!(
                "{}: {}",
                error_type,
                message
            )));
        }

        let path = args.get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| zero_core::ZeroError::Tool("Missing 'path' parameter".to_string()))?;

        // Extract filename for logging
        let filename = path.rsplit('/').next().unwrap_or(path);

        tracing::info!(
            file = %file!(),
            line = %line!(),
            "WRITE tool called: filename='{}', requested_path='{}'",
            filename, path
        );

        let content = args.get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| zero_core::ZeroError::Tool("Missing 'content' parameter".to_string()))?;

        // Get write mode (default: "write", can be "append")
        let mode = args.get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("write");
        let is_append = mode == "append";

        // Security: Reject paths with parent directory components
        if path.contains("..") {
            return Err(zero_core::ZeroError::Tool(
                "Path cannot contain '..' for security reasons.".to_string()
            ));
        }

        // Security: Reject absolute paths
        if path.starts_with('/') || path.starts_with('\\') {
            return Err(zero_core::ZeroError::Tool(
                "Absolute paths are not allowed. Use a relative path within the agent data directory.".to_string()
            ));
        }

        // Get session_id from state for path routing
        let session_id = ctx.get_state("session_id")
            .and_then(|v| v.as_str().map(|s| s.to_owned()))
            .ok_or_else(|| zero_core::ZeroError::Tool(
                "session_id not found in state.".to_string()
            ))?;

        tracing::info!(
            file = %file!(),
            line = %line!(),
            "{} file: path='{}' ({} bytes), session_id={}",
            if is_append { "Appending to" } else { "Writing" },
            path, content.len(), session_id
        );

        // Route based on path prefix:
        // attachments/ and scratchpad/ → agent_data/{session}/ (session-scoped)
        // everything else → wards/{ward_id}/ (ward-scoped, where shell runs)
        let final_path = if path.starts_with("attachments/") || path.starts_with("scratchpad/") {
            let data_dir = self.fs.session_data_dir(&session_id)
                .ok_or_else(|| zero_core::ZeroError::Tool(
                    "Session data dir unavailable".to_string()
                ))?;
            data_dir.join(path)
        } else {
            // Use ward_id if set, otherwise fall back to "scratch"
            let ward_id = ctx.get_state("ward_id")
                .and_then(|v| v.as_str().map(|s| s.to_owned()))
                .unwrap_or_else(|| "scratch".to_string());

            let ward_dir = self.fs.ward_dir(&ward_id)
                .ok_or_else(|| zero_core::ZeroError::Tool(
                    "Ward dir unavailable".to_string()
                ))?;
            ward_dir.join(path)
        };

        // Create parent directories
        if let Some(parent) = final_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| zero_core::ZeroError::Tool(format!("Failed to create directories: {}", e)))?;
        }

        // Write or append to the file
        tracing::info!(
            file = %file!(),
            line = %line!(),
            "{} file: {}",
            if is_append { "Appending to" } else { "Writing to" },
            final_path.display()
        );

        if is_append {
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&final_path)
                .map_err(|e| zero_core::ZeroError::Tool(format!("Failed to open file for append: {}", e)))?;
            file.write_all(content.as_bytes())
                .map_err(|e| zero_core::ZeroError::Tool(format!("Failed to append to file: {}", e)))?;
        } else {
            std::fs::write(&final_path, content)
                .map_err(|e| zero_core::ZeroError::Tool(format!("Failed to write file: {}", e)))?;
        }

        // Return the original requested path (not the absolute resolved path)
        // so the LLM continues to use relative paths
        Ok(json!({
            "path": path,
            "bytes_written": content.len(),
            "mode": mode
        }))
    }
}

// ============================================================================
// EDIT TOOL
// ============================================================================

/// Tool for editing files with search and replace
///
/// Note: This tool requires conversation context to resolve paths.
/// The conversation_id is read from the ToolContext's state during execution.
pub struct EditTool {
    /// File system context for resolving conversation paths
    fs: Arc<dyn FileSystemContext>,
}

impl EditTool {
    /// Create a new edit tool with file system context
    #[must_use]
    pub fn new(fs: Arc<dyn FileSystemContext>) -> Self {
        Self { fs }
    }
}

#[async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str {
        "edit"
    }

    fn description(&self) -> &str {
        "Edit a file using search/replace. Same path routing as write: default → current ward, \
         'attachments/' and 'scratchpad/' → agent_data/{session}/."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Relative path. Default goes to code dir. Prefix 'attachments/' or 'scratchpad/' routes to agent_data."
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

    fn permissions(&self) -> ToolPermissions {
        ToolPermissions::moderate(vec!["filesystem:write".into()])
    }

    async fn execute(&self, ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
        // Check for error markers from truncated tool calls
        if let Some(error_type) = args.get("__error__").and_then(|v| v.as_str()) {
            let message = args.get("__message__").and_then(|v| v.as_str()).unwrap_or("Unknown error");
            let _truncated = args.get("__truncated__").and_then(|v| v.as_bool()).unwrap_or(false);
            return Err(zero_core::ZeroError::Tool(format!(
                "{}: {}",
                error_type,
                message
            )));
        }

        let path = args.get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| zero_core::ZeroError::Tool("Missing 'path' parameter".to_string()))?;

        // Extract filename for logging
        let filename = path.rsplit('/').next().unwrap_or(path);

        tracing::info!(
            file = %file!(),
            line = %line!(),
            "EDIT tool called: filename='{}', requested_path='{}'",
            filename, path
        );

        let replacements = args.get("replacements")
            .and_then(|v| v.as_array())
            .ok_or_else(|| zero_core::ZeroError::Tool("Missing 'replacements' parameter".to_string()))?;

        // Security: Reject paths with parent directory components
        if path.contains("..") {
            return Err(zero_core::ZeroError::Tool(
                "Path cannot contain '..' for security reasons.".to_string()
            ));
        }

        // Security: Reject absolute paths
        if path.starts_with('/') || path.starts_with('\\') {
            return Err(zero_core::ZeroError::Tool(
                "Absolute paths are not allowed. Use a relative path within the agent data directory.".to_string()
            ));
        }

        // Get session_id from state for path routing
        let session_id = ctx.get_state("session_id")
            .and_then(|v| v.as_str().map(|s| s.to_owned()))
            .ok_or_else(|| zero_core::ZeroError::Tool(
                "session_id not found in state.".to_string()
            ))?;

        tracing::info!(
            file = %file!(),
            line = %line!(),
            "Editing file: path='{}', session_id={}",
            path, session_id
        );

        // Route based on path prefix (same as write tool)
        let final_path = if path.starts_with("attachments/") || path.starts_with("scratchpad/") {
            let data_dir = self.fs.session_data_dir(&session_id)
                .ok_or_else(|| zero_core::ZeroError::Tool(
                    "Session data dir unavailable".to_string()
                ))?;
            data_dir.join(path)
        } else {
            // Use ward_id if set, otherwise fall back to "scratch"
            let ward_id = ctx.get_state("ward_id")
                .and_then(|v| v.as_str().map(|s| s.to_owned()))
                .unwrap_or_else(|| "scratch".to_string());

            let ward_dir = self.fs.ward_dir(&ward_id)
                .ok_or_else(|| zero_core::ZeroError::Tool(
                    "Ward dir unavailable".to_string()
                ))?;
            ward_dir.join(path)
        };

        let mut content = std::fs::read_to_string(&final_path)
            .map_err(|e| zero_core::ZeroError::Tool(format!("Failed to read file: {}", e)))?;

        let mut count = 0;
        for repl in replacements {
            let old = repl.get("old")
                .and_then(|v| v.as_str())
                .ok_or_else(|| zero_core::ZeroError::Tool("Missing 'old' in replacement".to_string()))?;

            let new = repl.get("new")
                .and_then(|v| v.as_str())
                .ok_or_else(|| zero_core::ZeroError::Tool("Missing 'new' in replacement".to_string()))?;

            count += content.matches(old).count();
            content = content.replace(old, new);
        }

        tracing::info!(
            file = %file!(),
            line = %line!(),
            "Editing file: {} ({} replacements)",
            final_path.display(), count
        );

        std::fs::write(&final_path, content)
            .map_err(|e| zero_core::ZeroError::Tool(format!("Failed to write file: {}", e)))?;

        // Return the original requested path (not the absolute resolved path)
        Ok(json!({
            "path": path,
            "replacements_made": count,
        }))
    }
}
