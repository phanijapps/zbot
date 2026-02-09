// ============================================================================
// WARD TOOL
// Agent-managed project containers (named directories)
// ============================================================================

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use zero_core::{FileSystemContext, Result, Tool, ToolContext, ToolPermissions, ZeroError};

/// Ward memory file name (hidden file inside each ward)
const WARD_MEMORY_FILE: &str = ".ward_memory.json";

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
}

impl WardTool {
    /// Create a new WardTool with file system context.
    #[must_use]
    pub fn new(fs: Arc<dyn FileSystemContext>) -> Self {
        Self { fs }
    }

    /// List files in a ward directory (non-recursive, top-level only).
    fn list_ward_files(&self, ward_dir: &std::path::Path) -> Vec<String> {
        let mut files = Vec::new();
        if let Ok(entries) = std::fs::read_dir(ward_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                // Skip hidden files (like .ward_memory.json)
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

    /// Load ward memory from `.ward_memory.json`.
    fn load_ward_memory(&self, ward_dir: &std::path::Path) -> Value {
        let memory_path = ward_dir.join(WARD_MEMORY_FILE);
        if memory_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&memory_path) {
                if let Ok(parsed) = serde_json::from_str::<Value>(&content) {
                    return parsed;
                }
            }
        }
        json!({})
    }

    /// Get a short description from ward memory (if any).
    fn ward_description(&self, ward_dir: &std::path::Path) -> Option<String> {
        let memory = self.load_ward_memory(ward_dir);
        memory
            .get("entries")
            .and_then(|e| e.get("purpose"))
            .and_then(|p| p.get("value"))
            .and_then(|v| v.as_str())
            .map(String::from)
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
            let message = args.get("__message__").and_then(|v| v.as_str()).unwrap_or("Unknown error");
            return Err(ZeroError::Tool(format!("{}: {}", error_type, message)));
        }

        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ZeroError::Tool("Missing 'action' parameter".to_string()))?;

        let wards_root = self.fs.wards_root_dir().ok_or_else(|| {
            ZeroError::Tool("Wards directory not configured".to_string())
        })?;

        match action {
            "use" | "create" => {
                let name = args
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
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

                // Create ward directory if needed
                if created {
                    std::fs::create_dir_all(&ward_dir).map_err(|e| {
                        ZeroError::Tool(format!("Failed to create ward directory: {}", e))
                    })?;
                }

                // Set ward_id in context state
                ctx.set_state("ward_id".to_string(), json!(name));

                // List files in the ward
                let files = self.list_ward_files(&ward_dir);

                // Load ward memory
                let memory = self.load_ward_memory(&ward_dir);

                tracing::info!("Ward switched to '{}' (created: {})", name, created);

                // Return result with __ward_changed__ marker for the executor
                Ok(json!({
                    "__ward_changed__": true,
                    "ward_id": name,
                    "action": if created { "created" } else { "switched" },
                    "files": files,
                    "file_count": files.len(),
                    "ward_memory": memory,
                }))
            }

            "list" => {
                let mut wards = Vec::new();

                if wards_root.exists() {
                    if let Ok(entries) = std::fs::read_dir(&wards_root) {
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
                let name = args
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
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
                let memory = self.load_ward_memory(&ward_dir);

                Ok(json!({
                    "found": true,
                    "name": name,
                    "files": files,
                    "file_count": files.len(),
                    "ward_memory": memory,
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
        let tool = WardTool::new(fs);
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
        let tool = WardTool::new(fs);
        let ward_dir = dir.path().join("wards").join("test");
        std::fs::create_dir_all(&ward_dir).unwrap();

        // Create some files
        std::fs::write(ward_dir.join("app.js"), "console.log('hi')").unwrap();
        std::fs::write(ward_dir.join("readme.md"), "# Test").unwrap();
        std::fs::create_dir(ward_dir.join("src")).unwrap();

        // Create hidden file (should be excluded)
        std::fs::write(ward_dir.join(".ward_memory.json"), "{}").unwrap();

        let files = tool.list_ward_files(&ward_dir);
        assert_eq!(files.len(), 3);
        assert!(files.contains(&"app.js".to_string()));
        assert!(files.contains(&"readme.md".to_string()));
        assert!(files.contains(&"src/".to_string()));
    }

    #[test]
    fn test_load_ward_memory_missing() {
        let dir = TempDir::new().unwrap();
        let fs = Arc::new(TestFs {
            base: dir.path().to_path_buf(),
        });
        let tool = WardTool::new(fs);

        let memory = tool.load_ward_memory(dir.path());
        assert_eq!(memory, json!({}));
    }

    #[test]
    fn test_load_ward_memory_with_data() {
        let dir = TempDir::new().unwrap();
        let fs = Arc::new(TestFs {
            base: dir.path().to_path_buf(),
        });
        let tool = WardTool::new(fs);

        let memory_data = json!({
            "entries": {
                "purpose": {
                    "value": "Stock tracker using yfinance",
                    "tags": [],
                    "created_at": "2024-01-01T00:00:00Z",
                    "updated_at": "2024-01-01T00:00:00Z"
                }
            }
        });
        std::fs::write(
            dir.path().join(WARD_MEMORY_FILE),
            serde_json::to_string(&memory_data).unwrap(),
        )
        .unwrap();

        let memory = tool.load_ward_memory(dir.path());
        assert!(memory.get("entries").is_some());

        let desc = tool.ward_description(dir.path());
        assert_eq!(desc, Some("Stock tracker using yfinance".to_string()));
    }

    #[test]
    fn test_ward_description_missing() {
        let dir = TempDir::new().unwrap();
        let fs = Arc::new(TestFs {
            base: dir.path().to_path_buf(),
        });
        let tool = WardTool::new(fs);

        let desc = tool.ward_description(dir.path());
        assert!(desc.is_none());
    }
}
