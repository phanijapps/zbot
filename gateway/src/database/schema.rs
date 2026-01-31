// ============================================================================
// DATABASE SCHEMA
// SQLite schema for conversations, messages, and execution logs
// ============================================================================

use rusqlite::{Connection, Result};

/// Current schema version
const SCHEMA_VERSION: i32 = 2;

/// Initialize the database with all tables
pub fn initialize_database(conn: &Connection) -> Result<()> {
    // Enable foreign keys
    conn.execute("PRAGMA foreign_keys = ON", [])?;

    // Create conversations table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS conversations (
            id TEXT PRIMARY KEY,
            agent_id TEXT NOT NULL,
            title TEXT,
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

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_conversations_updated_at
         ON conversations(updated_at)",
        [],
    )?;

    // Create execution_logs table (for api-logs crate)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS execution_logs (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            conversation_id TEXT NOT NULL,
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

    // Create indexes for execution_logs
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_execution_logs_session_id
         ON execution_logs(session_id)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_execution_logs_conversation_id
         ON execution_logs(conversation_id)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_execution_logs_agent_id
         ON execution_logs(agent_id)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_execution_logs_timestamp
         ON execution_logs(timestamp)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_execution_logs_level
         ON execution_logs(level)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_execution_logs_parent_session_id
         ON execution_logs(parent_session_id)",
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
