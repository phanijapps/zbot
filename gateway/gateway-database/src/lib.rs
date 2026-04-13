//! # Gateway Database
//!
//! SQLite connection pool and schema management for the AgentZero gateway.
//!
//! Provides `DatabaseManager` with r2d2 connection pooling, WAL mode,
//! and performance pragmas applied to every connection.

pub mod compaction_repository;
mod connection;
pub mod distillation_repository;
pub mod episode_repository;
pub mod goal_repository;
pub mod kg_episode_repository;
pub mod knowledge_db;
pub mod knowledge_schema;
pub mod memory_fact_store;
pub mod memory_repository;
pub mod procedure_repository;
pub mod recall_log_repository;
pub mod repository;
mod schema;
pub mod sqlite_vec_loader;
pub mod wiki_repository;

pub use compaction_repository::{Compaction, CompactionRepository, RunSummary};
pub use connection::DatabaseManager;
pub use distillation_repository::{
    DistillationRepository, DistillationRun, DistillationStats, UndistilledSession,
};
pub use episode_repository::{EpisodeRepository, SessionEpisode};
pub use goal_repository::{Goal, GoalRepository};
pub use kg_episode_repository::{EpisodeSource, KgEpisode, KgEpisodeRepository};
pub use knowledge_db::KnowledgeDatabase;
pub use memory_fact_store::GatewayMemoryFactStore;
pub use memory_repository::{MemoryFact, MemoryRepository, ScoredFact};
pub use procedure_repository::{Procedure, ProcedureRepository};
pub use recall_log_repository::RecallLogRepository;
pub use repository::{ConversationRepository, Message};
pub use wiki_repository::{WardWikiRepository, WikiArticle};

pub mod vector_index;
pub use vector_index::{SqliteVecIndex, VectorIndex};
