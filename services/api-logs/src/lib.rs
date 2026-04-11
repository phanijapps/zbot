#![allow(clippy::missing_docs_in_private_items)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::module_name_repetitions)]
#![allow(missing_docs)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::fn_params_excessive_bools)]
#![allow(clippy::items_after_statements)]
#![allow(clippy::unnecessary_wraps)]
//! # API Logs
//!
//! Self-contained execution logs API for agent tracing.
//!
//! This crate provides:
//! - Log types (ExecutionLog, LogSession, etc.)
//! - Repository for database operations
//! - Service for business logic and log emission
//! - HTTP handlers for REST API
//!
//! ## Usage
//!
//! Gateway integrates this crate by:
//! 1. Implementing `DbProvider` trait with its DatabaseManager
//! 2. Creating a `LogService` instance
//! 3. Mounting routes with `routes(service)`
//! 4. Calling log emission methods from the execution runner
//!
//! ```ignore
//! // In gateway
//! use api_logs::{LogService, routes, DbProvider};
//!
//! impl DbProvider for DatabaseManager { ... }
//!
//! let log_service = Arc::new(LogService::new(db.clone()));
//!
//! // Mount routes
//! router.nest("/api/logs", routes(log_service.clone()));
//!
//! // Emit logs during execution
//! log_service.log_session_start(session_id, conv_id, agent_id, None);
//! ```

mod handlers;
mod repository;
mod service;
mod types;

// Re-export public types
pub use repository::DbProvider;
pub use service::LogService;
pub use types::*;

use axum::{routing::delete, routing::get, Router};
use std::sync::Arc;

/// Create the logs API router.
///
/// Mount this at `/api/logs` in the gateway.
///
/// # Endpoints
///
/// - `GET /sessions` - List sessions with optional filtering
/// - `GET /sessions/:id` - Get session detail with all logs
/// - `DELETE /sessions/:id` - Delete a session and its logs
/// - `DELETE /cleanup?older_than=<timestamp>` - Cleanup old logs
pub fn routes<D>(service: Arc<LogService<D>>) -> Router
where
    D: DbProvider + 'static,
{
    Router::new()
        .route("/sessions", get(handlers::list_sessions::<D>))
        .route(
            "/sessions/:id",
            get(handlers::get_session::<D>).delete(handlers::delete_session::<D>),
        )
        .route("/cleanup", delete(handlers::cleanup_old_logs::<D>))
        .with_state(service)
}

/// Schema SQL for the execution_logs table.
///
/// Gateway should execute this during database initialization.
pub const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS execution_logs (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    conversation_id TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    parent_session_id TEXT,
    timestamp TEXT NOT NULL,
    level TEXT NOT NULL,
    category TEXT NOT NULL,
    message TEXT NOT NULL,
    metadata TEXT,
    duration_ms INTEGER
);

CREATE INDEX IF NOT EXISTS idx_execution_logs_session_id
    ON execution_logs(session_id);

CREATE INDEX IF NOT EXISTS idx_execution_logs_conversation_id
    ON execution_logs(conversation_id);

CREATE INDEX IF NOT EXISTS idx_execution_logs_agent_id
    ON execution_logs(agent_id);

CREATE INDEX IF NOT EXISTS idx_execution_logs_timestamp
    ON execution_logs(timestamp);

CREATE INDEX IF NOT EXISTS idx_execution_logs_level
    ON execution_logs(level);

CREATE INDEX IF NOT EXISTS idx_execution_logs_parent_session_id
    ON execution_logs(parent_session_id);
"#;
