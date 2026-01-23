// ============================================================================
// SEARCH COMMANDS
// Tauri commands for full-text search across messages
// ============================================================================

use crate::settings::AppDirs;
use search_index::{SearchIndexManager, SearchQuery, SearchResult, IndexedDocument};
use std::sync::Arc;
use tokio::sync::Mutex;
use chrono::Utc;

/// Global search index manager
lazy_static::lazy_static! {
    static ref SEARCH_MANAGER: Arc<Mutex<Option<SearchIndexManager>>> =
        Arc::new(Mutex::new(None));
}

/// Initialize search index for active vault (idempotent)
#[tauri::command]
pub async fn initialize_search_index() -> Result<(), String> {
    let mut guard = SEARCH_MANAGER.lock().await;

    // Already initialized
    if guard.is_some() {
        return Ok(());
    }

    let app_dirs = AppDirs::get()
        .map_err(|e| e.to_string())?;

    let db_dir = app_dirs.db_dir;
    let index_dir = db_dir.join("search_index");

    // Create and initialize manager while holding the lock
    let manager = SearchIndexManager::new(index_dir)
        .map_err(|e| e.to_string())?;

    manager.initialize().await
        .map_err(|e| e.to_string())?;

    *guard = Some(manager);

    Ok(())
}

/// Search messages across active and archived
#[tauri::command]
pub async fn search_messages(query: SearchQuery) -> Result<Vec<SearchResult>, String> {
    let guard = SEARCH_MANAGER.lock().await;
    let manager = guard
        .as_ref()
        .ok_or("Search index not initialized. Call initialize_search_index first.")?;

    manager.search(&query).await
        .map_err(|e| e.to_string())
}

/// Index a new message (called when message is created)
#[tauri::command]
pub async fn index_message(doc: IndexedDocument) -> Result<(), String> {
    let guard = SEARCH_MANAGER.lock().await;
    let manager = guard
        .as_ref()
        .ok_or("Search index not initialized. Call initialize_search_index first.")?;

    manager.index_message(&doc).await
        .map_err(|e| e.to_string())
}

/// Batch index multiple messages
#[tauri::command]
pub async fn index_messages(docs: Vec<IndexedDocument>) -> Result<(), String> {
    let guard = SEARCH_MANAGER.lock().await;
    let manager = guard
        .as_ref()
        .ok_or("Search index not initialized. Call initialize_search_index first.")?;

    manager.index_messages(&docs).await
        .map_err(|e| e.to_string())
}

/// Rebuild index from scratch
#[tauri::command]
pub async fn rebuild_search_index() -> Result<String, String> {
    use crate::settings::AppDirs;

    let guard = SEARCH_MANAGER.lock().await;
    let manager = guard
        .as_ref()
        .ok_or("Search index not initialized.")?;

    // Clear existing index
    manager.clear().await
        .map_err(|e| e.to_string())?;

    // Get database path and collect all messages synchronously
    let docs = {
        let app_dirs = AppDirs::get()
            .map_err(|e| e.to_string())?;
        let db_path = app_dirs.agent_channels_db_path();

        if !db_path.exists() {
            return Ok("No messages found to index.".to_string());
        }

        // Collect all documents in a sync block
        collect_all_messages(&db_path)?
    };

    let count = docs.len();

    // Now batch index (async is fine since we're done with database)
    if !docs.is_empty() {
        manager.index_messages(&docs).await
            .map_err(|e| e.to_string())?;
    }

    Ok(format!("Indexed {} messages from database.", count))
}

/// Helper to collect all messages from database (synchronous)
fn collect_all_messages(db_path: &std::path::PathBuf) -> Result<Vec<IndexedDocument>, String> {
    use rusqlite::params;

    let conn = rusqlite::Connection::open(db_path)
        .map_err(|e| format!("Failed to open database: {}", e))?;

    let mut stmt = conn.prepare(
        "SELECT m.id, m.session_id, m.role, m.content, m.created_at,
                ds.agent_id, a.name as agent_name
         FROM messages m
         INNER JOIN daily_sessions ds ON m.session_id = ds.id
         INNER JOIN agents a ON ds.agent_id = a.id"
    ).map_err(|e| format!("Failed to prepare query: {}", e))?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,   // id
            row.get::<_, String>(1)?,   // session_id
            row.get::<_, String>(2)?,   // role
            row.get::<_, String>(3)?,   // content
            row.get::<_, String>(4)?,   // created_at
            row.get::<_, String>(5)?,   // agent_id
            row.get::<_, String>(6)?,   // agent_name
        ))
    }).map_err(|e| format!("Failed to query messages: {}", e))?;

    let mut docs = Vec::new();
    for row in rows {
        let (id, session_id, role, content, created_at, agent_id, agent_name) = row
            .map_err(|e| format!("Failed to read row: {}", e))?;

        let timestamp = chrono::DateTime::parse_from_rfc3339(&created_at)
            .map(|dt| dt.timestamp())
            .unwrap_or(0);

        docs.push(IndexedDocument {
            message_id: id,
            session_id,
            agent_id,
            agent_name,
            role,
            content,
            timestamp,
            source_type: "sqlite".to_string(),
            source_path: None,
        });
    }

    Ok(docs)
}

/// Delete messages from search index by session
#[tauri::command]
pub async fn delete_session_from_index(session_id: String) -> Result<usize, String> {
    let guard = SEARCH_MANAGER.lock().await;
    let manager = guard
        .as_ref()
        .ok_or("Search index not initialized.")?;

    manager.delete_session(&session_id).await
        .map_err(|e| e.to_string())
}

/// Delete messages from search index by agent
#[tauri::command]
pub async fn delete_agent_from_index(agent_id: String) -> Result<usize, String> {
    let guard = SEARCH_MANAGER.lock().await;
    let manager = guard
        .as_ref()
        .ok_or("Search index not initialized.")?;

    manager.delete_agent(&agent_id).await
        .map_err(|e| e.to_string())
}

/// Clear search index
#[tauri::command]
pub async fn clear_search_index() -> Result<(), String> {
    let guard = SEARCH_MANAGER.lock().await;
    let manager = guard
        .as_ref()
        .ok_or("Search index not initialized.")?;

    manager.clear().await
        .map_err(|e| e.to_string())
}

// ============================================================================
// INTERNAL HELPER FUNCTIONS
// ============================================================================

/// Index a message asynchronously (internal use, not a Tauri command)
/// This is called by agent_channels when new messages are created
pub async fn index_message_internal(
    message_id: String,
    session_id: String,
    agent_id: String,
    agent_name: String,
    role: String,
    content: String,
    timestamp: i64,
) {
    let guard = SEARCH_MANAGER.lock().await;
    if let Some(manager) = guard.as_ref() {
        let doc = IndexedDocument {
            message_id,
            session_id,
            agent_id,
            agent_name,
            role,
            content,
            timestamp,
            source_type: "sqlite".to_string(),
            source_path: None,
        };

        let _ = manager.index_message(&doc).await;
    }
}
