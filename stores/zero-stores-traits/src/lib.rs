//! `zero-stores-traits` — dependency-light home for store traits.
//!
//! This crate holds the trait surface that consumers deep in the dep graph
//! (e.g. `agent-tools`) need to call without inheriting the full
//! `zero-stores` transitive dependency on `knowledge-graph` (which loops
//! back to `agent-tools` via `gateway-database -> agent-runtime`).
//!
//! Re-exported from `zero-stores` for the design-canonical
//! `zero_stores::*` import paths.

pub mod conversation;
pub mod episodes;
pub mod memory_facts;
pub mod outbox;
pub mod procedures;
pub mod wiki;

pub use conversation::ConversationStore;
pub use episodes::{EpisodeStats, EpisodeStore};
pub use memory_facts::{MemoryAggregateStats, MemoryFactStore, MemoryHealthMetrics, SkillIndexRow};
pub use outbox::OutboxStore;
pub use procedures::{ProcedureStats, ProcedureStore};
pub use wiki::{WikiStats, WikiStore};
