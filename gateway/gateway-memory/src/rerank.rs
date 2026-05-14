//! Cross-encoder reranking. Trait-routed so the production fastembed
//! impl is one of several possible backends (others may be added later,
//! e.g. remote API rerankers like Cohere or Jina).
//!
//! The trait is intentionally infallible: a reranker that fails (model
//! load error, inference panic, network drop) must fall back to
//! identity behavior — the recall pipeline must never break because
//! reranking went wrong. Failure should be logged at warn level and
//! the input returned unchanged.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

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

/// Production cross-encoder reranker backed by fastembed-rs (ONNX).
///
/// Lazy-loads the model on first call. Both model download/load
/// failures and inference errors log a warning and fall back to
/// returning candidates unchanged — recall never fails hard because of
/// reranking.
///
/// The model and tokenizer hold thread-bound state inside fastembed's
/// `TextRerank` (an `ort::Session`), so the wrapped instance is held
/// behind a `Mutex` and the inference call runs on `spawn_blocking`.
pub struct FastembedReranker {
    model_id: String,
    candidate_pool: usize,
    top_k_after: usize,
    score_threshold: f64,
    cache_dir: PathBuf,
    /// Lazily-initialized fastembed reranker model. Wrapped in an
    /// `Arc<Mutex<...>>` so blocking inference tasks can take a short
    /// lock without holding the outer service mutex.
    model: Mutex<Option<Arc<Mutex<fastembed::TextRerank>>>>,
}

impl FastembedReranker {
    /// Construct a new reranker. Cheap — does NOT load the ONNX model.
    /// The model loads on the first `rerank()` call.
    pub fn new(
        model_id: impl Into<String>,
        candidate_pool: usize,
        top_k_after: usize,
        score_threshold: f64,
        cache_dir: PathBuf,
    ) -> Self {
        Self {
            model_id: model_id.into(),
            candidate_pool,
            top_k_after,
            score_threshold,
            cache_dir,
            model: Mutex::new(None),
        }
    }

    /// Get or initialize the model. Returns `None` if model parsing or
    /// load fails. First call may take several seconds (model download).
    fn get_or_init_model(&self) -> Option<Arc<Mutex<fastembed::TextRerank>>> {
        // Hold the outer mutex only for the duration of the
        // load — concurrent first-callers will serialize through here,
        // which is fine; only one process-wide model load is wanted.
        let mut guard = match self.model.lock() {
            Ok(g) => g,
            Err(poisoned) => {
                tracing::warn!("FastembedReranker model mutex poisoned — recovering");
                poisoned.into_inner()
            }
        };
        if let Some(m) = guard.as_ref() {
            return Some(m.clone());
        }

        let model_enum: fastembed::RerankerModel = match self.model_id.parse() {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(
                    model_id = %self.model_id,
                    error = %e,
                    "Unknown reranker model — falling back to identity"
                );
                return None;
            }
        };

        let init_opts = fastembed::RerankInitOptions::new(model_enum)
            .with_cache_dir(self.cache_dir.clone())
            .with_show_download_progress(false);

        match fastembed::TextRerank::try_new(init_opts) {
            Ok(model) => {
                tracing::info!(
                    model_id = %self.model_id,
                    cache_dir = %self.cache_dir.display(),
                    "Cross-encoder reranker model loaded"
                );
                let arc = Arc::new(Mutex::new(model));
                *guard = Some(arc.clone());
                Some(arc)
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    model_id = %self.model_id,
                    "fastembed reranker load failed — falling back to identity"
                );
                None
            }
        }
    }
}

#[async_trait]
impl CrossEncoderReranker for FastembedReranker {
    async fn rerank(&self, query: &str, candidates: Vec<ScoredFact>) -> Vec<ScoredFact> {
        if candidates.is_empty() {
            return candidates;
        }

        // 1. Truncate to candidate_pool before invoking the model.
        let pool_size = self.candidate_pool.min(candidates.len());
        let mut pool: Vec<ScoredFact> = candidates;
        pool.truncate(pool_size);

        // 2. Get or lazy-init the model. If it fails, return the pool
        // unchanged so the caller still sees the (already top-scored)
        // candidates.
        let model = match self.get_or_init_model() {
            Some(m) => m,
            None => return pool,
        };

        // 3. Run inference on a blocking thread. Fastembed's `rerank`
        // is CPU-bound (ONNX inference + tokenization with rayon).
        let documents: Vec<String> = pool.iter().map(|sf| sf.fact.content.clone()).collect();
        let query_owned = query.to_string();

        let inference_result = tokio::task::spawn_blocking(move || {
            let guard = match model.lock() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            // batch_size = None lets fastembed use its default.
            // return_documents = false — we don't need the docs back,
            // we map by index onto `pool`.
            guard.rerank(query_owned, documents, false, None)
        })
        .await;

        let results = match inference_result {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => {
                tracing::warn!(
                    error = %e,
                    "fastembed rerank inference failed — falling back to identity"
                );
                return pool;
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "fastembed rerank blocking task panicked — falling back to identity"
                );
                return pool;
            }
        };

        // 4. Map scores back onto pool by index, then sort + filter +
        // truncate to `top_k_after`. fastembed returns the list already
        // sorted descending by score, so we walk it in order.
        let mut out: Vec<ScoredFact> = Vec::with_capacity(results.len().min(self.top_k_after));
        for r in results {
            if (r.score as f64) < self.score_threshold {
                continue;
            }
            if r.index >= pool.len() {
                continue;
            }
            let mut item = pool[r.index].clone();
            item.score = r.score as f64;
            out.push(item);
            if out.len() >= self.top_k_after {
                break;
            }
        }
        out
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

    /// Test-only reranker that scores candidates by counting how many
    /// whitespace-separated query tokens appear in the candidate
    /// content (case-insensitive). Higher token-match count → earlier
    /// in the output.
    struct MockKeywordReranker;

    #[async_trait]
    impl CrossEncoderReranker for MockKeywordReranker {
        async fn rerank(
            &self,
            query: &str,
            mut candidates: Vec<ScoredFact>,
        ) -> Vec<ScoredFact> {
            let q = query.to_lowercase();
            let tokens: Vec<&str> = q.split_whitespace().collect();
            candidates.sort_by(|a, b| {
                let count = |c: &ScoredFact| {
                    let lower = c.fact.content.to_lowercase();
                    tokens.iter().filter(|t| lower.contains(*t)).count()
                };
                count(b).cmp(&count(a))
            });
            candidates
        }
    }

    #[tokio::test]
    async fn mock_reranker_reorders_by_keyword_match() {
        let reranker = MockKeywordReranker;
        // Pre-rerank order by base score: a (0.9) > b (0.7) > c (0.5).
        // Query "rust trait" should push b and c (both keyword-matching)
        // above a (no match).
        let candidates = vec![
            mk_scored("a", 0.9, "completely unrelated content"),
            mk_scored("b", 0.7, "contains rust keyword"),
            mk_scored("c", 0.5, "also has rust and trait mentions"),
        ];
        let output = reranker.rerank("rust trait", candidates).await;
        assert_eq!(output.len(), 3, "no candidates dropped");
        // c matches both tokens (rust + trait) → highest position
        assert_eq!(output[0].fact.id, "c", "c (2 token matches) should top");
        assert_eq!(output[1].fact.id, "b", "b (1 token match) second");
        assert_eq!(output[2].fact.id, "a", "a (no match) last");
    }

    #[tokio::test]
    async fn mock_reranker_empty_input_returns_empty() {
        let reranker = MockKeywordReranker;
        let output = reranker.rerank("anything", Vec::new()).await;
        assert!(output.is_empty(), "empty input → empty output");
    }
}
