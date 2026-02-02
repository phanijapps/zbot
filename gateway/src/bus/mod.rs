//! Gateway Bus - Unified intake interface for all session triggers.
//!
//! The Gateway Bus provides a single abstraction for creating and managing sessions
//! regardless of the trigger source (Web, CLI, Cron, API, Plugin).
//!
//! # Architecture
//!
//! ```text
//!                         TRIGGERS (intake)
//!     ┌───────┬───────┬───────┬──────────────┬─────────────────────────┐
//!     │  Web  │  CLI  │ Cron  │ Rust Plugins │ Foreign Plugins         │
//!     │       │       │       │              │ (JS, Python, Go bridge) │
//!     └───┬───┴───┬───┴───┬───┴───────┬──────┴───────────┬─────────────┘
//!         │       │       │           │                  │
//!         └───────┴───────┴───────────┴──────────────────┘
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
//!
//! # Example Usage
//!
//! ```ignore
//! use gateway::bus::{GatewayBus, SessionRequest};
//!
//! // Submit a new session
//! let request = SessionRequest::new("root", "Hello, agent!")
//!     .with_source(TriggerSource::Web);
//!
//! let handle = bus.submit(request).await?;
//! println!("Session: {}, Execution: {}", handle.session_id, handle.execution_id);
//!
//! // Check status
//! let status = bus.status(&handle.session_id).await?;
//!
//! // Cancel if needed
//! bus.cancel(&handle.session_id).await?;
//! ```

mod types;
mod http_bus;

pub use types::*;
pub use http_bus::HttpGatewayBus;

use async_trait::async_trait;
use execution_state::SessionStatus;

/// The unified intake interface for all session triggers.
///
/// This trait abstracts the mechanics of session creation and management,
/// allowing different trigger sources to use the same interface.
///
/// # Implementations
///
/// - [`HttpGatewayBus`]: Uses the HTTP execution runner (current default)
/// - Future: gRPC, Unix socket, etc.
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
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session ID to check
    ///
    /// # Returns
    ///
    /// The current [`SessionStatus`] of the session.
    async fn status(&self, session_id: &str) -> Result<SessionStatus, BusError>;

    /// Cancel a running session.
    ///
    /// This will stop all running executions within the session.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session ID to cancel
    async fn cancel(&self, session_id: &str) -> Result<(), BusError>;

    /// Pause a running session.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session ID to pause
    async fn pause(&self, session_id: &str) -> Result<(), BusError>;

    /// Resume a paused session.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session ID to resume
    async fn resume(&self, session_id: &str) -> Result<(), BusError>;
}
