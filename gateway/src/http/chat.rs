//! # Chat Session Endpoints
//!
//! HTTP API for persistent chat session initialization and message history.

use crate::state::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};

// ============================================================================
// REQUEST / RESPONSE TYPES
// ============================================================================

/// Response for POST /api/chat/init.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatInitResponse {
    pub session_id: String,
    pub conversation_id: String,
    pub created: bool,
}

/// Query parameters for GET /api/sessions/:id/messages.
#[derive(Debug, Deserialize)]
pub struct MessagesQuery {
    pub limit: Option<u32>,
}

/// A single message in the session history response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMessageResponse {
    pub id: String,
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_results: Option<String>,
    pub timestamp: String,
}

// ============================================================================
// ENDPOINTS
// ============================================================================

/// POST /api/chat/init
///
/// Creates the persistent chat session if it doesn't exist.
/// Returns existing session IDs if already created. Idempotent.
pub async fn init_chat_session(
    State(state): State<AppState>,
) -> Result<Json<ChatInitResponse>, (StatusCode, String)> {
    let settings = state
        .settings
        .get_execution_settings()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    // If chat session already exists, return it
    if let (Some(session_id), Some(conv_id)) =
        (&settings.chat.session_id, &settings.chat.conversation_id)
    {
        return Ok(Json(ChatInitResponse {
            session_id: session_id.clone(),
            conversation_id: conv_id.clone(),
            created: false,
        }));
    }

    // Generate new IDs
    let session_id = format!("sess-chat-{}", uuid::Uuid::new_v4());
    let conversation_id = format!("chat-{}", uuid::Uuid::new_v4());

    // Persist the chat session IDs in settings
    let mut updated_settings = settings.clone();
    updated_settings.chat = gateway_services::ChatConfig {
        session_id: Some(session_id.clone()),
        conversation_id: Some(conversation_id.clone()),
    };
    state
        .settings
        .update_execution_settings(updated_settings)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(ChatInitResponse {
        session_id,
        conversation_id,
        created: true,
    }))
}

/// GET /api/sessions/:id/messages?limit=100
///
/// Returns messages for a session, ordered by timestamp (oldest first).
/// Used by the chat UI to load history on mount.
pub async fn get_session_messages(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Query(query): Query<MessagesQuery>,
) -> Result<Json<Vec<SessionMessageResponse>>, (StatusCode, String)> {
    let limit = query.limit.unwrap_or(100);

    let messages = state
        .conversations
        .get_session_conversation(&session_id, limit as usize)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to load messages: {}", e),
            )
        })?;

    let response: Vec<SessionMessageResponse> = messages
        .into_iter()
        .map(|m| SessionMessageResponse {
            id: m.id,
            role: m.role,
            content: m.content,
            tool_calls: m.tool_calls,
            tool_results: m.tool_results,
            timestamp: m.created_at,
        })
        .collect();

    Ok(Json(response))
}
