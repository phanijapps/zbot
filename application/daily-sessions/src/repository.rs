// ============================================================================
// DAILY SESSION REPOSITORY
// Database operations for daily sessions
// ============================================================================

use async_trait::async_trait;
use rusqlite::{Connection, ToSql};
use std::sync::Arc;
use crate::types::{DailySession, SessionMessage, DaySummary, Agent, SystemPromptCheck, DailySessionError};
use chrono::{DateTime, Utc};

/// Async trait for daily session repository operations
#[async_trait]
pub trait DailySessionRepository: Send + Sync {
    async fn get_or_create_today_session(&self, agent_id: &str) -> std::result::Result<DailySession, DailySessionError>;
    async fn get_or_create_today_session_with_prompt(&self, agent_id: &str, system_prompt: &str) -> std::result::Result<(DailySession, SystemPromptCheck), DailySessionError>;
    async fn get_session(&self, session_id: &str) -> std::result::Result<Option<DailySession>, DailySessionError>;
    async fn list_previous_days(&self, agent_id: &str, limit: usize) -> std::result::Result<Vec<DaySummary>, DailySessionError>;
    async fn update_session_summary(&self, session_id: &str, summary: String) -> std::result::Result<(), DailySessionError>;
    async fn increment_message_count(&self, session_id: &str) -> std::result::Result<(), DailySessionError>;
    async fn add_token_count(&self, session_id: &str, tokens: i64) -> std::result::Result<(), DailySessionError>;
    async fn delete_sessions_before(&self, agent_id: &str, before_date: &str) -> std::result::Result<usize, DailySessionError>;
    async fn get_session_messages(&self, session_id: &str) -> std::result::Result<Vec<SessionMessage>, DailySessionError>;
    async fn create_message(&self, message: SessionMessage) -> std::result::Result<(), DailySessionError>;

    // Agent and system prompt methods
    async fn upsert_agent(&self, agent: Agent) -> std::result::Result<(), DailySessionError>;
    async fn get_agent(&self, agent_id: &str) -> std::result::Result<Option<Agent>, DailySessionError>;
    async fn check_system_prompt(&self, agent_id: &str, system_prompt: &str) -> std::result::Result<SystemPromptCheck, DailySessionError>;
}

pub struct SqlDailySessionRepository {
    conn: Arc<tokio::sync::Mutex<Connection>>,
}

impl SqlDailySessionRepository {
    pub fn new(db_path: &std::path::Path) -> std::result::Result<Self, DailySessionError> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| DailySessionError::NotFound(e.to_string()))?;
        }

        let conn = Connection::open(db_path)
            .map_err(|e| DailySessionError::Database(e))?;

        // Enable foreign key constraints
        conn.execute("PRAGMA foreign_keys = ON", [])
            .map_err(|e| DailySessionError::Database(e))?;

        let repo = Self {
            conn: Arc::new(tokio::sync::Mutex::new(conn))
        };

        repo.init_schema()?;
        Ok(repo)
    }

    fn init_schema(&self) -> std::result::Result<(), DailySessionError> {
        let conn = self.conn.blocking_lock();

        // Create agents table for system prompt version tracking
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

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_agents_name
             ON agents(name)",
            [],
        )?;

        // Create daily_sessions table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS daily_sessions (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                session_date TEXT NOT NULL,
                summary TEXT,
                previous_session_ids TEXT,
                message_count INTEGER DEFAULT 0,
                token_count INTEGER DEFAULT 0,
                system_prompt_version INTEGER DEFAULT 1,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (agent_id) REFERENCES agents(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // Migration: Add system_prompt_version column if it doesn't exist (for existing databases)
        if conn.prepare("SELECT system_prompt_version FROM daily_sessions LIMIT 1").is_err() {
            conn.execute(
                "ALTER TABLE daily_sessions ADD COLUMN system_prompt_version INTEGER DEFAULT 1",
                [],
            )?;
        }

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_daily_sessions_agent_date
             ON daily_sessions(agent_id, session_date DESC)",
            [],
        )?;

        // Create messages table
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

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_messages_session_created
             ON messages(session_id, created_at)",
            [],
        )?;

        Ok(())
    }
}

#[async_trait]
impl DailySessionRepository for SqlDailySessionRepository {
    async fn get_or_create_today_session(&self, agent_id: &str) -> std::result::Result<DailySession, DailySessionError> {
        let today = Utc::now().format("%Y-%m-%d").to_string();
        let session_id = DailySession::generate_id(agent_id, &today);

        let conn = self.conn.lock().await;

        // Try to get existing session
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, session_date, summary, previous_session_ids,
                    message_count, token_count, system_prompt_version, created_at, updated_at
             FROM daily_sessions WHERE id = ?1"
        )?;

        let result = stmt.query_row([&*session_id], |row| {
            let created_at_str: String = row.get(8)?;
            let updated_at_str: String = row.get(9)?;
            Ok(DailySession {
                id: row.get(0)?,
                agent_id: row.get(1)?,
                session_date: row.get(2)?,
                summary: row.get(3)?,
                previous_session_ids: {
                    let json_str: Option<String> = row.get(4)?;
                    json_str.and_then(|s| serde_json::from_str(&s).ok())
                },
                message_count: row.get(5)?,
                token_count: row.get(6)?,
                system_prompt_version: row.get(7)?,
                created_at: parse_datetime(&created_at_str)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
                updated_at: parse_datetime(&updated_at_str)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
            })
        });

        match result {
            Ok(session) => Ok(session),
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                // Create new session with default version 1
                let new_session = DailySession::new(agent_id.to_string(), today.clone());
                let now = Utc::now().to_rfc3339();

                conn.execute(
                    "INSERT INTO daily_sessions
                     (id, agent_id, session_date, message_count, token_count, system_prompt_version, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    &[
                        &new_session.id as &dyn ToSql,
                        &new_session.agent_id as &dyn ToSql,
                        &new_session.session_date as &dyn ToSql,
                        &0i64 as &dyn ToSql,
                        &0i64 as &dyn ToSql,
                        &1i64 as &dyn ToSql,
                        &now as &dyn ToSql,
                        &now as &dyn ToSql,
                    ] as &[&dyn ToSql]
                )?;

                Ok(new_session)
            }
            Err(e) => Err(DailySessionError::Database(e))
        }
    }

    async fn get_or_create_today_session_with_prompt(&self, agent_id: &str, system_prompt: &str) -> std::result::Result<(DailySession, SystemPromptCheck), DailySessionError> {
        let today = Utc::now().format("%Y-%m-%d").to_string();
        let session_id = DailySession::generate_id(agent_id, &today);

        // Check if system prompt has changed
        let prompt_check = self.check_system_prompt(agent_id, system_prompt).await?;

        let conn = self.conn.lock().await;

        // Try to get existing session
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, session_date, summary, previous_session_ids,
                    message_count, token_count, system_prompt_version, created_at, updated_at
             FROM daily_sessions WHERE id = ?1"
        )?;

        let result = stmt.query_row([&*session_id], |row| {
            let created_at_str: String = row.get(8)?;
            let updated_at_str: String = row.get(9)?;
            Ok(DailySession {
                id: row.get(0)?,
                agent_id: row.get(1)?,
                session_date: row.get(2)?,
                summary: row.get(3)?,
                previous_session_ids: {
                    let json_str: Option<String> = row.get(4)?;
                    json_str.and_then(|s| serde_json::from_str(&s).ok())
                },
                message_count: row.get(5)?,
                token_count: row.get(6)?,
                system_prompt_version: row.get(7)?,
                created_at: parse_datetime(&created_at_str)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
                updated_at: parse_datetime(&updated_at_str)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
            })
        });

        let session = match result {
            Ok(s) => s,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                // Create new session with current prompt version
                let new_session = DailySession::with_version(
                    agent_id.to_string(),
                    today.clone(),
                    prompt_check.new_version,
                );
                let now = Utc::now().to_rfc3339();

                conn.execute(
                    "INSERT INTO daily_sessions
                     (id, agent_id, session_date, message_count, token_count, system_prompt_version, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    &[
                        &new_session.id as &dyn ToSql,
                        &new_session.agent_id as &dyn ToSql,
                        &new_session.session_date as &dyn ToSql,
                        &0i64 as &dyn ToSql,
                        &0i64 as &dyn ToSql,
                        &prompt_check.new_version as &dyn ToSql,
                        &now as &dyn ToSql,
                        &now as &dyn ToSql,
                    ] as &[&dyn ToSql]
                )?;

                new_session
            }
            Err(e) => return Err(DailySessionError::Database(e)),
        };

        Ok((session, prompt_check))
    }

    async fn get_session(&self, session_id: &str) -> std::result::Result<Option<DailySession>, DailySessionError> {
        let conn = self.conn.lock().await;

        let mut stmt = conn.prepare(
            "SELECT id, agent_id, session_date, summary, previous_session_ids,
                    message_count, token_count, system_prompt_version, created_at, updated_at
             FROM daily_sessions WHERE id = ?1"
        )?;

        let result = stmt.query_row([&*session_id], |row| {
            let created_at_str: String = row.get(8)?;
            let updated_at_str: String = row.get(9)?;
            Ok(DailySession {
                id: row.get(0)?,
                agent_id: row.get(1)?,
                session_date: row.get(2)?,
                summary: row.get(3)?,
                previous_session_ids: {
                    let json_str: Option<String> = row.get(4)?;
                    json_str.and_then(|s| serde_json::from_str(&s).ok())
                },
                message_count: row.get(5)?,
                token_count: row.get(6)?,
                system_prompt_version: row.get(7)?,
                created_at: parse_datetime(&created_at_str)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
                updated_at: parse_datetime(&updated_at_str)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
            })
        });

        match result {
            Ok(session) => Ok(Some(session)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DailySessionError::Database(e))
        }
    }

    async fn list_previous_days(&self, agent_id: &str, limit: usize) -> std::result::Result<Vec<DaySummary>, DailySessionError> {
        let conn = self.conn.lock().await;

        let mut stmt = conn.prepare(
            "SELECT id, session_date, summary, message_count
             FROM daily_sessions
             WHERE agent_id = ?1 AND session_date < date('now')
             ORDER BY session_date DESC
             LIMIT ?2"
        )?;

        let rows = stmt.query_map([agent_id, &(limit as i64).to_string()], |row| {
            Ok(DaySummary {
                session_id: row.get(0)?,
                session_date: row.get(1)?,
                summary: row.get(2)?,
                message_count: row.get(3)?,
                is_archived: false,
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }

        Ok(results)
    }

    async fn update_session_summary(&self, session_id: &str, summary: String) -> std::result::Result<(), DailySessionError> {
        let conn = self.conn.lock().await;
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE daily_sessions SET summary = ?1, updated_at = ?2 WHERE id = ?3",
            [&*summary, &*now, &*session_id]
        )?;

        Ok(())
    }

    async fn increment_message_count(&self, session_id: &str) -> std::result::Result<(), DailySessionError> {
        let conn = self.conn.lock().await;
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE daily_sessions SET message_count = message_count + 1, updated_at = ?1 WHERE id = ?2",
            [&*now, &*session_id]
        )?;

        Ok(())
    }

    async fn add_token_count(&self, session_id: &str, tokens: i64) -> std::result::Result<(), DailySessionError> {
        let conn = self.conn.lock().await;
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE daily_sessions SET token_count = token_count + ?1, updated_at = ?2 WHERE id = ?3",
            &[&tokens as &dyn ToSql, &now as &dyn ToSql, &session_id as &dyn ToSql] as &[&dyn ToSql]
        )?;

        Ok(())
    }

    async fn delete_sessions_before(&self, agent_id: &str, before_date: &str) -> std::result::Result<usize, DailySessionError> {
        let conn = self.conn.lock().await;

        let rows = conn.execute(
            "DELETE FROM daily_sessions WHERE agent_id = ?1 AND session_date < ?2",
            [agent_id, before_date]
        )?;

        Ok(rows as usize)
    }

    async fn get_session_messages(&self, session_id: &str) -> std::result::Result<Vec<SessionMessage>, DailySessionError> {
        let conn = self.conn.lock().await;

        let mut stmt = conn.prepare(
            "SELECT id, session_id, role, content, created_at, token_count, tool_calls, tool_results
             FROM messages WHERE session_id = ?1 ORDER BY created_at"
        )?;

        let rows = stmt.query_map([session_id], |row| {
            let created_at_str: String = row.get(4)?;
            let tool_calls_str: Option<String> = row.get(6)?;
            let tool_results_str: Option<String> = row.get(7)?;
            Ok(SessionMessage {
                id: row.get(0)?,
                session_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                created_at: parse_datetime(&created_at_str)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
                token_count: row.get(5)?,
                tool_calls: tool_calls_str.and_then(|s| serde_json::from_str(&s).ok()),
                tool_results: tool_results_str.and_then(|s| serde_json::from_str(&s).ok()),
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }

        Ok(results)
    }

    async fn create_message(&self, message: SessionMessage) -> std::result::Result<(), DailySessionError> {
        let conn = self.conn.lock().await;

        let tool_calls_str = message.tool_calls
            .as_ref()
            .map(|v| serde_json::to_string(v).unwrap_or_default());

        let tool_results_str = message.tool_results
            .as_ref()
            .map(|v| serde_json::to_string(v).unwrap_or_default());

        let created_at_str = message.created_at.to_rfc3339();

        // Build params as Vec<&dyn ToSql> to handle mixed types and Options
        let tool_calls_param: &dyn ToSql = match &tool_calls_str {
            Some(s) => &s.as_str() as &dyn ToSql,
            None => &None::<String> as &dyn ToSql,
        };
        let tool_results_param: &dyn ToSql = match &tool_results_str {
            Some(s) => &s.as_str() as &dyn ToSql,
            None => &None::<String> as &dyn ToSql,
        };

        let params: &[&dyn ToSql] = &[
            &message.id,
            &message.session_id,
            &message.role,
            &message.content,
            &created_at_str,
            &message.token_count,
            tool_calls_param,
            tool_results_param,
        ];

        conn.execute(
            "INSERT INTO messages (id, session_id, role, content, created_at, token_count, tool_calls, tool_results)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params
        )?;

        // Update session counts
        let update_params: &[&dyn ToSql] = &[
            &message.token_count,
            &created_at_str,
            &message.session_id,
        ];
        conn.execute(
            "UPDATE daily_sessions
             SET message_count = message_count + 1,
                 token_count = token_count + ?1,
                 updated_at = ?2
             WHERE id = ?3",
            update_params
        )?;

        Ok(())
    }

    async fn upsert_agent(&self, agent: Agent) -> std::result::Result<(), DailySessionError> {
        let conn = self.conn.lock().await;
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO agents (id, name, display_name, description, config_path, system_prompt_version, current_system_prompt, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                display_name = excluded.display_name,
                description = excluded.description,
                config_path = excluded.config_path,
                system_prompt_version = excluded.system_prompt_version,
                current_system_prompt = excluded.current_system_prompt,
                updated_at = excluded.updated_at",
            &[
                &agent.id as &dyn ToSql,
                &agent.name as &dyn ToSql,
                &agent.display_name as &dyn ToSql,
                &agent.description as &dyn ToSql,
                &agent.config_path as &dyn ToSql,
                &agent.system_prompt_version as &dyn ToSql,
                &agent.current_system_prompt as &dyn ToSql,
                &now as &dyn ToSql,
                &now as &dyn ToSql,
            ] as &[&dyn ToSql]
        )?;

        Ok(())
    }

    async fn get_agent(&self, agent_id: &str) -> std::result::Result<Option<Agent>, DailySessionError> {
        let conn = self.conn.lock().await;

        let mut stmt = conn.prepare(
            "SELECT id, name, display_name, description, config_path, system_prompt_version, current_system_prompt, created_at, updated_at
             FROM agents WHERE id = ?1"
        )?;

        let result = stmt.query_row([&*agent_id], |row| {
            let created_at_str: String = row.get(7)?;
            let updated_at_str: String = row.get(8)?;
            Ok(Agent {
                id: row.get(0)?,
                name: row.get(1)?,
                display_name: row.get(2)?,
                description: row.get(3)?,
                config_path: row.get(4)?,
                system_prompt_version: row.get(5)?,
                current_system_prompt: row.get(6)?,
                created_at: parse_datetime(&created_at_str)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
                updated_at: parse_datetime(&updated_at_str)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
            })
        });

        match result {
            Ok(agent) => Ok(Some(agent)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DailySessionError::Database(e))
        }
    }

    async fn check_system_prompt(&self, agent_id: &str, system_prompt: &str) -> std::result::Result<SystemPromptCheck, DailySessionError> {
        let conn = self.conn.lock().await;

        // Get current agent record
        let current_agent = self.get_agent(agent_id).await?;

        match current_agent {
            Some(agent) => {
                // Check if prompt has changed
                let has_changed = match &agent.current_system_prompt {
                    Some(current_prompt) => current_prompt != system_prompt,
                    None => true, // No previous prompt, so treat as new
                };

                if has_changed {
                    // Increment version and update
                    let new_version = agent.system_prompt_version + 1;

                    conn.execute(
                        "UPDATE agents
                         SET system_prompt_version = ?1,
                             current_system_prompt = ?2,
                             updated_at = datetime('now')
                         WHERE id = ?3",
                        &[&new_version as &dyn ToSql, &system_prompt as &dyn ToSql, &agent_id as &dyn ToSql] as &[&dyn ToSql]
                    )?;

                    Ok(SystemPromptCheck {
                        has_changed: true,
                        previous_version: Some(agent.system_prompt_version),
                        new_version,
                        previous_prompt: agent.current_system_prompt,
                    })
                } else {
                    Ok(SystemPromptCheck {
                        has_changed: false,
                        previous_version: Some(agent.system_prompt_version),
                        new_version: agent.system_prompt_version,
                        previous_prompt: agent.current_system_prompt,
                    })
                }
            }
            None => {
                // Agent doesn't exist yet, create with version 1
                let now = Utc::now();

                conn.execute(
                    "INSERT INTO agents (id, name, display_name, config_path, system_prompt_version, current_system_prompt, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    &[
                        &agent_id as &dyn ToSql,
                        &agent_id as &dyn ToSql,  // Use agent_id as name for now
                        &agent_id as &dyn ToSql,  // Use agent_id as display_name for now
                        &"" as &dyn ToSql,        // Empty config_path for now
                        &1i64 as &dyn ToSql,
                        &system_prompt as &dyn ToSql,
                        &now.to_rfc3339() as &dyn ToSql,
                        &now.to_rfc3339() as &dyn ToSql,
                    ] as &[&dyn ToSql]
                )?;

                Ok(SystemPromptCheck {
                    has_changed: false,
                    previous_version: None,
                    new_version: 1,
                    previous_prompt: None,
                })
            }
        }
    }
}

/// Helper function to parse datetime string
fn parse_datetime(s: &str) -> std::result::Result<DateTime<Utc>, DailySessionError> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| DailySessionError::InvalidDateFormat(e.to_string()))
}
