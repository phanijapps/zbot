//! # Execution State
//!
//! Session state tracking, token metrics, and checkpointing for agent executions.
//!
//! This crate provides:
//! - Session lifecycle management (queued, running, paused, crashed, cancelled, completed)
//! - Token consumption tracking (input/output tokens per session)
//! - Checkpointing for crash recovery and resume functionality
//! - HTTP handlers for REST API
//!
//! ## Usage
//!
//! Gateway integrates this crate by:
//! 1. Implementing `StateDbProvider` trait with its DatabaseManager
//! 2. Creating a `StateService` instance
//! 3. Mounting routes with `routes(service)`
//! 4. Calling lifecycle methods from the execution runner
//!
//! ```ignore
//! // In gateway
//! use execution_state::{StateService, StateDbProvider, routes};
//!
//! impl StateDbProvider for DatabaseManager { ... }
//!
//! let state_service = Arc::new(StateService::new(db.clone()));
//!
//! // Mount routes
//! router.nest("/api/executions", routes(state_service.clone()));
//!
//! // During execution
//! let session = state_service.create_session(conv_id, agent_id, None)?;
//! state_service.start_session(&session.id)?;
//! state_service.update_tokens(&session.id, tokens_in, tokens_out)?;
//! state_service.complete_session(&session.id)?;
//! ```

mod handlers;
mod repository;
mod service;
mod types;

// Re-export public types
pub use repository::StateDbProvider;
pub use service::StateService;
pub use types::*;

use axum::{routing::delete, routing::get, routing::post, Router};
use std::sync::Arc;

/// Create the execution state API router.
///
/// Mount this at `/api/executions` in the gateway.
///
/// # Endpoints
///
/// ## Sessions
/// - `GET /sessions` - List sessions with optional filtering
/// - `GET /sessions/:id` - Get session detail
/// - `GET /sessions/:id/children` - Get child sessions
/// - `DELETE /sessions/:id` - Delete a session
///
/// ## Control
/// - `POST /sessions/:id/pause` - Pause a running session
/// - `POST /sessions/:id/resume` - Resume a paused/crashed session
/// - `POST /sessions/:id/cancel` - Cancel a session
///
/// ## Stats
/// - `GET /stats/counts` - Get session counts by status
/// - `GET /stats/daily/:date` - Get daily summary (YYYY-MM-DD)
///
/// ## Convenience
/// - `GET /running` - Get currently running sessions
/// - `GET /resumable` - Get resumable sessions (paused/crashed)
///
/// ## Cleanup
/// - `DELETE /cleanup?older_than=<timestamp>` - Cleanup old sessions
pub fn routes<D>(service: Arc<StateService<D>>) -> Router
where
    D: StateDbProvider + 'static,
{
    Router::new()
        // Session CRUD
        .route("/sessions", get(handlers::list_sessions::<D>))
        .route(
            "/sessions/{id}",
            get(handlers::get_session::<D>).delete(handlers::delete_session::<D>),
        )
        .route("/sessions/{id}/children", get(handlers::get_children::<D>))
        // Control
        .route("/sessions/{id}/pause", post(handlers::pause_session::<D>))
        .route("/sessions/{id}/resume", post(handlers::resume_session::<D>))
        .route("/sessions/{id}/cancel", post(handlers::cancel_session::<D>))
        // Stats
        .route("/stats/counts", get(handlers::get_status_counts::<D>))
        .route("/stats/daily/{date}", get(handlers::get_daily_summary::<D>))
        // Convenience
        .route("/running", get(handlers::get_running::<D>))
        .route("/resumable", get(handlers::get_resumable::<D>))
        // Cleanup
        .route("/cleanup", delete(handlers::cleanup_old_sessions::<D>))
        .with_state(service)
}

/// Schema SQL for the execution_sessions table.
///
/// Gateway should execute this during database initialization.
pub const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS execution_sessions (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    parent_session_id TEXT,

    status TEXT NOT NULL DEFAULT 'queued',

    created_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT,

    tokens_in INTEGER DEFAULT 0,
    tokens_out INTEGER DEFAULT 0,

    checkpoint TEXT,
    error TEXT,

    FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE,
    FOREIGN KEY (parent_session_id) REFERENCES execution_sessions(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_sessions_status ON execution_sessions(status);
CREATE INDEX IF NOT EXISTS idx_sessions_conversation ON execution_sessions(conversation_id);
CREATE INDEX IF NOT EXISTS idx_sessions_parent ON execution_sessions(parent_session_id);
CREATE INDEX IF NOT EXISTS idx_sessions_created ON execution_sessions(created_at);
CREATE INDEX IF NOT EXISTS idx_sessions_agent ON execution_sessions(agent_id);
"#;
