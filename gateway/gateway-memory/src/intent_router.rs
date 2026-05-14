//! Semantic intent router (MEM-008).
//!
//! kNN-based query intent classifier (Aurelio Semantic Router pattern) that
//! picks an intent label per query, plus a per-intent profile system that
//! deep-merges partial [`RecallConfig`] overlays on top of the base config.
//!
//! The classifier is intentionally infallible: when an embedding call fails
//! or the bank is empty, [`KnnIntentClassifier::classify`] returns `None`
//! and the recall pipeline silently falls back to the base config. Per-query
//! routing must never break recall.
//!
//! ## Composition
//!
//! - [`IntentClassifier`] — trait, swappable backends.
//! - [`IdentityClassifier`] — no-op default returning `None` for every query.
//! - [`KnnIntentClassifier`] — production impl: embeds exemplars once at
//!   construction, then per-query embeds the query and does a top-K cosine
//!   vote against the bank.
//! - `IntentProfiles` — partial [`RecallConfig`] overlays keyed by intent
//!   label (added in a later commit).
//!
//! See `memory-bank/future-state/2026-05-13-memory-backlog.md` (MEM-008) for
//! the design rationale and exemplar/profile taxonomy.

use std::collections::HashMap;
use std::sync::Arc;

use agent_runtime::llm::embedding::EmbeddingClient;
use async_trait::async_trait;

use crate::recall::cosine_similarity;

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

/// Production kNN intent classifier (Aurelio Semantic Router pattern).
///
/// At construction time, every exemplar utterance is embedded in one batch
/// call and stored alongside its intent label. At query time: embed the
/// query, cosine against every bank entry, take the top-K nearest, vote by
/// label. If the top-1 cosine is below `confidence_threshold`, return `None`.
///
/// The exemplar bank is built once and held for the lifetime of the
/// classifier — exemplars are never re-embedded on the recall hot path.
pub struct KnnIntentClassifier {
    embedding_client: Arc<dyn EmbeddingClient>,
    /// Pre-embedded exemplars. Each entry: `(intent_label, embedding)`.
    bank: Vec<(String, Vec<f32>)>,
    /// Top-K vote depth. Default 5 (passed from [`crate::IntentRouterConfig`]).
    k: usize,
    /// Minimum cosine similarity to the nearest exemplar required for a
    /// confident classification. Below this, [`Self::classify`] returns
    /// `None`. Default 0.55.
    confidence_threshold: f64,
}

impl KnnIntentClassifier {
    /// Build a new classifier. Batch-embeds every exemplar via the embedding
    /// client and stores `(label, embedding)` pairs internally.
    ///
    /// Returns an error if the embedding call fails or returns the wrong
    /// number of vectors. An empty `exemplars` map yields an empty bank,
    /// which is valid — every subsequent `classify` call returns `None`.
    pub async fn new(
        embedding_client: Arc<dyn EmbeddingClient>,
        exemplars: HashMap<String, Vec<String>>,
        k: usize,
        confidence_threshold: f64,
    ) -> Result<Self, String> {
        // Flatten to two parallel vectors so we can do one batch embed call
        // and zip back to (label, embedding) pairs afterwards.
        let mut labels: Vec<String> = Vec::new();
        let mut texts: Vec<String> = Vec::new();
        for (intent, utterances) in exemplars {
            for utt in utterances {
                labels.push(intent.clone());
                texts.push(utt);
            }
        }

        if texts.is_empty() {
            return Ok(Self {
                embedding_client,
                bank: Vec::new(),
                k,
                confidence_threshold,
            });
        }

        let text_refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
        let embeddings = embedding_client
            .embed(&text_refs)
            .await
            .map_err(|e| format!("Failed to embed intent exemplars: {e}"))?;

        if embeddings.len() != labels.len() {
            return Err(format!(
                "Embedding client returned {} vectors for {} exemplars",
                embeddings.len(),
                labels.len()
            ));
        }

        let bank: Vec<(String, Vec<f32>)> = labels.into_iter().zip(embeddings).collect();

        Ok(Self {
            embedding_client,
            bank,
            k,
            confidence_threshold,
        })
    }

    /// Size of the exemplar bank (number of `(label, embedding)` pairs).
    /// Exposed for tests and logging.
    pub fn bank_size(&self) -> usize {
        self.bank.len()
    }
}

#[async_trait]
impl IntentClassifier for KnnIntentClassifier {
    async fn classify(&self, query: &str) -> Option<String> {
        if self.bank.is_empty() {
            return None;
        }

        // Embed the query. On failure, log + return None (router degrades
        // gracefully — recall continues with base config).
        let query_emb = match self.embedding_client.embed(&[query]).await {
            Ok(mut v) if !v.is_empty() => v.remove(0),
            Ok(_) => {
                tracing::warn!("Intent classifier: embed returned empty vector list");
                return None;
            }
            Err(e) => {
                tracing::warn!("Intent classifier: embed failed — {e}");
                return None;
            }
        };

        // Compute cosine against every bank entry.
        let mut scored: Vec<(usize, f64)> = self
            .bank
            .iter()
            .enumerate()
            .map(|(idx, (_, emb))| (idx, cosine_similarity(&query_emb, emb)))
            .collect();

        // Sort descending by similarity. Stable sort: ties keep first-loaded
        // order, which is fine since ordering only drives top-K windowing.
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Confidence gate on the top-1 similarity.
        let top_sim = scored.first().map(|(_, s)| *s).unwrap_or(0.0);
        if top_sim < self.confidence_threshold {
            return None;
        }

        // Vote among top-K. Tally counts AND sum-of-similarity per label so
        // we can break count ties by total similarity mass rather than
        // arbitrary HashMap iteration order.
        let k = self.k.min(scored.len());
        let mut counts: HashMap<&str, (usize, f64)> = HashMap::new();
        for &(idx, sim) in &scored[..k] {
            let label = self.bank[idx].0.as_str();
            let entry = counts.entry(label).or_insert((0, 0.0));
            entry.0 += 1;
            entry.1 += sim;
        }

        // Winner: highest count, tie-broken by sum-of-similarity.
        counts
            .into_iter()
            .max_by(|a, b| {
                a.1 .0.cmp(&b.1 .0).then_with(|| {
                    a.1 .1
                        .partial_cmp(&b.1 .1)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
            })
            .map(|(label, _)| label.to_string())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use agent_runtime::llm::embedding::EmbeddingError;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // --- Mock embedding clients --------------------------------------------

    /// Counts the number of `embed` calls and the total number of texts seen.
    struct CountingEmbeddingClient {
        calls: AtomicUsize,
        texts_seen: AtomicUsize,
        /// Returned vector per text. Length must match `dimensions`.
        vec: Vec<f32>,
    }

    impl CountingEmbeddingClient {
        fn new(vec: Vec<f32>) -> Self {
            Self {
                calls: AtomicUsize::new(0),
                texts_seen: AtomicUsize::new(0),
                vec,
            }
        }
    }

    #[async_trait]
    impl EmbeddingClient for CountingEmbeddingClient {
        async fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.texts_seen.fetch_add(texts.len(), Ordering::SeqCst);
            Ok(texts.iter().map(|_| self.vec.clone()).collect())
        }

        fn dimensions(&self) -> usize {
            self.vec.len()
        }

        fn model_name(&self) -> String {
            "counting-mock".to_string()
        }
    }

    /// Per-text-keyword embedding client — returns a different vector
    /// depending on a substring match. Lets us simulate a query landing
    /// closer to one intent's exemplars than another.
    struct KeywordEmbeddingClient {
        rules: Vec<(String, Vec<f32>)>,
        default: Vec<f32>,
    }

    #[async_trait]
    impl EmbeddingClient for KeywordEmbeddingClient {
        async fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
            let mut out = Vec::with_capacity(texts.len());
            for &t in texts {
                let mut v = self.default.clone();
                for (needle, emb) in &self.rules {
                    if t.contains(needle.as_str()) {
                        v = emb.clone();
                        break;
                    }
                }
                out.push(v);
            }
            Ok(out)
        }

        fn dimensions(&self) -> usize {
            self.default.len()
        }

        fn model_name(&self) -> String {
            "keyword-mock".to_string()
        }
    }

    // --- Identity classifier ----------------------------------------------

    #[tokio::test]
    async fn identity_classifier_returns_none() {
        let c = IdentityClassifier;
        assert_eq!(c.classify("anything").await, None);
        assert_eq!(c.classify("").await, None);
    }

    // --- KnnIntentClassifier construction ---------------------------------

    #[tokio::test]
    async fn knn_constructor_embeds_all_exemplars() {
        let client = Arc::new(CountingEmbeddingClient::new(vec![1.0, 0.0]));
        let mut exemplars = HashMap::new();
        exemplars.insert(
            "a".to_string(),
            vec!["one".into(), "two".into(), "three".into(), "four".into()],
        );
        exemplars.insert(
            "b".to_string(),
            vec!["one".into(), "two".into(), "three".into(), "four".into()],
        );
        exemplars.insert(
            "c".to_string(),
            vec!["one".into(), "two".into(), "three".into(), "four".into()],
        );

        let classifier = KnnIntentClassifier::new(client.clone(), exemplars, 5, 0.55)
            .await
            .expect("construct");

        assert_eq!(classifier.bank_size(), 12, "12 exemplars in bank");
        assert_eq!(
            client.calls.load(Ordering::SeqCst),
            1,
            "exactly one batch embed call at construction"
        );
        assert_eq!(
            client.texts_seen.load(Ordering::SeqCst),
            12,
            "all 12 exemplars sent in the single batch"
        );
    }

    #[tokio::test]
    async fn knn_constructor_empty_exemplars_yields_empty_bank() {
        let client = Arc::new(CountingEmbeddingClient::new(vec![1.0, 0.0]));
        let classifier = KnnIntentClassifier::new(client.clone(), HashMap::new(), 5, 0.55)
            .await
            .expect("construct");

        assert_eq!(classifier.bank_size(), 0);
        // Empty bank short-circuits: no embed call needed.
        assert_eq!(client.calls.load(Ordering::SeqCst), 0);
        // And classify always returns None.
        assert_eq!(classifier.classify("anything").await, None);
    }

    // --- KnnIntentClassifier::classify ------------------------------------

    #[tokio::test]
    async fn knn_classify_picks_nearest_intent() {
        // Two intents. "factoid"-keyword exemplars + queries embed to [1, 0];
        // "code"-keyword to [0, 1]. Query "factoid lookup query" hits the
        // "factoid" rule → [1, 0]. Cosine vs factoid exemplars = 1.0, vs
        // code exemplars = 0.0. Threshold 0.55 → factoid-lookup wins.
        let rules = vec![
            ("factoid".to_string(), vec![1.0_f32, 0.0]),
            ("code".to_string(), vec![0.0_f32, 1.0]),
        ];
        let client = Arc::new(KeywordEmbeddingClient {
            rules,
            default: vec![0.5, 0.5],
        });

        let mut exemplars = HashMap::new();
        exemplars.insert(
            "factoid-lookup".into(),
            vec![
                "factoid one".into(),
                "factoid two".into(),
                "factoid three".into(),
            ],
        );
        exemplars.insert(
            "code-help".into(),
            vec!["code one".into(), "code two".into(), "code three".into()],
        );

        let classifier = KnnIntentClassifier::new(client, exemplars, 3, 0.55)
            .await
            .unwrap();
        let label = classifier.classify("factoid lookup query").await;
        assert_eq!(label, Some("factoid-lookup".to_string()));
    }

    #[tokio::test]
    async fn knn_classify_below_threshold_returns_none() {
        // Query embedding (default [0, 1]) is orthogonal to the factoid
        // exemplars (which match "factoid" → [1, 0]). Top similarity is 0.0,
        // below threshold 0.55 → None.
        let rules = vec![("factoid".to_string(), vec![1.0_f32, 0.0])];
        let client = Arc::new(KeywordEmbeddingClient {
            rules,
            default: vec![0.0_f32, 1.0],
        });

        let mut exemplars = HashMap::new();
        exemplars.insert(
            "factoid-lookup".into(),
            vec!["factoid one".into(), "factoid two".into()],
        );

        let classifier = KnnIntentClassifier::new(client, exemplars, 3, 0.55)
            .await
            .unwrap();
        // "stuff" contains no "factoid" keyword → query embedding falls to default.
        assert_eq!(classifier.classify("stuff").await, None);
    }
}
