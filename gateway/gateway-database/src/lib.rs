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
//! # Gateway Database
//!
//! SQLite connection pool and schema management for the AgentZero gateway.
//!
//! Provides `DatabaseManager` with r2d2 connection pooling, WAL mode,
//! and performance pragmas applied to every connection.

mod connection;
pub mod distillation_repository;
pub mod episode_repository;
pub mod memory_fact_store;
pub mod memory_repository;
pub mod recall_log_repository;
pub mod repository;
mod schema;

pub use connection::DatabaseManager;
pub use distillation_repository::{
    DistillationRepository, DistillationRun, DistillationStats, UndistilledSession,
};
pub use episode_repository::{EpisodeRepository, SessionEpisode};
pub use memory_fact_store::GatewayMemoryFactStore;
pub use memory_repository::{MemoryFact, MemoryRepository, ScoredFact};
pub use recall_log_repository::RecallLogRepository;
pub use repository::{ConversationRepository, Message};
