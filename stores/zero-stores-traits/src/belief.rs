//! `BeliefStore` trait — persistence interface for the Belief Network.
//!
//! Beliefs are aggregates over one or more `MemoryFact`s about a single
//! subject. This trait is dep-light by design so the `agent-tools` crate
//! can call it without dragging in `zero-stores-sqlite`.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use zero_stores_domain::Belief;

/// Abstract interface for durable belief storage.
///
/// Implementations can wrap a SQLite repository, a remote API, or an
/// in-memory store for testing. All methods are async to allow blocking
/// backends to be wrapped on a runtime executor without leaking shape.
#[async_trait]
pub trait BeliefStore: Send + Sync {
    /// Get the active belief for a subject at `as_of` (default: `Utc::now()`).
    /// Returns `None` if no belief exists for this subject.
    async fn get_belief(
        &self,
        partition_id: &str,
        subject: &str,
        as_of: Option<DateTime<Utc>>,
    ) -> Result<Option<Belief>, String>;

    /// List recent beliefs for a partition, ordered by `updated_at`
    /// descending.
    async fn list_beliefs(&self, partition_id: &str, limit: usize) -> Result<Vec<Belief>, String>;

    /// Insert or update a belief. UPSERT keyed on
    /// `(partition_id, subject, valid_from)`.
    async fn upsert_belief(&self, belief: &Belief) -> Result<(), String>;

    /// Mark a belief superseded by another. Mirrors `supersede_fact`
    /// semantics: the old belief's `valid_until` closes at
    /// `transition_time` and its `superseded_by` is set to `new_id`.
    async fn supersede_belief(
        &self,
        old_id: &str,
        new_id: &str,
        transition_time: DateTime<Utc>,
    ) -> Result<(), String>;
}
