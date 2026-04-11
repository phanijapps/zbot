// ============================================================================
// WRITE FILE TOOL
// Creates or overwrites a file with the given content.
// Simple, reliable — no diff format, no context matching.
// ============================================================================

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{Value, json};

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
        let mut path = path
            .trim_start_matches("~/")
            .trim_start_matches("/")
            .trim_start_matches("./")
            .to_string();

        // Reject paths that try to escape the ward
        if path.contains("..") {
            return Err(ZeroError::Tool(
                "Path cannot contain '..' — all paths must be relative to the ward".to_string(),
            ));
        }

        // Resolve CWD from ward context
        let cwd = resolve_ward_cwd(&self.fs, &ctx);

        // Fix path doubling: if the agent used an absolute path like
        // ~/Documents/zbot/wards/{ward}/specs/plan.md, the sanitizer strips ~/
        // but leaves Documents/zbot/wards/{ward}/specs/plan.md.
        // Detect and extract only the relative part after the ward directory.
        let cwd_str = cwd.to_string_lossy();
        if let Some(home) = dirs::home_dir() {
            let home_relative = cwd_str
                .trim_start_matches(home.to_string_lossy().as_ref())
                .trim_start_matches('/');
            if path.starts_with(home_relative) {
                path = path[home_relative.len()..]
                    .trim_start_matches('/')
                    .to_string();
                tracing::debug!(original = %args.get("path").and_then(|v| v.as_str()).unwrap_or(""), resolved = %path, "Fixed doubled ward path");
            }
        }
        // Also strip if path contains "wards/{ward_id}/" pattern
        if let Some(ward_pos) = path.find("wards/") {
            let after_wards = &path[ward_pos + 6..]; // skip "wards/"
            if let Some(slash) = after_wards.find('/') {
                let ward_relative = &after_wards[slash + 1..];
                if !ward_relative.is_empty() {
                    tracing::debug!(original = %path, resolved = %ward_relative, "Stripped wards/ prefix from path");
                    path = ward_relative.to_string();
                }
            }
        }

        let full_path = cwd.join(&path);

        // Create parent directories
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                ZeroError::Tool(format!("Failed to create directories for {}: {}", path, e))
            })?;
        }

        // Write the file
        let is_new = !full_path.exists();
        std::fs::write(&full_path, content)
            .map_err(|e| ZeroError::Tool(format!("Failed to write {}: {}", path, e)))?;

        let action = if is_new { "created" } else { "overwritten" };
        tracing::debug!("write_file: {} {} ({} bytes)", action, path, content.len());

        // Size warning for large files — nudge agent to split into modules
        let warning = if content.len() > 5120 {
            Some(format!(
                "⚠ This file is {}KB — consider splitting into smaller modules. Files > 5KB are harder to maintain and reuse.",
                content.len() / 1024
            ))
        } else {
            None
        };

        let mut result = json!({
            "success": true,
            "path": path,
            "action": action,
            "bytes": content.len(),
            "message": format!("File {} ({} bytes)", action, content.len())
        });
        if let Some(warn) = warning {
            result["warning"] = json!(warn);
        }
        Ok(result)
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
