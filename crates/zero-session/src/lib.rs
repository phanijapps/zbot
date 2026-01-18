//! # Zero Session
//!
//! Session and state management for the Zero framework.

pub mod state;
pub mod session;
pub mod service;

// Re-export from zero-core
pub use zero_core::context::{Session, State};

// Re-export from our modules
pub use state::InMemoryState;
pub use state::validate_key;
pub use session::{InMemorySession, MutexSession};
pub use service::{SessionService, InMemorySessionService};
