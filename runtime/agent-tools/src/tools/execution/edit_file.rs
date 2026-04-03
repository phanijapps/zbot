// ============================================================================
// EDIT FILE TOOL
// Find and replace text in an existing file.
// Simple string matching — no diff format, no context lines.
// ============================================================================

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use zero_core::{FileSystemContext, Result, Tool, ToolContext, ZeroError};

use super::apply_patch::resolve_ward_cwd;

/// Tool that performs find-and-replace edits on existing files.
pub struct EditFileTool {
    fs: Arc<dyn FileSystemContext>,
}

impl EditFileTool {
    pub fn new(fs: Arc<dyn FileSystemContext>) -> Self {
        Self { fs }
    }
}

#[async_trait]
impl Tool for EditFileTool {
    fn name(&self) -> &str {
        "edit_file"
    }

    fn description(&self) -> &str {
        "Edit an existing file by finding and replacing text. Provide the exact text to find and the replacement text."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path relative to the ward"
                },
                "old_text": {
                    "type": "string",
                    "description": "Exact text to find in the file (must match exactly)"
                },
                "new_text": {
                    "type": "string",
                    "description": "Replacement text"
                }
            },
            "required": ["path", "old_text", "new_text"]
        }))
    }

    async fn execute(&self, ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ZeroError::Tool("Missing 'path' parameter".to_string()))?;

        let old_text = args
            .get("old_text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ZeroError::Tool("Missing 'old_text' parameter".to_string()))?;

        let new_text = args
            .get("new_text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ZeroError::Tool("Missing 'new_text' parameter".to_string()))?;

        if path.is_empty() {
            return Err(ZeroError::Tool("Path cannot be empty".to_string()));
        }

        if old_text.is_empty() {
            return Err(ZeroError::Tool("old_text cannot be empty".to_string()));
        }

        // Resolve CWD from ward context
        let cwd = resolve_ward_cwd(&self.fs, &ctx);
        let full_path = cwd.join(path);

        if !full_path.exists() {
            return Ok(json!({
                "success": false,
                "error": format!("File not found: {}", path)
            }));
        }

        // Read the file
        let content = std::fs::read_to_string(&full_path).map_err(|e| {
            ZeroError::Tool(format!("Failed to read {}: {}", path, e))
        })?;

        // Count occurrences
        let count = content.matches(old_text).count();

        if count == 0 {
            // Try trimmed match as fallback
            let trimmed_old = old_text.trim();
            let trimmed_count = content.matches(trimmed_old).count();
            if trimmed_count > 0 {
                let new_content = content.replacen(trimmed_old, new_text, 1);
                std::fs::write(&full_path, &new_content).map_err(|e| {
                    ZeroError::Tool(format!("Failed to write {}: {}", path, e))
                })?;
                tracing::debug!("edit_file: replaced (trimmed match) in {}", path);
                return Ok(json!({
                    "success": true,
                    "path": path,
                    "replacements": 1,
                    "match_type": "trimmed",
                    "message": format!("Replaced 1 occurrence (trimmed match) in {}", path)
                }));
            }

            return Ok(json!({
                "success": false,
                "error": format!("old_text not found in {}. The text must match exactly.", path),
                "hint": "Use grep to find the exact text first, then copy it precisely."
            }));
        }

        // Replace first occurrence only (safer — avoids unintended bulk replacements)
        let new_content = content.replacen(old_text, new_text, 1);

        std::fs::write(&full_path, &new_content).map_err(|e| {
            ZeroError::Tool(format!("Failed to write {}: {}", path, e))
        })?;

        tracing::debug!("edit_file: replaced in {} ({} total occurrences, replaced 1)", path, count);

        Ok(json!({
            "success": true,
            "path": path,
            "replacements": 1,
            "total_occurrences": count,
            "message": if count > 1 {
                format!("Replaced 1 of {} occurrences in {}", count, path)
            } else {
                format!("Replaced 1 occurrence in {}", path)
            }
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edit_file_schema() {
        let tool = EditFileTool::new(Arc::new(zero_core::NoFileSystemContext));
        assert_eq!(tool.name(), "edit_file");
        let schema = tool.parameters_schema().unwrap();
        assert!(schema["properties"]["path"].is_object());
        assert!(schema["properties"]["old_text"].is_object());
        assert!(schema["properties"]["new_text"].is_object());
    }
}
