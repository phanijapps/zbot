// ============================================================================
// DELETION SERVICE
// Comprehensive deletion that removes from SQLite, cache, index, and archives
// ============================================================================

use crate::settings::AppDirs;
use anyhow::Result;
use chrono::{DateTime, Datelike, Utc};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Deletion result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletionResult {
    pub sessions_deleted: usize,
    pub messages_deleted: usize,
    pub cache_entries_invalidated: usize,
}

impl DeletionResult {
    pub fn is_empty(&self) -> bool {
        self.sessions_deleted == 0 && self.messages_deleted == 0
    }
}

/// Deletion scope for Chrome-style history clearing
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DeletionScope {
    #[serde(rename = "last_7_days")]
    Last7Days,
    #[serde(rename = "last_30_days")]
    Last30Days,
    #[serde(rename = "all_time")]
    AllTime,
    #[serde(rename = "custom_range")]
    CustomRange { start_date: String, end_date: String },
}

impl DeletionScope {
    /// Get date range for this scope
    pub fn get_date_range(&self) -> Option<(String, String)> {
        match self {
            DeletionScope::Last7Days => {
                let end_date = Utc::now();
                let start_date = end_date - chrono::Duration::days(7);
                Some((
                    start_date.format("%Y-%m-%d").to_string(),
                    end_date.format("%Y-%m-%d").to_string(),
                ))
            }
            DeletionScope::Last30Days => {
                let end_date = Utc::now();
                let start_date = end_date - chrono::Duration::days(30);
                Some((
                    start_date.format("%Y-%m-%d").to_string(),
                    end_date.format("%Y-%m-%d").to_string(),
                ))
            }
            DeletionScope::AllTime => None,
            DeletionScope::CustomRange { start_date, end_date } => {
                Some((start_date.clone(), end_date.clone()))
            }
        }
    }
}

/// Comprehensive deletion service
pub struct DeletionService {
    db_path: PathBuf,
    archive_dir: PathBuf,
}

impl DeletionService {
    /// Create deletion service for active vault
    pub fn for_active_vault() -> Result<Self> {
        let app_dirs = AppDirs::get()?;
        let db_path = app_dirs.db_dir.join("agent_channels.db");
        let archive_dir = app_dirs.db_dir.join("archive");
        Ok(Self {
            db_path,
            archive_dir,
        })
    }

    /// Create deletion service for specific vault
    pub fn new(vault_path: PathBuf) -> Self {
        let db_path = vault_path.join("db/agent_channels.db");
        let archive_dir = vault_path.join("db/archive");
        Self {
            db_path,
            archive_dir,
        }
    }

    /// Delete a single session (removes from ALL locations)
    pub async fn delete_session(&self, session_id: &str) -> Result<DeletionResult> {
        let mut result = DeletionResult {
            sessions_deleted: 0,
            messages_deleted: 0,
            cache_entries_invalidated: 0,
        };

        // 1. Delete from SQLite (daily_sessions and messages tables)
        if self.db_path.exists() {
            let conn = rusqlite::Connection::open(&self.db_path)?;

            // Get message count before deletion
            let msg_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM messages WHERE session_id = ?",
                params![session_id],
                |row| row.get(0),
            ).unwrap_or(0);

            // Delete messages
            conn.execute(
                "DELETE FROM messages WHERE session_id = ?",
                params![session_id],
            )?;

            // Delete session
            let rows_affected = conn.execute(
                "DELETE FROM daily_sessions WHERE id = ?",
                params![session_id],
            )?;

            result.messages_deleted = msg_count as usize;
            result.sessions_deleted = rows_affected;
        }

        // 2. Invalidate from cache (global cache)
        daily_sessions::CONVERSATION_CACHE.invalidate(session_id).await;
        result.cache_entries_invalidated = 1;

        // 3. Note: Search index deletion will be added when search-index crate is created
        // TODO: Delete from Tantivy search index

        // 4. Note: Parquet archive handling
        // Parquet files are immutable, so we maintain a deletion registry
        // For now, we'll just note that this session was deleted
        self.mark_as_deleted_in_registry(session_id).await?;

        Ok(result)
    }

    /// Delete all sessions for an agent
    pub async fn delete_agent_sessions(&self, agent_id: &str) -> Result<DeletionResult> {
        let mut total = DeletionResult {
            sessions_deleted: 0,
            messages_deleted: 0,
            cache_entries_invalidated: 0,
        };

        // Get all sessions for agent - collect IDs then drop connection before await
        let session_ids: Vec<String> = if self.db_path.exists() {
            let conn = rusqlite::Connection::open(&self.db_path)?;
            let mut stmt = conn.prepare(
                "SELECT id FROM daily_sessions WHERE agent_id = ?"
            )?;

            let result = stmt.query_map(params![agent_id], |row| {
                row.get::<_, String>(0)
            })?.collect::<std::result::Result<Vec<_>, _>>()?;
            result
        } else {
            vec![]
        };

        // Delete each session
        for session_id in session_ids {
            let result = self.delete_session(&session_id).await?;
            total.sessions_deleted += result.sessions_deleted;
            total.messages_deleted += result.messages_deleted;
            total.cache_entries_invalidated += result.cache_entries_invalidated;
        }

        // Invalidate all agent sessions from cache
        daily_sessions::CONVERSATION_CACHE.invalidate_agent(agent_id).await;

        Ok(total)
    }

    /// Delete sessions in a date range (Chrome-style history clearing)
    pub async fn delete_sessions_by_date_range(
        &self,
        agent_id: &str,
        start_date: &str,
        end_date: &str,
    ) -> Result<DeletionResult> {
        let mut total = DeletionResult {
            sessions_deleted: 0,
            messages_deleted: 0,
            cache_entries_invalidated: 0,
        };

        // Collect session IDs first, then drop connection before await
        let session_ids: Vec<String> = if self.db_path.exists() {
            let conn = rusqlite::Connection::open(&self.db_path)?;
            let mut stmt = conn.prepare(
                "SELECT id FROM daily_sessions
                 WHERE agent_id = ? AND session_date BETWEEN ? AND ?"
            )?;

            let result = stmt.query_map(
                params![agent_id, start_date, end_date],
                |row| row.get::<_, String>(0),
            )?.collect::<std::result::Result<Vec<_>, _>>()?;
            result
        } else {
            vec![]
        };

        // Now we can safely await since conn and stmt are dropped
        for session_id in session_ids {
            let result = self.delete_session(&session_id).await?;
            total.sessions_deleted += result.sessions_deleted;
            total.messages_deleted += result.messages_deleted;
            total.cache_entries_invalidated += result.cache_entries_invalidated;
        }

        Ok(total)
    }

    /// Delete all data (Chrome-style "Clear all history")
    pub async fn delete_all_sessions(&self, agent_id: &str) -> Result<DeletionResult> {
        self.delete_agent_sessions(agent_id).await
    }

    /// Mark session as deleted in registry (for Parquet archives)
    async fn mark_as_deleted_in_registry(&self, session_id: &str) -> Result<()> {
        let registry_path = self.archive_dir.join("deleted_sessions.jsonl");

        // Create archive dir if it doesn't exist
        if !self.archive_dir.exists() {
            std::fs::create_dir_all(&self.archive_dir)?;
        }

        // Append to deletion registry
        let entry = serde_json::json!({
            "session_id": session_id,
            "deleted_at": Utc::now().to_rfc3339(),
        });

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&registry_path)?;

        use std::io::Write;
        writeln!(file, "{}", entry)?;

        Ok(())
    }
}
