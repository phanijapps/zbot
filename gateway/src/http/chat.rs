//! # Chat Session Endpoints
//!
//! HTTP API for persistent chat session initialization and message history.

use super::{sessions::LoopbackBind, HttpErrorResponse, SameOrigin};
use crate::state::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use tokio::sync::Mutex;

// Serializes the read/create/update sequence for the singleton chat slot.
static CHAT_SESSION_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn chat_session_lock() -> &'static Mutex<()> {
    CHAT_SESSION_LOCK.get_or_init(|| Mutex::new(()))
}

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

/// Independent shell Chat identity; legacy ChatInitResponse is unchanged.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellChatResponse {
    pub session_id: String,
    pub conversation_id: String,
    pub created: bool,
    pub is_live: bool,
}

fn chat_error(status: StatusCode, message: &'static str) -> (StatusCode, Json<HttpErrorResponse>) {
    (
        status,
        Json(HttpErrorResponse {
            error: message.to_owned(),
        }),
    )
}

/// POST /api/sessions/chat — one insert, no singleton or history mutation.
pub async fn create_chat_session(
    _origin: SameOrigin,
    _bind: LoopbackBind,
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<ShellChatResponse>), (StatusCode, Json<HttpErrorResponse>)> {
    let mut session =
        execution_state::Session::new_queued("root", execution_state::TriggerSource::Web);
    session.mode = Some("fast".into());
    session.metadata = Some(serde_json::json!({"shell_chat_conversation_id": session.id}));
    state
        .state_service()
        .create_session_from(&session)
        .map_err(|_| {
            chat_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "chat session unavailable",
            )
        })?;
    Ok((
        StatusCode::CREATED,
        Json(ShellChatResponse {
            conversation_id: session.id.clone(),
            session_id: session.id,
            created: true,
            is_live: false,
        }),
    ))
}

/// GET /api/sessions/:id/chat — validate server mode before selected hydration.
pub async fn open_chat_session(
    _origin: SameOrigin,
    _bind: LoopbackBind,
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<Json<ShellChatResponse>, (StatusCode, Json<HttpErrorResponse>)> {
    if !gateway_execution::session_details::safe_id(&session_id) {
        return Err(chat_error(
            StatusCode::BAD_REQUEST,
            "invalid session identifier",
        ));
    }
    let selected = state
        .state_service()
        .get_session_with_executions(&session_id)
        .map_err(|_| {
            chat_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "chat session unavailable",
            )
        })?
        .ok_or_else(|| chat_error(StatusCode::NOT_FOUND, "chat session not found"))?;
    let session = selected.session;
    if !matches!(session.mode.as_deref(), Some("fast" | "chat"))
        || session.parent_session_id.is_some()
        || session.root_agent_id != "root"
    {
        return Err(chat_error(StatusCode::CONFLICT, "not a root chat session"));
    }
    let is_live = selected
        .executions
        .iter()
        .any(|execution| !execution.status.is_terminal());
    let canonical_key = session
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.get("shell_chat_conversation_id"))
        .and_then(serde_json::Value::as_str)
        .filter(|key| *key == session_id);
    let conversation_id = if canonical_key.is_some() {
        session_id.clone()
    } else {
        let settings = state.settings().get_execution_settings().map_err(|_| {
            chat_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "chat session unavailable",
            )
        })?;
        let reserved_key = settings.chat.conversation_id.filter(|key| {
            settings.chat.session_id.as_deref() == Some(session_id.as_str())
                && gateway_execution::session_details::safe_id(key)
        });
        match reserved_key {
            Some(key) => key,
            None if !is_live => session_id.clone(),
            None => {
                return Err(chat_error(
                    StatusCode::CONFLICT,
                    "active chat routing unavailable",
                ))
            }
        }
    };
    Ok(Json(ShellChatResponse {
        session_id,
        conversation_id,
        created: false,
        is_live,
    }))
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
/// Returns (or creates) the persistent chat session. Idempotent and
/// self-healing: if the cached session id in settings points at a row
/// that no longer exists in the database (orphaned slot), we rebuild
/// both the session row and the cached ids. Fresh installs create a new
/// session on first call.
pub async fn init_chat_session(
    State(state): State<AppState>,
) -> Result<Json<ChatInitResponse>, (StatusCode, String)> {
    let _guard = chat_session_lock().lock().await;

    let settings = state
        .settings()
        .get_execution_settings()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let runtime = state.runtime();
    let runner = runtime.runner().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "Runtime not available".to_string(),
        )
    })?;
    let state_service = runner.state_service();

    // Reuse the cached session only when its DB row actually exists.
    if let (Some(session_id), Some(conv_id)) =
        (&settings.chat.session_id, &settings.chat.conversation_id)
    {
        let row_present = matches!(state_service.get_session(session_id), Ok(Some(_)));
        if row_present {
            return Ok(Json(ChatInitResponse {
                session_id: session_id.clone(),
                conversation_id: conv_id.clone(),
                created: false,
            }));
        }
        tracing::warn!(
            "chat session {} cached in settings but missing in DB — rebuilding",
            session_id
        );
    }

    let session_id = format!("sess-chat-{}", uuid::Uuid::new_v4());
    let conversation_id = format!("chat-{}", uuid::Uuid::new_v4());

    let mut session =
        execution_state::Session::new_with_source("root", execution_state::TriggerSource::Web);
    session.id = session_id.clone();
    state_service.create_session_from(&session).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to create chat session: {}", e),
        )
    })?;
    state_service
        .set_session_mode(&session_id, "fast")
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to set chat mode: {}", e),
            )
        })?;

    let mut updated_settings = settings.clone();
    updated_settings.chat = gateway_services::ChatConfig {
        session_id: Some(session_id.clone()),
        conversation_id: Some(conversation_id.clone()),
    };
    state
        .settings()
        .update_execution_settings(updated_settings)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(ChatInitResponse {
        session_id,
        conversation_id,
        created: true,
    }))
}

/// DELETE /api/chat/session
///
/// Hard-deletes the current reserved chat session, every descendant
/// subagent session it spawned, and all per-session data for that
/// subtree, then clears the `settings.chat` slot. Memory facts and
/// the knowledge graph are preserved — only the conversation rows
/// disappear. The next call to `POST /api/chat/init` self-heals into
/// a fresh session.
pub async fn clear_chat_session(
    State(state): State<AppState>,
) -> Result<StatusCode, (StatusCode, String)> {
    let _guard = chat_session_lock().lock().await;

    let settings = state
        .settings()
        .get_execution_settings()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    // No-op if the slot is already empty — still return 204 for idempotency.
    if settings.chat.session_id.is_none() && settings.chat.conversation_id.is_none() {
        return Ok(StatusCode::NO_CONTENT);
    }

    // Cascade-delete the subtree before clearing the slot. If the cascade
    // fails, leave the slot intact so the next clear can retry — partial
    // success (slot cleared but rows still in DB) would be worse.
    if let Some(session_id) = settings.chat.session_id.clone() {
        let runtime = state.runtime();
        let runner = runtime.runner().ok_or_else(|| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                "Runtime not available".to_string(),
            )
        })?;
        let state_service = runner.state_service();
        let rows = state_service
            .delete_session_recursive_cascade(&session_id)
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to delete chat session subtree: {}", e),
                )
            })?;
        tracing::info!(session_id = %session_id, rows, "cleared chat session subtree");
    }

    let mut updated = settings.clone();
    updated.chat = gateway_services::ChatConfig::default();
    state
        .settings()
        .update_execution_settings(updated)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(StatusCode::NO_CONTENT)
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
        .messages()
        .replay(&session_id, None, limit as usize)
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
            tool_results: None,
            timestamp: m.created_at,
        })
        .collect();

    Ok(Json(response))
}
