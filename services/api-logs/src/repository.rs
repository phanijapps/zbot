//! # Logs Repository
//!
//! Database operations for execution logs.

use crate::types::*;
use rusqlite::{params, Connection, OptionalExtension};
use std::sync::Arc;

// ============================================================================
// DATABASE PROVIDER TRAIT
// ============================================================================

/// Trait for database connection access.
///
/// Gateway implements this with its DatabaseManager, keeping api-logs
/// decoupled from gateway internals.
pub trait DbProvider: Send + Sync {
    /// Execute a function with a database connection.
    fn with_connection<F, R>(&self, f: F) -> Result<R, String>
    where
        F: FnOnce(&Connection) -> Result<R, rusqlite::Error>;
}

// ============================================================================
// REPOSITORY
// ============================================================================

/// Repository for execution log operations.
pub struct LogsRepository<D: DbProvider> {
    db: Arc<D>,
}

impl<D: DbProvider> LogsRepository<D> {
    /// Create a new repository.
    pub fn new(db: Arc<D>) -> Self {
        Self { db }
    }

    // =========================================================================
    // WRITE OPERATIONS
    // =========================================================================

    /// Insert a single log entry.
    pub fn insert_log(&self, log: &ExecutionLog) -> Result<(), String> {
        self.db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO execution_logs (
                    id, session_id, conversation_id, agent_id, parent_session_id,
                    timestamp, level, category, message, metadata, duration_ms
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    log.id,
                    log.session_id,
                    log.conversation_id,
                    log.agent_id,
                    log.parent_session_id,
                    log.timestamp,
                    log.level.as_str(),
                    log.category.as_str(),
                    log.message,
                    log.metadata.as_ref().map(|m| m.to_string()),
                    log.duration_ms,
                ],
            )?;
            Ok(())
        })
    }

    /// Insert multiple log entries in a transaction.
    pub fn insert_batch(&self, logs: &[ExecutionLog]) -> Result<(), String> {
        if logs.is_empty() {
            return Ok(());
        }

        self.db.with_connection(|conn| {
            let tx = conn.unchecked_transaction()?;

            for log in logs {
                tx.execute(
                    "INSERT INTO execution_logs (
                        id, session_id, conversation_id, agent_id, parent_session_id,
                        timestamp, level, category, message, metadata, duration_ms
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    params![
                        log.id,
                        log.session_id,
                        log.conversation_id,
                        log.agent_id,
                        log.parent_session_id,
                        log.timestamp,
                        log.level.as_str(),
                        log.category.as_str(),
                        log.message,
                        log.metadata.as_ref().map(|m| m.to_string()),
                        log.duration_ms,
                    ],
                )?;
            }

            tx.commit()?;
            Ok(())
        })
    }

    // =========================================================================
    // QUERY OPERATIONS
    // =========================================================================

    /// List sessions with optional filtering.
    pub fn list_sessions(&self, filter: &LogFilter) -> Result<Vec<LogSession>, String> {
        self.db.with_connection(|conn| {
            let mut sql = String::from(
                "SELECT
                    e.session_id,
                    e.conversation_id,
                    e.agent_id,
                    MIN(e.timestamp) as started_at,
                    MAX(e.timestamp) as ended_at,
                    COUNT(*) as log_count,
                    SUM(CASE WHEN e.category = 'token' THEN 1 ELSE 0 END) as token_count,
                    SUM(CASE WHEN e.category = 'tool_call' THEN 1 ELSE 0 END) as tool_call_count,
                    SUM(CASE WHEN e.level = 'error' THEN 1 ELSE 0 END) as error_count,
                    MAX(e.parent_session_id) as parent_session_id,
                    s.title as session_title,
                    s.status as session_status,
                    ae.parent_execution_id as ae_parent
                FROM execution_logs e
                LEFT JOIN sessions s ON s.id = e.conversation_id
                LEFT JOIN agent_executions ae ON ae.id = e.session_id
                WHERE 1=1",
            );

            let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

            if let Some(agent_id) = &filter.agent_id {
                sql.push_str(" AND e.agent_id = ?");
                params_vec.push(Box::new(agent_id.clone()));
            }

            if let Some(conversation_id) = &filter.conversation_id {
                sql.push_str(" AND e.conversation_id = ?");
                params_vec.push(Box::new(conversation_id.clone()));
            }

            if let Some(from_time) = &filter.from_time {
                sql.push_str(" AND e.timestamp >= ?");
                params_vec.push(Box::new(from_time.clone()));
            }

            if let Some(to_time) = &filter.to_time {
                sql.push_str(" AND e.timestamp <= ?");
                params_vec.push(Box::new(to_time.clone()));
            }

            if filter.root_only {
                sql.push_str(" AND ae.parent_execution_id IS NULL");
            }

            sql.push_str(" GROUP BY e.session_id ORDER BY started_at DESC");

            if let Some(limit) = filter.limit {
                sql.push_str(&format!(" LIMIT {}", limit));
            } else {
                sql.push_str(" LIMIT 100"); // Default limit
            }

            if let Some(offset) = filter.offset {
                sql.push_str(&format!(" OFFSET {}", offset));
            }

            let params_refs: Vec<&dyn rusqlite::ToSql> =
                params_vec.iter().map(|p| p.as_ref()).collect();

            let mut stmt = conn.prepare(&sql)?;
            let sessions = stmt
                .query_map(params_refs.as_slice(), |row| {
                    Ok(LogSession {
                        session_id: row.get(0)?,
                        conversation_id: row.get(1)?,
                        agent_id: row.get(2)?,
                        agent_name: row.get::<_, String>(2)?, // Will be enriched later
                        title: row.get::<_, Option<String>>(10).ok().flatten(),
                        started_at: row.get(3)?,
                        ended_at: row.get(4)?,
                        status: match row.get::<_, Option<String>>(11).ok().flatten().as_deref() {
                            Some("running") => SessionStatus::Running,
                            Some("error") | Some("crashed") => SessionStatus::Error,
                            Some("stopped") => SessionStatus::Stopped,
                            _ => SessionStatus::Completed,
                        },
                        token_count: row.get(6)?,
                        tool_call_count: row.get(7)?,
                        error_count: row.get(8)?,
                        duration_ms: None, // Computed from started_at/ended_at
                        parent_session_id: row.get(9)?,
                        child_session_ids: Vec::new(), // Fetched separately if needed
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;

            Ok(sessions)
        })
    }

    /// Get a single session by ID.
    pub fn get_session(&self, session_id: &str) -> Result<Option<LogSession>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT
                    session_id,
                    conversation_id,
                    agent_id,
                    MIN(timestamp) as started_at,
                    MAX(timestamp) as ended_at,
                    COUNT(*) as log_count,
                    SUM(CASE WHEN category = 'token' THEN 1 ELSE 0 END) as token_count,
                    SUM(CASE WHEN category = 'tool_call' THEN 1 ELSE 0 END) as tool_call_count,
                    SUM(CASE WHEN level = 'error' THEN 1 ELSE 0 END) as error_count,
                    MAX(parent_session_id) as parent_session_id
                FROM execution_logs
                WHERE session_id = ?
                GROUP BY session_id",
            )?;

            let session = stmt
                .query_row(params![session_id], |row| {
                    Ok(LogSession {
                        session_id: row.get(0)?,
                        conversation_id: row.get(1)?,
                        agent_id: row.get(2)?,
                        agent_name: row.get::<_, String>(2)?,
                        title: None, // Enriched by service layer
                        started_at: row.get(3)?,
                        ended_at: row.get(4)?,
                        status: SessionStatus::Completed,
                        token_count: row.get(6)?,
                        tool_call_count: row.get(7)?,
                        error_count: row.get(8)?,
                        duration_ms: None,
                        parent_session_id: row.get(9)?,
                        child_session_ids: Vec::new(),
                    })
                })
                .optional()?;

            Ok(session)
        })
    }

    /// Get the status from the sessions table (not execution_logs).
    /// Returns the raw status string: "running", "completed", "crashed", "error", etc.
    pub fn get_session_status_from_sessions_table(&self, conversation_id: &str) -> Option<String> {
        self.db
            .with_connection(|conn| {
                let mut stmt = conn.prepare("SELECT status FROM sessions WHERE id = ?1 LIMIT 1")?;
                let status = stmt
                    .query_row([conversation_id], |row| row.get::<_, String>(0))
                    .ok();
                Ok(status)
            })
            .ok()
            .flatten()
    }

    /// Get all logs for a session.
    pub fn get_session_logs(&self, session_id: &str) -> Result<Vec<ExecutionLog>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT
                    id, session_id, conversation_id, agent_id, parent_session_id,
                    timestamp, level, category, message, metadata, duration_ms
                FROM execution_logs
                WHERE session_id = ?
                ORDER BY timestamp ASC",
            )?;

            let logs = stmt
                .query_map(params![session_id], |row| {
                    let level_str: String = row.get(6)?;
                    let category_str: String = row.get(7)?;
                    let metadata_str: Option<String> = row.get(9)?;

                    Ok(ExecutionLog {
                        id: row.get(0)?,
                        session_id: row.get(1)?,
                        conversation_id: row.get(2)?,
                        agent_id: row.get(3)?,
                        parent_session_id: row.get(4)?,
                        timestamp: row.get(5)?,
                        level: level_str.parse().unwrap_or(LogLevel::Info),
                        category: category_str.parse().unwrap_or(LogCategory::System),
                        message: row.get(8)?,
                        metadata: metadata_str.and_then(|s| serde_json::from_str(&s).ok()),
                        duration_ms: row.get(10)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;

            Ok(logs)
        })
    }

    /// Check whether a session has at least one log with the given category.
    pub fn has_category_log(&self, session_id: &str, category: &str) -> Result<bool, String> {
        self.db.with_connection(|conn| {
            let exists: bool = conn
                .prepare(
                    "SELECT 1 FROM execution_logs WHERE session_id = ? AND category = ? LIMIT 1",
                )?
                .exists(params![session_id, category])?;
            Ok(exists)
        })
    }

    /// Get child sessions for a parent session.
    pub fn get_child_sessions(&self, parent_session_id: &str) -> Result<Vec<String>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT DISTINCT session_id
                FROM execution_logs
                WHERE parent_session_id = ?",
            )?;

            let children = stmt
                .query_map(params![parent_session_id], |row| row.get(0))?
                .collect::<Result<Vec<String>, _>>()?;

            Ok(children)
        })
    }

    // =========================================================================
    // TITLE ENRICHMENT
    // =========================================================================

    /// Fetch first user message (truncated to 80 chars) for each session ID.
    ///
    /// Queries the `messages` table (shared database) to derive a human-readable
    /// title from the first user message in each session.
    pub fn get_session_titles(
        &self,
        session_ids: &[String],
    ) -> Result<std::collections::HashMap<String, String>, String> {
        if session_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }

        self.db.with_connection(|conn| {
            let mut titles = std::collections::HashMap::new();

            // Query one-by-one to avoid building large IN clauses.
            // Session lists are typically <=100 so this is fine.
            let mut stmt = conn.prepare(
                "SELECT SUBSTR(content, 1, 80)
                 FROM messages
                 WHERE session_id = ? AND role = 'user'
                 ORDER BY created_at ASC
                 LIMIT 1",
            )?;

            for sid in session_ids {
                if let Ok(title) = stmt.query_row(params![sid], |row| row.get::<_, String>(0)) {
                    if !title.is_empty() {
                        titles.insert(sid.clone(), title);
                    }
                }
            }

            Ok(titles)
        })
    }

    // =========================================================================
    // DELETE OPERATIONS
    // =========================================================================

    /// Delete all logs for a session.
    pub fn delete_session(&self, session_id: &str) -> Result<u64, String> {
        self.db.with_connection(|conn| {
            let count = conn.execute(
                "DELETE FROM execution_logs WHERE session_id = ?",
                params![session_id],
            )?;
            Ok(count as u64)
        })
    }

    /// Delete logs older than the specified timestamp.
    pub fn delete_old_logs(&self, older_than: &str) -> Result<u64, String> {
        self.db.with_connection(|conn| {
            let count = conn.execute(
                "DELETE FROM execution_logs WHERE timestamp < ?",
                params![older_than],
            )?;
            Ok(count as u64)
        })
    }
}

// ============================================================================
// UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;
    use std::sync::Mutex;

    /// In-memory test database provider.
    struct TestDbProvider {
        conn: Mutex<Connection>,
    }

    impl TestDbProvider {
        fn new() -> Self {
            let conn = Connection::open_in_memory().expect("Failed to create in-memory database");

            conn.execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS execution_logs (
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
                );

                CREATE TABLE IF NOT EXISTS sessions (
                    id TEXT PRIMARY KEY,
                    status TEXT NOT NULL DEFAULT 'completed',
                    root_agent_id TEXT NOT NULL,
                    title TEXT,
                    created_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS agent_executions (
                    id TEXT PRIMARY KEY,
                    session_id TEXT NOT NULL,
                    parent_execution_id TEXT
                );

                CREATE TABLE IF NOT EXISTS messages (
                    id TEXT PRIMARY KEY,
                    session_id TEXT,
                    role TEXT NOT NULL,
                    content TEXT NOT NULL,
                    created_at TEXT NOT NULL
                );
                "#,
            )
            .expect("Failed to create tables");

            Self {
                conn: Mutex::new(conn),
            }
        }
    }

    impl DbProvider for TestDbProvider {
        fn with_connection<F, R>(&self, f: F) -> Result<R, String>
        where
            F: FnOnce(&Connection) -> Result<R, rusqlite::Error>,
        {
            let conn = self.conn.lock().map_err(|e| e.to_string())?;
            f(&conn).map_err(|e| e.to_string())
        }
    }

    fn setup_repo() -> LogsRepository<TestDbProvider> {
        let db = Arc::new(TestDbProvider::new());
        LogsRepository::new(db)
    }

    fn make_log(
        session_id: &str,
        conversation_id: &str,
        agent_id: &str,
        level: LogLevel,
        category: LogCategory,
        message: &str,
    ) -> ExecutionLog {
        ExecutionLog::new(session_id, conversation_id, agent_id, level, category, message)
    }

    #[test]
    fn test_log_tool_call_and_result() {
        let repo = setup_repo();

        let tool_call = make_log(
            "sess-1",
            "conv-1",
            "agent-1",
            LogLevel::Info,
            LogCategory::ToolCall,
            "Calling tool: search",
        )
        .with_metadata(serde_json::json!({
            "tool_name": "search",
            "tool_id": "tc-1",
            "args": {"query": "hello"}
        }));

        let tool_result = make_log(
            "sess-1",
            "conv-1",
            "agent-1",
            LogLevel::Info,
            LogCategory::ToolResult,
            "Tool search completed",
        )
        .with_metadata(serde_json::json!({
            "tool_name": "search",
            "tool_id": "tc-1",
            "result": "found 3 items"
        }))
        .with_duration(150);

        repo.insert_log(&tool_call).unwrap();
        repo.insert_log(&tool_result).unwrap();

        let logs = repo.get_session_logs("sess-1").unwrap();
        assert_eq!(logs.len(), 2);

        // Verify tool_call log
        let tc = logs.iter().find(|l| l.category == LogCategory::ToolCall).unwrap();
        assert_eq!(tc.session_id, "sess-1");
        assert!(tc.metadata.is_some());
        let meta = tc.metadata.as_ref().unwrap();
        assert_eq!(meta["tool_name"], "search");

        // Verify tool_result log
        let tr = logs.iter().find(|l| l.category == LogCategory::ToolResult).unwrap();
        assert_eq!(tr.duration_ms, Some(150));
        assert!(tr.metadata.as_ref().unwrap()["result"].as_str().unwrap().contains("found"));
    }

    #[test]
    fn test_list_sessions_with_filters() {
        let repo = setup_repo();

        // Insert logs for two different agents
        let log_a1 = make_log("sess-a", "conv-a", "agent-alpha", LogLevel::Info, LogCategory::Session, "start");
        let log_a2 = make_log("sess-a", "conv-a", "agent-alpha", LogLevel::Error, LogCategory::Error, "oops");
        let log_b1 = make_log("sess-b", "conv-b", "agent-beta", LogLevel::Info, LogCategory::Session, "start");

        repo.insert_log(&log_a1).unwrap();
        repo.insert_log(&log_a2).unwrap();
        repo.insert_log(&log_b1).unwrap();

        // Filter by agent_id
        let filter = LogFilter {
            agent_id: Some("agent-alpha".to_string()),
            ..Default::default()
        };
        let sessions = repo.list_sessions(&filter).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].agent_id, "agent-alpha");
        assert_eq!(sessions[0].error_count, 1);

        // No filter — should return both sessions
        let filter_all = LogFilter::default();
        let sessions = repo.list_sessions(&filter_all).unwrap();
        assert_eq!(sessions.len(), 2);
    }

    #[test]
    fn test_get_session_detail_with_children() {
        let repo = setup_repo();

        // Parent session logs
        let parent_log = make_log("sess-parent", "conv-1", "root-agent", LogLevel::Info, LogCategory::Session, "parent start");
        repo.insert_log(&parent_log).unwrap();

        // Child session logs with parent_session_id set
        let child_log = make_log("sess-child", "conv-1", "child-agent", LogLevel::Info, LogCategory::Session, "child start")
            .with_parent("sess-parent");
        repo.insert_log(&child_log).unwrap();

        // Verify parent session exists
        let parent = repo.get_session("sess-parent").unwrap();
        assert!(parent.is_some());
        let parent = parent.unwrap();
        assert_eq!(parent.session_id, "sess-parent");

        // Verify child sessions can be retrieved
        let children = repo.get_child_sessions("sess-parent").unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0], "sess-child");

        // Verify child session logs
        let child_logs = repo.get_session_logs("sess-child").unwrap();
        assert_eq!(child_logs.len(), 1);
        assert_eq!(child_logs[0].parent_session_id.as_deref(), Some("sess-parent"));
    }

    #[test]
    fn test_delete_session_cascades() {
        let repo = setup_repo();

        // Insert multiple logs for a session
        for i in 0..5 {
            let log = make_log(
                "sess-del",
                "conv-del",
                "agent-del",
                LogLevel::Info,
                LogCategory::Session,
                &format!("message {}", i),
            );
            repo.insert_log(&log).unwrap();
        }

        // Verify logs exist
        let logs = repo.get_session_logs("sess-del").unwrap();
        assert_eq!(logs.len(), 5);

        // Delete session
        let deleted = repo.delete_session("sess-del").unwrap();
        assert_eq!(deleted, 5);

        // Verify logs are gone
        let logs = repo.get_session_logs("sess-del").unwrap();
        assert!(logs.is_empty());

        // Verify session query returns None
        let session = repo.get_session("sess-del").unwrap();
        assert!(session.is_none());
    }
}
