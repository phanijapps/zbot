// ============================================================================
// AGENT CHANNELS COMMANDS
// Agent Channel model - daily sessions, history management, and UI commands
// ============================================================================

use crate::settings::AppDirs;
use daily_sessions::{DailySessionManager, DailySessionRepository, DaySummary, DailySession, SessionMessage, DailySessionError};
use daily_sessions::types::{Agent as DailySessionAgent, SystemPromptCheck};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use chrono::{DateTime, Utc};
use std::path::PathBuf;

// Import zero_app types for session loading
use zero_app::{MutexSession, Content};
use zero_core::Part;

// ============================================================================
// TYPES
// ============================================================================

/// Agent information for the UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: String,
    pub name: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    pub description: Option<String>,
    #[serde(rename = "configPath")]
    pub config_path: String,
    #[serde(rename = "systemPromptVersion")]
    pub system_prompt_version: i32,
    #[serde(rename = "currentSystemPrompt")]
    pub current_system_prompt: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[serde(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
}

/// Agent channel info for UI display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentChannel {
    #[serde(rename = "agentId")]
    pub agent_id: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "todayMessageCount")]
    pub today_message_count: i64,
    #[serde(rename = "hasHistory")]
    pub has_history: bool,
    #[serde(rename = "lastActivity")]
    pub last_activity: DateTime<Utc>,
    #[serde(rename = "lastActivityText")]
    pub last_activity_text: String,
}

/// Message in a session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    #[serde(rename = "sessionId")]
    pub session_id: String,
    pub role: String,
    pub content: String,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[serde(rename = "tokenCount")]
    pub token_count: i64,
    #[serde(rename = "toolCalls")]
    pub tool_calls: Option<serde_json::Value>,
    #[serde(rename = "toolResults")]
    pub tool_results: Option<serde_json::Value>,
}

// ============================================================================
// SQLITE REPOSITORY IMPLEMENTATION
// ============================================================================

/// SQLite implementation of DailySessionRepository
pub struct SqliteSessionRepository {
    conn: Arc<tokio::sync::Mutex<Connection>>,
}

impl SqliteSessionRepository {
    pub fn new(db_path: std::path::PathBuf) -> Result<Self, String> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create database directory: {}", e))?;
        }

        let conn = Connection::open(&db_path)
            .map_err(|e| format!("Failed to open database: {}", e))?;

        // Enable foreign key constraints for CASCADE deletes to work
        conn.execute("PRAGMA foreign_keys = ON", [])
            .map_err(|e| format!("Failed to enable foreign keys: {}", e))?;

        // Initialize schema
        initialize_schema(&conn)?;

        Ok(Self {
            conn: Arc::new(tokio::sync::Mutex::new(conn)),
        })
    }
}

/// Initialize the Agent Channel database schema
fn initialize_schema(conn: &Connection) -> Result<(), String> {
    // Create agents table
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
    ).map_err(|e| format!("Failed to create agents table: {}", e))?;

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
    ).map_err(|e| format!("Failed to create daily_sessions table: {}", e))?;

    // Migration: Add system_prompt_version column if it doesn't exist (for existing databases)
    // Check if column exists by trying to select it
    let column_exists = conn.prepare("SELECT system_prompt_version FROM daily_sessions LIMIT 1").is_ok();
    if !column_exists {
        conn.execute(
            "ALTER TABLE daily_sessions ADD COLUMN system_prompt_version INTEGER DEFAULT 1",
            [],
        ).map_err(|e| format!("Failed to add system_prompt_version column: {}", e))?;
    }

    // Create index for daily sessions
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_daily_sessions_agent_date
         ON daily_sessions(agent_id, session_date DESC)",
        [],
    ).map_err(|e| format!("Failed to create index: {}", e))?;

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
    ).map_err(|e| format!("Failed to create messages table: {}", e))?;

    // Create index for messages
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_messages_session_created
         ON messages(session_id, created_at)",
        [],
    ).map_err(|e| format!("Failed to create index: {}", e))?;

    // Create knowledge graph entities table
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
    ).map_err(|e| format!("Failed to create kg_entities table: {}", e))?;

    // Create knowledge graph relationships table
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
    ).map_err(|e| format!("Failed to create kg_relationships table: {}", e))?;

    // Create session_state table for persisting agent session state
    conn.execute(
        "CREATE TABLE IF NOT EXISTS session_state (
            agent_id TEXT PRIMARY KEY,
            state_json TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (agent_id) REFERENCES agents(id) ON DELETE CASCADE
        )",
        [],
    ).map_err(|e| format!("Failed to create session_state table: {}", e))?;

    Ok(())
}

/// Ensure an agent exists in the agents table
/// If not, reads the agent config from disk and registers it
fn ensure_agent_exists(conn: &Connection, agent_id: &str) -> Result<(), String> {
    // Check if agent already exists
    let mut stmt = conn.prepare("SELECT id FROM agents WHERE id = ?1")
        .map_err(|e| format!("Failed to prepare query: {}", e))?;

    let exists = stmt.query_row([agent_id], |_| Ok(())).is_ok();

    if exists {
        return Ok(());
    }

    // Agent doesn't exist, register it from config
    let dirs = crate::settings::AppDirs::get()
        .map_err(|e| format!("Failed to get app dirs: {}", e))?;

    let agent_dir = dirs.config_dir.join("agents").join(agent_id);
    let config_file = agent_dir.join("config.yaml");

    if !config_file.exists() {
        return Err(format!("Agent config not found for: {}", agent_id));
    }

    // Read and parse the agent config
    let config_content = std::fs::read_to_string(&config_file)
        .map_err(|e| format!("Failed to read agent config: {}", e))?;

    let config: serde_yaml::Value = serde_yaml::from_str(&config_content)
        .map_err(|e| format!("Failed to parse agent config: {}", e))?;

    let name = config.get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing 'name' in config".to_string())?;

    let display_name = config.get("displayName")
        .and_then(|v| v.as_str())
        .unwrap_or(name);

    let description = config.get("description")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let system_instruction = config.get("systemInstruction")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let now = Utc::now().to_rfc3339();
    let config_path_str = config_file.to_str()
        .ok_or_else(|| "Invalid config path".to_string())?;

    // Insert the agent into the agents table
    conn.execute(
        "INSERT INTO agents (id, name, display_name, description, config_path, system_prompt_version, current_system_prompt, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            agent_id,
            name,
            display_name,
            description,
            config_path_str,
            1, // initial version
            system_instruction,
            now,
            now,
        ],
    ).map_err(|e| format!("Failed to register agent: {}", e))?;

    Ok(())
}

/// Helper function to parse datetime string
fn parse_datetime(s: &str) -> Result<DateTime<Utc>, String> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| format!("Failed to parse datetime: {}", e))
}

#[async_trait::async_trait]
impl DailySessionRepository for SqliteSessionRepository {
    async fn get_or_create_today_session(&self, agent_id: &str) -> daily_sessions::Result<DailySession> {
        let today = Utc::now().format("%Y-%m-%d").to_string();
        let session_id = DailySession::generate_id(agent_id, &today);

        let conn = self.conn.lock().await;

        // First, ensure the agent exists in the agents table
        ensure_agent_exists(&conn, agent_id)
            .map_err(DailySessionError::NotFound)?;

        // Try to get existing session
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, session_date, summary, previous_session_ids,
                    message_count, token_count, system_prompt_version, created_at, updated_at
             FROM daily_sessions WHERE id = ?1"
        )?;

        let session = stmt.query_row([&session_id], |row| {
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
                created_at: parse_datetime(&created_at_str).unwrap_or_else(|_| Utc::now()),
                updated_at: parse_datetime(&updated_at_str).unwrap_or_else(|_| Utc::now()),
            })
        });

        match session {
            Ok(s) => Ok(s),
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                // Create new session
                let new_session = DailySession::new(agent_id.to_string(), today);
                let now = Utc::now().to_rfc3339();

                conn.execute(
                    "INSERT INTO daily_sessions (id, agent_id, session_date, message_count, token_count, system_prompt_version, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![&new_session.id, &new_session.agent_id, &new_session.session_date,
                            0i64, 0i64, 1i64, &now, &now]
                )?;

                Ok(new_session)
            }
            Err(e) => Err(DailySessionError::Database(e))
        }
    }

    async fn get_session(&self, session_id: &str) -> daily_sessions::Result<Option<DailySession>> {
        let conn = self.conn.lock().await;

        let mut stmt = conn.prepare(
            "SELECT id, agent_id, session_date, summary, previous_session_ids,
                    message_count, token_count, system_prompt_version, created_at, updated_at
             FROM daily_sessions WHERE id = ?1"
        )?;

        let session = stmt.query_row([&session_id], |row| {
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
                created_at: parse_datetime(&created_at_str).unwrap_or_else(|_| Utc::now()),
                updated_at: parse_datetime(&updated_at_str).unwrap_or_else(|_| Utc::now()),
            })
        });

        match session {
            Ok(s) => Ok(Some(s)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DailySessionError::Database(e))
        }
    }

    async fn list_previous_days(&self, agent_id: &str, limit: usize) -> daily_sessions::Result<Vec<DaySummary>> {
        let conn = self.conn.lock().await;

        let mut stmt = conn.prepare(
            "SELECT id, session_date, summary, message_count
             FROM daily_sessions
             WHERE agent_id = ?1 AND session_date < date('now')
             ORDER BY session_date DESC
             LIMIT ?2"
        )?;

        let rows = stmt.query_map(params![agent_id, limit as i64], |row| {
            Ok(DaySummary {
                session_id: row.get(0)?,
                session_date: row.get(1)?,
                summary: row.get(2)?,
                message_count: row.get(3)?,
                is_archived: false, // TODO: check archive table
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }

        Ok(results)
    }

    async fn update_session_summary(&self, session_id: &str, summary: String) -> daily_sessions::Result<()> {
        let conn = self.conn.lock().await;

        conn.execute(
            "UPDATE daily_sessions SET summary = ?1, updated_at = ?2 WHERE id = ?3",
            params![&summary, &Utc::now().to_rfc3339(), session_id]
        )?;

        Ok(())
    }

    async fn increment_message_count(&self, session_id: &str) -> daily_sessions::Result<()> {
        let conn = self.conn.lock().await;

        conn.execute(
            "UPDATE daily_sessions SET message_count = message_count + 1, updated_at = ?1 WHERE id = ?2",
            params![&Utc::now().to_rfc3339(), session_id]
        )?;

        Ok(())
    }

    async fn add_token_count(&self, session_id: &str, tokens: i64) -> daily_sessions::Result<()> {
        let conn = self.conn.lock().await;

        conn.execute(
            "UPDATE daily_sessions SET token_count = token_count + ?1, updated_at = ?2 WHERE id = ?3",
            params![tokens, &Utc::now().to_rfc3339(), session_id]
        )?;

        Ok(())
    }

    async fn delete_sessions_before(&self, agent_id: &str, before_date: &str) -> daily_sessions::Result<usize> {
        let conn = self.conn.lock().await;

        let rows = conn.execute(
            "DELETE FROM daily_sessions WHERE agent_id = ?1 AND session_date < ?2",
            params![agent_id, before_date]
        )?;

        Ok(rows as usize)
    }

    async fn get_session_messages(&self, session_id: &str) -> daily_sessions::Result<Vec<SessionMessage>> {
        let conn = self.conn.lock().await;

        let mut stmt = conn.prepare(
            "SELECT id, session_id, role, content, created_at, token_count, tool_calls, tool_results
             FROM messages WHERE session_id = ?1 ORDER BY created_at"
        )?;

        let rows = stmt.query_map(params![session_id], |row| {
            let created_at_str: String = row.get(4)?;
            let tool_calls_str: Option<String> = row.get(6)?;
            let tool_results_str: Option<String> = row.get(7)?;
            Ok(SessionMessage {
                id: row.get(0)?,
                session_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                created_at: parse_datetime(&created_at_str).unwrap_or_else(|_| Utc::now()),
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

    async fn create_message(&self, message: SessionMessage) -> daily_sessions::Result<()> {
        let conn = self.conn.lock().await;

        let tool_calls_str = message.tool_calls
            .as_ref()
            .map(|v| serde_json::to_string(v).unwrap_or_default());

        let tool_results_str = message.tool_results
            .as_ref()
            .map(|v| serde_json::to_string(v).unwrap_or_default());

        conn.execute(
            "INSERT INTO messages (id, session_id, role, content, created_at, token_count, tool_calls, tool_results)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                &message.id,
                &message.session_id,
                &message.role,
                &message.content,
                &message.created_at.to_rfc3339(),
                message.token_count,
                tool_calls_str.as_deref(),
                tool_results_str.as_deref(),
            ]
        )?;

        Ok(())
    }

    async fn get_or_create_today_session_with_prompt(&self, agent_id: &str, _system_prompt: &str) -> daily_sessions::Result<(DailySession, SystemPromptCheck)> {
        // For now, delegate to the simpler method and create a default check
        // A full implementation would require checking the agent table first
        let session = self.get_or_create_today_session(agent_id).await?;
        let version = session.system_prompt_version;
        Ok((session, SystemPromptCheck {
            has_changed: false,
            previous_version: Some(version),
            new_version: version,
            previous_prompt: None,
        }))
    }

    async fn upsert_agent(&self, agent: DailySessionAgent) -> daily_sessions::Result<()> {
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
            params![
                &agent.id,
                &agent.name,
                &agent.display_name,
                &agent.description,
                &agent.config_path,
                &agent.system_prompt_version,
                &agent.current_system_prompt,
                &now,
                &now,
            ],
        )?;

        Ok(())
    }

    async fn get_agent(&self, agent_id: &str) -> daily_sessions::Result<Option<DailySessionAgent>> {
        let conn = self.conn.lock().await;

        let mut stmt = conn.prepare(
            "SELECT id, name, display_name, description, config_path, system_prompt_version, current_system_prompt, created_at, updated_at
             FROM agents WHERE id = ?1"
        )?;

        let result = stmt.query_row([&agent_id], |row| {
            let created_at_str: String = row.get(7)?;
            let updated_at_str: String = row.get(8)?;
            Ok(DailySessionAgent {
                id: row.get(0)?,
                name: row.get(1)?,
                display_name: row.get(2)?,
                description: row.get(3)?,
                config_path: row.get(4)?,
                system_prompt_version: row.get(5)?,
                current_system_prompt: row.get(6)?,
                created_at: parse_datetime(&created_at_str).unwrap_or_else(|_| Utc::now()),
                updated_at: parse_datetime(&updated_at_str).unwrap_or_else(|_| Utc::now()),
            })
        });

        match result {
            Ok(agent) => Ok(Some(agent)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DailySessionError::Database(e))
        }
    }

    async fn check_system_prompt(&self, agent_id: &str, system_prompt: &str) -> daily_sessions::Result<SystemPromptCheck> {
        let conn = self.conn.lock().await;

        // Get current agent record
        let current_agent = self.get_agent(agent_id).await?;

        match current_agent {
            Some(agent) => {
                // Check if prompt has changed
                let has_changed = match &agent.current_system_prompt {
                    Some(current_prompt) => current_prompt != system_prompt,
                    None => true,
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
                        params![new_version, system_prompt, agent_id],
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
                    params![
                        agent_id,
                        agent_id,  // Use agent_id as name for now
                        agent_id,  // Use agent_id as display_name for now
                        "",         // Empty config_path for now
                        1i64,
                        system_prompt,
                        now.to_rfc3339(),
                        now.to_rfc3339(),
                    ],
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

// ============================================================================
// SESSION STATE PERSISTENCE (Agent Memory)
// ============================================================================

impl SqliteSessionRepository {
    /// Load session state for an agent from SQLite
    pub async fn load_session_state(&self, agent_id: &str) -> Result<Option<std::collections::HashMap<String, serde_json::Value>>, String> {
        let conn = self.conn.lock().await;

        let mut stmt = conn.prepare(
            "SELECT state_json FROM session_state WHERE agent_id = ?1"
        ).map_err(|e| format!("Failed to prepare query: {}", e))?;

        let result = stmt.query_row([agent_id], |row| {
            let state_json: String = row.get(0)?;
            Ok(state_json)
        });

        match result {
            Ok(json_str) => {
                let state: std::collections::HashMap<String, serde_json::Value> = serde_json::from_str(&json_str)
                    .map_err(|e| format!("Failed to parse session state JSON: {}", e))?;
                Ok(Some(state))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(format!("Database error: {}", e))
        }
    }

    /// Save session state for an agent to SQLite
    pub async fn save_session_state(&self, agent_id: &str, state: &std::collections::HashMap<String, serde_json::Value>) -> Result<(), String> {
        let state_json = serde_json::to_string(state)
            .map_err(|e| format!("Failed to serialize session state: {}", e))?;

        let conn = self.conn.lock().await;
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT OR REPLACE INTO session_state (agent_id, state_json, updated_at)
             VALUES (?1, ?2, ?3)",
            params![agent_id, &state_json, &now],
        ).map_err(|e| format!("Failed to save session state: {}", e))?;

        Ok(())
    }

    /// Load conversation history for an agent's today session and populate a session
    pub async fn load_conversation_history_into_session(
        &self,
        agent_id: &str,
        session: &MutexSession,
    ) -> Result<usize, String> {
        // Get today's session ID
        let today = Utc::now().format("%Y-%m-%d").to_string();
        let session_id = daily_sessions::DailySession::generate_id(agent_id, &today);

        // Load messages from SQLite
        let messages = self.get_session_messages(&session_id).await
            .map_err(|e| format!("Failed to load session messages: {}", e))?;

        // Convert messages to Content and add to session
        let mut count = 0;
        if let Ok(mut sess) = session.0.lock() {
            for msg in messages {
                let has_tool_calls = msg.tool_calls.as_ref()
                    .and_then(|v| v.as_array())
                    .map(|a| !a.is_empty())
                    .unwrap_or(false);

                let has_tool_results = msg.tool_results.as_ref()
                    .and_then(|v| v.as_array())
                    .map(|a| !a.is_empty())
                    .unwrap_or(false);

                // If the message has both tool_calls AND tool_results, we need to split it
                // into TWO separate Content objects to match OpenAI's expected format:
                // 1. Assistant message with tool_calls
                // 2. Separate tool messages for each result
                if has_tool_calls && has_tool_results {
                    // First Content: Assistant message with text + tool_calls
                    let mut assistant_parts = Vec::new();
                    assistant_parts.push(Part::Text { text: msg.content.clone() });

                    if let Some(tool_calls) = &msg.tool_calls {
                        if let Some(calls_array) = tool_calls.as_array() {
                            for call in calls_array {
                                if let (Some(id), Some(name), Some(args)) = (
                                    call.get("id").and_then(|v| v.as_str()),
                                    call.get("name").and_then(|v| v.as_str()),
                                    call.get("arguments"),
                                ) {
                                    assistant_parts.push(Part::FunctionCall {
                                        id: Some(id.to_string()),
                                        name: name.to_string(),
                                        args: args.clone(),
                                    });
                                }
                            }
                        }
                    }

                    let assistant_content = Content {
                        role: msg.role.clone(),
                        parts: assistant_parts,
                    };
                    sess.add_content(assistant_content);
                    count += 1;

                    // Second Content: Tool results (becomes "tool" role in OpenAI format)
                    // Group all tool results into a single Content with multiple FunctionResponse parts
                    let mut tool_result_parts = Vec::new();
                    if let Some(tool_results) = &msg.tool_results {
                        if let Some(results_array) = tool_results.as_array() {
                            for result in results_array {
                                if let (Some(id), Some(output)) = (
                                    result.get("tool_call_id").and_then(|v| v.as_str()),
                                    result.get("output"),
                                ) {
                                    let response = if let Some(s) = output.as_str() {
                                        s.to_string()
                                    } else {
                                        output.to_string()
                                    };
                                    tool_result_parts.push(Part::FunctionResponse {
                                        id: id.to_string(),
                                        response,
                                    });
                                }
                            }
                        }
                    }

                    // Create a separate content for tool results
                    // The OpenAI converter will handle splitting multiple FunctionResponse parts
                    // into separate "tool" role messages
                    if !tool_result_parts.is_empty() {
                        let tool_results_content = Content {
                            role: "tool".to_string(),  // Mark as tool role
                            parts: tool_result_parts,
                        };
                        sess.add_content(tool_results_content);
                        count += 1;
                    }
                } else {
                    // Normal case: message has only text, only tool_calls, or only tool_results
                    let mut parts = Vec::new();

                    // Add text part
                    parts.push(Part::Text { text: msg.content });

                    // Add tool calls if present
                    if let Some(tool_calls) = &msg.tool_calls {
                        if let Some(calls_array) = tool_calls.as_array() {
                            for call in calls_array {
                                if let (Some(id), Some(name), Some(args)) = (
                                    call.get("id").and_then(|v| v.as_str()),
                                    call.get("name").and_then(|v| v.as_str()),
                                    call.get("arguments"),
                                ) {
                                    parts.push(Part::FunctionCall {
                                        id: Some(id.to_string()),
                                        name: name.to_string(),
                                        args: args.clone(),
                                    });
                                }
                            }
                        }
                    }

                    // Add tool results if present
                    if let Some(tool_results) = &msg.tool_results {
                        if let Some(results_array) = tool_results.as_array() {
                            for result in results_array {
                                if let (Some(id), Some(output)) = (
                                    result.get("tool_call_id").and_then(|v| v.as_str()),
                                    result.get("output"),
                                ) {
                                    let response = if let Some(s) = output.as_str() {
                                        s.to_string()
                                    } else {
                                        output.to_string()
                                    };
                                    parts.push(Part::FunctionResponse {
                                        id: id.to_string(),
                                        response,
                                    });
                                }
                            }
                        }
                    }

                    let content = Content {
                        role: msg.role,
                        parts,
                    };

                    sess.add_content(content);
                    count += 1;
                }
            }
        }

        Ok(count)
    }
}

// ============================================================================
// COMMANDS
// ============================================================================

/// Get or create today's session for an agent
#[tauri::command]
pub async fn get_or_create_today_session(agent_id: String) -> Result<DailySession, String> {
    let db_path = AppDirs::get()
        .map_err(|e| e.to_string())?
        .agent_channels_db_path();

    let repo = SqliteSessionRepository::new(db_path)?;
    let manager = DailySessionManager::new(Arc::new(repo));

    manager.get_or_create_today(&agent_id).await
        .map_err(|e| e.to_string())
}

/// List previous days for an agent
#[tauri::command]
pub async fn list_previous_days(agent_id: String, limit: usize) -> Result<Vec<DaySummary>, String> {
    let db_path = AppDirs::get()
        .map_err(|e| e.to_string())?
        .agent_channels_db_path();

    let repo = SqliteSessionRepository::new(db_path)?;
    let manager = DailySessionManager::new(Arc::new(repo));

    manager.list_previous_days(&agent_id, limit).await
        .map_err(|e| e.to_string())
}

/// Load messages for a specific session
#[tauri::command]
pub async fn load_session_messages(session_id: String) -> Result<Vec<Message>, String> {
    let db_path = AppDirs::get()
        .map_err(|e| e.to_string())?
        .agent_channels_db_path();

    let repo = SqliteSessionRepository::new(db_path)?;

    repo.get_session_messages(&session_id).await
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|m| Ok(Message {
            id: m.id,
            session_id: m.session_id,
            role: m.role,
            content: m.content,
            created_at: m.created_at,
            token_count: m.token_count,
            tool_calls: m.tool_calls,
            tool_results: m.tool_results,
        }))
        .collect()
}

/// Delete agent history before a certain date
#[tauri::command]
pub async fn delete_agent_history(agent_id: String, before_date: String) -> Result<usize, String> {
    let db_path = AppDirs::get()
        .map_err(|e| e.to_string())?
        .agent_channels_db_path();

    let repo = SqliteSessionRepository::new(db_path)?;
    let manager = DailySessionManager::new(Arc::new(repo));

    manager.clear_agent_history(&agent_id, &before_date).await
        .map_err(|e| e.to_string())
}

/// Record a message in a session
#[tauri::command]
pub async fn record_session_message(
    session_id: String,
    role: String,
    content: String,
    tool_calls: Option<serde_json::Value>,
    tool_results: Option<serde_json::Value>,
) -> Result<String, String> {
    let db_path = AppDirs::get()
        .map_err(|e| e.to_string())?
        .agent_channels_db_path();

    let repo = SqliteSessionRepository::new(db_path.clone())?;
    let manager = DailySessionManager::new(Arc::new(repo));

    let message = SessionMessage {
        id: uuid::Uuid::new_v4().to_string(),
        session_id: session_id.clone(),
        role: role.clone(),
        content: content.clone(),
        created_at: Utc::now(),
        token_count: 0,
        tool_calls,
        tool_results,
    };

    manager.record_message(&session_id, message.clone()).await
        .map_err(|e| e.to_string())?;

    // Extract agent_id from session_id (format: session_{agent_id}_{date})
    let agent_id = session_id
        .strip_prefix("session_")
        .and_then(|s| s.rsplit('_').nth(1)) // Get agent_id from "agent_id_date"
        .unwrap_or(&session_id)
        .to_string();

    // Get agent name from database for indexing
    let agent_name = get_agent_name(&db_path, &agent_id).await.unwrap_or_else(|_| agent_id.clone());

    // Index the message asynchronously (fire and forget)
    let message_id = message.id.clone();
    let session_id_clone = session_id.clone();
    let content_clone = content.clone();
    let role_clone = role.clone();
    let timestamp = message.created_at.timestamp();

    tokio::spawn(async move {
        crate::commands::search::index_message_internal(
            message_id,
            session_id_clone,
            agent_id,
            agent_name,
            role_clone,
            content_clone,
            timestamp,
        ).await;
    });

    Ok(message.id)
}

/// Helper to get agent name from database
async fn get_agent_name(db_path: &PathBuf, agent_id: &str) -> Result<String, String> {
    let conn = Connection::open(db_path)
        .map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare("SELECT name FROM agents WHERE id = ?1")
        .map_err(|e| e.to_string())?;

    let name = stmt.query_row(params![agent_id], |row| row.get::<_, String>(0))
        .unwrap_or_else(|_| agent_id.to_string());

    Ok(name)
}

/// Generate end-of-day summary for a session using LLM
#[tauri::command]
pub async fn generate_session_summary(session_id: String) -> Result<String, String> {
    use zero_app::{Content, Llm, LlmConfig, OpenAiLlm, LlmRequest};

    let app_dirs = AppDirs::get()
        .map_err(|e| e.to_string())?;

    let db_path = app_dirs.agent_channels_db_path();

    // Scope the database query to ensure connection is dropped before await
    let messages: Vec<(String, String)> = {
        use rusqlite::params;

        let conn = rusqlite::Connection::open(&db_path)
            .map_err(|e| format!("Failed to open database: {}", e))?;

        let mut stmt = conn.prepare(
            "SELECT role, content FROM messages WHERE session_id = ? ORDER BY created_at ASC"
        ).map_err(|e| format!("Failed to prepare query: {}", e))?;

        // Collect rows into a Vec first (this consumes the iterator)
        let rows = stmt.query_map(params![&session_id], |row| {
            Ok((row.get(0)?, row.get(1)?))
        }).map_err(|e| format!("Failed to query messages: {}", e))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to collect messages: {}", e))?;

        // Drop stmt and conn explicitly before returning
        drop(stmt);
        drop(conn);

        rows
    }; // conn is dropped here

    if messages.is_empty() {
        return Ok("No messages in this session.".to_string());
    }

    // Read providers.json to get the default provider
    let providers_path = app_dirs.config_dir.join("providers.json");
    let providers_json = std::fs::read_to_string(&providers_path)
        .map_err(|e| format!("Failed to read providers.json: {}", e))?;

    let providers: Vec<crate::commands::providers::Provider> = serde_json::from_str(&providers_json)
        .map_err(|e| format!("Failed to parse providers.json: {}", e))?;

    let provider = providers.first()
        .ok_or("No provider configured. Please add a provider first.")?;

    // Create LLM config (check if base_url is provided)
    let llm_config = if !provider.base_url.is_empty() {
        LlmConfig::compatible(&provider.api_key, &provider.base_url,
            provider.models.first().unwrap_or(&"gpt-4o-mini".to_string()))
    } else {
        LlmConfig::new(&provider.api_key,
            provider.models.first().unwrap_or(&"gpt-4o-mini".to_string()))
    };

    // Build conversation text for summarization
    let conversation_text: String = messages
        .iter()
        .map(|(role, content)| format!("{}: {}", role, content))
        .collect::<Vec<_>>()
        .join("\n\n");

    // Create summary prompt
    let system_prompt = r#"You are a conversation summarizer. Your task is to create a concise summary (2-3 sentences) of a day's conversation between a user and an AI assistant.

Focus on:
- Main topics discussed
- Key decisions made
- Important outcomes or action items

Keep the summary brief and factual. Do not include the conversation details themselves, just the summary."#;

    let user_prompt = format!(
        "Here is the conversation to summarize:\n\n{}\n\nPlease provide a 2-3 sentence summary of this conversation.",
        conversation_text
    );

    // Create LLM request
    let llm = OpenAiLlm::new(llm_config)
        .map_err(|e| format!("Failed to create LLM: {}", e))?;

    let request = LlmRequest::new()
        .with_system_instruction(system_prompt)
        .with_content(Content::user(user_prompt));

    // Call LLM (use spawn_blocking for potentially blocking HTTP operations)
    let response = tokio::task::spawn_blocking(move || {
        // Get the current runtime handle or create a new one if needed
        let handle = tokio::runtime::Handle::try_current();
        match handle {
            Ok(h) => h.block_on(llm.generate(request)),
            Err(_) => {
                // No runtime exists, create a temporary one
                let rt = tokio::runtime::Runtime::new()
                    .map_err(|e| zero_app::ZeroError::Generic(format!("Failed to create runtime: {}", e)))?;
                rt.block_on(llm.generate(request))
            }
        }
    })
    .await
    .map_err(|e| format!("Task failed: {}", e))?
    .map_err(|e| format!("LLM request failed: {}", e))?;

    // Extract the summary text from the response
    let summary = response.content
        .and_then(|c| c.text().map(|s| s.to_string()))
        .unwrap_or_else(|| "No summary generated.".to_string());

    // Save summary to database
    let repo = SqliteSessionRepository::new(db_path)?;
    repo.update_session_summary(&session_id, summary.clone()).await
        .map_err(|e| e.to_string())?;

    Ok(summary)
}

/// Get agent channel info for the sidebar
#[tauri::command]
pub async fn list_agent_channels() -> Result<Vec<AgentChannel>, String> {
    // For now, this is a placeholder
    // TODO: Implement by joining agents with daily_sessions to get today's message count
    Ok(vec![])
}
