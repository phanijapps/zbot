// ============================================================================
// MEMORY TOOL
// Persistent key-value storage for agents
// ============================================================================

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use zero_core::{FileSystemContext, Result, Tool, ToolContext, ToolPermissions, ZeroError};

// ============================================================================
// CONFIGURATION
// ============================================================================

/// Maximum number of memory entries per agent
const MAX_ENTRIES: usize = 1000;

/// Maximum size of a single entry value (100 KB)
const MAX_ENTRY_SIZE: usize = 100 * 1024;

/// Memory file name for agent-scoped memory
const MEMORY_FILE: &str = "memory.json";

/// Valid shared memory files
const SHARED_FILES: [&str; 4] = ["user_info", "workspace", "patterns", "session_summaries"];

// ============================================================================
// MEMORY ENTRY
// ============================================================================

/// A single memory entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// The stored value
    pub value: String,
    /// Optional tags for organization
    #[serde(default)]
    pub tags: Vec<String>,
    /// When the entry was created
    pub created_at: String,
    /// When the entry was last updated
    pub updated_at: String,
}

/// Memory store structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryStore {
    /// All memory entries keyed by name
    pub entries: HashMap<String, MemoryEntry>,
}

// ============================================================================
// MEMORY TOOL
// ============================================================================

/// Tool for persistent memory across sessions
pub struct MemoryTool {
    fs: Arc<dyn FileSystemContext>,
}

impl MemoryTool {
    /// Create a new MemoryTool with file system context
    #[must_use]
    pub fn new(fs: Arc<dyn FileSystemContext>) -> Self {
        Self { fs }
    }

    /// Get memory file path based on scope.
    ///
    /// - `scope="agent"` (default): `agents_data/{agent_id}/memory.json`
    /// - `scope="shared"`: `agents_data/shared/{file}.json`
    fn resolve_memory_path(
        &self,
        agent_id: &str,
        scope: &str,
        file: Option<&str>,
    ) -> Result<PathBuf> {
        match scope {
            "shared" => {
                let file = file.ok_or_else(|| {
                    ZeroError::Tool("'file' parameter required for shared scope".to_string())
                })?;

                // Validate file name
                if !SHARED_FILES.contains(&file) {
                    return Err(ZeroError::Tool(format!(
                        "Invalid shared file '{}'. Valid options: {}",
                        file,
                        SHARED_FILES.join(", ")
                    )));
                }

                self.fs
                    .vault_path()
                    .map(|p| {
                        p.join("agents_data")
                            .join("shared")
                            .join(format!("{}.json", file))
                    })
                    .ok_or_else(|| ZeroError::Tool("No vault path configured".to_string()))
            }
            "agent" | _ => self
                .fs
                .agent_data_dir(agent_id)
                .map(|dir| dir.join(MEMORY_FILE))
                .ok_or_else(|| {
                    ZeroError::Tool("No agent data directory configured".to_string())
                }),
        }
    }

    /// Load memory store from disk
    fn load_store_at_path(&self, path: &PathBuf) -> Result<MemoryStore> {
        if !path.exists() {
            return Ok(MemoryStore::default());
        }

        let content = fs::read_to_string(path)
            .map_err(|e| ZeroError::Tool(format!("Failed to read memory file: {}", e)))?;

        serde_json::from_str(&content)
            .map_err(|e| ZeroError::Tool(format!("Failed to parse memory file: {}", e)))
    }

    /// Save memory store to disk
    fn save_store_at_path(&self, path: &PathBuf, store: &MemoryStore) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| ZeroError::Tool(format!("Failed to create directory: {}", e)))?;
        }

        let content = serde_json::to_string_pretty(store)
            .map_err(|e| ZeroError::Tool(format!("Failed to serialize memory: {}", e)))?;

        fs::write(path, content)
            .map_err(|e| ZeroError::Tool(format!("Failed to write memory file: {}", e)))?;

        Ok(())
    }

    /// Get current timestamp
    fn now() -> String {
        chrono::Utc::now().to_rfc3339()
    }
}

#[async_trait]
impl Tool for MemoryTool {
    fn name(&self) -> &str {
        "memory"
    }

    fn description(&self) -> &str {
        "Persistent memory for storing facts, notes, and context across sessions. \
        Supports two scopes: 'agent' (default, per-agent) and 'shared' (cross-session). \
        Shared memory requires a 'file' parameter: user_info, workspace, patterns, or session_summaries."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["get", "set", "delete", "list", "search"],
                    "description": "The memory operation to perform"
                },
                "scope": {
                    "type": "string",
                    "enum": ["agent", "shared"],
                    "default": "agent",
                    "description": "Memory scope: 'agent' for agent-specific, 'shared' for cross-session"
                },
                "file": {
                    "type": "string",
                    "enum": ["user_info", "workspace", "patterns", "session_summaries"],
                    "description": "Shared memory file (required when scope is 'shared')"
                },
                "key": {
                    "type": "string",
                    "description": "Memory key (required for get, set, delete)"
                },
                "value": {
                    "type": "string",
                    "description": "Value to store (required for set)"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional tags for organization (for set)"
                },
                "query": {
                    "type": "string",
                    "description": "Search query (for search action)"
                },
                "tag_filter": {
                    "type": "string",
                    "description": "Filter by tag (for list action)"
                }
            },
            "required": ["action"]
        }))
    }

    fn permissions(&self) -> ToolPermissions {
        ToolPermissions::safe()
    }

    async fn execute(&self, ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
        // Get agent ID from context
        let agent_id = ctx
            .get_state("app:agent_id")
            .and_then(|v| v.as_str().map(String::from))
            .or_else(|| {
                ctx.get_state("app:root_agent_id")
                    .and_then(|v| v.as_str().map(String::from))
            })
            .ok_or_else(|| ZeroError::Tool("No agent ID in context".to_string()))?;

        // Get scope and file parameters
        let scope = args
            .get("scope")
            .and_then(|v| v.as_str())
            .unwrap_or("agent");
        let file = args.get("file").and_then(|v| v.as_str());

        // Resolve memory path
        let path = self.resolve_memory_path(&agent_id, scope, file)?;

        // Get action
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ZeroError::Tool("Missing 'action' parameter".to_string()))?;

        match action {
            "get" => self.action_get(&path, &args).await,
            "set" => self.action_set(&path, &args).await,
            "delete" => self.action_delete(&path, &args).await,
            "list" => self.action_list(&path, scope, file, &args).await,
            "search" => self.action_search(&path, &args).await,
            _ => Err(ZeroError::Tool(format!("Unknown action: {}", action))),
        }
    }
}

impl MemoryTool {
    /// Get a memory entry by key
    async fn action_get(&self, path: &PathBuf, args: &Value) -> Result<Value> {
        let key = args
            .get("key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ZeroError::Tool("Missing 'key' parameter for get".to_string()))?;

        let store = self.load_store_at_path(path)?;

        match store.entries.get(key) {
            Some(entry) => Ok(json!({
                "found": true,
                "key": key,
                "value": entry.value,
                "tags": entry.tags,
                "created_at": entry.created_at,
                "updated_at": entry.updated_at,
            })),
            None => Ok(json!({
                "found": false,
                "key": key,
                "message": "Memory entry not found"
            })),
        }
    }

    /// Set a memory entry
    async fn action_set(&self, path: &PathBuf, args: &Value) -> Result<Value> {
        let key = args
            .get("key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ZeroError::Tool("Missing 'key' parameter for set".to_string()))?;

        let value = args
            .get("value")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ZeroError::Tool("Missing 'value' parameter for set".to_string()))?;

        // Check value size
        if value.len() > MAX_ENTRY_SIZE {
            return Err(ZeroError::Tool(format!(
                "Value too large: {} bytes (max: {} bytes)",
                value.len(),
                MAX_ENTRY_SIZE
            )));
        }

        let tags: Vec<String> = args
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let mut store = self.load_store_at_path(path)?;

        // Check entry limit (only for new entries)
        if !store.entries.contains_key(key) && store.entries.len() >= MAX_ENTRIES {
            return Err(ZeroError::Tool(format!(
                "Memory limit reached: {} entries (max: {})",
                store.entries.len(),
                MAX_ENTRIES
            )));
        }

        let now = Self::now();
        let is_update = store.entries.contains_key(key);

        let entry = MemoryEntry {
            value: value.to_string(),
            tags,
            created_at: store
                .entries
                .get(key)
                .map(|e| e.created_at.clone())
                .unwrap_or_else(|| now.clone()),
            updated_at: now,
        };

        store.entries.insert(key.to_string(), entry);
        self.save_store_at_path(path, &store)?;

        Ok(json!({
            "success": true,
            "action": if is_update { "updated" } else { "created" },
            "key": key,
            "total_entries": store.entries.len(),
        }))
    }

    /// Delete a memory entry
    async fn action_delete(&self, path: &PathBuf, args: &Value) -> Result<Value> {
        let key = args
            .get("key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ZeroError::Tool("Missing 'key' parameter for delete".to_string()))?;

        let mut store = self.load_store_at_path(path)?;

        let deleted = store.entries.remove(key).is_some();

        if deleted {
            self.save_store_at_path(path, &store)?;
        }

        Ok(json!({
            "success": deleted,
            "key": key,
            "message": if deleted { "Entry deleted" } else { "Entry not found" },
            "total_entries": store.entries.len(),
        }))
    }

    /// List all memory entries
    async fn action_list(
        &self,
        path: &PathBuf,
        scope: &str,
        file: Option<&str>,
        args: &Value,
    ) -> Result<Value> {
        let tag_filter = args.get("tag_filter").and_then(|v| v.as_str());

        let store = self.load_store_at_path(path)?;

        let entries: Vec<Value> = store
            .entries
            .iter()
            .filter(|(_, entry)| {
                tag_filter
                    .map(|tag| entry.tags.iter().any(|t| t.contains(tag)))
                    .unwrap_or(true)
            })
            .map(|(key, entry)| {
                json!({
                    "key": key,
                    "value_preview": if entry.value.len() > 100 {
                        format!("{}...", &entry.value[..100])
                    } else {
                        entry.value.clone()
                    },
                    "tags": entry.tags,
                    "updated_at": entry.updated_at,
                })
            })
            .collect();

        Ok(json!({
            "scope": scope,
            "file": file,
            "total": entries.len(),
            "entries": entries,
            "tag_filter": tag_filter,
        }))
    }

    /// Search memory entries
    async fn action_search(&self, path: &PathBuf, args: &Value) -> Result<Value> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ZeroError::Tool("Missing 'query' parameter for search".to_string()))?;

        let query_lower = query.to_lowercase();
        let store = self.load_store_at_path(path)?;

        let matches: Vec<Value> = store
            .entries
            .iter()
            .filter(|(key, entry)| {
                key.to_lowercase().contains(&query_lower)
                    || entry.value.to_lowercase().contains(&query_lower)
                    || entry.tags.iter().any(|t| t.to_lowercase().contains(&query_lower))
            })
            .map(|(key, entry)| {
                json!({
                    "key": key,
                    "value": entry.value,
                    "tags": entry.tags,
                    "updated_at": entry.updated_at,
                })
            })
            .collect();

        Ok(json!({
            "query": query,
            "matches": matches.len(),
            "results": matches,
        }))
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Test file system context that uses a temp directory
    struct TestFileSystem {
        base_dir: PathBuf,
    }

    impl TestFileSystem {
        fn new(base_dir: PathBuf) -> Self {
            Self { base_dir }
        }
    }

    impl FileSystemContext for TestFileSystem {
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
        fn agent_data_dir(&self, agent_id: &str) -> Option<PathBuf> {
            Some(self.base_dir.join("agents_data").join(agent_id))
        }
        fn python_executable(&self) -> Option<PathBuf> {
            None
        }
        fn vault_path(&self) -> Option<PathBuf> {
            Some(self.base_dir.clone())
        }
    }

    #[test]
    fn test_memory_entry_serialization() {
        let entry = MemoryEntry {
            value: "test value".to_string(),
            tags: vec!["tag1".to_string(), "tag2".to_string()],
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&entry).unwrap();
        let parsed: MemoryEntry = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.value, "test value");
        assert_eq!(parsed.tags.len(), 2);
    }

    #[test]
    fn test_memory_store_default() {
        let store = MemoryStore::default();
        assert!(store.entries.is_empty());
    }

    #[test]
    fn test_resolve_agent_memory_path() {
        let dir = TempDir::new().unwrap();
        let fs = Arc::new(TestFileSystem::new(dir.path().to_path_buf()));
        let tool = MemoryTool::new(fs);

        let path = tool.resolve_memory_path("test-agent", "agent", None).unwrap();
        assert!(path.ends_with("agents_data/test-agent/memory.json"));
    }

    #[test]
    fn test_resolve_shared_memory_path() {
        let dir = TempDir::new().unwrap();
        let fs = Arc::new(TestFileSystem::new(dir.path().to_path_buf()));
        let tool = MemoryTool::new(fs);

        let path = tool
            .resolve_memory_path("test-agent", "shared", Some("patterns"))
            .unwrap();
        assert!(path.ends_with("agents_data/shared/patterns.json"));
    }

    #[test]
    fn test_shared_memory_requires_file() {
        let dir = TempDir::new().unwrap();
        let fs = Arc::new(TestFileSystem::new(dir.path().to_path_buf()));
        let tool = MemoryTool::new(fs);

        let result = tool.resolve_memory_path("test-agent", "shared", None);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("'file' parameter required"));
    }

    #[test]
    fn test_shared_memory_invalid_file() {
        let dir = TempDir::new().unwrap();
        let fs = Arc::new(TestFileSystem::new(dir.path().to_path_buf()));
        let tool = MemoryTool::new(fs);

        let result = tool.resolve_memory_path("test-agent", "shared", Some("invalid_file"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid shared file"));
    }

    #[test]
    fn test_all_shared_files_valid() {
        let dir = TempDir::new().unwrap();
        let fs = Arc::new(TestFileSystem::new(dir.path().to_path_buf()));
        let tool = MemoryTool::new(fs);

        for file in SHARED_FILES {
            let result = tool.resolve_memory_path("test-agent", "shared", Some(file));
            assert!(result.is_ok(), "Failed for file: {}", file);
        }
    }

    #[test]
    fn test_load_store_creates_default_when_missing() {
        let dir = TempDir::new().unwrap();
        let fs = Arc::new(TestFileSystem::new(dir.path().to_path_buf()));
        let tool = MemoryTool::new(fs);

        let path = dir.path().join("nonexistent").join("memory.json");
        let store = tool.load_store_at_path(&path).unwrap();
        assert!(store.entries.is_empty());
    }

    #[test]
    fn test_save_and_load_store() {
        let dir = TempDir::new().unwrap();
        let fs = Arc::new(TestFileSystem::new(dir.path().to_path_buf()));
        let tool = MemoryTool::new(fs);

        let path = dir.path().join("test_memory.json");

        let mut store = MemoryStore::default();
        store.entries.insert(
            "key1".to_string(),
            MemoryEntry {
                value: "value1".to_string(),
                tags: vec!["tag1".to_string()],
                created_at: "2024-01-01T00:00:00Z".to_string(),
                updated_at: "2024-01-01T00:00:00Z".to_string(),
            },
        );

        tool.save_store_at_path(&path, &store).unwrap();

        let loaded = tool.load_store_at_path(&path).unwrap();
        assert_eq!(loaded.entries.len(), 1);
        assert_eq!(loaded.entries.get("key1").unwrap().value, "value1");
    }
}
