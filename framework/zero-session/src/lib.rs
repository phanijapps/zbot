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
