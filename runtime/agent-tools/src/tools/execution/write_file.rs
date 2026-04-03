// ============================================================================
// WRITE FILE TOOL
// Creates or overwrites a file with the given content.
// Simple, reliable — no diff format, no context matching.
// ============================================================================

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use zero_core::{FileSystemContext, Result, Tool, ToolContext, ZeroError};

use super::apply_patch::resolve_ward_cwd;

/// Tool that creates or overwrites a file with content.
pub struct WriteFileTool {
    fs: Arc<dyn FileSystemContext>,
}

impl WriteFileTool {
    pub fn new(fs: Arc<dyn FileSystemContext>) -> Self {
        Self { fs }
    }
}

#[async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Create or overwrite a file with the given content. Path is relative to the current ward."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path relative to the ward (e.g., 'core/utils.py', 'data/config.json')"
                },
                "content": {
                    "type": "string",
                    "description": "Complete file content to write"
                }
            },
            "required": ["path", "content"]
        }))
    }

    async fn execute(&self, ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ZeroError::Tool("Missing 'path' parameter".to_string()))?;

        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ZeroError::Tool("Missing 'content' parameter".to_string()))?;

        if path.is_empty() {
            return Err(ZeroError::Tool("Path cannot be empty".to_string()));
        }

        // Sanitize path — reject absolute paths and home-relative paths
        let path = path.trim_start_matches("~/")
            .trim_start_matches("/")
            .trim_start_matches("./");

        // Reject paths that try to escape the ward
        if path.contains("..") {
            return Err(ZeroError::Tool("Path cannot contain '..' — all paths must be relative to the ward".to_string()));
        }

        // Resolve CWD from ward context
        let cwd = resolve_ward_cwd(&self.fs, &ctx);
        let full_path = cwd.join(path);

        // Create parent directories
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                ZeroError::Tool(format!("Failed to create directories for {}: {}", path, e))
            })?;
        }

        // Write the file
        let is_new = !full_path.exists();
        std::fs::write(&full_path, content).map_err(|e| {
            ZeroError::Tool(format!("Failed to write {}: {}", path, e))
        })?;

        let action = if is_new { "created" } else { "overwritten" };
        tracing::debug!("write_file: {} {} ({} bytes)", action, path, content.len());

        Ok(json!({
            "success": true,
            "path": path,
            "action": action,
            "bytes": content.len(),
            "message": format!("File {} ({} bytes)", action, content.len())
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_file_schema() {
        let tool = WriteFileTool::new(Arc::new(zero_core::NoFileSystemContext));
        assert_eq!(tool.name(), "write_file");
        let schema = tool.parameters_schema().unwrap();
        assert!(schema["properties"]["path"].is_object());
        assert!(schema["properties"]["content"].is_object());
    }
}
