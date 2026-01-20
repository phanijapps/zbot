// ============================================================================
// DELETION COMMANDS
// Tauri commands for comprehensive deletion operations
// ============================================================================

use crate::domains::conversation_runtime::deletion::{DeletionService, DeletionResult, DeletionScope};
use daily_sessions::CacheStats;

/// Delete a single session with full cleanup
#[tauri::command]
pub async fn delete_session(session_id: String) -> Result<DeletionResult, String> {
    let service = DeletionService::for_active_vault()
        .map_err(|e| e.to_string())?;

    service.delete_session(&session_id).await
        .map_err(|e| e.to_string())
}

/// Delete agent history with scope selection (Chrome-style history clearing)
#[tauri::command]
pub async fn delete_agent_history_with_scope(
    agent_id: String,
    scope: DeletionScope,
) -> Result<DeletionResult, String> {
    let service = DeletionService::for_active_vault()
        .map_err(|e| e.to_string())?;

    match scope {
        DeletionScope::AllTime => {
            service.delete_all_sessions(&agent_id).await
        }
        DeletionScope::Last7Days | DeletionScope::Last30Days | DeletionScope::CustomRange { .. } => {
            if let Some((start_date, end_date)) = scope.get_date_range() {
                service.delete_sessions_by_date_range(&agent_id, &start_date, &end_date).await
            } else {
                // AllTime case
                service.delete_all_sessions(&agent_id).await
            }
        }
    }.map_err(|e| e.to_string())
}

/// Get cache statistics
#[tauri::command]
pub async fn get_cache_stats() -> Result<CacheStats, String> {
    Ok(daily_sessions::CONVERSATION_CACHE.stats())
}

/// Clear conversation cache
#[tauri::command]
pub async fn clear_cache() -> Result<(), String> {
    daily_sessions::CONVERSATION_CACHE.clear().await;
    Ok(())
}

/// Invalidate cache for a specific session
#[tauri::command]
pub async fn invalidate_session_cache(session_id: String) -> Result<(), String> {
    daily_sessions::CONVERSATION_CACHE.invalidate(&session_id).await;
    Ok(())
}

/// Invalidate all cache for an agent
#[tauri::command]
pub async fn invalidate_agent_cache(agent_id: String) -> Result<(), String> {
    daily_sessions::CONVERSATION_CACHE.invalidate_agent(&agent_id).await;
    Ok(())
}
