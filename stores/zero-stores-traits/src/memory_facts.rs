// ============================================================================
// MEMORY FACT STORE TRAIT
// Abstract interface for durable memory fact storage
// ============================================================================

use async_trait::async_trait;
use serde_json::Value;

/// Aggregate counts across the memory subsystem. Returned by
/// `MemoryFactStore::aggregate_stats` for the `GET /api/memory/stats`
/// endpoint. Tables that aren't present in the backing store
/// (e.g. wiki, procedures) report `0` rather than erroring — the
/// trait contract is "best-effort snapshot".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemoryAggregateStats {
    /// `memory_facts` row count.
    pub facts: i64,
    /// `kg_episodes` row count (durable per-session episode log).
    pub episodes: i64,
    /// `procedures` row count.
    pub procedures: i64,
    /// `ward_wiki_articles` row count.
    pub wiki_articles: i64,
    /// `kg_goals` row count where `state = 'active'`.
    pub goals_active: i64,
}

/// Snapshot of ingestion / consolidation health for
/// `GET /api/memory/health`. Pending and running counts are reported
/// from the kg-episode lifecycle table; failed_recent counts the
/// `status='failed'` rows. Compaction metrics live on a separate
/// repository (`compaction_repo`) and are not part of this snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemoryHealthMetrics {
    /// Episodes currently waiting in the pending queue.
    pub queue_pending: u64,
    /// Episodes currently running through extraction.
    pub queue_running: u64,
    /// Episodes that failed during extraction.
    pub failed_recent: u64,
}

/// One row in the per-skill staleness tracker. Lives in `zero-core`
/// (rather than `gateway-database`) so this trait can use it without
/// dragging the SQLite stack into agent-tools.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillIndexRow {
    /// Skill identifier (directory name, after vault-wins dedup).
    pub name: String,
    /// `'vault'` or `'agent'` — diagnostic only, not a join key.
    pub source_root: String,
    /// Absolute path to `<root>/<name>/SKILL.md` as last indexed.
    pub file_path: String,
    /// `SKILL.md` mtime in seconds since the Unix epoch as last indexed.
    pub mtime_unix: i64,
    /// `SKILL.md` size in bytes as last indexed. Breaks ties when two
    /// edits within the same second produce different content.
    pub size_bytes: i64,
    /// DB write time of the row, seconds since the Unix epoch.
    pub last_indexed_unix: i64,
    /// Embedding-content schema version. The reindex diff treats any
    /// row whose stored version disagrees with the running code's
    /// `CURRENT_INDEX_FORMAT_VERSION` as "modified", forcing one
    /// re-embed pass after a content-format change.
    pub format_version: i64,
}

/// Abstract interface for durable memory fact storage.
///
/// Implementations can wrap a database (SQLite via `MemoryRepository`),
/// a remote API, or an in-memory store for testing.
///
/// This trait lives in `zero-core` so that `agent-tools` (which depends on
/// `zero-core` but not `gateway-database`) can call DB operations via the trait.
#[async_trait]
pub trait MemoryFactStore: Send + Sync {
    /// Save a structured fact to durable memory.
    ///
    /// On conflict (same agent_id + scope + key), updates content and bumps
    /// mention_count. Returns a JSON summary of the operation.
    async fn save_fact(
        &self,
        agent_id: &str,
        category: &str,
        key: &str,
        content: &str,
        confidence: f64,
        session_id: Option<&str>,
    ) -> Result<Value, String>;

    /// Recall facts relevant to a query using hybrid search.
    ///
    /// Combines FTS5 keyword matching and vector cosine similarity
    /// (when embeddings are available). Returns a JSON array of results.
    async fn recall_facts(
        &self,
        agent_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Value, String>;

    /// Recall facts with priority scoring applied (category weights, etc.).
    ///
    /// This is the upgraded version of `recall_facts` that applies the same
    /// priority engine used by system-level recall: corrections first,
    /// strategies second, user preferences third, etc.
    ///
    /// Default implementation falls back to `recall_facts`.
    async fn recall_facts_prioritized(
        &self,
        agent_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Value, String> {
        self.recall_facts(agent_id, query, limit).await
    }

    /// Exact-key lookup in the session-scoped ctx namespace.
    ///
    /// Returns the single row matching the ctx key, or `None` if absent.
    /// Unlike `recall_facts`, this is a precise lookup — no ranking, no
    /// fuzzy match. Used by subagents to fetch canonical session state
    /// (intent, prompt, plan, state.<exec_id>) by exact key.
    ///
    /// Default implementation returns `Ok(None)` for stores that don't
    /// support ctx storage.
    async fn get_ctx_fact(&self, _ward_id: &str, _key: &str) -> Result<Option<Value>, String> {
        Ok(None)
    }

    /// Save a ctx-namespaced fact for the current session.
    ///
    /// Ctx facts use a fixed schema: `category='ctx'`, `scope='session'`,
    /// `agent_id='__ctx__'` (sentinel — not tied to any single agent),
    /// and the caller-supplied `ward_id` + `key`. The `owner` argument
    /// identifies who wrote the fact: `"root"` for session-canonical
    /// content (intent, prompt, plan) or `"subagent:<exec_id>"` for a
    /// subagent's handoff state.
    ///
    /// This method does NOT perform permission checks — those happen at
    /// the tool layer where the runtime knows if the caller is delegated.
    /// Ctx facts are excluded from fuzzy recall by default.
    ///
    /// Default implementation returns an error for stores that don't
    /// support ctx storage.
    async fn save_ctx_fact(
        &self,
        _session_id: &str,
        _ward_id: &str,
        _key: &str,
        _content: &str,
        _owner: &str,
        _pinned: bool,
    ) -> Result<Value, String> {
        Err("ctx storage not implemented for this store".to_string())
    }

    /// Upsert a ward-scoped primitive (function signature) extracted
    /// from a source file by the runtime's AST hook.
    ///
    /// Fixed schema: `category='primitive'`, `scope='global'`,
    /// `agent_id='__ward__'` (sentinel), caller-supplied `ward_id` +
    /// `key` (conventionally `primitive.<relative_path>.<symbol>`).
    /// `signature` is the one-line call form; `summary` is the first
    /// line of the function's docstring.
    ///
    /// Idempotent: re-extraction of the same symbol upserts in place.
    /// Ctx writes are cheap (no embedding generated) — primitives are
    /// queried by key + ward prefix, not by fuzzy similarity.
    ///
    /// Default implementation returns an error for stores that don't
    /// implement primitive storage.
    async fn upsert_primitive(
        &self,
        _ward_id: &str,
        _key: &str,
        _signature: &str,
        _summary: &str,
    ) -> Result<Value, String> {
        Err("primitive storage not implemented for this store".to_string())
    }

    /// List all primitives for a ward, grouped for ward-snapshot rendering.
    ///
    /// Returns an array of {key, signature, summary} ordered by key.
    /// Default implementation returns an empty array.
    async fn list_primitives(&self, _ward_id: &str) -> Result<Value, String> {
        Ok(serde_json::json!({ "primitives": [] }))
    }

    // =========================================================================
    // SKILL INDEX STATE
    // Per-skill staleness tracker for the incremental skill reindex.
    // Default implementations return empty / no-op, so stores that don't
    // care (mocks, in-memory) inherit safe behavior without code changes.
    // =========================================================================

    /// Delete every fact matching `(category, key)`. Used by the skill
    /// reindexer to clear ghost embeddings when a skill is removed from
    /// disk. Returns the number of rows deleted.
    async fn delete_facts_by_key(&self, _category: &str, _key: &str) -> Result<usize, String> {
        Ok(0)
    }

    /// Read every row from the per-skill staleness tracker. Returns an
    /// empty Vec when the table is missing or empty (e.g. fresh DB).
    async fn list_skill_index(&self) -> Result<Vec<SkillIndexRow>, String> {
        Ok(Vec::new())
    }

    /// Insert or replace one row in the per-skill staleness tracker.
    async fn upsert_skill_index(&self, _row: SkillIndexRow) -> Result<(), String> {
        Ok(())
    }

    /// Delete a single row from the per-skill staleness tracker.
    async fn delete_skill_index(&self, _name: &str) -> Result<bool, String> {
        Ok(false)
    }

    // =========================================================================
    // AGGREGATE / HEALTH METRICS (HTTP handlers)
    // Used by `GET /api/memory/stats` and `/api/memory/health`. Default
    // implementations return zeros so stores that don't track these
    // (mocks, in-memory) inherit safe behavior.
    // =========================================================================

    /// Aggregate counts across memory_facts, kg_episodes, procedures,
    /// ward_wiki_articles, and active kg_goals. Used by the memory
    /// stats endpoint. Default returns all zeros.
    async fn aggregate_stats(&self) -> Result<MemoryAggregateStats, String> {
        Ok(MemoryAggregateStats::default())
    }

    /// Counts of pending / running / failed episodes for the memory
    /// health endpoint. Default returns all zeros.
    async fn health_metrics(&self) -> Result<MemoryHealthMetrics, String> {
        Ok(MemoryHealthMetrics::default())
    }

    /// Count of all memory facts visible to `agent_id` (`Some`) or
    /// across all agents (`None`). Used by aggregate graph stats. The
    /// default returns `0`.
    async fn count_all_facts(&self, _agent_id: Option<&str>) -> Result<i64, String> {
        Ok(0)
    }

    /// Paginated list of memory facts with optional `agent_id`, `category`,
    /// and `scope` filters. Returns each row as a `serde_json::Value` so
    /// that the trait surface stays free of the gateway-database
    /// `MemoryFact` struct (dep-cycle avoidance). Default returns empty.
    async fn list_memory_facts(
        &self,
        _agent_id: Option<&str>,
        _category: Option<&str>,
        _scope: Option<&str>,
        _limit: usize,
        _offset: usize,
    ) -> Result<Vec<Value>, String> {
        Ok(Vec::new())
    }
}
