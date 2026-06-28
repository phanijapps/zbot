// ============================================================================
// SEARCH TOOLS
// Grep and Glob tools
// ============================================================================

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use regex::Regex;
use serde_json::{Value, json};

use agent_primitives::{Result, Tool, ToolContext};

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
        "Search files for a regex pattern, or set literal=true for plain text."
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
                "literal": {
                    "type": "boolean",
                    "default": false,
                    "description": "Treat pattern as plain text instead of regex"
                },
                "max_results": {
                    "type": "integer",
                    "default": 100
                }
            },
            "required": ["pattern"]
        }))
    }

    async fn execute(&self, _ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
        let pattern = args
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                agent_primitives::AgentError::Tool("Missing 'pattern' parameter".to_string())
            })?;

        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");

        let case_insensitive = args
            .get("case_insensitive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let literal = args
            .get("literal")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let max_results = args
            .get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(100) as usize;

        tracing::debug!(
            "Grep: pattern={}, path={}, case_insensitive={}, literal={}",
            pattern,
            path,
            case_insensitive,
            literal
        );

        let regex = Self::compile_pattern(pattern, case_insensitive, literal)?;

        let mut results = Vec::new();

        // Simple recursive search
        let search_path = PathBuf::from(path);
        if search_path.is_file() {
            self.search_file(&search_path, &regex, &mut results, max_results)?;
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
        "txt", "md", "rs", "js", "ts", "jsx", "tsx", "py", "json", "yaml", "yml", "toml",
    ];

    fn compile_pattern(pattern: &str, case_insensitive: bool, literal: bool) -> Result<Regex> {
        let source_pattern = if literal {
            regex::escape(pattern)
        } else {
            pattern.to_string()
        };
        let compiled = if case_insensitive {
            format!("(?i){source_pattern}")
        } else {
            source_pattern.clone()
        };

        Regex::new(&compiled).map_err(|e| {
            agent_primitives::AgentError::Tool(format!(
                "Invalid regex pattern {source_pattern:?}: {e}. Set literal=true for plain-text search or use Rust regex syntax."
            ))
        })
    }

    fn search_directory(
        &self,
        dir: &PathBuf,
        regex: &Regex,
        results: &mut Vec<Value>,
        max_results: usize,
    ) -> Result<()> {
        if results.len() >= max_results {
            return Ok(());
        }

        let entries = std::fs::read_dir(dir).map_err(|e| {
            agent_primitives::AgentError::Tool(format!("Failed to read directory: {}", e))
        })?;

        for entry in entries.flatten() {
            let path = entry.path();

            if path.is_dir() {
                // Skip hidden directories and common exclusions
                if let Some(name) = path.file_name() {
                    let name_str = name.to_string_lossy();
                    if name_str.starts_with('.')
                        || name_str == "node_modules"
                        || name_str == "target"
                    {
                        continue;
                    }
                }
                self.search_directory(&path, regex, results, max_results)?;
            } else if let Some(ext) = path.extension() {
                let ext_str = ext.to_string_lossy();
                if Self::TEXT_EXTENSIONS.contains(&ext_str.as_ref()) {
                    self.search_file(&path, regex, results, max_results)?;
                    if results.len() >= max_results {
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    fn search_file(
        &self,
        path: &PathBuf,
        regex: &Regex,
        results: &mut Vec<Value>,
        max_results: usize,
    ) -> Result<()> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            agent_primitives::AgentError::Tool(format!("Failed to read file: {}", e))
        })?;

        for (line_num, line) in content.lines().enumerate() {
            if results.len() >= max_results {
                break;
            }
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

    async fn execute(&self, _ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
        let pattern = args
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                agent_primitives::AgentError::Tool("Missing 'pattern' parameter".to_string())
            })?;

        let _include_hidden = args
            .get("include_hidden")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        tracing::debug!("Glob: pattern={}", pattern);

        let matches = glob::glob(pattern)
            .map_err(|e| {
                agent_primitives::AgentError::Tool(format!("Invalid glob pattern: {}", e))
            })?
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grep_default_regex_does_not_inject_empty_inline_flag() {
        let regex = GrepTool::compile_pattern(r"^##|^###|^\\|", false, false)
            .expect("case-sensitive regex should compile directly");

        assert!(regex.is_match("## Heading"));
    }

    #[test]
    fn grep_invalid_regex_error_suggests_literal_mode() {
        let err = GrepTool::compile_pattern("(?", false, false)
            .expect_err("invalid regex should return a tool error");
        let message = err.to_string();

        assert!(message.contains("Invalid regex pattern"));
        assert!(message.contains("literal=true"));
    }

    #[test]
    fn grep_literal_mode_escapes_regex_metacharacters() {
        let regex = GrepTool::compile_pattern("a+b", false, true)
            .expect("literal search should compile escaped pattern");

        assert!(regex.is_match("a+b"));
        assert!(!regex.is_match("aaab"));
    }

    #[test]
    fn grep_single_file_respects_max_results() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let file = tmp.path().join("matches.txt");
        std::fs::write(&file, "hit one\nhit two\nhit three\n").expect("write fixture");
        let regex = GrepTool::compile_pattern("hit", false, false).expect("regex");
        let tool = GrepTool;
        let mut results = Vec::new();

        tool.search_file(&file, &regex, &mut results, 2)
            .expect("search file");

        assert_eq!(results.len(), 2);
    }
}
