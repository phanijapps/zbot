//! # Zero Session
//!
//! Session and state management for the Zero framework.

pub mod service;
pub mod session;
pub mod state;

// Re-export from zero-core
pub use zero_core::context::{Session, State};

// Re-export from our modules
pub use service::{InMemorySessionService, SessionService};
pub use session::{InMemorySession, MutexSession};
pub use state::validate_key;
pub use state::InMemoryState;
