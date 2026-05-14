//! Cross-encoder reranking. Trait-routed so the production fastembed
//! impl is one of several possible backends (others may be added later,
//! e.g. remote API rerankers like Cohere or Jina).
//!
//! The trait is intentionally infallible: a reranker that fails (model
//! load error, inference panic, network drop) must fall back to
//! identity behavior — the recall pipeline must never break because
//! reranking went wrong. Failure should be logged at warn level and
//! the input returned unchanged.

use async_trait::async_trait;
use zero_stores_domain::ScoredFact;

/// Cross-encoder reranker that scores `(query, candidate.content)` pairs.
///
/// The default implementation ([`IdentityReranker`]) returns candidates
/// unchanged — used when reranking is disabled or the model failed to
/// load. Production impls (e.g. `FastembedReranker`) replace this.
#[async_trait]
pub trait CrossEncoderReranker: Send + Sync {
    /// Reorder candidates by cross-encoder score.
    ///
    /// Returns a new vector with the same items in a potentially
    /// different order, potentially truncated. Implementations must
    /// never fail — on internal error they should log a warning and
    /// return the input unchanged.
    async fn rerank(&self, query: &str, candidates: Vec<ScoredFact>) -> Vec<ScoredFact>;
}

/// No-op reranker — returns input unchanged. Used as the default when
/// reranking is disabled or as a fallback when the production reranker
/// fails to load.
pub struct IdentityReranker;

#[async_trait]
impl CrossEncoderReranker for IdentityReranker {
    async fn rerank(&self, _query: &str, candidates: Vec<ScoredFact>) -> Vec<ScoredFact> {
        candidates
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zero_stores_domain::MemoryFact;

    fn mk_scored(id: &str, score: f64, content: &str) -> ScoredFact {
        ScoredFact {
            fact: MemoryFact {
                id: id.to_string(),
                session_id: None,
                agent_id: "agent-test".to_string(),
                scope: "agent".to_string(),
                category: "misc".to_string(),
                key: format!("rerank.{id}"),
                content: content.to_string(),
                confidence: 0.9,
                mention_count: 1,
                source_summary: None,
                embedding: None,
                ward_id: "__global__".to_string(),
                contradicted_by: None,
                created_at: String::new(),
                updated_at: String::new(),
                expires_at: None,
                valid_from: None,
                valid_until: None,
                superseded_by: None,
                pinned: false,
                epistemic_class: Some("current".to_string()),
                source_episode_id: None,
                source_ref: None,
            },
            score,
        }
    }

    #[tokio::test]
    async fn identity_reranker_preserves_order() {
        let reranker = IdentityReranker;
        let input = vec![
            mk_scored("a", 1.0, "alpha content"),
            mk_scored("b", 0.5, "beta content"),
            mk_scored("c", 0.2, "gamma content"),
        ];
        let cloned_ids: Vec<String> = input.iter().map(|sf| sf.fact.id.clone()).collect();
        let output = reranker.rerank("query", input).await;
        assert_eq!(output.len(), cloned_ids.len(), "length preserved");
        for (i, o) in cloned_ids.iter().zip(output.iter()) {
            assert_eq!(i, &o.fact.id, "order preserved");
        }
    }

    #[tokio::test]
    async fn identity_reranker_returns_input_unchanged_for_empty() {
        let reranker = IdentityReranker;
        let output = reranker.rerank("query", Vec::new()).await;
        assert!(output.is_empty());
    }
}
