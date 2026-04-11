// ============================================================================
// WARD TOOL
// Agent-managed project containers (named directories)
// ============================================================================

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{Value, json};

use zero_core::{
    FileSystemContext, MemoryFactStore, Result, Tool, ToolContext, ToolPermissions, ZeroError,
};

/// AGENTS.md file name - living readme for agent executions
const WARD_AGENTS_MD: &str = "AGENTS.md";

/// Tool for managing wards (named project directories).
///
/// Wards are persistent, agent-named project directories under `vault/wards/`.
/// The agent autonomously creates and switches between wards.
///
/// Actions:
/// - `use`: Switch to a ward (creates if needed), returns file listing
/// - `create`: Alias for `use` (semantically clearer for new wards)
/// - `list`: List all wards with descriptions
/// - `info`: Detailed info about a specific ward
pub struct WardTool {
    fs: Arc<dyn FileSystemContext>,
    fact_store: Option<Arc<dyn MemoryFactStore>>,
}

impl WardTool {
    /// Create a new WardTool with file system context and optional fact store.
    #[must_use]
    pub fn new(
        fs: Arc<dyn FileSystemContext>,
        fact_store: Option<Arc<dyn MemoryFactStore>>,
    ) -> Self {
        Self { fs, fact_store }
    }

    /// List files in a ward directory (non-recursive, top-level only).
    fn list_ward_files(&self, ward_dir: &std::path::Path) -> Vec<String> {
        let mut files = Vec::new();
        if let Ok(entries) = std::fs::read_dir(ward_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                // Skip hidden files
                if name.starts_with('.') {
                    continue;
                }
                if entry.path().is_dir() {
                    files.push(format!("{}/", name));
                } else {
                    files.push(name);
                }
            }
        }
        files.sort();
        files
    }

    /// Read AGENTS.md content from a ward directory, if it exists.
    fn read_agents_md(&self, ward_dir: &std::path::Path) -> Option<String> {
        let agents_md_path = ward_dir.join(WARD_AGENTS_MD);
        std::fs::read_to_string(&agents_md_path).ok()
    }

    /// Create AGENTS.md in a new ward, enriched with intent context if available.
    fn create_agents_md(&self, ward_dir: &std::path::Path, ward_name: &str, ctx: &dyn ToolContext) {
        let purpose = ctx
            .get_state("ward_purpose")
            .and_then(|v| v.as_str().map(String::from));
        let structure = ctx.get_state("ward_structure").and_then(|v| {
            v.as_object().map(|obj| {
                obj.iter()
                    .map(|(dir, desc)| format!("- `{}` — {}", dir, desc.as_str().unwrap_or("")))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
        });
        Self::write_agents_md(
            ward_dir,
            ward_name,
            purpose.as_deref(),
            structure.as_deref(),
        );
    }

    /// Write AGENTS.md with purpose and structure. Testable without ToolContext.
    fn write_agents_md(
        ward_dir: &std::path::Path,
        ward_name: &str,
        purpose: Option<&str>,
        structure: Option<&str>,
    ) {
        let agents_md_path = ward_dir.join(WARD_AGENTS_MD);
        if agents_md_path.exists() {
            return;
        }

        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

        let default_purpose = format!("Domain workspace for {} projects.", ward_name);
        let purpose = purpose.unwrap_or(&default_purpose);

        let mut content = format!(
            "# {name}\n\n## Purpose\n{purpose}\n",
            name = ward_name,
            purpose = purpose,
        );

        if let Some(structure) = structure {
            content.push_str(&format!("\n## Directory Layout\n{}\n", structure));
        }

        content.push_str(&format!(
            "\n## Core Modules\n*(auto-indexed after each session)*\n\n## History\n- {}: Ward created\n",
            today,
        ));

        if let Err(e) = std::fs::write(&agents_md_path, content) {
            tracing::warn!("Failed to create AGENTS.md in ward '{}': {}", ward_name, e);
        }
    }

    /// Recall facts relevant to the ward being entered.
    ///
    /// Best-effort: if no fact store is configured, or the recall fails,
    /// returns None and the ward switch still succeeds.
    async fn recall_ward_facts(
        &self,
        ward_name: &str,
        ctx: &Arc<dyn ToolContext>,
    ) -> Option<Value> {
        let store = self.fact_store.as_ref()?;

        let agent_id = ctx
            .get_state("app:agent_id")
            .and_then(|v| v.as_str().map(String::from))
            .or_else(|| {
                ctx.get_state("app:root_agent_id")
                    .and_then(|v| v.as_str().map(String::from))
            })?;

        let query = format!("ward {} context patterns corrections", ward_name);
        match store.recall_facts_prioritized(&agent_id, &query, 5).await {
            Ok(result) => {
                let count = result.get("count").and_then(|c| c.as_u64()).unwrap_or(0);
                if count > 0 {
                    tracing::info!(
                        "Ward-entry recall for '{}': {} facts loaded",
                        ward_name,
                        count
                    );
                    Some(result)
                } else {
                    None
                }
            }
            Err(e) => {
                tracing::warn!(
                    "Ward-entry recall failed for '{}': {} (non-fatal)",
                    ward_name,
                    e
                );
                None
            }
        }
    }

    /// Get a short description from AGENTS.md purpose section (if any).
    fn ward_description(&self, ward_dir: &std::path::Path) -> Option<String> {
        let content = self.read_agents_md(ward_dir)?;
        // Look for Purpose section and extract first non-empty, non-comment line
        let mut in_purpose = false;
        for line in content.lines() {
            if line.starts_with("## Purpose") {
                in_purpose = true;
                continue;
            }
            if in_purpose {
                if line.starts_with("## ") {
                    break; // Next section
                }
                let trimmed = line.trim();
                if !trimmed.is_empty() && !trimmed.starts_with("<!--") {
                    return Some(trimmed.to_string());
                }
            }
        }
        None
    }
}

#[async_trait]
impl Tool for WardTool {
    fn name(&self) -> &str {
        "ward"
    }

    fn description(&self) -> &str {
        "Manage code wards (named project directories). Wards persist across sessions.\n\
         Actions:\n\
         - use: Switch to a ward (creates if needed). Sets working directory for shell/write/edit.\n\
         - create: Alias for use. Creates and switches to a new ward.\n\
         - list: List all wards with descriptions.\n\
         - info: Detailed info about a specific ward."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["use", "create", "list", "info"],
                    "description": "The ward operation to perform"
                },
                "name": {
                    "type": "string",
                    "description": "Ward name (required for use, create, info). Use concise, descriptive names."
                }
            },
            "required": ["action"]
        }))
    }

    fn permissions(&self) -> ToolPermissions {
        ToolPermissions::safe()
    }

    async fn execute(&self, ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
        // Check for error markers from truncated/malformed tool calls
        if let Some(error_type) = args.get("__error__").and_then(|v| v.as_str()) {
            let message = args
                .get("__message__")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error");
            return Err(ZeroError::Tool(format!("{}: {}", error_type, message)));
        }

        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ZeroError::Tool("Missing 'action' parameter".to_string()))?;

        let wards_root = self
            .fs
            .wards_root_dir()
            .ok_or_else(|| ZeroError::Tool("Wards directory not configured".to_string()))?;

        match action {
            "use" | "create" => {
                let name = args.get("name").and_then(|v| v.as_str()).ok_or_else(|| {
                    ZeroError::Tool("Missing 'name' parameter for use/create".to_string())
                })?;

                // Validate ward name: alphanumeric, hyphens, underscores only
                if !name
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
                {
                    return Err(ZeroError::Tool(
                        "Ward name must contain only letters, numbers, hyphens, and underscores"
                            .to_string(),
                    ));
                }

                if name.is_empty() || name.len() > 64 {
                    return Err(ZeroError::Tool(
                        "Ward name must be 1-64 characters".to_string(),
                    ));
                }

                let ward_dir = wards_root.join(name);
                let created = !ward_dir.exists();

                // Subagents cannot create wards — only root can
                let is_delegated = ctx
                    .get_state("app:is_delegated")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if created && is_delegated {
                    return Err(ZeroError::Tool(format!(
                        "Subagents cannot create wards. Use the ward specified in your task: '{}' does not exist. \
                         Ask the root agent to create it first.",
                        name
                    )));
                }

                // Create ward directory if needed
                if created {
                    std::fs::create_dir_all(&ward_dir).map_err(|e| {
                        ZeroError::Tool(format!("Failed to create ward directory: {}", e))
                    })?;
                }

                // Create ward scaffold for new wards
                if created {
                    self.create_agents_md(&ward_dir, name, ctx.as_ref());
                    // Create mandatory directories
                    let _ = std::fs::create_dir_all(ward_dir.join("memory-bank"));
                    let _ = std::fs::create_dir_all(ward_dir.join("specs"));
                }

                // Set ward_id in context state
                ctx.set_state("ward_id".to_string(), json!(name));

                // List files in the ward
                let files = self.list_ward_files(&ward_dir);

                // Read AGENTS.md if it exists
                let agents_md = self.read_agents_md(&ward_dir);

                tracing::info!("Ward switched to '{}' (created: {})", name, created);

                // Best-effort recall of ward-scoped knowledge
                let ward_knowledge = self.recall_ward_facts(name, &ctx).await;

                // Return result with __ward_changed__ marker for the executor
                let mut result = json!({
                    "__ward_changed__": true,
                    "ward_id": name,
                    "action": if created { "created" } else { "switched" },
                    "files": files,
                    "file_count": files.len(),
                    "agents_md": agents_md,
                });

                if let Some(knowledge) = ward_knowledge {
                    result["ward_knowledge"] = knowledge;
                }

                // Nudge the agent to recall ward-specific knowledge
                result["recall_nudge"] = json!(format!(
                    "[Recall] You entered ward '{}'. Use the memory tool to recall ward-specific knowledge before proceeding.",
                    name
                ));

                Ok(result)
            }

            "list" => {
                let mut wards = Vec::new();

                if wards_root.exists()
                    && let Ok(entries) = std::fs::read_dir(&wards_root) {
                        for entry in entries.flatten() {
                            if entry.path().is_dir() {
                                let name = entry.file_name().to_string_lossy().to_string();
                                // Skip hidden directories (.venv, .node_env)
                                if name.starts_with('.') {
                                    continue;
                                }
                                let files = self.list_ward_files(&entry.path());
                                let description = self.ward_description(&entry.path());
                                wards.push(json!({
                                    "name": name,
                                    "files": files.len(),
                                    "description": description,
                                }));
                            }
                        }
                    }

                wards.sort_by(|a, b| {
                    a.get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .cmp(b.get("name").and_then(|v| v.as_str()).unwrap_or(""))
                });

                Ok(json!({
                    "wards": wards,
                    "total": wards.len(),
                }))
            }

            "info" => {
                let name = args.get("name").and_then(|v| v.as_str()).ok_or_else(|| {
                    ZeroError::Tool("Missing 'name' parameter for info".to_string())
                })?;

                let ward_dir = wards_root.join(name);
                if !ward_dir.exists() {
                    return Ok(json!({
                        "found": false,
                        "name": name,
                        "message": "Ward not found",
                    }));
                }

                let files = self.list_ward_files(&ward_dir);
                let agents_md = self.read_agents_md(&ward_dir);

                Ok(json!({
                    "found": true,
                    "name": name,
                    "files": files,
                    "file_count": files.len(),
                    "agents_md": agents_md,
                }))
            }

            _ => Err(ZeroError::Tool(format!("Unknown ward action: {}", action))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    struct TestFs {
        base: PathBuf,
    }

    impl FileSystemContext for TestFs {
        fn conversation_dir(&self, _id: &str) -> Option<PathBuf> {
            None
        }
        fn outputs_dir(&self) -> Option<PathBuf> {
            None
        }
        fn skills_dir(&self) -> Option<PathBuf> {
            None
        }
        fn agents_dir(&self) -> Option<PathBuf> {
            None
        }
        fn agent_data_dir(&self, _id: &str) -> Option<PathBuf> {
            None
        }
        fn python_executable(&self) -> Option<PathBuf> {
            None
        }
        fn vault_path(&self) -> Option<PathBuf> {
            Some(self.base.clone())
        }
    }

    #[test]
    fn test_list_ward_files_empty() {
        let dir = TempDir::new().unwrap();
        let fs = Arc::new(TestFs {
            base: dir.path().to_path_buf(),
        });
        let tool = WardTool::new(fs, None);
        let ward_dir = dir.path().join("wards").join("test");
        std::fs::create_dir_all(&ward_dir).unwrap();

        let files = tool.list_ward_files(&ward_dir);
        assert!(files.is_empty());
    }

    #[test]
    fn test_list_ward_files_with_content() {
        let dir = TempDir::new().unwrap();
        let fs = Arc::new(TestFs {
            base: dir.path().to_path_buf(),
        });
        let tool = WardTool::new(fs, None);
        let ward_dir = dir.path().join("wards").join("test");
        std::fs::create_dir_all(&ward_dir).unwrap();

        // Create some files
        std::fs::write(ward_dir.join("app.js"), "console.log('hi')").unwrap();
        std::fs::write(ward_dir.join("readme.md"), "# Test").unwrap();
        std::fs::create_dir(ward_dir.join("src")).unwrap();

        // Create hidden file (should be excluded)
        std::fs::write(ward_dir.join(".hidden_file"), "{}").unwrap();

        let files = tool.list_ward_files(&ward_dir);
        assert_eq!(files.len(), 3);
        assert!(files.contains(&"app.js".to_string()));
        assert!(files.contains(&"readme.md".to_string()));
        assert!(files.contains(&"src/".to_string()));
    }

    #[test]
    fn test_ward_description_from_agents_md() {
        let dir = TempDir::new().unwrap();
        let fs = Arc::new(TestFs {
            base: dir.path().to_path_buf(),
        });
        let tool = WardTool::new(fs, None);

        std::fs::write(
            dir.path().join("AGENTS.md"),
            "# My Project\n\n## Purpose\nStock tracker using yfinance\n\n## Structure\n",
        )
        .unwrap();

        let desc = tool.ward_description(dir.path());
        assert_eq!(desc, Some("Stock tracker using yfinance".to_string()));
    }

    #[test]
    fn test_ward_description_missing() {
        let dir = TempDir::new().unwrap();
        let fs = Arc::new(TestFs {
            base: dir.path().to_path_buf(),
        });
        let tool = WardTool::new(fs, None);

        let desc = tool.ward_description(dir.path());
        assert!(desc.is_none());
    }

    #[test]
    fn test_create_agents_md() {
        let ward_dir = TempDir::new().unwrap();
        let ward_path = ward_dir.path().to_path_buf();

        WardTool::write_agents_md(&ward_path, "test-project", None, None);

        let content = std::fs::read_to_string(ward_path.join("AGENTS.md")).unwrap();
        assert!(content.contains("# test-project"));
        assert!(content.contains("## Purpose"));
        assert!(content.contains("Domain workspace for test-project"));
        assert!(content.contains("auto-indexed"));
        assert!(content.contains("Ward created"));
        // Should NOT contain hardcoded Python conventions
        assert!(!content.contains("yfinance"));
        assert!(!content.contains("pandas"));
    }

    #[test]
    fn test_create_agents_md_with_context() {
        let ward_dir = TempDir::new().unwrap();
        let ward_path = ward_dir.path().to_path_buf();

        WardTool::write_agents_md(
            &ward_path,
            "financial-analysis",
            Some("Comprehensive investment analysis and professional reports"),
            Some(
                "- `core/` — Shared data fetching and analysis modules\n- `stocks/` — Per-ticker analysis\n- `output/` — Reports and charts",
            ),
        );

        let content = std::fs::read_to_string(ward_path.join("AGENTS.md")).unwrap();
        assert!(content.contains("# financial-analysis"));
        assert!(content.contains("Comprehensive investment analysis"));
        assert!(content.contains("Per-ticker analysis"));
        assert!(content.contains("output/"));
    }

    #[test]
    fn test_read_agents_md() {
        let dir = TempDir::new().unwrap();
        let fs = Arc::new(TestFs {
            base: dir.path().to_path_buf(),
        });
        let tool = WardTool::new(fs, None);

        std::fs::write(dir.path().join("AGENTS.md"), "# My Project\n\nTest content").unwrap();

        let content = tool.read_agents_md(dir.path());
        assert!(content.is_some());
        assert!(content.unwrap().contains("# My Project"));
    }

    #[test]
    fn test_create_agents_md_does_not_overwrite() {
        let ward_dir = TempDir::new().unwrap();
        let ward_path = ward_dir.path().to_path_buf();
        std::fs::write(ward_path.join("AGENTS.md"), "# Custom content").unwrap();

        WardTool::write_agents_md(&ward_path, "existing", None, None);

        let content = std::fs::read_to_string(ward_path.join("AGENTS.md")).unwrap();
        assert!(content.contains("# Custom content")); // Not overwritten
    }
}
