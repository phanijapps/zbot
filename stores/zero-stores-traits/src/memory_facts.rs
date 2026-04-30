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

    /// Fetch a single memory fact by id. Returns `None` if the row is
    /// absent. The shape mirrors the same JSON layout that
    /// `list_memory_facts` emits per row.
    async fn get_memory_fact_by_id(&self, _fact_id: &str) -> Result<Option<Value>, String> {
        Ok(None)
    }

    /// Delete a single memory fact by id. Returns `true` if a row was
    /// removed, `false` if the id was absent.
    async fn delete_memory_fact(&self, _fact_id: &str) -> Result<bool, String> {
        Ok(false)
    }

    /// Upsert a fully-shaped memory fact. The `fact` Value must contain the
    /// gateway-database `MemoryFact` JSON shape (id, agent_id, scope,
    /// category, key, content, confidence, mention_count, source_summary,
    /// ward_id, contradicted_by, created_at, updated_at, expires_at,
    /// valid_from, valid_until, superseded_by, pinned, etc.). The optional
    /// `embedding` is the L2-normalized name vector to persist alongside.
    /// Default returns an error so impls that don't support typed upsert
    /// fail loudly rather than silently dropping writes.
    async fn upsert_typed_fact(
        &self,
        _fact: Value,
        _embedding: Option<Vec<f32>>,
    ) -> Result<(), String> {
        Err("upsert_typed_fact not implemented for this store".to_string())
    }

    /// Mark a fact as superseded by a newer fact. Both ids should already
    /// exist. Default returns no-op error so misuse is loud.
    async fn supersede_fact(&self, _old_id: &str, _new_id: &str) -> Result<(), String> {
        Err("supersede_fact not implemented for this store".to_string())
    }

    /// Mark a fact as archived (soft-delete). Used by sleep-time pruning.
    async fn archive_fact(&self, _fact_id: &str) -> Result<bool, String> {
        Ok(false)
    }

    /// Hybrid FTS + vector search across memory facts. `mode` is one of
    /// `"fts"`, `"semantic"`, or `"hybrid"`. `query_embedding` is supplied
    /// only for semantic / hybrid modes (caller pre-embeds the query string).
    /// `ward_id` filters to a specific ward when set.
    /// Each row in the returned Vec carries a `match_source` field
    /// (`"fts"`, `"vec"`, `"hybrid"`) for downstream ranking display.
    async fn search_memory_facts_hybrid(
        &self,
        _agent_id: Option<&str>,
        _query: &str,
        _mode: &str,
        _limit: usize,
        _ward_id: Option<&str>,
        _query_embedding: Option<&[f32]>,
    ) -> Result<Vec<Value>, String> {
        Ok(Vec::new())
    }

    // ---- Sleep-time synthesis (Phase D4) -------------------------------
    //
    // Reads/writes needed by the `Synthesizer` to dedup against
    // existing strategy facts and persist new ones. Default impls
    // return None / Err so backends that haven't implemented yet make
    // the synthesis cycle a quiet no-op rather than corrupting state.

    /// Find an existing strategy fact whose embedding's cosine
    /// similarity with `embedding` is at or above `threshold`. Scans
    /// up to `scan_limit` candidate facts in `category = "strategy"`
    /// for the agent. Default: no match.
    async fn find_strategy_fact_by_similarity(
        &self,
        _agent_id: &str,
        _embedding: &[f32],
        _threshold: f32,
        _scan_limit: usize,
    ) -> Result<Option<StrategyFactMatch>, String> {
        Ok(None)
    }

    /// Bump an existing strategy fact's `mention_count` and replace
    /// its `source_episode_id` with `merged_source_episode_id`.
    /// `now_rfc3339` is the timestamp to record under `updated_at`.
    /// Default: no-op error so misuse is loud.
    async fn bump_strategy_fact_episodes(
        &self,
        _fact_id: &str,
        _merged_source_episode_id: &str,
        _now_rfc3339: &str,
    ) -> Result<(), String> {
        Err("bump_strategy_fact_episodes not implemented for this store".to_string())
    }

    /// Insert a synthesised strategy fact. Returns the fact id used.
    /// The trait crate stays dep-light by taking a purpose-built
    /// `StrategyFactInsert` rather than a full `MemoryFact` struct
    /// (which lives in `zero-stores-sqlite`). Default: no-op error.
    async fn insert_strategy_fact(&self, _req: StrategyFactInsert) -> Result<String, String> {
        Err("insert_strategy_fact not implemented for this store".to_string())
    }
}

/// Result of `find_strategy_fact_by_similarity`. Captures only what
/// the Synthesizer needs to decide whether to bump or insert.
#[derive(Debug, Clone)]
pub struct StrategyFactMatch {
    pub fact_id: String,
    /// Comma-separated csv of episode ids previously attributed to
    /// this fact, or `None` if the column is null.
    pub source_episode_id: Option<String>,
}

/// Request shape for `insert_strategy_fact`. Flat field set chosen so
/// the trait crate doesn't need to depend on the full `MemoryFact`
/// type from `zero-stores-sqlite`.
#[derive(Debug, Clone)]
pub struct StrategyFactInsert {
    pub agent_id: String,
    pub key: String,
    pub content: String,
    pub confidence: f64,
    pub source_summary: Option<String>,
    pub embedding: Option<Vec<f32>>,
    pub source_episode_id: Option<String>,
}
