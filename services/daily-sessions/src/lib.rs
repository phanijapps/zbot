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
// ============================================================================
// DAILY SESSIONS
// Daily session management for Agent Channel architecture
// ============================================================================

pub mod cache;
pub mod manager;
pub mod repository;
pub mod types;

pub use cache::*;
pub use manager::*;
pub use repository::*;
pub use types::*;

// Result type for this crate
pub type Result<T> = std::result::Result<T, DailySessionError>;
