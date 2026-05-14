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
//! - [`IntentProfiles`] — partial [`RecallConfig`] overlays keyed by intent
//!   label, applied via the same [`crate::deep_merge`] function used by
//!   [`RecallConfig::load_from_path`].
//!
//! See `memory-bank/future-state/2026-05-13-memory-backlog.md` (MEM-008) for
//! the design rationale and exemplar/profile taxonomy.

use std::collections::HashMap;
use std::sync::Arc;

use agent_runtime::llm::embedding::EmbeddingClient;
use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::OnceCell;

use crate::recall::cosine_similarity;
use crate::{deep_merge, RecallConfig};

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
/// At construction time, raw exemplar utterances are stored. On the first
/// `classify()` call, the entire bank is batch-embedded once and cached
/// via a [`tokio::sync::OnceCell`] — subsequent calls reuse the cached
/// bank, so exemplars are never re-embedded on the recall hot path. This
/// matches `FastembedReranker`'s lazy-load-on-first-call pattern and
/// keeps the constructor sync so it slots into `AppState::new`.
///
/// At query time: embed the query, cosine against every bank entry, take
/// the top-K nearest, vote by label. If the top-1 cosine is below
/// `confidence_threshold`, return `None`.
pub struct KnnIntentClassifier {
    embedding_client: Arc<dyn EmbeddingClient>,
    /// Raw exemplars: flat parallel vectors so the bank-build call sees a
    /// single batched `embed(&[&str])`.
    raw_labels: Vec<String>,
    raw_texts: Vec<String>,
    /// Pre-embedded bank, lazily initialized on first `classify` call.
    /// `Some(vec)` means "ready"; `None` (never set) is impossible after
    /// `ensure_bank` returns. An empty inner vec means "no exemplars
    /// configured — every classify returns `None`".
    bank: OnceCell<Vec<(String, Vec<f32>)>>,
    /// Top-K vote depth. Default 5 (passed from [`crate::IntentRouterConfig`]).
    k: usize,
    /// Minimum cosine similarity to the nearest exemplar required for a
    /// confident classification. Below this, [`Self::classify`] returns
    /// `None`. Default 0.55.
    confidence_threshold: f64,
}

impl KnnIntentClassifier {
    /// Build a new classifier. Construction is cheap and sync — the bank
    /// is embedded lazily on the first `classify()` call (and cached for
    /// the lifetime of the classifier).
    ///
    /// An empty `exemplars` map yields an empty bank, which is valid —
    /// every subsequent `classify` call returns `None`.
    pub fn new(
        embedding_client: Arc<dyn EmbeddingClient>,
        exemplars: HashMap<String, Vec<String>>,
        k: usize,
        confidence_threshold: f64,
    ) -> Self {
        let mut raw_labels: Vec<String> = Vec::new();
        let mut raw_texts: Vec<String> = Vec::new();
        for (intent, utterances) in exemplars {
            for utt in utterances {
                raw_labels.push(intent.clone());
                raw_texts.push(utt);
            }
        }
        Self {
            embedding_client,
            raw_labels,
            raw_texts,
            bank: OnceCell::new(),
            k,
            confidence_threshold,
        }
    }

    /// Get-or-initialize the embedded bank. Returns a reference to the
    /// shared bank vec. On embed failure, returns an empty bank (cached
    /// forever — subsequent calls won't retry; the recall pipeline keeps
    /// running, just without routing).
    async fn ensure_bank(&self) -> &Vec<(String, Vec<f32>)> {
        self.bank
            .get_or_init(|| async {
                if self.raw_texts.is_empty() {
                    return Vec::new();
                }
                let text_refs: Vec<&str> = self.raw_texts.iter().map(|s| s.as_str()).collect();
                match self.embedding_client.embed(&text_refs).await {
                    Ok(embs) if embs.len() == self.raw_labels.len() => self
                        .raw_labels
                        .iter()
                        .cloned()
                        .zip(embs)
                        .collect::<Vec<_>>(),
                    Ok(embs) => {
                        tracing::warn!(
                            "Intent classifier bank build: {} embeddings for {} exemplars — \
                             router disabled",
                            embs.len(),
                            self.raw_labels.len()
                        );
                        Vec::new()
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Intent classifier bank build failed: {e} — router disabled"
                        );
                        Vec::new()
                    }
                }
            })
            .await
    }

    /// Size of the configured exemplar list (before embedding). Exposed
    /// for startup logging — the real bank size after embed only matches
    /// this when the embed call succeeded.
    pub fn exemplar_count(&self) -> usize {
        self.raw_texts.len()
    }
}

#[async_trait]
impl IntentClassifier for KnnIntentClassifier {
    async fn classify(&self, query: &str) -> Option<String> {
        let bank = self.ensure_bank().await;
        if bank.is_empty() {
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
        let mut scored: Vec<(usize, f64)> = bank
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
            let label = bank[idx].0.as_str();
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

/// Per-intent recall config overlays.
///
/// Each entry maps an intent label (e.g. `"factoid-lookup"`) to a partial
/// JSON object that gets deep-merged onto the base [`RecallConfig`] when
/// the classifier returns that intent. Unknown intents return the base
/// config unchanged.
///
/// The overlay JSON has the same shape as `recall_config.json` itself —
/// any subset of fields is valid, and nested objects merge by key (see
/// [`crate::deep_merge`]).
pub struct IntentProfiles {
    overrides: HashMap<String, Value>,
}

impl IntentProfiles {
    /// Build profiles from a parsed JSON value. The value must be an object
    /// whose keys are intent labels and whose values are partial
    /// [`RecallConfig`] overlays.
    ///
    /// Non-object input yields an empty profile bank — every intent falls
    /// back to the base config.
    pub fn from_json(value: Value) -> Self {
        let overrides = match value {
            Value::Object(map) => map.into_iter().collect(),
            _ => HashMap::new(),
        };
        Self { overrides }
    }

    /// Build profiles from an explicit map (used by tests).
    pub fn from_map(overrides: HashMap<String, Value>) -> Self {
        Self { overrides }
    }

    /// Number of intent profiles registered. Exposed for tests + logging.
    pub fn len(&self) -> usize {
        self.overrides.len()
    }

    /// True when no profiles are registered.
    pub fn is_empty(&self) -> bool {
        self.overrides.is_empty()
    }

    /// Apply the overlay for `intent` on top of `base`.
    ///
    /// If `intent` is not in the bank, returns a clone of `base`. If the
    /// merged JSON can't deserialize back into [`RecallConfig`] (e.g.
    /// because the overlay has malformed fields), logs a warning and
    /// returns `base` unchanged.
    pub fn apply(&self, base: &RecallConfig, intent: &str) -> RecallConfig {
        let Some(overlay) = self.overrides.get(intent) else {
            return base.clone();
        };

        let base_v = match serde_json::to_value(base) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("IntentProfiles::apply: base config failed to serialize: {e}");
                return base.clone();
            }
        };
        let merged = deep_merge(base_v, overlay.clone());

        match serde_json::from_value(merged) {
            Ok(cfg) => cfg,
            Err(e) => {
                tracing::warn!(
                    "IntentProfiles::apply: failed to deserialize merged config for intent \
                     '{intent}': {e} — falling back to base"
                );
                base.clone()
            }
        }
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
    async fn knn_lazily_embeds_all_exemplars_in_single_batch() {
        // Bank build is lazy: until the first classify(), no embed runs.
        // Subsequent classify calls reuse the cached bank — only the
        // query gets embedded.
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

        let classifier = KnnIntentClassifier::new(client.clone(), exemplars, 5, 0.55);
        assert_eq!(classifier.exemplar_count(), 12);

        // Construction itself triggers zero embed calls.
        assert_eq!(
            client.calls.load(Ordering::SeqCst),
            0,
            "construction is sync and does not embed"
        );

        // First classify triggers the bank build (1 batch with 12 texts)
        // plus the query embed (1 batch with 1 text) = 2 calls, 13 texts.
        let _ = classifier.classify("query").await;
        assert_eq!(client.calls.load(Ordering::SeqCst), 2);
        assert_eq!(client.texts_seen.load(Ordering::SeqCst), 13);

        // Second classify reuses the cached bank — only the query is
        // re-embedded.
        let _ = classifier.classify("another").await;
        assert_eq!(
            client.calls.load(Ordering::SeqCst),
            3,
            "bank not re-built on subsequent calls"
        );
        assert_eq!(client.texts_seen.load(Ordering::SeqCst), 14);
    }

    #[tokio::test]
    async fn knn_constructor_empty_exemplars_yields_empty_bank() {
        let client = Arc::new(CountingEmbeddingClient::new(vec![1.0, 0.0]));
        let classifier = KnnIntentClassifier::new(client.clone(), HashMap::new(), 5, 0.55);

        assert_eq!(classifier.exemplar_count(), 0);
        // classify short-circuits: empty bank means no query embed either.
        assert_eq!(classifier.classify("anything").await, None);
        assert_eq!(client.calls.load(Ordering::SeqCst), 0);
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

        let classifier = KnnIntentClassifier::new(client, exemplars, 3, 0.55);
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

        let classifier = KnnIntentClassifier::new(client, exemplars, 3, 0.55);
        // "stuff" contains no "factoid" keyword → query embedding falls to default.
        assert_eq!(classifier.classify("stuff").await, None);
    }

    // --- IntentProfiles ---------------------------------------------------

    #[test]
    fn profiles_apply_overlays_category_weights() {
        let base = RecallConfig::default();
        assert_eq!(base.category_weight("correction"), 1.5);

        let overlay = serde_json::json!({
            "correction-recall": {
                "category_weights": { "correction": 2.5 }
            }
        });
        let profiles = IntentProfiles::from_json(overlay);

        let effective = profiles.apply(&base, "correction-recall");
        assert_eq!(effective.category_weight("correction"), 2.5);
        // Other weights preserved.
        assert_eq!(effective.category_weight("strategy"), 1.4);
        assert_eq!(effective.category_weight("schema"), 1.6);
    }

    #[test]
    fn profiles_apply_unknown_intent_returns_base() {
        let base = RecallConfig::default();
        let overlay = serde_json::json!({
            "factoid-lookup": { "max_facts": 5 }
        });
        let profiles = IntentProfiles::from_json(overlay);

        let effective = profiles.apply(&base, "made-up-intent");
        assert_eq!(effective.max_facts, base.max_facts);
        assert_eq!(effective.category_weight("correction"), 1.5);
    }

    #[test]
    fn profiles_apply_partial_overlay_preserves_other_fields() {
        let base = RecallConfig::default();
        let overlay = serde_json::json!({
            "factoid-lookup": {
                "category_weights": { "correction": 0.8 }
            }
        });
        let profiles = IntentProfiles::from_json(overlay);

        let effective = profiles.apply(&base, "factoid-lookup");
        // Overlay applied.
        assert_eq!(effective.category_weight("correction"), 0.8);
        // Unrelated fields preserved.
        assert_eq!(effective.max_facts, base.max_facts);
        assert_eq!(effective.vector_weight, base.vector_weight);
        assert_eq!(effective.bm25_weight, base.bm25_weight);
        assert_eq!(effective.min_score, base.min_score);
    }

    #[test]
    fn profiles_from_non_object_yields_empty_bank() {
        let profiles = IntentProfiles::from_json(serde_json::json!([1, 2, 3]));
        assert!(profiles.is_empty());

        let base = RecallConfig::default();
        // Any intent falls through to base.
        let effective = profiles.apply(&base, "anything");
        assert_eq!(effective.max_facts, base.max_facts);
    }

    #[test]
    fn profiles_overlay_deeply_nested_graph_traversal() {
        let base = RecallConfig::default();
        let overlay = serde_json::json!({
            "code-help": {
                "graph_traversal": { "max_hops": 4 }
            }
        });
        let profiles = IntentProfiles::from_json(overlay);

        let effective = profiles.apply(&base, "code-help");
        assert_eq!(effective.graph_traversal.max_hops, 4);
        // Sibling fields preserved.
        assert_eq!(
            effective.graph_traversal.enabled,
            base.graph_traversal.enabled
        );
        assert_eq!(
            effective.graph_traversal.hop_decay,
            base.graph_traversal.hop_decay
        );
    }
}
