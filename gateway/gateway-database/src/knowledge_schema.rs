//! Schema v24 for `knowledge.db`.
//!
//! All long-term memory + graph + vector indexes live here.
//! Applied idempotently on daemon boot. No migrations — clean slate.

use rusqlite::Connection;

const SCHEMA_VERSION: i32 = 24;

/// v23 delta: full-text search over `ward_wiki_articles` with sync triggers.
const V23_WIKI_FTS_SQL: &str = include_str!("../migrations/v23_wiki_fts.sql");

/// v24 delta: one-time backfill that promotes facts in global-type categories
/// (domain / reference / book / research / user) from the default
/// `scope='agent'` to `scope='global'`, making them visible to every agent
/// via the scope-aware search filter.
const V24_GLOBAL_SCOPE_BACKFILL_SQL: &str =
    include_str!("../migrations/v24_global_scope_backfill.sql");

/// Initialize the knowledge database schema (v24).
///
/// Creates all tables and indexes if they don't exist. Records the
/// schema version in `schema_version` table. Safe to call on an
/// already-initialized DB — every delta is idempotent.
pub fn initialize_knowledge_database(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(SCHEMA_SQL)?;
    conn.execute_batch(V23_WIKI_FTS_SQL)?;
    conn.execute_batch(V24_GLOBAL_SCOPE_BACKFILL_SQL)?;
    conn.execute(
        "INSERT OR IGNORE INTO schema_version (version, applied_at) VALUES (?1, datetime('now'))",
        rusqlite::params![SCHEMA_VERSION],
    )?;
    Ok(())
}

const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL
);

-- ========================================================================
-- Knowledge Graph — entities, relationships, aliases
-- ========================================================================

CREATE TABLE IF NOT EXISTS kg_entities (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    name TEXT NOT NULL,
    normalized_name TEXT NOT NULL,
    normalized_hash TEXT NOT NULL,
    properties TEXT,
    epistemic_class TEXT NOT NULL DEFAULT 'current',
    confidence REAL NOT NULL DEFAULT 0.8,
    mention_count INTEGER NOT NULL DEFAULT 1,
    access_count INTEGER NOT NULL DEFAULT 0,
    first_seen_at TEXT NOT NULL,
    last_seen_at TEXT NOT NULL,
    last_accessed_at TEXT,
    valid_from TEXT,
    valid_until TEXT,
    invalidated_by TEXT,
    compressed_into TEXT,
    source_episode_ids TEXT
);
CREATE INDEX IF NOT EXISTS idx_entities_normalized_hash
    ON kg_entities(agent_id, entity_type, normalized_hash);
CREATE INDEX IF NOT EXISTS idx_entities_agent_type
    ON kg_entities(agent_id, entity_type);
CREATE INDEX IF NOT EXISTS idx_entities_name ON kg_entities(name);
CREATE INDEX IF NOT EXISTS idx_entities_last_accessed ON kg_entities(last_accessed_at);
CREATE INDEX IF NOT EXISTS idx_entities_epistemic
    ON kg_entities(agent_id, epistemic_class);

CREATE TABLE IF NOT EXISTS kg_relationships (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    source_entity_id TEXT NOT NULL,
    target_entity_id TEXT NOT NULL,
    relationship_type TEXT NOT NULL,
    properties TEXT,
    epistemic_class TEXT NOT NULL DEFAULT 'current',
    confidence REAL NOT NULL DEFAULT 0.8,
    mention_count INTEGER NOT NULL DEFAULT 1,
    access_count INTEGER NOT NULL DEFAULT 0,
    first_seen_at TEXT NOT NULL,
    last_seen_at TEXT NOT NULL,
    last_accessed_at TEXT,
    valid_at TEXT,
    invalidated_at TEXT,
    invalidated_by TEXT,
    source_episode_ids TEXT,
    UNIQUE(source_entity_id, target_entity_id, relationship_type),
    FOREIGN KEY (source_entity_id) REFERENCES kg_entities(id) ON DELETE CASCADE,
    FOREIGN KEY (target_entity_id) REFERENCES kg_entities(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_rels_source ON kg_relationships(source_entity_id);
CREATE INDEX IF NOT EXISTS idx_rels_target ON kg_relationships(target_entity_id);
CREATE INDEX IF NOT EXISTS idx_rels_agent ON kg_relationships(agent_id);
CREATE INDEX IF NOT EXISTS idx_rels_valid ON kg_relationships(valid_at);

CREATE TABLE IF NOT EXISTS kg_aliases (
    id TEXT PRIMARY KEY,
    entity_id TEXT NOT NULL,
    surface_form TEXT NOT NULL,
    normalized_form TEXT NOT NULL,
    source TEXT NOT NULL,
    confidence REAL NOT NULL DEFAULT 1.0,
    first_seen_at TEXT NOT NULL,
    FOREIGN KEY (entity_id) REFERENCES kg_entities(id) ON DELETE CASCADE,
    UNIQUE(normalized_form, entity_id)
);
CREATE INDEX IF NOT EXISTS idx_aliases_normalized ON kg_aliases(normalized_form);
CREATE INDEX IF NOT EXISTS idx_aliases_entity ON kg_aliases(entity_id);

CREATE TABLE IF NOT EXISTS kg_episodes (
    id TEXT PRIMARY KEY,
    source_type TEXT NOT NULL,
    source_ref TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    session_id TEXT,
    agent_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    retry_count INTEGER NOT NULL DEFAULT 0,
    error TEXT,
    created_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT,
    UNIQUE(content_hash, source_type)
);
CREATE INDEX IF NOT EXISTS idx_episodes_status ON kg_episodes(status);
CREATE INDEX IF NOT EXISTS idx_episodes_source_ref ON kg_episodes(source_ref);
CREATE INDEX IF NOT EXISTS idx_episodes_session ON kg_episodes(session_id);

CREATE TABLE IF NOT EXISTS kg_goals (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    ward_id TEXT,
    title TEXT NOT NULL,
    description TEXT,
    state TEXT NOT NULL DEFAULT 'active',
    parent_goal_id TEXT,
    slots TEXT,
    filled_slots TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    completed_at TEXT,
    FOREIGN KEY (parent_goal_id) REFERENCES kg_goals(id)
);
CREATE INDEX IF NOT EXISTS idx_goals_agent_state ON kg_goals(agent_id, state);
CREATE INDEX IF NOT EXISTS idx_goals_ward ON kg_goals(ward_id);

CREATE TABLE IF NOT EXISTS kg_compactions (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    operation TEXT NOT NULL,
    entity_id TEXT,
    relationship_id TEXT,
    merged_into TEXT,
    reason TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_compactions_run ON kg_compactions(run_id);

CREATE TABLE IF NOT EXISTS kg_causal_edges (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    cause_entity_id TEXT NOT NULL,
    effect_entity_id TEXT NOT NULL,
    relationship TEXT NOT NULL,
    confidence REAL DEFAULT 0.7,
    session_id TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY (cause_entity_id) REFERENCES kg_entities(id) ON DELETE CASCADE,
    FOREIGN KEY (effect_entity_id) REFERENCES kg_entities(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_causal_cause ON kg_causal_edges(cause_entity_id);
CREATE INDEX IF NOT EXISTS idx_causal_effect ON kg_causal_edges(effect_entity_id);

-- ========================================================================
-- Memory facts — no embedding column; embeddings live in memory_facts_index
-- ========================================================================

CREATE TABLE IF NOT EXISTS memory_facts (
    id TEXT PRIMARY KEY,
    session_id TEXT,
    agent_id TEXT NOT NULL,
    scope TEXT NOT NULL,
    category TEXT NOT NULL,
    key TEXT NOT NULL,
    content TEXT NOT NULL,
    confidence REAL NOT NULL DEFAULT 0.8,
    mention_count INTEGER NOT NULL DEFAULT 1,
    source_summary TEXT,
    source_episode_id TEXT,
    source_ref TEXT,
    ward_id TEXT NOT NULL DEFAULT '__global__',
    epistemic_class TEXT NOT NULL DEFAULT 'current',
    contradicted_by TEXT,
    valid_from TEXT,
    valid_until TEXT,
    superseded_by TEXT,
    pinned INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    expires_at TEXT,
    UNIQUE(agent_id, scope, ward_id, key)
);
CREATE INDEX IF NOT EXISTS idx_facts_agent_scope ON memory_facts(agent_id, scope);
CREATE INDEX IF NOT EXISTS idx_facts_category ON memory_facts(agent_id, category);
CREATE INDEX IF NOT EXISTS idx_facts_ward ON memory_facts(ward_id);
CREATE INDEX IF NOT EXISTS idx_facts_epistemic ON memory_facts(epistemic_class);
CREATE VIRTUAL TABLE IF NOT EXISTS memory_facts_fts USING fts5(
    key, content, category, content=memory_facts
);

CREATE TABLE IF NOT EXISTS memory_facts_archive (
    id TEXT PRIMARY KEY,
    session_id TEXT,
    agent_id TEXT NOT NULL,
    scope TEXT NOT NULL,
    category TEXT NOT NULL,
    key TEXT NOT NULL,
    content TEXT NOT NULL,
    confidence REAL NOT NULL,
    ward_id TEXT NOT NULL,
    epistemic_class TEXT NOT NULL,
    archived_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_facts_archive_agent ON memory_facts_archive(agent_id);

CREATE TABLE IF NOT EXISTS ward_wiki_articles (
    id TEXT PRIMARY KEY,
    ward_id TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    tags TEXT,
    source_fact_ids TEXT,
    version INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(ward_id, title)
);
CREATE INDEX IF NOT EXISTS idx_wiki_ward ON ward_wiki_articles(ward_id);

CREATE TABLE IF NOT EXISTS procedures (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    ward_id TEXT NOT NULL DEFAULT '__global__',
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    trigger_pattern TEXT,
    steps TEXT NOT NULL,
    parameters TEXT,
    success_count INTEGER NOT NULL DEFAULT 1,
    failure_count INTEGER NOT NULL DEFAULT 0,
    avg_duration_ms INTEGER,
    avg_token_cost INTEGER,
    last_used TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_procedures_agent ON procedures(agent_id);
CREATE INDEX IF NOT EXISTS idx_procedures_ward ON procedures(ward_id);

CREATE TABLE IF NOT EXISTS session_episodes (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL UNIQUE,
    agent_id TEXT NOT NULL,
    ward_id TEXT,
    task_summary TEXT,
    outcome TEXT,
    strategy_used TEXT,
    key_learnings TEXT,
    token_cost INTEGER,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_session_episodes_agent ON session_episodes(agent_id);
CREATE INDEX IF NOT EXISTS idx_session_episodes_ward ON session_episodes(ward_id);
CREATE INDEX IF NOT EXISTS idx_session_episodes_outcome ON session_episodes(outcome);

CREATE TABLE IF NOT EXISTS embedding_cache (
    content_hash TEXT NOT NULL,
    model TEXT NOT NULL,
    embedding BLOB NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (content_hash, model)
);

CREATE TABLE IF NOT EXISTS kg_episode_payloads (
    episode_id TEXT PRIMARY KEY,
    text TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (episode_id) REFERENCES kg_episodes(id) ON DELETE CASCADE
);
"#;

#[allow(dead_code)] // retained for reference/tests; runtime uses initialize_vec_tables_with_dim
const VEC0_SQL: &str = r#"
CREATE VIRTUAL TABLE IF NOT EXISTS kg_name_index USING vec0(
    entity_id TEXT PRIMARY KEY,
    name_embedding FLOAT[384]
);

CREATE VIRTUAL TABLE IF NOT EXISTS memory_facts_index USING vec0(
    fact_id TEXT PRIMARY KEY,
    embedding FLOAT[384]
);

CREATE VIRTUAL TABLE IF NOT EXISTS wiki_articles_index USING vec0(
    article_id TEXT PRIMARY KEY,
    embedding FLOAT[384]
);

CREATE VIRTUAL TABLE IF NOT EXISTS procedures_index USING vec0(
    procedure_id TEXT PRIMARY KEY,
    embedding FLOAT[384]
);

CREATE VIRTUAL TABLE IF NOT EXISTS session_episodes_index USING vec0(
    episode_id TEXT PRIMARY KEY,
    embedding FLOAT[384]
);
"#;

const TRIGGERS_SQL: &str = r#"
-- Clean up vec0 partner rows when base rows are deleted.

CREATE TRIGGER IF NOT EXISTS trg_entities_delete_vec
AFTER DELETE ON kg_entities
BEGIN
    DELETE FROM kg_name_index WHERE entity_id = OLD.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_facts_delete_vec
AFTER DELETE ON memory_facts
BEGIN
    DELETE FROM memory_facts_index WHERE fact_id = OLD.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_wiki_delete_vec
AFTER DELETE ON ward_wiki_articles
BEGIN
    DELETE FROM wiki_articles_index WHERE article_id = OLD.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_procedures_delete_vec
AFTER DELETE ON procedures
BEGIN
    DELETE FROM procedures_index WHERE procedure_id = OLD.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_episodes_delete_vec
AFTER DELETE ON session_episodes
BEGIN
    DELETE FROM session_episodes_index WHERE episode_id = OLD.id;
END;

-- Keep the memory_facts_fts contentless FTS5 index in sync with memory_facts.

CREATE TRIGGER IF NOT EXISTS memory_facts_ai
AFTER INSERT ON memory_facts
BEGIN
    INSERT INTO memory_facts_fts(rowid, key, content, category)
    VALUES (new.rowid, new.key, new.content, new.category);
END;

CREATE TRIGGER IF NOT EXISTS memory_facts_ad
AFTER DELETE ON memory_facts
BEGIN
    INSERT INTO memory_facts_fts(memory_facts_fts, rowid, key, content, category)
    VALUES ('delete', old.rowid, old.key, old.content, old.category);
END;

CREATE TRIGGER IF NOT EXISTS memory_facts_au
AFTER UPDATE ON memory_facts
BEGIN
    INSERT INTO memory_facts_fts(memory_facts_fts, rowid, key, content, category)
    VALUES ('delete', old.rowid, old.key, old.content, old.category);
    INSERT INTO memory_facts_fts(rowid, key, content, category)
    VALUES (new.rowid, new.key, new.content, new.category);
END;
"#;

/// Initialize vec0 virtual tables and cleanup triggers.
///
/// Call AFTER `load_sqlite_vec()` AND AFTER `initialize_knowledge_database()`.
/// Triggers reference both vec0 tables and base tables.
///
/// Uses the default embedding dimension of 384.
pub fn initialize_vec_tables(conn: &Connection) -> Result<(), rusqlite::Error> {
    initialize_vec_tables_with_dim(conn, 384)
}

/// The five vec0 virtual tables materialised by
/// [`initialize_vec_tables_with_dim`]. Kept in one place so the post-init
/// presence check and the reindex pipeline stay aligned with the DDL.
pub const REQUIRED_VEC_TABLES: &[&str] = &[
    "memory_facts_index",
    "kg_name_index",
    "session_episodes_index",
    "wiki_articles_index",
    "procedures_index",
];

/// Variant of [`initialize_vec_tables`] that parameterizes the embedding
/// dimension for the vec0 virtual tables.
///
/// Phase 1 of embedding-backend-selection: callers that know the active
/// embedding dimension (e.g. `EmbeddingService`) pass it here so fresh
/// installs honor the user's chosen backend dim. Existing installs still
/// use 384 via [`initialize_vec_tables`].
///
/// Note: `CREATE VIRTUAL TABLE IF NOT EXISTS` is a no-op when the table
/// already exists with a different dim — reindex must drop-and-recreate.
///
/// After running the CREATE batch we verify that all five expected vec0
/// tables materialised. If sqlite-vec failed to load on the connection,
/// `CREATE VIRTUAL TABLE ... USING vec0(...)` silently no-ops; returning an
/// error here lets callers (notably `KnowledgeDatabase::new`'s `.expect`
/// at daemon boot) fail loud with a descriptive message instead of leaving
/// `memory.recall` to blow up on the first query.
pub fn initialize_vec_tables_with_dim(
    conn: &Connection,
    dim: usize,
) -> Result<(), rusqlite::Error> {
    let sql = format!(
        r#"
CREATE VIRTUAL TABLE IF NOT EXISTS kg_name_index USING vec0(
    entity_id TEXT PRIMARY KEY,
    name_embedding FLOAT[{dim}]
);

CREATE VIRTUAL TABLE IF NOT EXISTS memory_facts_index USING vec0(
    fact_id TEXT PRIMARY KEY,
    embedding FLOAT[{dim}]
);

CREATE VIRTUAL TABLE IF NOT EXISTS wiki_articles_index USING vec0(
    article_id TEXT PRIMARY KEY,
    embedding FLOAT[{dim}]
);

CREATE VIRTUAL TABLE IF NOT EXISTS procedures_index USING vec0(
    procedure_id TEXT PRIMARY KEY,
    embedding FLOAT[{dim}]
);

CREATE VIRTUAL TABLE IF NOT EXISTS session_episodes_index USING vec0(
    episode_id TEXT PRIMARY KEY,
    embedding FLOAT[{dim}]
);
"#
    );
    conn.execute_batch(&sql)?;
    conn.execute_batch(TRIGGERS_SQL)?;
    verify_vec_tables_present(conn)?;
    Ok(())
}

/// Verify all [`REQUIRED_VEC_TABLES`] exist in `sqlite_master`.
///
/// Returns a descriptive error when the count is short — the usual cause
/// is that the sqlite-vec extension failed to load on this connection, so
/// the `CREATE VIRTUAL TABLE ... USING vec0(...)` statements became
/// silent no-ops.
fn verify_vec_tables_present(conn: &Connection) -> Result<(), rusqlite::Error> {
    let expected = REQUIRED_VEC_TABLES.len() as i64;
    let row_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN \
         ('memory_facts_index', 'kg_name_index', 'session_episodes_index', \
          'wiki_articles_index', 'procedures_index')",
        [],
        |r| r.get(0),
    )?;
    if row_count != expected {
        let message = format!(
            "vec0 table init incomplete: expected {expected} virtual tables, \
             found {row_count}. sqlite-vec extension likely failed to load — \
             check logs for sqlite_vec errors."
        );
        // Re-use ToSqlConversionFailure as a general error carrier —
        // rusqlite 0.32's `ModuleError` / `UserFunctionError` variants are
        // feature-gated behind `vtab` / `functions` which this crate does
        // not enable. Callers bubble this up via `KnowledgeDatabase::new`'s
        // `.expect()` at daemon boot so the operator sees the full context.
        return Err(rusqlite::Error::ToSqlConversionFailure(
            std::io::Error::other(message).into(),
        ));
    }
    Ok(())
}

/// Query `sqlite_master` for which [`REQUIRED_VEC_TABLES`] exist.
///
/// Returns `(present, missing)` as `Vec<String>` lists suitable for the
/// `/api/embeddings/health` endpoint. Unlike [`verify_vec_tables_present`]
/// this never errors — it returns empty-vectors on a DB error so the
/// health endpoint keeps responding.
#[must_use]
pub fn list_vec_table_presence(conn: &Connection) -> (Vec<String>, Vec<String>) {
    let mut present = Vec::new();
    let mut missing = Vec::new();
    for &name in REQUIRED_VEC_TABLES {
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1",
                rusqlite::params![name],
                |_r| Ok(true),
            )
            .unwrap_or(false);
        if exists {
            present.push(name.to_string());
        } else {
            missing.push(name.to_string());
        }
    }
    (present, missing)
}

/// Drop every live vec0 index and recreate it at `dim`.
///
/// Called by the boot-time dim reconciler in `AppState::new` when the
/// configured `EmbeddingService` dim disagrees with the `.embedding-state`
/// marker. Data loss is intentional — the reindex pipeline repopulates
/// from the source tables on the next sleep cycle. Until that repopulates,
/// recall returns empty results rather than blowing up with a dim mismatch.
///
/// # Errors
///
/// Returns an error if DROP or CREATE fails on the connection (e.g.
/// sqlite-vec failed to load).
pub fn drop_and_recreate_vec_tables_at_dim(
    conn: &Connection,
    dim: usize,
) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "DROP TABLE IF EXISTS memory_facts_index;
         DROP TABLE IF EXISTS kg_name_index;
         DROP TABLE IF EXISTS session_episodes_index;
         DROP TABLE IF EXISTS wiki_articles_index;
         DROP TABLE IF EXISTS procedures_index;",
    )?;
    initialize_vec_tables_with_dim(conn, dim)
}

/// Drop any orphan `*__new` reindex tables left behind by a crash. Idempotent.
///
/// # Errors
///
/// Returns an error if any of the `DROP TABLE` statements fail.
pub fn cleanup_orphan_reindex_tables(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "DROP TABLE IF EXISTS memory_facts_index__new;
         DROP TABLE IF EXISTS kg_name_index__new;
         DROP TABLE IF EXISTS session_episodes_index__new;
         DROP TABLE IF EXISTS wiki_articles_index__new;
         DROP TABLE IF EXISTS procedures_index__new;",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_v22_non_vec_tables_initialize_on_fresh_in_memory_db() {
        let conn = Connection::open_in_memory().expect("open");
        initialize_knowledge_database(&conn).expect("init");

        let version: i32 = conn
            .query_row("SELECT version FROM schema_version", [], |r| r.get(0))
            .expect("version");
        assert_eq!(version, 24);

        // Regular tables.
        for table in [
            "kg_entities",
            "kg_relationships",
            "kg_aliases",
            "kg_episodes",
            "kg_goals",
            "kg_compactions",
            "memory_facts",
            "memory_facts_archive",
            "ward_wiki_articles",
            "procedures",
            "session_episodes",
            "embedding_cache",
            "kg_episode_payloads",
        ] {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    rusqlite::params![table],
                    |r| r.get(0),
                )
                .expect("query");
            assert_eq!(count, 1, "missing table: {table}");
        }

        // FTS5 virtual table for memory_facts.
        let fts_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name='memory_facts_fts'",
                [],
                |r| r.get(0),
            )
            .expect("query fts");
        assert!(fts_count >= 1, "memory_facts_fts not created");

        // Structural assertion: the base tables carry NO embedding column.
        for table in [
            "memory_facts",
            "ward_wiki_articles",
            "procedures",
            "session_episodes",
        ] {
            let has_embedding: i64 = conn
                .query_row(
                    &format!(
                        "SELECT COUNT(*) FROM pragma_table_info('{}') WHERE name='embedding'",
                        table
                    ),
                    [],
                    |r| r.get(0),
                )
                .expect("pragma");
            assert_eq!(
                has_embedding, 0,
                "table {table} must not have embedding BLOB column"
            );
        }
    }

    use crate::sqlite_vec_loader::load_sqlite_vec;

    #[test]
    fn full_v22_schema_initializes_with_vec_tables_and_triggers() {
        let conn = Connection::open_in_memory().expect("open");
        load_sqlite_vec(&conn).expect("load sqlite-vec");

        initialize_knowledge_database(&conn).expect("init base schema");
        initialize_vec_tables(&conn).expect("init vec tables");

        // All 5 vec0 virtual tables exist.
        for vt in [
            "kg_name_index",
            "memory_facts_index",
            "wiki_articles_index",
            "procedures_index",
            "session_episodes_index",
        ] {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE name=?1",
                    rusqlite::params![vt],
                    |r| r.get(0),
                )
                .expect("query");
            assert!(count >= 1, "missing vec0 table: {vt}");
        }

        // All 5 triggers exist.
        for trg in [
            "trg_entities_delete_vec",
            "trg_facts_delete_vec",
            "trg_wiki_delete_vec",
            "trg_procedures_delete_vec",
            "trg_episodes_delete_vec",
        ] {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='trigger' AND name=?1",
                    rusqlite::params![trg],
                    |r| r.get(0),
                )
                .expect("query");
            assert_eq!(count, 1, "missing trigger: {trg}");
        }
    }

    #[test]
    fn delete_entity_cascades_to_kg_name_index_via_trigger() {
        let conn = Connection::open_in_memory().expect("open");
        load_sqlite_vec(&conn).expect("load");
        initialize_knowledge_database(&conn).expect("init");
        initialize_vec_tables(&conn).expect("init vec");

        // Insert entity + its vec row.
        conn.execute(
            "INSERT INTO kg_entities(id, agent_id, entity_type, name, normalized_name, normalized_hash, first_seen_at, last_seen_at)
             VALUES ('e1', 'root', 'person', 'Alice', 'alice', 'h1', datetime('now'), datetime('now'))",
            [],
        )
        .expect("insert entity");

        let vec_json = serde_json::to_string(&vec![0.1_f32; 384]).unwrap();
        conn.execute(
            "INSERT INTO kg_name_index(entity_id, name_embedding) VALUES ('e1', ?1)",
            rusqlite::params![vec_json],
        )
        .expect("insert vec row");

        // Delete the entity — trigger must cascade to vec index.
        conn.execute("DELETE FROM kg_entities WHERE id = 'e1'", [])
            .expect("delete");

        let vec_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM kg_name_index WHERE entity_id = 'e1'",
                [],
                |r| r.get(0),
            )
            .expect("count");
        assert_eq!(vec_count, 0, "vec0 row should be cleaned up by trigger");
    }

    // -----------------------------------------------------------------
    // Fix 1: verify_vec_tables_present / initialize_vec_tables_with_dim
    // fails loud when sqlite-vec didn't load on the connection.
    // -----------------------------------------------------------------

    #[test]
    fn initialize_vec_tables_errors_when_sqlite_vec_not_loaded() {
        // Open a plain connection WITHOUT load_sqlite_vec — the CREATE VIRTUAL
        // TABLE ... USING vec0(...) statements become "no such module: vec0"
        // which is already caught by execute_batch; but belt-and-braces the
        // verify_vec_tables_present call also guarantees we never silently
        // return Ok with missing tables.
        let conn = Connection::open_in_memory().expect("open");
        initialize_knowledge_database(&conn).expect("init base schema");

        let err = initialize_vec_tables_with_dim(&conn, 384)
            .expect_err("expected failure when sqlite-vec is not loaded");
        let msg = err.to_string();
        assert!(
            msg.contains("vec0") || msg.contains("no such module"),
            "error message should mention vec0: {msg}"
        );
    }

    #[test]
    fn list_vec_table_presence_reports_all_five_when_initialized() {
        let conn = Connection::open_in_memory().expect("open");
        load_sqlite_vec(&conn).expect("load sqlite-vec");
        initialize_knowledge_database(&conn).expect("init base schema");
        initialize_vec_tables_with_dim(&conn, 384).expect("init vec tables");

        let (present, missing) = list_vec_table_presence(&conn);
        assert_eq!(present.len(), 5, "expected 5 present, got: {present:?}");
        assert!(missing.is_empty(), "unexpected missing: {missing:?}");
    }

    #[test]
    fn list_vec_table_presence_reports_missing_when_dropped() {
        let conn = Connection::open_in_memory().expect("open");
        load_sqlite_vec(&conn).expect("load sqlite-vec");
        initialize_knowledge_database(&conn).expect("init base schema");
        initialize_vec_tables_with_dim(&conn, 384).expect("init vec tables");

        // Manually drop one to simulate a partial state.
        conn.execute("DROP TABLE memory_facts_index", [])
            .expect("drop");

        let (present, missing) = list_vec_table_presence(&conn);
        assert_eq!(present.len(), 4);
        assert_eq!(missing, vec!["memory_facts_index".to_string()]);
    }
}
