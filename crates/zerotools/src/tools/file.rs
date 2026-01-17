// ============================================================================
// FILE TOOLS
// Read, Write, and Edit tools
// ============================================================================

use std::sync::Arc;
use std::path::PathBuf;

use async_trait::async_trait;
use serde_json::{json, Value};

use agent_runtime::tools::{Tool, ToolExecError};
use agent_runtime::tools::context::ToolContext as BaseToolContext;
use agent_runtime::tools::error::ToolResult;
use agent_runtime::tools::builtin::FileSystemContext;

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

        // Security: Reject paths with parent directory components
        if path.contains("..") {
            return Err(ToolExecError::InvalidArguments(
                "Path cannot contain '..' for security reasons. Files must be written within the conversation directory.".to_string()
            ));
        }

        // Security: Reject absolute paths
        if path.starts_with('/') || path.starts_with('\\') {
            return Err(ToolExecError::InvalidArguments(
                "Absolute paths are not allowed. Use a relative path within the conversation directory.".to_string()
            ));
        }

        tracing::info!("Writing file: path='{}' ({} bytes), conversation_id={:?}", path, content.len(), ctx.conversation_id);

        // Handle outputs/ prefix
        let final_path = if path.starts_with("outputs/") {
            if let Some(outputs_dir) = self.fs.outputs_dir() {
                outputs_dir.join(&path[8..])
            } else {
                return Err(ToolExecError::ExecutionFailed(
                    "outputs/ directory is not configured".to_string()
                ));
            }
        } else {
            // All other paths must be within the conversation directory
            // Use FileSystemContext which knows the actual conversation directory
            let conv_id = ctx.conversation_id.as_deref().unwrap_or("");
            let conv_dir = self.fs.conversation_dir(conv_id);

            if let Some(dir) = conv_dir {
                dir.join(path)
            } else {
                return Err(ToolExecError::ExecutionFailed(
                    "No conversation context available. Cannot write file.".to_string()
                ));
            }
        };

        // Create parent directories
        if let Some(parent) = final_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ToolExecError::ExecutionFailed(format!("Failed to create directories: {}", e)))?;
        }

        // Write the file
        tracing::info!("Writing to file: {}", final_path.display());
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

        // Security: Reject paths with parent directory components
        if path.contains("..") {
            return Err(ToolExecError::InvalidArguments(
                "Path cannot contain '..' for security reasons. Files must be within the conversation directory.".to_string()
            ));
        }

        // Security: Reject absolute paths
        if path.starts_with('/') || path.starts_with('\\') {
            return Err(ToolExecError::InvalidArguments(
                "Absolute paths are not allowed. Use a relative path within the conversation directory.".to_string()
            ));
        }

        // Resolve path in conversation context (required)
        let final_path = if let Some(conv_dir) = ctx.conversation_dir() {
            conv_dir.join(path)
        } else {
            return Err(ToolExecError::ExecutionFailed(
                "No conversation context available. Cannot edit file.".to_string()
            ));
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
