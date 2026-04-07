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
                    s.status as session_status
                FROM execution_logs e
                LEFT JOIN sessions s ON s.id = e.conversation_id
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
                .prepare("SELECT 1 FROM execution_logs WHERE session_id = ? AND category = ? LIMIT 1")?
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
