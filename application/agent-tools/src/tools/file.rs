// ============================================================================
// FILE TOOLS
// Read, Write, and Edit tools
// ============================================================================

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use zero_core::{Tool, ToolContext, Result};
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
        "Write content to a file in your agent's data directory. Creates parent directories automatically. \
        Use subdirectories like 'outputs/', 'documents/', 'images/', etc. to organize files. \
        Example: 'outputs/comic.html' writes to {vault}/agent_data/{agent_id}/outputs/comic.html"
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Relative path for the file. Will be written under agent_data/{agent_id}/. Use subdirectories like 'outputs/', 'documents/', etc."
                },
                "content": {
                    "type": "string",
                    "description": "Content to write to the file"
                }
            },
            "required": ["path", "content"]
        }))
    }

    async fn execute(&self, ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
        // Check for error markers from truncated tool calls
        if let Some(error_type) = args.get("__error__").and_then(|v| v.as_str()) {
            let message = args.get("__message__").and_then(|v| v.as_str()).unwrap_or("Unknown error");
            let truncated = args.get("__truncated__").and_then(|v| v.as_bool()).unwrap_or(false);
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

        // Get agent_id from session state (required for all writes)
        let agent_id = ctx.get_state("app:agent_id")
            .and_then(|v| v.as_str().map(|s| s.to_owned()))
            .ok_or_else(|| zero_core::ZeroError::Tool(
                "Agent ID not found in state. Cannot write file.".to_string()
            ))?;

        tracing::info!(
            file = %file!(),
            line = %line!(),
            "Writing file: path='{}' ({} bytes), agent_id={}",
            path, content.len(), agent_id
        );

        // Resolve path: all paths go under agent_data/<agent_id>/
        let agent_data_dir = self.fs.agent_data_dir(&agent_id)
            .ok_or_else(|| zero_core::ZeroError::Tool(
                "Agent data directory not available".to_string()
            ))?;

        let final_path = agent_data_dir.join(path);

        // Create parent directories
        if let Some(parent) = final_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| zero_core::ZeroError::Tool(format!("Failed to create directories: {}", e)))?;
        }

        // Write the file
        tracing::info!(
            file = %file!(),
            line = %line!(),
            "Writing to file: {}",
            final_path.display()
        );
        std::fs::write(&final_path, content)
            .map_err(|e| zero_core::ZeroError::Tool(format!("Failed to write file: {}", e)))?;

        // Return the original requested path (not the absolute resolved path)
        // so the LLM continues to use relative paths
        Ok(json!({
            "path": path,
            "bytes_written": content.len(),
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
        "Edit a file in your agent's data directory by performing search and replace operations. \
        Files must exist within agent_data/{agent_id}/. Use the same relative path as when writing."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Relative path to the file to edit. Will be resolved under agent_data/{agent_id}/"
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

    async fn execute(&self, ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
        // Check for error markers from truncated tool calls
        if let Some(error_type) = args.get("__error__").and_then(|v| v.as_str()) {
            let message = args.get("__message__").and_then(|v| v.as_str()).unwrap_or("Unknown error");
            let truncated = args.get("__truncated__").and_then(|v| v.as_bool()).unwrap_or(false);
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

        // Get agent_id from session state (required for all edits)
        let agent_id = ctx.get_state("app:agent_id")
            .and_then(|v| v.as_str().map(|s| s.to_owned()))
            .ok_or_else(|| zero_core::ZeroError::Tool(
                "Agent ID not found in state. Cannot edit file.".to_string()
            ))?;

        tracing::info!(
            file = %file!(),
            line = %line!(),
            "Editing file: path='{}', agent_id={}",
            path, agent_id
        );

        // Resolve path: all paths go under agent_data/<agent_id>/
        let agent_data_dir = self.fs.agent_data_dir(&agent_id)
            .ok_or_else(|| zero_core::ZeroError::Tool(
                "Agent data directory not available".to_string()
            ))?;

        let final_path = agent_data_dir.join(path);

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
