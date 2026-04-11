// ============================================================================
// DAILY SESSION REPOSITORY
// Database operations for daily sessions
// ============================================================================

use crate::types::{
    Agent, DailySession, DailySessionError, DaySummary, SessionMessage, SystemPromptCheck,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rusqlite::{Connection, ToSql};
use std::sync::Arc;

/// Async trait for daily session repository operations
#[async_trait]
pub trait DailySessionRepository: Send + Sync {
    async fn get_or_create_today_session(
        &self,
        agent_id: &str,
    ) -> std::result::Result<DailySession, DailySessionError>;
    async fn get_or_create_today_session_with_prompt(
        &self,
        agent_id: &str,
        system_prompt: &str,
    ) -> std::result::Result<(DailySession, SystemPromptCheck), DailySessionError>;
    async fn get_session(
        &self,
        session_id: &str,
    ) -> std::result::Result<Option<DailySession>, DailySessionError>;
    async fn list_previous_days(
        &self,
        agent_id: &str,
        limit: usize,
    ) -> std::result::Result<Vec<DaySummary>, DailySessionError>;
    async fn update_session_summary(
        &self,
        session_id: &str,
        summary: String,
    ) -> std::result::Result<(), DailySessionError>;
    async fn increment_message_count(
        &self,
        session_id: &str,
    ) -> std::result::Result<(), DailySessionError>;
    async fn add_token_count(
        &self,
        session_id: &str,
        tokens: i64,
    ) -> std::result::Result<(), DailySessionError>;
    async fn delete_sessions_before(
        &self,
        agent_id: &str,
        before_date: &str,
    ) -> std::result::Result<usize, DailySessionError>;
    async fn get_session_messages(
        &self,
        session_id: &str,
    ) -> std::result::Result<Vec<SessionMessage>, DailySessionError>;
    async fn create_message(
        &self,
        message: SessionMessage,
    ) -> std::result::Result<(), DailySessionError>;

    // Agent and system prompt methods
    async fn upsert_agent(&self, agent: Agent) -> std::result::Result<(), DailySessionError>;
    async fn get_agent(
        &self,
        agent_id: &str,
    ) -> std::result::Result<Option<Agent>, DailySessionError>;
    async fn check_system_prompt(
        &self,
        agent_id: &str,
        system_prompt: &str,
    ) -> std::result::Result<SystemPromptCheck, DailySessionError>;
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

        let conn = Connection::open(db_path).map_err(DailySessionError::Database)?;

        // Enable foreign key constraints
        conn.execute("PRAGMA foreign_keys = ON", [])
            .map_err(DailySessionError::Database)?;

        let repo = Self {
            conn: Arc::new(tokio::sync::Mutex::new(conn)),
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
        if conn
            .prepare("SELECT system_prompt_version FROM daily_sessions LIMIT 1")
            .is_err()
        {
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
    async fn get_or_create_today_session(
        &self,
        agent_id: &str,
    ) -> std::result::Result<DailySession, DailySessionError> {
        let today = Utc::now().format("%Y-%m-%d").to_string();
        let session_id = DailySession::generate_id(agent_id, &today);

        let conn = self.conn.lock().await;

        // Try to get existing session
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, session_date, summary, previous_session_ids,
                    message_count, token_count, system_prompt_version, created_at, updated_at
             FROM daily_sessions WHERE id = ?1",
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
            Err(e) => Err(DailySessionError::Database(e)),
        }
    }

    async fn get_or_create_today_session_with_prompt(
        &self,
        agent_id: &str,
        system_prompt: &str,
    ) -> std::result::Result<(DailySession, SystemPromptCheck), DailySessionError> {
        let today = Utc::now().format("%Y-%m-%d").to_string();
        let session_id = DailySession::generate_id(agent_id, &today);

        // Check if system prompt has changed
        let prompt_check = self.check_system_prompt(agent_id, system_prompt).await?;

        let conn = self.conn.lock().await;

        // Try to get existing session
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, session_date, summary, previous_session_ids,
                    message_count, token_count, system_prompt_version, created_at, updated_at
             FROM daily_sessions WHERE id = ?1",
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

    async fn get_session(
        &self,
        session_id: &str,
    ) -> std::result::Result<Option<DailySession>, DailySessionError> {
        let conn = self.conn.lock().await;

        let mut stmt = conn.prepare(
            "SELECT id, agent_id, session_date, summary, previous_session_ids,
                    message_count, token_count, system_prompt_version, created_at, updated_at
             FROM daily_sessions WHERE id = ?1",
        )?;

        let result = stmt.query_row([session_id], |row| {
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
            Err(e) => Err(DailySessionError::Database(e)),
        }
    }

    async fn list_previous_days(
        &self,
        agent_id: &str,
        limit: usize,
    ) -> std::result::Result<Vec<DaySummary>, DailySessionError> {
        let conn = self.conn.lock().await;

        let mut stmt = conn.prepare(
            "SELECT id, session_date, summary, message_count
             FROM daily_sessions
             WHERE agent_id = ?1 AND session_date < date('now')
             ORDER BY session_date DESC
             LIMIT ?2",
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

    async fn update_session_summary(
        &self,
        session_id: &str,
        summary: String,
    ) -> std::result::Result<(), DailySessionError> {
        let conn = self.conn.lock().await;
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE daily_sessions SET summary = ?1, updated_at = ?2 WHERE id = ?3",
            [&*summary, &*now, session_id],
        )?;

        Ok(())
    }

    async fn increment_message_count(
        &self,
        session_id: &str,
    ) -> std::result::Result<(), DailySessionError> {
        let conn = self.conn.lock().await;
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE daily_sessions SET message_count = message_count + 1, updated_at = ?1 WHERE id = ?2",
            [&*now, session_id]
        )?;

        Ok(())
    }

    async fn add_token_count(
        &self,
        session_id: &str,
        tokens: i64,
    ) -> std::result::Result<(), DailySessionError> {
        let conn = self.conn.lock().await;
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE daily_sessions SET token_count = token_count + ?1, updated_at = ?2 WHERE id = ?3",
            &[&tokens as &dyn ToSql, &now as &dyn ToSql, &session_id as &dyn ToSql] as &[&dyn ToSql]
        )?;

        Ok(())
    }

    async fn delete_sessions_before(
        &self,
        agent_id: &str,
        before_date: &str,
    ) -> std::result::Result<usize, DailySessionError> {
        let conn = self.conn.lock().await;

        let rows = conn.execute(
            "DELETE FROM daily_sessions WHERE agent_id = ?1 AND session_date < ?2",
            [agent_id, before_date],
        )?;

        Ok(rows)
    }

    async fn get_session_messages(
        &self,
        session_id: &str,
    ) -> std::result::Result<Vec<SessionMessage>, DailySessionError> {
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

    async fn create_message(
        &self,
        message: SessionMessage,
    ) -> std::result::Result<(), DailySessionError> {
        let conn = self.conn.lock().await;

        let tool_calls_str = message
            .tool_calls
            .as_ref()
            .map(|v| serde_json::to_string(v).unwrap_or_default());

        let tool_results_str = message
            .tool_results
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
        let update_params: &[&dyn ToSql] =
            &[&message.token_count, &created_at_str, &message.session_id];
        conn.execute(
            "UPDATE daily_sessions
             SET message_count = message_count + 1,
                 token_count = token_count + ?1,
                 updated_at = ?2
             WHERE id = ?3",
            update_params,
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

    async fn get_agent(
        &self,
        agent_id: &str,
    ) -> std::result::Result<Option<Agent>, DailySessionError> {
        let conn = self.conn.lock().await;

        let mut stmt = conn.prepare(
            "SELECT id, name, display_name, description, config_path, system_prompt_version, current_system_prompt, created_at, updated_at
             FROM agents WHERE id = ?1"
        )?;

        let result = stmt.query_row([agent_id], |row| {
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
            Err(e) => Err(DailySessionError::Database(e)),
        }
    }

    async fn check_system_prompt(
        &self,
        agent_id: &str,
        system_prompt: &str,
    ) -> std::result::Result<SystemPromptCheck, DailySessionError> {
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
                        &[
                            &new_version as &dyn ToSql,
                            &system_prompt as &dyn ToSql,
                            &agent_id as &dyn ToSql,
                        ] as &[&dyn ToSql],
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

// ============================================================================
// UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Agent;
    use tempfile::tempdir;

    /// Create a repository backed by a temporary SQLite database.
    /// Must be called from a blocking context (spawn_blocking or outside runtime).
    async fn setup_repo() -> SqlDailySessionRepository {
        tokio::task::spawn_blocking(|| {
            let dir = tempdir().unwrap();
            let db_path = dir.keep().join("test_daily.db");
            SqlDailySessionRepository::new(&db_path).unwrap()
        })
        .await
        .unwrap()
    }

    /// Helper: insert an agent so foreign key constraints are satisfied.
    async fn insert_agent(repo: &SqlDailySessionRepository, agent_id: &str) {
        let agent = Agent {
            id: agent_id.to_string(),
            name: agent_id.to_string(),
            display_name: agent_id.to_string(),
            description: None,
            config_path: "/tmp/config.yaml".to_string(),
            system_prompt_version: 1,
            current_system_prompt: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        repo.upsert_agent(agent).await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_create_daily_session() {
        let repo = setup_repo().await;
        insert_agent(&repo, "agent-1").await;

        // Create a session for today
        let session = repo.get_or_create_today_session("agent-1").await.unwrap();
        assert_eq!(session.agent_id, "agent-1");
        assert_eq!(session.message_count, 0);
        assert_eq!(session.token_count, 0);

        // Retrieve the same session (should return same one, not duplicate)
        let session2 = repo.get_or_create_today_session("agent-1").await.unwrap();
        assert_eq!(session.id, session2.id);

        // Verify via get_session
        let fetched = repo.get_session(&session.id).await.unwrap();
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.id, session.id);
        assert_eq!(fetched.agent_id, "agent-1");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_list_daily_sessions() {
        let repo = setup_repo().await;
        insert_agent(&repo, "agent-list").await;

        // Insert sessions for different past dates directly via SQL
        let conn = repo.conn.lock().await;
        let now = Utc::now().to_rfc3339();
        for date in &["2025-01-01", "2025-01-02", "2025-01-03"] {
            let id = DailySession::generate_id("agent-list", date);
            conn.execute(
                "INSERT INTO daily_sessions (id, agent_id, session_date, message_count, token_count, system_prompt_version, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 5, 100, 1, ?4, ?5)",
                rusqlite::params![id, "agent-list", date, now, now],
            )
            .unwrap();
        }
        drop(conn);

        // list_previous_days only returns sessions before today — all 3 qualify
        let days = repo.list_previous_days("agent-list", 10).await.unwrap();
        assert_eq!(days.len(), 3);
        // Verify ordering (most recent first)
        assert_eq!(days[0].session_date, "2025-01-03");
        assert_eq!(days[1].session_date, "2025-01-02");

        // Test limit
        let days = repo.list_previous_days("agent-list", 1).await.unwrap();
        assert_eq!(days.len(), 1);

        // Test different agent returns empty
        let days = repo.list_previous_days("other-agent", 10).await.unwrap();
        assert!(days.is_empty());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_update_daily_session() {
        let repo = setup_repo().await;
        insert_agent(&repo, "agent-upd").await;

        let session = repo.get_or_create_today_session("agent-upd").await.unwrap();

        // Update summary
        repo.update_session_summary(&session.id, "Today was productive".to_string())
            .await
            .unwrap();
        let fetched = repo.get_session(&session.id).await.unwrap().unwrap();
        assert_eq!(fetched.summary.as_deref(), Some("Today was productive"));

        // Increment message count
        repo.increment_message_count(&session.id).await.unwrap();
        repo.increment_message_count(&session.id).await.unwrap();
        let fetched = repo.get_session(&session.id).await.unwrap().unwrap();
        assert_eq!(fetched.message_count, 2);

        // Add token count
        repo.add_token_count(&session.id, 50).await.unwrap();
        repo.add_token_count(&session.id, 30).await.unwrap();
        let fetched = repo.get_session(&session.id).await.unwrap().unwrap();
        assert_eq!(fetched.token_count, 80);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_delete_old_sessions() {
        let repo = setup_repo().await;
        insert_agent(&repo, "agent-del").await;

        // Insert old sessions via SQL
        let conn = repo.conn.lock().await;
        let now = Utc::now().to_rfc3339();
        for date in &["2024-06-01", "2024-07-01", "2025-03-01"] {
            let id = DailySession::generate_id("agent-del", date);
            conn.execute(
                "INSERT INTO daily_sessions (id, agent_id, session_date, message_count, token_count, system_prompt_version, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 0, 0, 1, ?4, ?5)",
                rusqlite::params![id, "agent-del", date, now, now],
            )
            .unwrap();
        }
        drop(conn);

        // Delete sessions before 2025-01-01 — should remove 2 of 3
        let deleted = repo
            .delete_sessions_before("agent-del", "2025-01-01")
            .await
            .unwrap();
        assert_eq!(deleted, 2);

        // Verify only the March session remains
        let days = repo.list_previous_days("agent-del", 10).await.unwrap();
        assert_eq!(days.len(), 1);
        assert_eq!(days[0].session_date, "2025-03-01");
    }
}
