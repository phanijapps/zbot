// ============================================================================
// AGENT CHANNEL DATABASE SCHEMA v2
// SQLite schema for agent channels, daily sessions, and knowledge graph
// ============================================================================

use rusqlite::{Connection, Result};

/// Agent Channel database schema version
const SCHEMA_VERSION: i32 = 2;

/// Initialize the database with all tables for Agent Channel model
pub fn initialize_database_v2(conn: &Connection) -> Result<()> {
    // Enable foreign keys
    conn.execute("PRAGMA foreign_keys = ON", [])?;

    // Create agents table (loaded from ~/.config/zeroagent/agents/)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS agents (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            display_name TEXT NOT NULL,
            description TEXT,
            config_path TEXT NOT NULL,
            system_prompt_version INTEGER DEFAULT 1,
            current_system_prompt TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
        [],
    )?;

    // Create daily_sessions table (replaces conversations table)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS daily_sessions (
            id TEXT PRIMARY KEY,
            agent_id TEXT NOT NULL,
            session_date TEXT NOT NULL,
            summary TEXT,
            previous_session_ids TEXT,
            message_count INTEGER DEFAULT 0,
            token_count INTEGER DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (agent_id) REFERENCES agents(id) ON DELETE CASCADE
        )",
        [],
    )?;

    // Create indexes for daily_sessions
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_daily_sessions_agent_date
         ON daily_sessions(agent_id, session_date DESC)",
        [],
    )?;

    // Create messages table (uses session_id instead of conversation_id)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS messages (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL,
            token_count INTEGER DEFAULT 0,
            tool_calls TEXT,
            tool_results TEXT,
            FOREIGN KEY (session_id) REFERENCES daily_sessions(id) ON DELETE CASCADE
        )",
        [],
    )?;

    // Create indexes for messages
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_messages_session_created
         ON messages(session_id, created_at)",
        [],
    )?;

    // Create knowledge graph entities table (placeholder for future)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS kg_entities (
            id TEXT PRIMARY KEY,
            agent_id TEXT NOT NULL,
            entity_type TEXT NOT NULL,
            name TEXT NOT NULL,
            properties TEXT,
            first_seen_at TEXT NOT NULL,
            last_seen_at TEXT NOT NULL,
            FOREIGN KEY (agent_id) REFERENCES agents(id) ON DELETE CASCADE
        )",
        [],
    )?;

    // Create knowledge graph relationships table (placeholder for future)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS kg_relationships (
            id TEXT PRIMARY KEY,
            agent_id TEXT NOT NULL,
            source_entity_id TEXT NOT NULL,
            target_entity_id TEXT NOT NULL,
            relationship_type TEXT NOT NULL,
            properties TEXT,
            first_seen_at TEXT NOT NULL,
            last_seen_at TEXT NOT NULL,
            FOREIGN KEY (agent_id) REFERENCES agents(id) ON DELETE CASCADE
        )",
        [],
    )?;

    // Create indexes for knowledge graph
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_kg_entities_agent
         ON kg_entities(agent_id)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_kg_relationships_agent
         ON kg_relationships(agent_id)",
        [],
    )?;

    // Create schema version table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER PRIMARY KEY
        )",
        [],
    )?;

    // Set schema version
    conn.execute(
        "INSERT OR REPLACE INTO schema_version (version) VALUES (?1)",
        [SCHEMA_VERSION],
    )?;

    Ok(())
}

/// Get current schema version
pub fn get_schema_version_v2(conn: &Connection) -> Result<i32> {
    conn.query_row(
        "SELECT version FROM schema_version",
        [],
        |row| row.get(0),
    )
}

/// Check if migrations are needed
pub fn needs_migration_v2(conn: &Connection) -> bool {
    match get_schema_version_v2(conn) {
        Ok(version) => version < SCHEMA_VERSION,
        Err(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialize_database_v2() {
        let conn = Connection::open_in_memory().unwrap();
        initialize_database_v2(&conn).unwrap();

        // Verify tables exist
        let table_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table'",
            [],
            |row| row.get(0),
        ).unwrap();

        // Expected: agents, daily_sessions, messages, kg_entities, kg_relationships, schema_version
        assert_eq!(table_count, 6);
    }

    #[test]
    fn test_schema_version_v2() {
        let conn = Connection::open_in_memory().unwrap();
        initialize_database_v2(&conn).unwrap();

        let version = get_schema_version_v2(&conn).unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        assert!(!needs_migration_v2(&conn));
    }

    #[test]
    fn test_session_id_format() {
        // Test that session IDs follow the format: session_{agent_id}_{YYYY_MM_DD}
        let agent_id = "story-time";
        let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let session_id = format!("session_{}_{}", agent_id, date.replace("-", "_"));

        assert!(session_id.starts_with("session_"));
        assert!(session_id.contains(&agent_id.replace("-", "_")));
    }
}
