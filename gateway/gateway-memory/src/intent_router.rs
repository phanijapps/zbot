//! Semantic intent router (MEM-008).
//!
//! kNN-based query intent classifier (Aurelio Semantic Router pattern) that
//! picks an intent label per query, plus a per-intent profile system that
//! deep-merges partial [`RecallConfig`] overlays on top of the base config.
//!
//! The classifier is intentionally infallible: when an embedding call fails
//! or the bank is empty, a production impl returns `None` and the recall
//! pipeline silently falls back to the base config. Per-query routing must
//! never break recall.
//!
//! ## Composition
//!
//! - [`IntentClassifier`] — trait, swappable backends.
//! - [`IdentityClassifier`] — no-op default returning `None` for every query.
//! - `KnnIntentClassifier` — production impl (added in a later commit).
//! - `IntentProfiles` — partial [`RecallConfig`] overlays keyed by intent
//!   label (added in a later commit).
//!
//! See `memory-bank/future-state/2026-05-13-memory-backlog.md` (MEM-008) for
//! the design rationale and exemplar/profile taxonomy.

use async_trait::async_trait;

/// Classifies a query into a (possibly-absent) intent label.
///
/// Implementations must be cheap to call on the hot path (target sub-100 ms
/// for the production kNN impl). Returning `None` means "no confident intent —
/// fall back to the base [`crate::RecallConfig`]"; the string is opaque to
/// the recall pipeline and is only used as a lookup key into the per-intent
/// profile bank.
#[async_trait]
pub trait IntentClassifier: Send + Sync {
    /// Classify the query. Return `None` to fall back to the default
    /// [`crate::RecallConfig`].
    async fn classify(&self, query: &str) -> Option<String>;
}

/// No-op classifier — returns `None` for every query. Used as the default
/// when intent routing is disabled or when exemplar JSON is missing.
pub struct IdentityClassifier;

#[async_trait]
impl IntentClassifier for IdentityClassifier {
    async fn classify(&self, _query: &str) -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn identity_classifier_returns_none() {
        let c = IdentityClassifier;
        assert_eq!(c.classify("anything").await, None);
        assert_eq!(c.classify("").await, None);
    }
}
