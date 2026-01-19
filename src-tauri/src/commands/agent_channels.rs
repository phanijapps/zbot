// ============================================================================
// AGENT CHANNELS COMMANDS
// Agent Channel model - daily sessions, history management, and UI commands
// ============================================================================

use crate::settings::AppDirs;
use daily_sessions::{DailySessionManager, DailySessionRepository, DaySummary, DailySession, SessionMessage};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use chrono::{DateTime, Utc};
use zero_core::ZeroError;

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
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (agent_id) REFERENCES agents(id) ON DELETE CASCADE
        )",
        [],
    ).map_err(|e| format!("Failed to create daily_sessions table: {}", e))?;

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
            .map_err(|e| ZeroError::Generic(e))?;

        // Try to get existing session
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, session_date, summary, previous_session_ids,
                    message_count, token_count, created_at, updated_at
             FROM daily_sessions WHERE id = ?1"
        ).map_err(|e| ZeroError::Generic(format!("Failed to prepare query: {}", e)))?;

        let session = stmt.query_row([&session_id], |row| {
            let created_at_str: String = row.get(7)?;
            let updated_at_str: String = row.get(8)?;
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
                    "INSERT INTO daily_sessions (id, agent_id, session_date, message_count, token_count, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![&new_session.id, &new_session.agent_id, &new_session.session_date,
                            0i64, 0i64, &now, &now]
                ).map_err(|e| ZeroError::Generic(format!("Failed to insert session: {}", e)))?;

                Ok(new_session)
            }
            Err(e) => Err(ZeroError::Generic(format!("Database error: {}", e)))
        }
    }

    async fn get_session(&self, session_id: &str) -> daily_sessions::Result<Option<DailySession>> {
        let conn = self.conn.lock().await;

        let mut stmt = conn.prepare(
            "SELECT id, agent_id, session_date, summary, previous_session_ids,
                    message_count, token_count, created_at, updated_at
             FROM daily_sessions WHERE id = ?1"
        ).map_err(|e| ZeroError::Generic(format!("Failed to prepare query: {}", e)))?;

        let session = stmt.query_row([&session_id], |row| {
            let created_at_str: String = row.get(7)?;
            let updated_at_str: String = row.get(8)?;
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
                created_at: parse_datetime(&created_at_str).unwrap_or_else(|_| Utc::now()),
                updated_at: parse_datetime(&updated_at_str).unwrap_or_else(|_| Utc::now()),
            })
        });

        match session {
            Ok(s) => Ok(Some(s)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(ZeroError::Generic(format!("Database error: {}", e)))
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
        ).map_err(|e| ZeroError::Generic(format!("Failed to prepare query: {}", e)))?;

        let rows = stmt.query_map(params![agent_id, limit as i64], |row| {
            Ok(DaySummary {
                session_id: row.get(0)?,
                session_date: row.get(1)?,
                summary: row.get(2)?,
                message_count: row.get(3)?,
                is_archived: false, // TODO: check archive table
            })
        }).map_err(|e| ZeroError::Generic(format!("Failed to execute query: {}", e)))?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|e| ZeroError::Generic(format!("Row error: {}", e)))?);
        }

        Ok(results)
    }

    async fn update_session_summary(&self, session_id: &str, summary: String) -> daily_sessions::Result<()> {
        let conn = self.conn.lock().await;

        conn.execute(
            "UPDATE daily_sessions SET summary = ?1, updated_at = ?2 WHERE id = ?3",
            params![&summary, &Utc::now().to_rfc3339(), session_id]
        ).map_err(|e| ZeroError::Generic(format!("Failed to update summary: {}", e)))?;

        Ok(())
    }

    async fn increment_message_count(&self, session_id: &str) -> daily_sessions::Result<()> {
        let conn = self.conn.lock().await;

        conn.execute(
            "UPDATE daily_sessions SET message_count = message_count + 1, updated_at = ?1 WHERE id = ?2",
            params![&Utc::now().to_rfc3339(), session_id]
        ).map_err(|e| ZeroError::Generic(format!("Failed to increment message count: {}", e)))?;

        Ok(())
    }

    async fn add_token_count(&self, session_id: &str, tokens: i64) -> daily_sessions::Result<()> {
        let conn = self.conn.lock().await;

        conn.execute(
            "UPDATE daily_sessions SET token_count = token_count + ?1, updated_at = ?2 WHERE id = ?3",
            params![tokens, &Utc::now().to_rfc3339(), session_id]
        ).map_err(|e| ZeroError::Generic(format!("Failed to add token count: {}", e)))?;

        Ok(())
    }

    async fn delete_sessions_before(&self, agent_id: &str, before_date: &str) -> daily_sessions::Result<usize> {
        let conn = self.conn.lock().await;

        let rows = conn.execute(
            "DELETE FROM daily_sessions WHERE agent_id = ?1 AND session_date < ?2",
            params![agent_id, before_date]
        ).map_err(|e| ZeroError::Generic(format!("Failed to delete sessions: {}", e)))?;

        Ok(rows as usize)
    }

    async fn get_session_messages(&self, session_id: &str) -> daily_sessions::Result<Vec<SessionMessage>> {
        let conn = self.conn.lock().await;

        let mut stmt = conn.prepare(
            "SELECT id, session_id, role, content, created_at, token_count, tool_calls, tool_results
             FROM messages WHERE session_id = ?1 ORDER BY created_at"
        ).map_err(|e| ZeroError::Generic(format!("Failed to prepare query: {}", e)))?;

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
        }).map_err(|e| ZeroError::Generic(format!("Failed to execute query: {}", e)))?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|e| ZeroError::Generic(format!("Row error: {}", e)))?);
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
        ).map_err(|e| ZeroError::Generic(format!("Failed to insert message: {}", e)))?;

        Ok(())
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

    let repo = SqliteSessionRepository::new(db_path)?;
    let manager = DailySessionManager::new(Arc::new(repo));

    let message = SessionMessage {
        id: uuid::Uuid::new_v4().to_string(),
        session_id: session_id.clone(),
        role,
        content,
        created_at: Utc::now(),
        token_count: 0,
        tool_calls,
        tool_results,
    };

    manager.record_message(&session_id, message.clone()).await
        .map_err(|e| e.to_string())?;

    Ok(message.id)
}

/// Generate end-of-day summary for a session
#[tauri::command]
pub async fn generate_session_summary(session_id: String) -> Result<String, String> {
    let db_path = AppDirs::get()
        .map_err(|e| e.to_string())?
        .agent_channels_db_path();

    let repo = SqliteSessionRepository::new(db_path)?;
    let manager = DailySessionManager::new(Arc::new(repo));

    manager.generate_end_of_day_summary(&session_id).await
        .map_err(|e| e.to_string())
}

/// Get agent channel info for the sidebar
#[tauri::command]
pub async fn list_agent_channels() -> Result<Vec<AgentChannel>, String> {
    // For now, this is a placeholder
    // TODO: Implement by joining agents with daily_sessions to get today's message count
    Ok(vec![])
}
