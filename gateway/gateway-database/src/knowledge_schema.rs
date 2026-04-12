//! Schema v22 for `knowledge.db`.
//!
//! All long-term memory + graph + vector indexes live here.
//! Applied idempotently on daemon boot. No migrations — clean slate.

use rusqlite::Connection;

const SCHEMA_VERSION: i32 = 22;

/// Initialize the knowledge database schema (v22).
///
/// Creates all tables and indexes if they don't exist. Records the
/// schema version in `schema_version` table. Safe to call on an
/// already-initialized DB.
pub fn initialize_knowledge_database(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(SCHEMA_SQL)?;
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
    t_valid_from TEXT,
    t_valid_to TEXT,
    t_invalidated_by TEXT,
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
    t_invalidated_by TEXT,
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
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_graph_tables_initialize_on_fresh_in_memory_db() {
        let conn = Connection::open_in_memory().expect("open");
        initialize_knowledge_database(&conn).expect("init");

        let version: i32 = conn
            .query_row("SELECT version FROM schema_version", [], |r| r.get(0))
            .expect("version");
        assert_eq!(version, 22);

        for table in [
            "kg_entities",
            "kg_relationships",
            "kg_aliases",
            "kg_episodes",
            "kg_goals",
            "kg_compactions",
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
    }
}
