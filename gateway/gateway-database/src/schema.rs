// ============================================================================
// DATABASE SCHEMA
// SQLite schema for sessions, agent executions, and messages
// ============================================================================

use rusqlite::{Connection, Result};

/// Current schema version
const SCHEMA_VERSION: i32 = 17;

/// Run migrations for existing databases.
///
/// Checks the current schema version and applies any needed migrations.
fn migrate_database(conn: &Connection) -> Result<()> {
    // Check if schema_version table exists
    let has_version: bool = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='schema_version'",
        [],
        |row| row.get::<_, i64>(0),
    )? > 0;

    if !has_version {
        return Ok(()); // Fresh database, no migration needed
    }

    let version: i32 = conn
        .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| {
            row.get(0)
        })
        .unwrap_or(0);

    // v8 → v9: Add routing fields to sessions
    if version < 9 {
        // Use try/ignore pattern since columns may already exist on fresh DB
        let _ = conn.execute("ALTER TABLE sessions ADD COLUMN thread_id TEXT", []);
        let _ = conn.execute("ALTER TABLE sessions ADD COLUMN connector_id TEXT", []);
        let _ = conn.execute("ALTER TABLE sessions ADD COLUMN respond_to TEXT", []);
    }

    // v9 → v10: Add bridge_outbox table
    if version < 10 {
        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS bridge_outbox (
                id TEXT PRIMARY KEY,
                adapter_id TEXT NOT NULL,
                capability TEXT NOT NULL,
                payload TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                session_id TEXT,
                thread_id TEXT,
                agent_id TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                sent_at TEXT,
                error TEXT,
                retry_count INTEGER NOT NULL DEFAULT 0,
                retry_after TEXT
            )",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_outbox_adapter_status ON bridge_outbox(adapter_id, status)",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_outbox_created ON bridge_outbox(created_at)",
            [],
        );
    }

    // v10 → v11: Add distillation_runs, session_episodes tables; add ward_id to memory_facts
    if version < 11 {
        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS distillation_runs (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL UNIQUE,
                status TEXT NOT NULL,
                facts_extracted INTEGER DEFAULT 0,
                entities_extracted INTEGER DEFAULT 0,
                relationships_extracted INTEGER DEFAULT 0,
                episode_created INTEGER DEFAULT 0,
                error TEXT,
                retry_count INTEGER DEFAULT 0,
                duration_ms INTEGER,
                created_at TEXT NOT NULL
            )",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_distillation_runs_status ON distillation_runs(status)",
            [],
        );

        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS session_episodes (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                agent_id TEXT NOT NULL,
                ward_id TEXT NOT NULL DEFAULT '__global__',
                task_summary TEXT NOT NULL,
                outcome TEXT NOT NULL,
                strategy_used TEXT,
                key_learnings TEXT,
                token_cost INTEGER,
                embedding BLOB,
                created_at TEXT NOT NULL
            )",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_session_episodes_agent ON session_episodes(agent_id)",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_session_episodes_ward ON session_episodes(ward_id)",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_session_episodes_outcome ON session_episodes(outcome)",
            [],
        );

        // Add ward_id column to memory_facts
        let _ = conn.execute(
            "ALTER TABLE memory_facts ADD COLUMN ward_id TEXT NOT NULL DEFAULT '__global__'",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memory_facts_ward ON memory_facts(ward_id)",
            [],
        );
        // Drop the old unique constraint (it was an inline UNIQUE, which SQLite
        // implements as an auto-named index "sqlite_autoindex_memory_facts_1")
        let _ = conn.execute("DROP INDEX IF EXISTS sqlite_autoindex_memory_facts_1", []);
        // Create the new unique constraint including ward_id
        let _ = conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS uq_memory_facts_agent_scope_ward_key ON memory_facts(agent_id, scope, ward_id, key)",
            [],
        );
    }

    // v11 → v12: Add contradicted_by column to memory_facts
    if version < 12 {
        let _ = conn.execute(
            "ALTER TABLE memory_facts ADD COLUMN contradicted_by TEXT",
            [],
        );
    }

    // v12 → v13: Add recall_log, memory_facts_archive tables; add archived to sessions
    if version < 13 {
        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS recall_log (
                session_id TEXT NOT NULL,
                fact_key TEXT NOT NULL,
                recalled_at TEXT NOT NULL,
                PRIMARY KEY (session_id, fact_key)
            )",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_recall_log_session ON recall_log(session_id)",
            [],
        );

        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS memory_facts_archive (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                scope TEXT NOT NULL DEFAULT 'agent',
                category TEXT NOT NULL,
                key TEXT NOT NULL,
                content TEXT NOT NULL,
                confidence REAL NOT NULL DEFAULT 0.8,
                ward_id TEXT NOT NULL DEFAULT '__global__',
                mention_count INTEGER NOT NULL DEFAULT 1,
                source_summary TEXT,
                embedding BLOB,
                contradicted_by TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                archived_at TEXT NOT NULL
            )",
            [],
        );

        let _ = conn.execute(
            "ALTER TABLE sessions ADD COLUMN archived INTEGER NOT NULL DEFAULT 0",
            [],
        );
    }

    // v13 → v14: Add pinned column to memory_facts
    if version < 14 {
        let _ = conn.execute(
            "ALTER TABLE memory_facts ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0",
            [],
        );
    }

    // v14 → v15: Add child_session_id to agent_executions for smart resume
    if version < 15 {
        let _ = conn.execute(
            "ALTER TABLE agent_executions ADD COLUMN child_session_id TEXT",
            [],
        );
    }

    // v15 → v16: Add artifacts table for agent-generated file tracking
    if version < 16 {
        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS artifacts (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                ward_id TEXT,
                execution_id TEXT,
                agent_id TEXT,
                file_path TEXT NOT NULL,
                file_name TEXT NOT NULL,
                file_type TEXT,
                file_size INTEGER,
                label TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
            )",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_artifacts_session ON artifacts(session_id)",
            [],
        );
    }

    // v16 → v17: Add mode column to sessions for persistent execution mode
    if version < 17 {
        let _ = conn.execute("ALTER TABLE sessions ADD COLUMN mode TEXT", []);
    }

    Ok(())
}

/// Initialize the database with all tables
pub fn initialize_database(conn: &Connection) -> Result<()> {
    // Run migrations for existing databases before creating tables
    migrate_database(conn)?;

    // Enable foreign keys
    conn.execute("PRAGMA foreign_keys = ON", [])?;

    // =========================================================================
    // SESSIONS
    // Top-level container for a user's work session
    // =========================================================================
    conn.execute(
        "CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            status TEXT NOT NULL DEFAULT 'running',
            source TEXT NOT NULL DEFAULT 'web',
            root_agent_id TEXT NOT NULL,
            title TEXT,
            created_at TEXT NOT NULL,
            started_at TEXT,
            completed_at TEXT,
            total_tokens_in INTEGER DEFAULT 0,
            total_tokens_out INTEGER DEFAULT 0,
            metadata TEXT,
            pending_delegations INTEGER DEFAULT 0,
            continuation_needed INTEGER DEFAULT 0,
            ward_id TEXT,
            parent_session_id TEXT,
            thread_id TEXT,
            connector_id TEXT,
            respond_to TEXT,
            archived INTEGER NOT NULL DEFAULT 0,
            mode TEXT
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_sessions_status ON sessions(status)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_sessions_created ON sessions(created_at)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_sessions_root_agent ON sessions(root_agent_id)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_sessions_source ON sessions(source)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_sessions_parent ON sessions(parent_session_id)",
        [],
    )?;

    // =========================================================================
    // AGENT EXECUTIONS
    // An agent's participation in a session (root or delegated subagent)
    // =========================================================================
    conn.execute(
        "CREATE TABLE IF NOT EXISTS agent_executions (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            agent_id TEXT NOT NULL,
            parent_execution_id TEXT,
            delegation_type TEXT NOT NULL DEFAULT 'root',
            task TEXT,
            status TEXT NOT NULL DEFAULT 'queued',
            started_at TEXT,
            completed_at TEXT,
            tokens_in INTEGER DEFAULT 0,
            tokens_out INTEGER DEFAULT 0,
            checkpoint TEXT,
            error TEXT,
            log_path TEXT,
            child_session_id TEXT,
            FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE,
            FOREIGN KEY (parent_execution_id) REFERENCES agent_executions(id) ON DELETE SET NULL,
            FOREIGN KEY (child_session_id) REFERENCES sessions(id) ON DELETE SET NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_executions_session ON agent_executions(session_id)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_executions_parent ON agent_executions(parent_execution_id)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_executions_status ON agent_executions(status)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_executions_agent ON agent_executions(agent_id)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_executions_started ON agent_executions(started_at)",
        [],
    )?;

    // =========================================================================
    // MESSAGES
    // Individual messages in an agent's conversation
    // =========================================================================
    conn.execute(
        "CREATE TABLE IF NOT EXISTS messages (
            id TEXT PRIMARY KEY,
            execution_id TEXT,
            session_id TEXT,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL,
            token_count INTEGER DEFAULT 0,
            tool_calls TEXT,
            tool_results TEXT,
            tool_call_id TEXT,
            FOREIGN KEY (execution_id) REFERENCES agent_executions(id) ON DELETE CASCADE,
            FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_messages_execution ON messages(execution_id)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_messages_created ON messages(created_at)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_messages_session ON messages(session_id)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_messages_session_created ON messages(session_id, created_at)",
        [],
    )?;

    // =========================================================================
    // EXECUTION LOGS
    // Detailed logs for debugging and tracing agent execution
    // =========================================================================
    conn.execute(
        "CREATE TABLE IF NOT EXISTS execution_logs (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            conversation_id TEXT,
            agent_id TEXT NOT NULL,
            parent_session_id TEXT,
            timestamp TEXT NOT NULL,
            level TEXT NOT NULL,
            category TEXT NOT NULL,
            message TEXT NOT NULL,
            metadata TEXT,
            duration_ms INTEGER
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_logs_session ON execution_logs(session_id)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_logs_timestamp ON execution_logs(timestamp)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_logs_agent ON execution_logs(agent_id)",
        [],
    )?;

    // =========================================================================
    // MEMORY FACTS
    // Structured memory facts from session distillation or manual save
    // =========================================================================
    conn.execute(
        "CREATE TABLE IF NOT EXISTS memory_facts (
            id TEXT PRIMARY KEY,
            session_id TEXT,
            agent_id TEXT NOT NULL,
            scope TEXT NOT NULL DEFAULT 'agent',
            category TEXT NOT NULL,
            key TEXT NOT NULL,
            content TEXT NOT NULL,
            confidence REAL NOT NULL DEFAULT 0.8,
            mention_count INTEGER NOT NULL DEFAULT 1,
            source_summary TEXT,
            embedding BLOB,
            ward_id TEXT NOT NULL DEFAULT '__global__',
            contradicted_by TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            expires_at TEXT,
            pinned INTEGER NOT NULL DEFAULT 0,
            UNIQUE(agent_id, scope, ward_id, key)
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_memory_facts_agent ON memory_facts(agent_id, scope)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_memory_facts_category ON memory_facts(agent_id, category)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_memory_facts_updated ON memory_facts(updated_at)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_memory_facts_ward ON memory_facts(ward_id)",
        [],
    )?;

    // FTS5 virtual table for BM25 keyword search over memory facts.
    // content='' makes it an external-content table (we sync manually).
    conn.execute_batch(
        "CREATE VIRTUAL TABLE IF NOT EXISTS memory_facts_fts USING fts5(
            key, content, category,
            content='memory_facts',
            content_rowid='rowid'
        );",
    )?;

    // Triggers to keep FTS index in sync with memory_facts table.
    // These fire on INSERT, UPDATE, and DELETE.
    conn.execute_batch(
        "CREATE TRIGGER IF NOT EXISTS memory_facts_ai AFTER INSERT ON memory_facts BEGIN
            INSERT INTO memory_facts_fts(rowid, key, content, category)
            VALUES (new.rowid, new.key, new.content, new.category);
        END;

        CREATE TRIGGER IF NOT EXISTS memory_facts_ad AFTER DELETE ON memory_facts BEGIN
            INSERT INTO memory_facts_fts(memory_facts_fts, rowid, key, content, category)
            VALUES ('delete', old.rowid, old.key, old.content, old.category);
        END;

        CREATE TRIGGER IF NOT EXISTS memory_facts_au AFTER UPDATE ON memory_facts BEGIN
            INSERT INTO memory_facts_fts(memory_facts_fts, rowid, key, content, category)
            VALUES ('delete', old.rowid, old.key, old.content, old.category);
            INSERT INTO memory_facts_fts(rowid, key, content, category)
            VALUES (new.rowid, new.key, new.content, new.category);
        END;",
    )?;

    // =========================================================================
    // EMBEDDING CACHE
    // Hash-based dedup to avoid re-embedding unchanged content
    // =========================================================================
    conn.execute(
        "CREATE TABLE IF NOT EXISTS embedding_cache (
            content_hash TEXT NOT NULL,
            model TEXT NOT NULL,
            embedding BLOB NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            PRIMARY KEY (content_hash, model)
        )",
        [],
    )?;

    // =========================================================================
    // BRIDGE OUTBOX
    // Reliable delivery queue for outbound messages to bridge workers
    // =========================================================================
    conn.execute(
        "CREATE TABLE IF NOT EXISTS bridge_outbox (
            id TEXT PRIMARY KEY,
            adapter_id TEXT NOT NULL,
            capability TEXT NOT NULL,
            payload TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            session_id TEXT,
            thread_id TEXT,
            agent_id TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            sent_at TEXT,
            error TEXT,
            retry_count INTEGER NOT NULL DEFAULT 0,
            retry_after TEXT
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_outbox_adapter_status ON bridge_outbox(adapter_id, status)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_outbox_created ON bridge_outbox(created_at)",
        [],
    )?;

    // =========================================================================
    // DISTILLATION RUNS
    // Tracks distillation health per session
    // =========================================================================
    conn.execute(
        "CREATE TABLE IF NOT EXISTS distillation_runs (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL UNIQUE,
            status TEXT NOT NULL,
            facts_extracted INTEGER DEFAULT 0,
            entities_extracted INTEGER DEFAULT 0,
            relationships_extracted INTEGER DEFAULT 0,
            episode_created INTEGER DEFAULT 0,
            error TEXT,
            retry_count INTEGER DEFAULT 0,
            duration_ms INTEGER,
            created_at TEXT NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_distillation_runs_status ON distillation_runs(status)",
        [],
    )?;

    // =========================================================================
    // SESSION EPISODES
    // Episodic memory with execution outcomes
    // =========================================================================
    conn.execute(
        "CREATE TABLE IF NOT EXISTS session_episodes (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            agent_id TEXT NOT NULL,
            ward_id TEXT NOT NULL DEFAULT '__global__',
            task_summary TEXT NOT NULL,
            outcome TEXT NOT NULL,
            strategy_used TEXT,
            key_learnings TEXT,
            token_cost INTEGER,
            embedding BLOB,
            created_at TEXT NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_session_episodes_agent ON session_episodes(agent_id)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_session_episodes_ward ON session_episodes(ward_id)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_session_episodes_outcome ON session_episodes(outcome)",
        [],
    )?;

    // =========================================================================
    // RECALL LOG
    // Tracks which facts were recalled per session for predictive recall
    // =========================================================================
    conn.execute(
        "CREATE TABLE IF NOT EXISTS recall_log (
            session_id TEXT NOT NULL,
            fact_key TEXT NOT NULL,
            recalled_at TEXT NOT NULL,
            PRIMARY KEY (session_id, fact_key)
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_recall_log_session ON recall_log(session_id)",
        [],
    )?;

    // =========================================================================
    // MEMORY FACTS ARCHIVE
    // Pruned facts preserved for audit trail
    // =========================================================================
    conn.execute(
        "CREATE TABLE IF NOT EXISTS memory_facts_archive (
            id TEXT PRIMARY KEY,
            agent_id TEXT NOT NULL,
            scope TEXT NOT NULL DEFAULT 'agent',
            category TEXT NOT NULL,
            key TEXT NOT NULL,
            content TEXT NOT NULL,
            confidence REAL NOT NULL DEFAULT 0.8,
            ward_id TEXT NOT NULL DEFAULT '__global__',
            mention_count INTEGER NOT NULL DEFAULT 1,
            source_summary TEXT,
            embedding BLOB,
            contradicted_by TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            archived_at TEXT NOT NULL
        )",
        [],
    )?;

    // =========================================================================
    // ARTIFACTS
    // File artifacts produced by agent executions
    // =========================================================================
    conn.execute(
        "CREATE TABLE IF NOT EXISTS artifacts (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            ward_id TEXT,
            execution_id TEXT,
            agent_id TEXT,
            file_path TEXT NOT NULL,
            file_name TEXT NOT NULL,
            file_type TEXT,
            file_size INTEGER,
            label TEXT,
            created_at TEXT NOT NULL,
            FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_artifacts_session ON artifacts(session_id)",
        [],
    )?;

    // =========================================================================
    // SCHEMA VERSION
    // =========================================================================
    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER PRIMARY KEY
        )",
        [],
    )?;

    conn.execute(
        "INSERT OR REPLACE INTO schema_version (version) VALUES (?1)",
        [SCHEMA_VERSION],
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    /// Helper: create a fresh in-memory database and initialize it.
    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        initialize_database(&conn).expect("initialize_database");
        conn
    }

    #[test]
    fn test_migration_creates_distillation_runs_table() {
        let conn = setup_db();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='distillation_runs'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "distillation_runs table should exist");
    }

    #[test]
    fn test_migration_creates_session_episodes_table() {
        let conn = setup_db();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='session_episodes'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "session_episodes table should exist");
    }

    #[test]
    fn test_memory_facts_has_ward_id_column() {
        let conn = setup_db();
        // Insert a row without specifying ward_id — it should default to '__global__'
        conn.execute(
            "INSERT INTO memory_facts (id, agent_id, scope, category, key, content, created_at, updated_at)
             VALUES ('f1', 'agent1', 'agent', 'pref', 'color', 'blue', datetime('now'), datetime('now'))",
            [],
        )
        .expect("insert with default ward_id");

        let ward_id: String = conn
            .query_row(
                "SELECT ward_id FROM memory_facts WHERE id = 'f1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            ward_id, "__global__",
            "default ward_id should be '__global__'"
        );
    }

    #[test]
    fn test_different_ward_ids_allow_same_key() {
        let conn = setup_db();
        // Insert fact with ward_id = 'ward_a'
        conn.execute(
            "INSERT INTO memory_facts (id, agent_id, scope, category, key, content, ward_id, created_at, updated_at)
             VALUES ('f1', 'agent1', 'agent', 'pref', 'color', 'blue', 'ward_a', datetime('now'), datetime('now'))",
            [],
        )
        .expect("insert ward_a");

        // Insert same agent_id/scope/key but with ward_id = 'ward_b' — should succeed
        conn.execute(
            "INSERT INTO memory_facts (id, agent_id, scope, category, key, content, ward_id, created_at, updated_at)
             VALUES ('f2', 'agent1', 'agent', 'pref', 'color', 'red', 'ward_b', datetime('now'), datetime('now'))",
            [],
        )
        .expect("insert ward_b with same key should succeed");

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM memory_facts", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 2, "two facts with different ward_ids should coexist");
    }

    #[test]
    fn test_same_ward_id_same_key_conflicts() {
        let conn = setup_db();
        conn.execute(
            "INSERT INTO memory_facts (id, agent_id, scope, category, key, content, ward_id, created_at, updated_at)
             VALUES ('f1', 'agent1', 'agent', 'pref', 'color', 'blue', 'ward_a', datetime('now'), datetime('now'))",
            [],
        )
        .expect("insert first");

        // Same agent_id, scope, ward_id, key — should fail with UNIQUE constraint
        let result = conn.execute(
            "INSERT INTO memory_facts (id, agent_id, scope, category, key, content, ward_id, created_at, updated_at)
             VALUES ('f2', 'agent1', 'agent', 'pref', 'color', 'red', 'ward_a', datetime('now'), datetime('now'))",
            [],
        );
        assert!(
            result.is_err(),
            "duplicate (agent_id, scope, ward_id, key) should be rejected"
        );
    }

    #[test]
    fn test_schema_version_is_16() {
        let conn = setup_db();
        let version: i32 = conn
            .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(version, 17, "schema version should be 17");
    }

    #[test]
    fn test_recall_log_table_exists() {
        let conn = setup_db();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='recall_log'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "recall_log table should exist");

        // Verify we can insert and query
        conn.execute(
            "INSERT INTO recall_log (session_id, fact_key, recalled_at) VALUES ('s1', 'k1', datetime('now'))",
            [],
        )
        .expect("insert into recall_log");

        let fact_key: String = conn
            .query_row(
                "SELECT fact_key FROM recall_log WHERE session_id = 's1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(fact_key, "k1");
    }

    #[test]
    fn test_memory_facts_archive_table_exists() {
        let conn = setup_db();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='memory_facts_archive'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "memory_facts_archive table should exist");

        // Verify we can insert and query
        conn.execute(
            "INSERT INTO memory_facts_archive (id, agent_id, scope, category, key, content, confidence, ward_id, mention_count, created_at, updated_at, archived_at)
             VALUES ('a1', 'agent1', 'agent', 'pref', 'color', 'blue', 0.9, '__global__', 3, datetime('now'), datetime('now'), datetime('now'))",
            [],
        )
        .expect("insert into memory_facts_archive");

        let content: String = conn
            .query_row(
                "SELECT content FROM memory_facts_archive WHERE id = 'a1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(content, "blue");
    }

    #[test]
    fn test_sessions_has_archived_column() {
        let conn = setup_db();
        // Insert a session and verify archived defaults to 0
        conn.execute(
            "INSERT INTO sessions (id, root_agent_id, created_at) VALUES ('s1', 'agent1', datetime('now'))",
            [],
        )
        .expect("insert session");

        let archived: i32 = conn
            .query_row("SELECT archived FROM sessions WHERE id = 's1'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(archived, 0, "default archived should be 0");

        // Update to archived
        conn.execute("UPDATE sessions SET archived = 1 WHERE id = 's1'", [])
            .expect("update archived");

        let archived: i32 = conn
            .query_row("SELECT archived FROM sessions WHERE id = 's1'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(archived, 1, "archived should be 1 after update");
    }

    #[test]
    fn test_memory_facts_has_contradicted_by_column() {
        let conn = setup_db();
        // Insert a row — contradicted_by defaults to NULL
        conn.execute(
            "INSERT INTO memory_facts (id, agent_id, scope, category, key, content, created_at, updated_at)
             VALUES ('f_contra', 'agent1', 'agent', 'pref', 'color', 'blue', datetime('now'), datetime('now'))",
            [],
        )
        .expect("insert with default contradicted_by");

        let contradicted_by: Option<String> = conn
            .query_row(
                "SELECT contradicted_by FROM memory_facts WHERE id = 'f_contra'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(
            contradicted_by.is_none(),
            "default contradicted_by should be NULL"
        );

        // Update contradicted_by
        conn.execute(
            "UPDATE memory_facts SET contradicted_by = 'other.key' WHERE id = 'f_contra'",
            [],
        )
        .expect("update contradicted_by");

        let contradicted_by: Option<String> = conn
            .query_row(
                "SELECT contradicted_by FROM memory_facts WHERE id = 'f_contra'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(contradicted_by, Some("other.key".to_string()));
    }
}
