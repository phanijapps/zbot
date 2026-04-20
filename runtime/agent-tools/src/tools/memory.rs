// ============================================================================
// MEMORY TOOL
// Persistent key-value storage for agents + structured fact storage via DB
// ============================================================================

use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use fs2::FileExt;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use zero_core::{
    FileSystemContext, MemoryFactStore, Result, Tool, ToolContext, ToolPermissions, ZeroError,
};

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
    fact_store: Option<Arc<dyn MemoryFactStore>>,
}

impl MemoryTool {
    /// Create a new MemoryTool with file system context and optional fact store.
    #[must_use]
    pub fn new(
        fs: Arc<dyn FileSystemContext>,
        fact_store: Option<Arc<dyn MemoryFactStore>>,
    ) -> Self {
        Self { fs, fact_store }
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
            _ => self
                .fs
                .agent_data_dir(agent_id)
                .map(|dir| dir.join(MEMORY_FILE))
                .ok_or_else(|| ZeroError::Tool("No agent data directory configured".to_string())),
        }
    }

    /// Load memory store from disk
    /// Load memory store with shared lock (allows concurrent reads).
    fn load_store_at_path(&self, path: &PathBuf) -> Result<MemoryStore> {
        if !path.exists() {
            return Ok(MemoryStore::default());
        }

        let file = File::open(path)
            .map_err(|e| ZeroError::Tool(format!("Failed to open memory file: {}", e)))?;

        // Acquire shared lock (allows other readers)
        file.lock_shared()
            .map_err(|e| ZeroError::Tool(format!("Failed to lock memory file: {}", e)))?;

        let mut content = String::new();
        (&file)
            .read_to_string(&mut content)
            .map_err(|e| ZeroError::Tool(format!("Failed to read memory file: {}", e)))?;

        // Lock released when file is dropped
        serde_json::from_str(&content)
            .map_err(|e| ZeroError::Tool(format!("Failed to parse memory file: {}", e)))
    }

    /// Save memory store with exclusive lock (blocks other readers/writers).
    fn save_store_at_path(&self, path: &PathBuf, store: &MemoryStore) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| ZeroError::Tool(format!("Failed to create directory: {}", e)))?;
        }

        let content = serde_json::to_string_pretty(store)
            .map_err(|e| ZeroError::Tool(format!("Failed to serialize memory: {}", e)))?;

        // Open/create file with write access
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
            .map_err(|e| {
                ZeroError::Tool(format!("Failed to open memory file for writing: {}", e))
            })?;

        // Acquire exclusive lock (blocks all other access)
        file.lock_exclusive()
            .map_err(|e| ZeroError::Tool(format!("Failed to lock memory file: {}", e)))?;

        file.write_all(content.as_bytes())
            .map_err(|e| ZeroError::Tool(format!("Failed to write memory file: {}", e)))?;

        // Lock released when file is dropped
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
        Actions: get/set/delete/list/search (key-value store), \
        save_fact (structured fact with category/key/content/confidence — automatically embedded for semantic search), \
        recall (hybrid semantic + keyword search over saved facts), \
        get_fact (exact-key lookup for ctx-namespaced session state — use this to fetch intent/prompt/plan/state.<exec_id> by precise key). \
        Scopes: 'agent' (default), 'shared' (cross-session). \
        Shared memory requires a 'file' parameter: user_info, workspace, patterns, or session_summaries."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["get", "set", "delete", "list", "search", "save_fact", "recall", "get_fact"],
                    "description": "The memory operation to perform"
                },
                "category": {
                    "type": "string",
                    "enum": ["user", "pattern", "domain", "instruction", "correction", "ctx"],
                    "description": "Fact category (for save_fact action). 'ctx' is reserved for session state — root writes canonicals (intent/prompt/plan); subagents can only write state.<exec_id> under their own session."
                },
                "content": {
                    "type": "string",
                    "description": "Fact content — 1-2 sentence description (for save_fact action)"
                },
                "confidence": {
                    "type": "number",
                    "description": "Confidence 0.0-1.0 (for save_fact, default 0.8)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum results to return (for recall, default 5)"
                },
                "scope": {
                    "type": "string",
                    "enum": ["agent", "shared"],
                    "default": "agent",
                    "description": "Memory scope: 'agent' (per-agent), 'shared' (cross-session)"
                },
                "file": {
                    "type": "string",
                    "enum": ["user_info", "workspace", "patterns", "session_summaries"],
                    "description": "Shared memory file (required when scope is 'shared')"
                },
                "key": {
                    "type": "string",
                    "description": "Memory key (required for get, set, delete, save_fact). For save_fact use dot-notation like 'user.preferred_format'"
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
                    "description": "Search query (for search/recall action)"
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
        // Check for error markers from truncated/malformed tool calls
        if let Some(error_type) = args.get("__error__").and_then(|v| v.as_str()) {
            let message = args
                .get("__message__")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error");
            return Err(ZeroError::Tool(format!("{}: {}", error_type, message)));
        }

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

        // Resolve memory path (only needed for KV actions, not save_fact/recall)
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ZeroError::Tool("Missing 'action' parameter".to_string()))?;

        match action {
            "get" | "set" | "delete" | "list" | "search" => {
                let path = self.resolve_memory_path(&agent_id, scope, file)?;
                match action {
                    "get" => self.action_get(&path, &args).await,
                    "set" => self.action_set(&path, &args).await,
                    "delete" => self.action_delete(&path, &args).await,
                    "list" => self.action_list(&path, scope, file, &args).await,
                    "search" => self.action_search(&path, &args).await,
                    _ => unreachable!(),
                }
            }
            "save_fact" => self.action_save_fact(ctx.as_ref(), &agent_id, &args).await,
            "recall" => self.action_recall(ctx.as_ref(), &agent_id, &args).await,
            "get_fact" => self.action_get_fact(ctx.as_ref(), &args).await,
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
                        format!("{}...", zero_core::truncate_str(&entry.value, 100))
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

    /// Save a structured memory fact via the DB-backed fact store.
    async fn action_save_fact(
        &self,
        ctx: &dyn ToolContext,
        agent_id: &str,
        args: &Value,
    ) -> Result<Value> {
        let category = args
            .get("category")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ZeroError::Tool("Missing 'category' for save_fact".to_string()))?;

        let key = args
            .get("key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ZeroError::Tool("Missing 'key' for save_fact".to_string()))?;

        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ZeroError::Tool("Missing 'content' for save_fact".to_string()))?;

        let confidence = args
            .get("confidence")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.8);

        // Validate category
        let valid_categories = [
            "user",
            "pattern",
            "domain",
            "instruction",
            "correction",
            "ctx",
        ];
        if !valid_categories.contains(&category) {
            return Err(ZeroError::Tool(format!(
                "Invalid category '{}'. Valid: {}",
                category,
                valid_categories.join(", ")
            )));
        }

        // Ctx category is session state — writes gate through a separate
        // path because the permission rules + storage sentinels differ.
        if category == "ctx" {
            return self.action_save_ctx_fact(ctx, agent_id, key, content).await;
        }

        if content.len() > 500 {
            return Err(ZeroError::Tool(
                "Fact content too long. Keep to 1-2 sentences (max 500 chars).".to_string(),
            ));
        }

        // Use DB-backed fact store if available
        match &self.fact_store {
            Some(store) => store
                .save_fact(agent_id, category, key, content, confidence, None)
                .await
                .map_err(ZeroError::Tool),
            None => {
                // Fallback: store in legacy KV file
                let kv_path = self.resolve_memory_path(agent_id, "agent", None)?;
                let mut store = self.load_store_at_path(&kv_path)?;

                let fact_key = format!("fact:{}", key);
                let now = Self::now();
                let entry = MemoryEntry {
                    value: format!("[{}] {} (confidence: {:.2})", category, content, confidence),
                    tags: vec!["fact".to_string(), category.to_string()],
                    created_at: store
                        .entries
                        .get(&fact_key)
                        .map(|e| e.created_at.clone())
                        .unwrap_or_else(|| now.clone()),
                    updated_at: now,
                };

                store.entries.insert(fact_key, entry);
                self.save_store_at_path(&kv_path, &store)?;

                Ok(json!({
                    "success": true,
                    "action": "save_fact",
                    "key": key,
                    "category": category,
                    "confidence": confidence,
                    "message": format!("Fact saved (file fallback): [{}] {}", category, content),
                }))
            }
        }
    }

    /// Recall relevant facts using prioritized hybrid search via the DB-backed fact store.
    ///
    /// When the DB-backed store is available, uses `recall_facts_prioritized` which
    /// applies category weights (corrections > strategies > user prefs > ...) to
    /// surface the most important facts first. Falls back to flat scoring if
    /// the store doesn't support prioritization.
    ///
    /// Defensive guard: when the underlying store returns a vec0-degraded
    /// error (missing `memory_facts_index` table, or `embedding dim
    /// mismatch`), this returns a structured `{ recalled: [], degraded:
    /// true, reason: … }` instead of propagating a fatal tool error. The
    /// agent keeps going with empty recall rather than wedging on a
    /// sticky red "Tool error" in the news ticker.
    async fn action_recall(
        &self,
        ctx: &dyn ToolContext,
        agent_id: &str,
        args: &Value,
    ) -> Result<Value> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ZeroError::Tool("Missing 'query' for recall".to_string()))?;

        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(5) as usize;

        // Use DB-backed fact store if available — prioritized recall
        match &self.fact_store {
            Some(store) => {
                let result = store.recall_facts_prioritized(agent_id, query, limit).await;
                match result {
                    Ok(v) => Ok(v),
                    Err(e) => {
                        if let Some(reason) = classify_recall_degradation(&e) {
                            let sid = ctx.session_id();
                            if should_log_degradation(sid) {
                                tracing::warn!(
                                    session_id = sid,
                                    reason = reason,
                                    error = %e,
                                    "memory.recall degraded — vec0 index unavailable; returning empty result"
                                );
                            }
                            Ok(json!({
                                "query": query,
                                "results": [],
                                "recalled": [],
                                "count": 0,
                                "degraded": true,
                                "reason": reason,
                                "source": "memory_db",
                            }))
                        } else {
                            Err(ZeroError::Tool(e))
                        }
                    }
                }
            }
            None => {
                // Fallback: search KV store with category-aware ordering
                let kv_path = self.resolve_memory_path(agent_id, "agent", None)?;
                let store = self.load_store_at_path(&kv_path)?;

                let query_lower = query.to_lowercase();

                // Category priority weights for KV fallback ordering
                let category_weight = |tags: &[String]| -> f64 {
                    for tag in tags {
                        match tag.as_str() {
                            "correction" => return 1.5,
                            "strategy" => return 1.4,
                            "user" => return 1.3,
                            "instruction" => return 1.2,
                            "domain" => return 1.0,
                            "pattern" => return 0.9,
                            _ => {}
                        }
                    }
                    1.0
                };

                let mut matches: Vec<(f64, &String, &MemoryEntry)> = store
                    .entries
                    .iter()
                    .filter(|(k, entry)| {
                        k.to_lowercase().contains(&query_lower)
                            || entry.value.to_lowercase().contains(&query_lower)
                            || entry
                                .tags
                                .iter()
                                .any(|t| t.to_lowercase().contains(&query_lower))
                    })
                    .map(|(key, entry)| {
                        let weight = category_weight(&entry.tags);
                        (weight, key, entry)
                    })
                    .collect();

                // Sort by category weight descending
                matches.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
                matches.truncate(limit);

                let results: Vec<Value> = matches
                    .iter()
                    .map(|(weight, key, entry)| {
                        json!({
                            "key": key,
                            "content": entry.value,
                            "tags": entry.tags,
                            "score": weight,
                            "source": "kv_store",
                            "prioritized": true,
                        })
                    })
                    .collect();

                Ok(json!({
                    "query": query,
                    "results": results,
                    "count": results.len(),
                    "source": "kv_store",
                    "prioritized": true,
                }))
            }
        }
    }

    /// Save a ctx-namespaced fact (session state).
    ///
    /// Dispatched from `action_save_fact` when `category='ctx'`. Enforces
    /// the permission rules defined in
    /// `docs/specs/2026-04-17-session-ctx-memory-bundle.md`:
    /// - Root (not delegated) can write any ctx key.
    /// - Delegated subagents can ONLY write keys matching
    ///   `ctx.<sid>.state.<anything>` — they cannot overwrite
    ///   root-owned canonicals (intent, prompt, plan, session.meta,
    ///   ward_briefing, memory).
    async fn action_save_ctx_fact(
        &self,
        ctx: &dyn ToolContext,
        agent_id: &str,
        key: &str,
        content: &str,
    ) -> Result<Value> {
        let is_delegated = ctx
            .get_state("app:is_delegated")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Pure-function permission check. Returns the session id on
        // success so we can pass it to the store; on failure it carries
        // the user-facing error message.
        let sid = check_ctx_write_permission(is_delegated, key).map_err(ZeroError::Tool)?;

        // Ward comes from current context; ctx facts are stored per-ward
        // so cleanup on ward deletion is straightforward.
        let ward_id = ctx
            .get_state("ward_id")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "__global__".to_string());

        // Owner: root when not delegated, otherwise the subagent's id.
        let owner = if is_delegated {
            format!("subagent:{}", agent_id)
        } else {
            "root".to_string()
        };

        // State handoffs (pinned=false) can be overwritten on rerun;
        // root-owned canonicals (pinned=true) are protected from drift.
        let pinned = !is_delegated;

        match &self.fact_store {
            Some(store) => store
                .save_ctx_fact(&sid, &ward_id, key, content, &owner, pinned)
                .await
                .map_err(ZeroError::Tool),
            None => Err(ZeroError::Tool(
                "Ctx facts require a DB-backed fact store (not available in this runtime)"
                    .to_string(),
            )),
        }
    }

    /// Exact-key lookup for ctx-namespaced session state.
    ///
    /// Unlike `recall` (fuzzy), this returns the exact row matching the
    /// key, or `{found: false}` on miss — never a nearest-neighbor.
    /// Used by subagents to fetch session canonicals (intent, prompt,
    /// plan) and prior step handoffs (state.<exec_id>) by precise key.
    async fn action_get_fact(&self, ctx: &dyn ToolContext, args: &Value) -> Result<Value> {
        let key = args
            .get("key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ZeroError::Tool("Missing 'key' for get_fact".to_string()))?;

        if !key.starts_with("ctx.") {
            return Err(ZeroError::Tool(format!(
                "get_fact only retrieves ctx-namespaced keys. Got '{}' — use 'recall' for fuzzy search on non-ctx facts.",
                key
            )));
        }

        let ward_id = ctx
            .get_state("ward_id")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "__global__".to_string());

        match &self.fact_store {
            Some(store) => {
                let result = store
                    .get_ctx_fact(&ward_id, key)
                    .await
                    .map_err(ZeroError::Tool)?;
                match result {
                    Some(value) => Ok(value),
                    None => Ok(json!({ "found": false, "key": key })),
                }
            }
            None => Err(ZeroError::Tool(
                "Ctx facts require a DB-backed fact store (not available in this runtime)"
                    .to_string(),
            )),
        }
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
                    || entry
                        .tags
                        .iter()
                        .any(|t| t.to_lowercase().contains(&query_lower))
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

/// Classify a ctx key and enforce the writer's permission.
///
/// Pure function — no tool context, no I/O. Testable in isolation.
/// Returns the extracted `session_id` on success so the caller can pass
/// it down to storage; returns a user-facing error message on reject.
///
/// Permission rules:
/// - Root (`is_delegated=false`) may write any well-formed ctx key.
/// - Delegated subagents may only write `ctx.<sid>.state.<anything>`;
///   they cannot overwrite root-owned canonicals (intent, prompt,
///   plan, session.meta, ward_briefing, memory) nor invent sub-keys
///   outside the `state.*` namespace.
fn check_ctx_write_permission(
    is_delegated: bool,
    key: &str,
) -> std::result::Result<String, String> {
    let Some(rest) = key.strip_prefix("ctx.") else {
        return Err(format!(
            "Ctx key '{}' must start with 'ctx.<session_id>.'",
            key
        ));
    };
    let Some((sid, sub_key)) = rest.split_once('.') else {
        return Err(format!(
            "Ctx key '{}' must include session_id: ctx.<sid>.<sub_key>",
            key
        ));
    };

    if !is_delegated {
        // Root can write anything well-formed.
        return Ok(sid.to_string());
    }

    const ROOT_OWNED: &[&str] = &[
        "intent",
        "prompt",
        "plan",
        "session.meta",
        "ward_briefing",
        "memory",
    ];

    if ROOT_OWNED.contains(&sub_key) {
        return Err(format!(
            "Subagent cannot write to root-owned ctx key '{}'. Root owns: {}. Subagents may only write 'ctx.<sid>.state.<...>'.",
            key,
            ROOT_OWNED.join(", ")
        ));
    }

    if !sub_key.starts_with("state.") {
        return Err(format!(
            "Subagent ctx writes must target 'ctx.<sid>.state.<...>'. Got sub-key '{}'.",
            sub_key
        ));
    }

    Ok(sid.to_string())
}

// ============================================================================
// RECALL DEGRADATION CLASSIFIER (Fix 3)
// ============================================================================

/// Inspect a fact-store error and return a short reason string when it
/// matches a known vec0 "index not ready" / "dim mismatch" pattern.
///
/// Returns `None` for errors we want to propagate as genuine tool errors
/// (e.g. arg validation, provider outages). Pure function — no state —
/// tested in isolation.
fn classify_recall_degradation(msg: &str) -> Option<&'static str> {
    if msg.contains("no such table: memory_facts_index")
        || msg.contains("no such table: kg_name_index")
        || msg.contains("no such table: session_episodes_index")
        || msg.contains("no such table: wiki_articles_index")
        || msg.contains("no such table: procedures_index")
    {
        return Some("vec0 index table missing — recall disabled until reindex");
    }
    if msg.contains("embedding dim mismatch") {
        return Some("embedding dim mismatch — recall disabled until reindex");
    }
    None
}

/// Track which session ids have already been notified about degraded
/// recall. Prevents the news ticker from being spammed with identical
/// warnings every time root calls `memory.recall` during a long session.
fn degradation_log_cache() -> &'static Mutex<HashSet<String>> {
    static CACHE: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Returns `true` iff this is the first time we've seen `session_id`
/// report a degraded recall this process-lifetime. Subsequent calls
/// with the same id return `false` so the warn log stays once-per-session.
fn should_log_degradation(session_id: &str) -> bool {
    // Ignore empty session ids — those are short-lived test/setup
    // contexts where per-session dedup isn't meaningful.
    if session_id.is_empty() {
        return true;
    }
    let Ok(mut guard) = degradation_log_cache().lock() else {
        // Poisoned lock shouldn't happen but, if it does, bias toward
        // logging so the operator sees the signal.
        return true;
    };
    guard.insert(session_id.to_string())
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
        let tool = MemoryTool::new(fs, None);

        let path = tool
            .resolve_memory_path("test-agent", "agent", None)
            .unwrap();
        assert!(path.ends_with("agents_data/test-agent/memory.json"));
    }

    #[test]
    fn test_resolve_shared_memory_path() {
        let dir = TempDir::new().unwrap();
        let fs = Arc::new(TestFileSystem::new(dir.path().to_path_buf()));
        let tool = MemoryTool::new(fs, None);

        let path = tool
            .resolve_memory_path("test-agent", "shared", Some("patterns"))
            .unwrap();
        assert!(path.ends_with("agents_data/shared/patterns.json"));
    }

    #[test]
    fn test_shared_memory_requires_file() {
        let dir = TempDir::new().unwrap();
        let fs = Arc::new(TestFileSystem::new(dir.path().to_path_buf()));
        let tool = MemoryTool::new(fs, None);

        let result = tool.resolve_memory_path("test-agent", "shared", None);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("'file' parameter required")
        );
    }

    #[test]
    fn test_shared_memory_invalid_file() {
        let dir = TempDir::new().unwrap();
        let fs = Arc::new(TestFileSystem::new(dir.path().to_path_buf()));
        let tool = MemoryTool::new(fs, None);

        let result = tool.resolve_memory_path("test-agent", "shared", Some("invalid_file"));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid shared file")
        );
    }

    // ========================================================================
    // Ctx write permission tests (Phase 1b — memory-as-ctx bundle)
    //
    // Pure-function tests for the permission classifier. No tool context,
    // no fact store — these verify the rules in isolation.
    // ========================================================================

    #[test]
    fn test_ctx_perm_root_allowed_on_intent() {
        let sid = check_ctx_write_permission(false, "ctx.sess-abc.intent").unwrap();
        assert_eq!(sid, "sess-abc");
    }

    #[test]
    fn test_ctx_perm_root_allowed_on_state() {
        let sid = check_ctx_write_permission(false, "ctx.sess-abc.state.exec-1").unwrap();
        assert_eq!(sid, "sess-abc");
    }

    #[test]
    fn test_ctx_perm_subagent_rejected_on_intent() {
        let err = check_ctx_write_permission(true, "ctx.sess-abc.intent").unwrap_err();
        assert!(err.contains("root-owned"), "error was: {}", err);
        assert!(err.contains("intent"), "error was: {}", err);
    }

    #[test]
    fn test_ctx_perm_subagent_rejected_on_prompt() {
        let err = check_ctx_write_permission(true, "ctx.sess-abc.prompt").unwrap_err();
        assert!(err.contains("root-owned"), "error was: {}", err);
    }

    #[test]
    fn test_ctx_perm_subagent_rejected_on_plan() {
        let err = check_ctx_write_permission(true, "ctx.sess-abc.plan").unwrap_err();
        assert!(err.contains("root-owned"), "error was: {}", err);
    }

    #[test]
    fn test_ctx_perm_subagent_rejected_on_session_meta() {
        let err = check_ctx_write_permission(true, "ctx.sess-abc.session.meta").unwrap_err();
        assert!(err.contains("root-owned"), "error was: {}", err);
    }

    #[test]
    fn test_ctx_perm_subagent_allowed_on_state() {
        let sid = check_ctx_write_permission(true, "ctx.sess-abc.state.exec-1").unwrap();
        assert_eq!(sid, "sess-abc");
    }

    #[test]
    fn test_ctx_perm_subagent_rejected_on_unknown_sub_key() {
        // Anything not root-owned and not state.* is outside the
        // namespace shape subagents are allowed to invent.
        let err = check_ctx_write_permission(true, "ctx.sess-abc.scratchpad").unwrap_err();
        assert!(err.contains("state"), "error was: {}", err);
    }

    #[test]
    fn test_ctx_perm_malformed_no_ctx_prefix() {
        let err = check_ctx_write_permission(false, "state.exec-1").unwrap_err();
        assert!(err.contains("ctx."), "error was: {}", err);
    }

    #[test]
    fn test_ctx_perm_malformed_no_session_id() {
        let err = check_ctx_write_permission(false, "ctx.").unwrap_err();
        assert!(err.contains("session_id"), "error was: {}", err);
    }

    #[test]
    fn test_all_shared_files_valid() {
        let dir = TempDir::new().unwrap();
        let fs = Arc::new(TestFileSystem::new(dir.path().to_path_buf()));
        let tool = MemoryTool::new(fs, None);

        for file in SHARED_FILES {
            let result = tool.resolve_memory_path("test-agent", "shared", Some(file));
            assert!(result.is_ok(), "Failed for file: {}", file);
        }
    }

    #[test]
    fn test_load_store_creates_default_when_missing() {
        let dir = TempDir::new().unwrap();
        let fs = Arc::new(TestFileSystem::new(dir.path().to_path_buf()));
        let tool = MemoryTool::new(fs, None);

        let path = dir.path().join("nonexistent").join("memory.json");
        let store = tool.load_store_at_path(&path).unwrap();
        assert!(store.entries.is_empty());
    }

    // -----------------------------------------------------------------
    // Fix 3: recall degradation classifier + once-per-session dedup log
    // -----------------------------------------------------------------

    #[test]
    fn classifier_recognises_missing_memory_facts_index() {
        let e = "Knowledge DB operation failed: no such table: memory_facts_index";
        assert!(classify_recall_degradation(e).is_some());
    }

    #[test]
    fn classifier_recognises_missing_kg_name_index() {
        let e = "Knowledge DB operation failed: no such table: kg_name_index";
        assert!(classify_recall_degradation(e).is_some());
    }

    #[test]
    fn classifier_recognises_dim_mismatch() {
        let e = "vector_index error: embedding dim mismatch: got 1024, expected 384";
        assert!(classify_recall_degradation(e).is_some());
    }

    #[test]
    fn classifier_ignores_unrelated_errors() {
        assert!(classify_recall_degradation("network timeout").is_none());
        assert!(classify_recall_degradation("permission denied").is_none());
        assert!(classify_recall_degradation("Missing 'query' for recall").is_none());
    }

    #[test]
    fn log_dedup_once_per_session() {
        let sid = "sess-logdedup-fixture-unique-1";
        assert!(should_log_degradation(sid), "first call must log");
        assert!(!should_log_degradation(sid), "second call must dedup");
        assert!(!should_log_degradation(sid), "third call must dedup");
    }

    #[test]
    fn log_dedup_distinct_sessions_each_log() {
        let a = "sess-logdedup-fixture-unique-a";
        let b = "sess-logdedup-fixture-unique-b";
        assert!(should_log_degradation(a));
        assert!(should_log_degradation(b));
    }

    #[test]
    fn log_dedup_always_logs_empty_session_id() {
        // Empty session id means setup/test — don't rely on dedup there.
        assert!(should_log_degradation(""));
        assert!(should_log_degradation(""));
    }

    // Integration: action_recall returns degraded result when the fact
    // store surfaces a "no such table" error — no fatal tool error.
    #[tokio::test]
    async fn action_recall_returns_degraded_result_on_missing_index() {
        use async_trait::async_trait;
        use zero_core::MemoryFactStore;

        struct BrokenStore;

        #[async_trait]
        impl MemoryFactStore for BrokenStore {
            async fn save_fact(
                &self,
                _agent_id: &str,
                _category: &str,
                _key: &str,
                _content: &str,
                _confidence: f64,
                _session_id: Option<&str>,
            ) -> std::result::Result<Value, String> {
                Ok(json!({}))
            }

            async fn recall_facts(
                &self,
                _agent_id: &str,
                _query: &str,
                _limit: usize,
            ) -> std::result::Result<Value, String> {
                Err("Knowledge DB operation failed: no such table: memory_facts_index".to_string())
            }

            async fn recall_facts_prioritized(
                &self,
                _agent_id: &str,
                _query: &str,
                _limit: usize,
            ) -> std::result::Result<Value, String> {
                Err("Knowledge DB operation failed: no such table: memory_facts_index".to_string())
            }
        }

        // Minimal ToolContext stub with the session_id() accessor we use
        // for dedup. Everything else returns a reasonable default.
        use zero_core::{CallbackContext, Content, EventActions, ReadonlyContext, ToolContext};

        struct Ctx;

        impl ReadonlyContext for Ctx {
            fn invocation_id(&self) -> &str {
                "test"
            }
            fn agent_name(&self) -> &str {
                "test"
            }
            fn user_id(&self) -> &str {
                "test"
            }
            fn app_name(&self) -> &str {
                "test"
            }
            fn session_id(&self) -> &str {
                "sess-fix3-integration"
            }
            fn branch(&self) -> &str {
                "test"
            }
            fn user_content(&self) -> &Content {
                use std::sync::LazyLock;
                static C: LazyLock<Content> = LazyLock::new(|| Content {
                    role: "user".to_string(),
                    parts: vec![],
                });
                &C
            }
        }

        impl CallbackContext for Ctx {
            fn get_state(&self, _key: &str) -> Option<Value> {
                None
            }
            fn set_state(&self, _key: String, _value: Value) {}
        }

        impl ToolContext for Ctx {
            fn function_call_id(&self) -> String {
                "test".to_string()
            }
            fn actions(&self) -> EventActions {
                EventActions::default()
            }
            fn set_actions(&self, _actions: EventActions) {}
        }

        let dir = TempDir::new().unwrap();
        let fs = Arc::new(TestFileSystem::new(dir.path().to_path_buf()));
        let store: Arc<dyn MemoryFactStore> = Arc::new(BrokenStore);
        let tool = MemoryTool::new(fs, Some(store));
        let args = json!({ "query": "anything" });
        let ctx = Ctx;

        let result = tool.action_recall(&ctx, "root", &args).await.unwrap();

        assert_eq!(result["degraded"], json!(true));
        assert_eq!(result["count"], json!(0));
        assert!(
            result["reason"]
                .as_str()
                .unwrap_or("")
                .contains("vec0 index table missing"),
            "got: {result}"
        );
        assert_eq!(result["results"], json!([]));
    }

    #[test]
    fn test_save_and_load_store() {
        let dir = TempDir::new().unwrap();
        let fs = Arc::new(TestFileSystem::new(dir.path().to_path_buf()));
        let tool = MemoryTool::new(fs, None);

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
