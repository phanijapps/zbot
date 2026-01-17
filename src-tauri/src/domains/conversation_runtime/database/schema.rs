// ============================================================================
// DATABASE SCHEMA
// SQLite schema for conversations and messages
// ============================================================================

use rusqlite::{Connection, Result};

/// Conversation database schema version
const SCHEMA_VERSION: i32 = 1;

/// Initialize the database with all tables
pub fn initialize_database(conn: &Connection) -> Result<()> {
    // Enable foreign keys
    conn.execute("PRAGMA foreign_keys = ON", [])?;

    // Create conversations table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS conversations (
            id TEXT PRIMARY KEY,
            agent_id TEXT NOT NULL,
            title TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            metadata TEXT
        )",
        [],
    )?;

    // Create messages table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS messages (
            id TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL,
            token_count INTEGER DEFAULT 0,
            tool_calls TEXT,
            tool_results TEXT,
            FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
        )",
        [],
    )?;

    // Create indexes for performance
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_messages_conversation_id
         ON messages(conversation_id)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_messages_created_at
         ON messages(created_at)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_conversations_agent_id
         ON conversations(agent_id)",
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
pub fn get_schema_version(conn: &Connection) -> Result<i32> {
    conn.query_row(
        "SELECT version FROM schema_version",
        [],
        |row| row.get(0),
    )
}

/// Check if migrations are needed
pub fn needs_migration(conn: &Connection) -> bool {
    match get_schema_version(conn) {
        Ok(version) => version < SCHEMA_VERSION,
        Err(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialize_database() {
        let conn = Connection::open_in_memory().unwrap();
        initialize_database(&conn).unwrap();

        // Verify tables exist
        let table_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table'",
            [],
            |row| row.get(0),
        ).unwrap();

        assert_eq!(table_count, 3); // conversations, messages, schema_version
    }

    #[test]
    fn test_schema_version() {
        let conn = Connection::open_in_memory().unwrap();
        initialize_database(&conn).unwrap();

        let version = get_schema_version(&conn).unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        assert!(!needs_migration(&conn));
    }
}
