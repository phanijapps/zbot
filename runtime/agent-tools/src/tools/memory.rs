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

/// Memory file name
const MEMORY_FILE: &str = "memory.json";

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

    /// Get the memory file path for an agent
    fn memory_path(&self, agent_id: &str) -> Option<PathBuf> {
        self.fs
            .agent_data_dir(agent_id)
            .map(|dir| dir.join(MEMORY_FILE))
    }

    /// Load memory store from disk
    fn load_store(&self, agent_id: &str) -> Result<MemoryStore> {
        let path = self
            .memory_path(agent_id)
            .ok_or_else(|| ZeroError::Tool("No agent data directory configured".to_string()))?;

        if !path.exists() {
            return Ok(MemoryStore::default());
        }

        let content = fs::read_to_string(&path)
            .map_err(|e| ZeroError::Tool(format!("Failed to read memory file: {}", e)))?;

        serde_json::from_str(&content)
            .map_err(|e| ZeroError::Tool(format!("Failed to parse memory file: {}", e)))
    }

    /// Save memory store to disk
    fn save_store(&self, agent_id: &str, store: &MemoryStore) -> Result<()> {
        let path = self
            .memory_path(agent_id)
            .ok_or_else(|| ZeroError::Tool("No agent data directory configured".to_string()))?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| ZeroError::Tool(format!("Failed to create directory: {}", e)))?;
        }

        let content = serde_json::to_string_pretty(store)
            .map_err(|e| ZeroError::Tool(format!("Failed to serialize memory: {}", e)))?;

        fs::write(&path, content)
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
        Use to remember important information about users, projects, or decisions. \
        Supports get, set, delete, list, and search actions."
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

        // Get action
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ZeroError::Tool("Missing 'action' parameter".to_string()))?;

        match action {
            "get" => self.action_get(&agent_id, &args).await,
            "set" => self.action_set(&agent_id, &args).await,
            "delete" => self.action_delete(&agent_id, &args).await,
            "list" => self.action_list(&agent_id, &args).await,
            "search" => self.action_search(&agent_id, &args).await,
            _ => Err(ZeroError::Tool(format!("Unknown action: {}", action))),
        }
    }
}

impl MemoryTool {
    /// Get a memory entry by key
    async fn action_get(&self, agent_id: &str, args: &Value) -> Result<Value> {
        let key = args
            .get("key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ZeroError::Tool("Missing 'key' parameter for get".to_string()))?;

        let store = self.load_store(agent_id)?;

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
    async fn action_set(&self, agent_id: &str, args: &Value) -> Result<Value> {
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

        let mut store = self.load_store(agent_id)?;

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
        self.save_store(agent_id, &store)?;

        Ok(json!({
            "success": true,
            "action": if is_update { "updated" } else { "created" },
            "key": key,
            "total_entries": store.entries.len(),
        }))
    }

    /// Delete a memory entry
    async fn action_delete(&self, agent_id: &str, args: &Value) -> Result<Value> {
        let key = args
            .get("key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ZeroError::Tool("Missing 'key' parameter for delete".to_string()))?;

        let mut store = self.load_store(agent_id)?;

        let deleted = store.entries.remove(key).is_some();

        if deleted {
            self.save_store(agent_id, &store)?;
        }

        Ok(json!({
            "success": deleted,
            "key": key,
            "message": if deleted { "Entry deleted" } else { "Entry not found" },
            "total_entries": store.entries.len(),
        }))
    }

    /// List all memory entries
    async fn action_list(&self, agent_id: &str, args: &Value) -> Result<Value> {
        let tag_filter = args.get("tag_filter").and_then(|v| v.as_str());

        let store = self.load_store(agent_id)?;

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
            "total": entries.len(),
            "entries": entries,
            "tag_filter": tag_filter,
        }))
    }

    /// Search memory entries
    async fn action_search(&self, agent_id: &str, args: &Value) -> Result<Value> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ZeroError::Tool("Missing 'query' parameter for search".to_string()))?;

        let query_lower = query.to_lowercase();
        let store = self.load_store(agent_id)?;

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
}
