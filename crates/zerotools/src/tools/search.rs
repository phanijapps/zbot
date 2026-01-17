// ============================================================================
// SEARCH TOOLS
// Grep and Glob tools
// ============================================================================

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use regex::Regex;

use agent_runtime::tools::{Tool, ToolExecError};
use agent_runtime::tools::context::ToolContext as BaseToolContext;
use agent_runtime::tools::error::ToolResult;

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
