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
//! # Gateway Bus
//!
//! Unified intake interface for all session triggers.
//!
//! The Gateway Bus provides a single abstraction for creating and managing sessions
//! regardless of the trigger source (Web, CLI, Cron, API, Connector).
//!
//! # Architecture
//!
//! ```text
//!                         TRIGGERS (intake)
//!     ┌───────┬───────┬───────┬───────┬────────────────────┐
//!     │  Web  │  CLI  │ Cron  │  API  │ External Connectors│
//!     │       │       │       │       │ (HTTP webhooks)    │
//!     └───┬───┴───┬───┴───┬───┴───┬───┴─────────┬──────────┘
//!         │       │       │       │              │
//!         └───────┴───────┴───────┴──────────────┘
//!                               │
//!                               ▼
//!                 ┌───────────────────────────┐
//!                 │    ZERO GATEWAY BUS       │
//!                 │   (unified intake)        │
//!                 └─────────────┬─────────────┘
//!                               │
//!                               ▼
//!                 ┌───────────────────────────┐
//!                 │       ROOT AGENT          │
//!                 │     (does its magic)      │
//!                 └───────────────────────────┘
//! ```

mod types;

pub use types::*;

use async_trait::async_trait;
use execution_state::SessionStatus;

/// The unified intake interface for all session triggers.
///
/// This trait abstracts the mechanics of session creation and management,
/// allowing different trigger sources to use the same interface.
#[async_trait]
pub trait GatewayBus: Send + Sync {
    /// Submit a new session request or continue an existing session.
    ///
    /// # Arguments
    ///
    /// * `request` - The session request containing agent ID, message, and metadata
    ///
    /// # Returns
    ///
    /// A [`SessionHandle`] containing the session ID and execution ID.
    ///
    /// # Session Behavior
    ///
    /// - If `request.session_id` is `None`: Creates a new session
    /// - If `request.session_id` is `Some`: Continues the existing session with a new execution
    async fn submit(&self, request: SessionRequest) -> Result<SessionHandle, BusError>;

    /// Get the current status of a session.
    async fn status(&self, session_id: &str) -> Result<SessionStatus, BusError>;

    /// Cancel a running session.
    async fn cancel(&self, session_id: &str) -> Result<(), BusError>;

    /// Pause a running session.
    async fn pause(&self, session_id: &str) -> Result<(), BusError>;

    /// Resume a paused session.
    async fn resume(&self, session_id: &str) -> Result<(), BusError>;
}
