//! # Gateway Database
//!
//! SQLite connection pool and schema management for the AgentZero gateway.
//!
//! Provides `DatabaseManager` with r2d2 connection pooling, WAL mode,
//! and performance pragmas applied to every connection.

mod connection;
pub mod repository;
pub mod memory_repository;
pub mod memory_fact_store;
pub mod distillation_repository;
pub mod episode_repository;
mod schema;

pub use connection::DatabaseManager;
pub use repository::{ConversationRepository, Message};
pub use memory_repository::{MemoryRepository, MemoryFact, ScoredFact};
pub use memory_fact_store::GatewayMemoryFactStore;
pub use distillation_repository::{DistillationRepository, DistillationRun, DistillationStats, UndistilledSession};
pub use episode_repository::{EpisodeRepository, SessionEpisode};
