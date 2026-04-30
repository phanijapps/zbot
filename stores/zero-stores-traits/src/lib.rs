//! `zero-stores-traits` — dependency-light home for store traits.
//!
//! This crate holds the trait surface that consumers deep in the dep graph
//! (e.g. `agent-tools`) need to call without inheriting the full
//! `zero-stores` transitive dependency on `knowledge-graph` (which loops
//! back to `agent-tools` via `gateway-database -> agent-runtime`).
//!
//! Re-exported from `zero-stores` for the design-canonical
//! `zero_stores::*` import paths.

pub mod auxiliary;
pub mod compaction;
pub mod conversation;
pub mod episodes;
pub mod kg_episodes;
pub mod memory_facts;
pub mod outbox;
pub mod procedures;
pub mod wiki;

pub use auxiliary::{DistillationStore, GoalStore, RecallLogStore};
pub use compaction::{CompactionRunSummary, CompactionStore};
pub use conversation::ConversationStore;
pub use episodes::{EpisodeStats, EpisodeStore, SessionEpisode, SuccessfulEpisode};
pub use kg_episodes::{KgEpisodeStatusCounts, KgEpisodeStore};
pub use memory_facts::{
    MemoryAggregateStats, MemoryFactStore, MemoryHealthMetrics, SkillIndexRow, StrategyFactInsert,
    StrategyFactMatch,
};
pub use outbox::OutboxStore;
pub use procedures::{PatternProcedureInsert, ProcedureStats, ProcedureStore, ProcedureSummary};
pub use wiki::{WikiStats, WikiStore};
