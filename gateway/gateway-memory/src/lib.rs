//! Memory subsystem configuration types. Owned by gateway-memory crate;
//! re-exported through gateway-services for backward compat.

pub mod intent_router;
pub mod llm_factory;
pub mod recall;
pub mod rerank;
pub mod services;
pub mod sleep;
pub mod util;

pub use intent_router::{IdentityClassifier, IntentClassifier, KnnIntentClassifier};
pub use llm_factory::{LlmClientConfig, MemoryLlmFactory};
pub use rerank::{CrossEncoderReranker, FastembedReranker, IdentityReranker};
pub use services::{MemoryServices, MemoryServicesConfig};
pub use util::{parse_llm_json, strip_code_fence};

pub use recall::scored_item::{
    intent_boost, rrf_merge, GoalLite, ItemKind, Provenance, ScoredItem,
};
pub use recall::{format_recall_failure_message, format_scored_items, MemoryRecall};
pub use sleep::compactor::{CompactionStats, Compactor, PairwiseVerifier};
pub use sleep::conflict_resolver::{
    ConflictJudgeLlm, ConflictResolver, ConflictResponse, ConflictStats,
};
pub use sleep::corrections_abstractor::{
    AbstractionLlm, AbstractionResponse, AbstractionStats, CorrectionsAbstractor,
};
pub use sleep::decay::{DecayConfig, DecayEngine, KgDecayStats, PruneCandidate};
pub use sleep::handoff_writer::{
    read_handoff_block, should_inject, HandoffEntry, HandoffInput, HandoffLlm,
};
pub use sleep::orphan_archiver::{OrphanArchiver, OrphanArchiverStats};
pub use sleep::pattern_extractor::{
    PatternExtractLlm, PatternExtractor, PatternInput, PatternResponse, PatternStats, PatternStep,
};
pub use sleep::pruner::{PruneStats, Pruner};
pub use sleep::synthesizer::{
    SynthesisInput, SynthesisLlm, SynthesisResponse, SynthesisStats, Synthesizer,
};
pub use sleep::verifier::LlmPairwiseVerifier;
pub use sleep::worker::{CycleStats, SleepOps, SleepTimeWorker};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

// ============================================================================
// RECALL CONFIG
// Configurable recall priority engine with compiled defaults and JSON merge.
// Missing file → defaults, corrupted file → defaults, partial file → deep merge.
// The config file is NEVER auto-created or modified by the system.
// ============================================================================

/// Mid-session recall configuration — controls whether the system re-recalls
/// facts during an ongoing conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidSessionRecallConfig {
    pub enabled: bool,
    pub every_n_turns: usize,
    pub min_novelty_score: f64,
}

impl Default for MidSessionRecallConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            every_n_turns: 5,
            min_novelty_score: 0.3,
        }
    }
}

/// MMR (Maximal Marginal Relevance) diversity reranking configuration.
///
/// MMR re-orders top-N candidates to balance relevance and diversity:
/// `score = λ · base_score − (1 − λ) · max_similarity_to_already_picked`.
/// Applied after rescore but before the final truncate-to-top-K in
/// `MemoryRecall::recall`. Carbonell & Goldstein 1998 (SIGIR).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MmrConfig {
    /// Master toggle. When false, the recall pipeline skips MMR entirely.
    pub enabled: bool,
    /// Relevance-vs-diversity tradeoff. 1.0 = pure score-rank (no diversity),
    /// 0.0 = pure novelty (ignore score). Default 0.6.
    pub lambda: f64,
    /// How many top-scored candidates to consider before MMR re-ordering.
    /// Larger pool = more diversity opportunity at higher latency. Default 30.
    pub candidate_pool: usize,
}

impl Default for MmrConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            lambda: 0.6,
            candidate_pool: 30,
        }
    }
}

/// Cross-encoder reranker configuration (ONNX via fastembed-rs).
///
/// Cross-encoders score `(query, candidate)` pairs jointly through a
/// transformer — more accurate than bi-encoder similarity but slower.
/// Applied in `MemoryRecall::recall` after MMR and before the final
/// truncate-to-top-K.
///
/// Disabled by default: enabling triggers a one-time ONNX model download
/// (~280 MB for `BAAI/bge-reranker-base`) on the first recall.
/// `BAAI/bge-reranker-base` reaches ~55 NDCG@10 on BEIR vs ~40 for raw
/// bi-encoder recall; latency is ~5–15 ms/candidate on CPU.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankConfig {
    /// Master toggle. When false, the recall pipeline skips reranking
    /// entirely.
    pub enabled: bool,
    /// Model identifier passed to fastembed. Default
    /// `"BAAI/bge-reranker-base"`. Other options:
    /// `"BAAI/bge-reranker-v2-m3"`,
    /// `"jinaai/jina-reranker-v1-turbo-en"`.
    pub model_id: String,
    /// How many top-scored candidates to actually run through the
    /// reranker. Latency is O(N). Default 20.
    pub candidate_pool: usize,
    /// Final top-K after reranking. Default 10.
    pub top_k_after: usize,
    /// Drop candidates whose rerank score is below this threshold.
    /// Default 0.0 (keep all reranked).
    pub score_threshold: f64,
}

impl Default for RerankConfig {
    fn default() -> Self {
        Self {
            // OFF by default — opt-in until validated. Enabling requires
            // a 280 MB ONNX model download.
            enabled: false,
            model_id: "BAAI/bge-reranker-base".to_string(),
            candidate_pool: 20,
            top_k_after: 10,
            score_threshold: 0.0,
        }
    }
}

/// Semantic intent router configuration (MEM-008).
///
/// Tunes the kNN classifier that picks an intent label per query for the
/// per-intent profile overlay system. The exemplar and profile JSON files
/// live at `<vault>/config/memory/intent_{exemplars,profiles}.json`; this
/// struct only carries the two scalar knobs that don't belong in those
/// bulky files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentRouterConfig {
    /// Top-K vote depth used by [`crate::KnnIntentClassifier`]. Default 5.
    pub k: usize,
    /// Minimum cosine similarity to the nearest exemplar required for a
    /// confident classification. Below this, the classifier returns `None`
    /// and the recall pipeline falls back to the base [`RecallConfig`].
    /// Default 0.55.
    pub confidence_threshold: f64,
}

impl Default for IntentRouterConfig {
    fn default() -> Self {
        Self {
            k: 5,
            confidence_threshold: 0.55,
        }
    }
}

/// Graph traversal configuration — controls how related facts are discovered
/// by walking knowledge-graph edges outward from directly recalled nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphTraversalConfig {
    pub enabled: bool,
    pub max_hops: u8,
    pub hop_decay: f64,
    pub max_graph_facts: usize,
}

impl Default for GraphTraversalConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_hops: 2,
            hop_decay: 0.6,
            max_graph_facts: 5,
        }
    }
}

/// Temporal decay configuration — controls how fact relevance diminishes over
/// time, with per-category half-lives and pruning thresholds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalDecayConfig {
    pub enabled: bool,
    pub half_life_days: HashMap<String, f64>,
    pub prune_threshold: f64,
    pub prune_after_days: u32,
}

impl Default for TemporalDecayConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            half_life_days: HashMap::from([
                ("correction".to_string(), 90.0),
                ("strategy".to_string(), 60.0),
                ("domain".to_string(), 30.0),
                ("user".to_string(), 180.0),
                ("pattern".to_string(), 45.0),
                ("instruction".to_string(), 120.0),
            ]),
            prune_threshold: 0.05,
            prune_after_days: 30,
        }
    }
}

/// Predictive recall configuration — controls whether the system proactively
/// recalls facts based on patterns observed in similar past episodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictiveRecallConfig {
    pub enabled: bool,
    pub min_similar_successes: usize,
    pub predictive_boost: f64,
    pub max_episodes_to_check: usize,
}

impl Default for PredictiveRecallConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_similar_successes: 2,
            predictive_boost: 1.3,
            max_episodes_to_check: 5,
        }
    }
}

/// Session offload configuration — controls when and how old session data is
/// archived to keep the active store lean.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionOffloadConfig {
    pub enabled: bool,
    pub offload_after_days: u32,
    pub keep_session_metadata: bool,
    pub archive_path: String,
}

impl Default for SessionOffloadConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            offload_after_days: 7,
            keep_session_metadata: true,
            archive_path: "data/archive".to_string(),
        }
    }
}

/// Knowledge-graph decay configuration — controls how entity and
/// relationship `confidence` is reduced over time based on `last_seen_at`.
/// Applied during the sleep-time cycle. Unlike `temporal_decay` (which is
/// per-category for `memory_facts`), KG decay uses a single half-life
/// for entities and another for relationships.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KgDecayConfig {
    pub enabled: bool,
    pub entity_half_life_days: f64,
    pub relationship_half_life_days: f64,
    /// Floor — confidence never drops below this value.
    pub min_confidence: f64,
    /// Skip rows whose `last_seen_at` is within this many hours.
    pub skip_recent_hours: i64,
}

impl Default for KgDecayConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            entity_half_life_days: 90.0,
            relationship_half_life_days: 90.0,
            min_confidence: 0.01,
            skip_recent_hours: 24,
        }
    }
}

/// Recall priority configuration — weights, limits, and thresholds that
/// control how memory facts and episodes are scored and retrieved.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecallConfig {
    pub category_weights: HashMap<String, f64>,
    pub ward_affinity_boost: f64,
    pub max_recall_tokens: usize,
    pub vector_weight: f64,
    pub bm25_weight: f64,
    pub max_facts: usize,
    pub max_episodes: usize,
    pub high_confidence_threshold: f64,
    /// Multiplier applied to the recall score of contradicted facts (0.0–1.0).
    pub contradiction_penalty: f64,
    /// Minimum score threshold — results scoring below this are suppressed.
    /// Prevents low-relevance facts from appearing for short generic queries.
    pub min_score: f64,
    pub mid_session_recall: MidSessionRecallConfig,
    pub graph_traversal: GraphTraversalConfig,
    pub temporal_decay: TemporalDecayConfig,
    pub predictive_recall: PredictiveRecallConfig,
    pub session_offload: SessionOffloadConfig,
    pub kg_decay: KgDecayConfig,
    pub mmr: MmrConfig,
    pub rerank: RerankConfig,
    /// Semantic intent router knobs (MEM-008). Bulky bits — exemplar bank
    /// and per-intent profile overlays — live in separate JSON files under
    /// `<vault>/config/memory/`; this struct only holds the two scalar
    /// knobs (k, confidence_threshold).
    #[serde(default)]
    pub intent_router: IntentRouterConfig,
}

impl Default for RecallConfig {
    fn default() -> Self {
        let category_weights = HashMap::from([
            ("schema".to_string(), 1.6),
            ("correction".to_string(), 1.5),
            ("strategy".to_string(), 1.4),
            ("user".to_string(), 1.3),
            ("instruction".to_string(), 1.2),
            ("domain".to_string(), 1.0),
            ("pattern".to_string(), 0.9),
            ("ward".to_string(), 0.8),
            ("skill".to_string(), 0.7),
            ("agent".to_string(), 0.7),
        ]);

        Self {
            category_weights,
            ward_affinity_boost: 1.3,
            max_recall_tokens: 3000,
            vector_weight: 0.7,
            bm25_weight: 0.3,
            max_facts: 10,
            max_episodes: 3,
            high_confidence_threshold: 0.9,
            contradiction_penalty: 0.7,
            min_score: 0.3,
            mid_session_recall: MidSessionRecallConfig::default(),
            graph_traversal: GraphTraversalConfig::default(),
            temporal_decay: TemporalDecayConfig::default(),
            predictive_recall: PredictiveRecallConfig::default(),
            session_offload: SessionOffloadConfig::default(),
            kg_decay: KgDecayConfig::default(),
            mmr: MmrConfig::default(),
            rerank: RerankConfig::default(),
            intent_router: IntentRouterConfig::default(),
        }
    }
}

impl RecallConfig {
    /// Load recall config from `{path}/config/recall_config.json`.
    ///
    /// - Missing file → compiled defaults (info log)
    /// - Corrupted file → compiled defaults (warning log)
    /// - Partial file → deep merge with defaults (user values win per key)
    pub fn load_from_path(path: &Path) -> Self {
        let file_path = path.join("config").join("recall_config.json");

        if !file_path.exists() {
            tracing::info!(
                "No recall config at {} — using compiled defaults",
                file_path.display()
            );
            return Self::default();
        }

        let content = match std::fs::read_to_string(&file_path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(
                    "Cannot read recall config at {} — using defaults: {}",
                    file_path.display(),
                    e
                );
                return Self::default();
            }
        };

        let overlay: Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    "Corrupted recall config at {} — using defaults: {}",
                    file_path.display(),
                    e
                );
                return Self::default();
            }
        };

        // Deep merge: serialize defaults to Value, merge overlay on top, deserialize back.
        let base = serde_json::to_value(Self::default()).expect("default config must serialize");
        let merged = deep_merge(base, overlay);

        match serde_json::from_value(merged) {
            Ok(config) => {
                tracing::info!(
                    "Loaded recall config from {} (merged with defaults)",
                    file_path.display()
                );
                config
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to deserialize merged recall config from {} — using defaults: {}",
                    file_path.display(),
                    e
                );
                Self::default()
            }
        }
    }

    /// Look up the weight for a memory category. Returns 1.0 for unknown categories.
    pub fn category_weight(&self, category: &str) -> f64 {
        *self.category_weights.get(category).unwrap_or(&1.0)
    }
}

/// Recursively merge two JSON values. Object keys from `overlay` overwrite
/// matching keys in `base`; nested objects are merged recursively.
/// Non-object values from `overlay` replace `base` entirely.
fn deep_merge(base: Value, overlay: Value) -> Value {
    match (base, overlay) {
        (Value::Object(mut base_map), Value::Object(overlay_map)) => {
            for (key, value) in overlay_map {
                let base_val = base_map.remove(&key).unwrap_or(Value::Null);
                base_map.insert(key, deep_merge(base_val, value));
            }
            Value::Object(base_map)
        }
        (_, overlay) => overlay,
    }
}

// ============================================================================
// MEMORY SETTINGS
// Background memory worker configuration — sleep cycle intervals.
// ============================================================================

/// Background memory worker configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySettings {
    /// Minimum hours between corrections-abstraction LLM calls.
    /// Default: 24. Set to 0 to run on every sleep cycle (hourly).
    #[serde(default = "default_corrections_abstractor_interval_hours")]
    pub corrections_abstractor_interval_hours: u32,
    /// Minimum hours between conflict-resolution LLM judge passes.
    /// Default: 24. Set to 0 to run on every sleep cycle (hourly).
    #[serde(default = "default_conflict_resolver_interval_hours")]
    pub conflict_resolver_interval_hours: u32,
    /// User-facing MMR overrides. When `Some`, replaces the
    /// corresponding field in `recall_config.json` at startup.
    /// `None` (default) means use whatever's in `recall_config.json`
    /// (or compiled defaults).
    #[serde(default)]
    pub mmr: Option<MmrConfig>,
    /// User-facing cross-encoder reranker overrides. When `Some`,
    /// replaces the corresponding field in `recall_config.json` at
    /// startup. `None` (default) means use whatever's in
    /// `recall_config.json` (or compiled defaults — disabled by default).
    #[serde(default)]
    pub rerank: Option<RerankConfig>,
    /// User-facing semantic intent router overrides (MEM-008). When `Some`,
    /// replaces the corresponding field in `recall_config.json` at
    /// startup. Only the scalar knobs (k, confidence_threshold) live here;
    /// exemplar and profile JSON banks are read directly from the vault.
    #[serde(default)]
    pub intent_router: Option<IntentRouterConfig>,
}

pub fn default_corrections_abstractor_interval_hours() -> u32 {
    24
}

pub fn default_conflict_resolver_interval_hours() -> u32 {
    24
}

impl Default for MemorySettings {
    fn default() -> Self {
        Self {
            corrections_abstractor_interval_hours: default_corrections_abstractor_interval_hours(),
            conflict_resolver_interval_hours: default_conflict_resolver_interval_hours(),
            mmr: None,
            rerank: None,
            intent_router: None,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn default_config() {
        let config = RecallConfig::default();

        assert_eq!(config.category_weights.len(), 10);
        assert_eq!(config.category_weights["correction"], 1.5);
        assert_eq!(config.category_weights["strategy"], 1.4);
        assert_eq!(config.category_weights["user"], 1.3);
        assert_eq!(config.category_weights["instruction"], 1.2);
        assert_eq!(config.category_weights["domain"], 1.0);
        assert_eq!(config.category_weights["pattern"], 0.9);
        assert_eq!(config.category_weights["ward"], 0.8);
        assert_eq!(config.category_weights["skill"], 0.7);
        assert_eq!(config.category_weights["agent"], 0.7);

        assert_eq!(config.ward_affinity_boost, 1.3);
        assert_eq!(config.max_recall_tokens, 3000);
        assert_eq!(config.vector_weight, 0.7);
        assert_eq!(config.bm25_weight, 0.3);
        assert_eq!(config.max_facts, 10);
        assert_eq!(config.max_episodes, 3);
        assert_eq!(config.high_confidence_threshold, 0.9);

        assert!(config.mid_session_recall.enabled);
        assert_eq!(config.mid_session_recall.every_n_turns, 5);
        assert_eq!(config.mid_session_recall.min_novelty_score, 0.3);
    }

    #[test]
    fn load_missing_file() {
        let tmp = tempfile::tempdir().unwrap();
        let config = RecallConfig::load_from_path(tmp.path());

        // Should return defaults when file doesn't exist
        assert_eq!(config.max_recall_tokens, 3000);
        assert_eq!(config.max_facts, 10);
        assert_eq!(config.category_weights.len(), 10);
    }

    #[test]
    fn load_partial_override() {
        let tmp = tempfile::tempdir().unwrap();
        let config_dir = tmp.path().join("config");
        fs::create_dir_all(&config_dir).unwrap();

        // Override only a few fields — the rest should come from defaults
        let override_json = serde_json::json!({
            "max_facts": 25,
            "vector_weight": 0.8,
            "mid_session_recall": {
                "every_n_turns": 10
            },
            "category_weights": {
                "correction": 2.0,
                "custom_category": 1.1
            }
        });

        fs::write(
            config_dir.join("recall_config.json"),
            serde_json::to_string_pretty(&override_json).unwrap(),
        )
        .unwrap();

        let config = RecallConfig::load_from_path(tmp.path());

        // Overridden values
        assert_eq!(config.max_facts, 25);
        assert_eq!(config.vector_weight, 0.8);
        assert_eq!(config.mid_session_recall.every_n_turns, 10);

        // Deep merge: mid_session_recall fields not in overlay keep defaults
        assert!(config.mid_session_recall.enabled);
        assert_eq!(config.mid_session_recall.min_novelty_score, 0.3);

        // category_weights: overlay replaces the entire map (overlay wins at leaf level)
        // Since category_weights is an object, deep merge merges keys:
        // - "correction" overridden to 2.0
        // - "custom_category" added as 1.1
        // - other default keys preserved
        assert_eq!(config.category_weights["correction"], 2.0);
        assert_eq!(config.category_weights["custom_category"], 1.1);
        assert_eq!(config.category_weights["strategy"], 1.4); // default preserved

        // Non-overridden top-level values remain default
        assert_eq!(config.max_recall_tokens, 3000);
        assert_eq!(config.bm25_weight, 0.3);
        assert_eq!(config.max_episodes, 3);
        assert_eq!(config.high_confidence_threshold, 0.9);
        assert_eq!(config.ward_affinity_boost, 1.3);
    }

    #[test]
    fn load_corrupted_file() {
        let tmp = tempfile::tempdir().unwrap();
        let config_dir = tmp.path().join("config");
        fs::create_dir_all(&config_dir).unwrap();

        fs::write(
            config_dir.join("recall_config.json"),
            "this is not valid json {{{",
        )
        .unwrap();

        let config = RecallConfig::load_from_path(tmp.path());

        // Should fall back to defaults
        assert_eq!(config.max_recall_tokens, 3000);
        assert_eq!(config.max_facts, 10);
        assert_eq!(config.category_weights.len(), 10);
    }

    #[test]
    fn test_default_config_has_new_sections() {
        let config = RecallConfig::default();
        assert!(config.graph_traversal.enabled);
        assert_eq!(config.graph_traversal.max_hops, 2);
        assert_eq!(config.graph_traversal.hop_decay, 0.6);
        assert!(config.temporal_decay.enabled);
        assert_eq!(
            *config
                .temporal_decay
                .half_life_days
                .get("correction")
                .unwrap(),
            90.0
        );
        assert!(config.predictive_recall.enabled);
        assert_eq!(config.predictive_recall.predictive_boost, 1.3);
        assert!(config.session_offload.enabled);
        assert_eq!(config.session_offload.offload_after_days, 7);
    }

    #[test]
    fn graph_traversal_defaults_remain_enabled_depth_two() {
        // Pack A contract: these defaults must stay in sync with the activation spec.
        // See docs/superpowers/specs/2026-04-12-kg-activation-pack-a-design.md (Fix 6).
        let c = RecallConfig::default();
        assert!(
            c.graph_traversal.enabled,
            "graph_traversal.enabled default must remain true (Pack A contract)"
        );
        assert_eq!(
            c.graph_traversal.max_hops, 2,
            "graph_traversal.max_hops default must remain 2 (Pack A contract)"
        );
        assert!(
            c.graph_traversal.max_graph_facts >= 5,
            "graph_traversal.max_graph_facts default must be >= 5"
        );
    }

    #[test]
    fn test_partial_override_new_sections() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config");
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(
            path.join("recall_config.json"),
            r#"{"graph_traversal": {"max_hops": 3}}"#,
        )
        .unwrap();
        let config = RecallConfig::load_from_path(dir.path());
        assert_eq!(config.graph_traversal.max_hops, 3); // overridden
        assert_eq!(config.graph_traversal.hop_decay, 0.6); // default preserved
        assert!(config.temporal_decay.enabled); // entirely default
    }

    #[test]
    fn category_weight_known_and_unknown() {
        let config = RecallConfig::default();

        // Known categories return their weight
        assert_eq!(config.category_weight("correction"), 1.5);
        assert_eq!(config.category_weight("agent"), 0.7);

        // Unknown categories return 1.0 fallback
        assert_eq!(config.category_weight("nonexistent"), 1.0);
        assert_eq!(config.category_weight(""), 1.0);
    }

    #[test]
    fn default_min_score_is_0_3() {
        let config = RecallConfig::default();
        assert_eq!(config.min_score, 0.3);
    }

    #[test]
    fn schema_category_weight_is_higher_than_correction() {
        let config = RecallConfig::default();
        let schema_w = config.category_weight("schema");
        let correction_w = config.category_weight("correction");
        assert!(
            schema_w > correction_w,
            "schema weight ({schema_w}) must exceed correction weight ({correction_w})"
        );
    }

    #[test]
    fn min_score_can_be_overridden() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config");
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("recall_config.json"), r#"{"min_score": 0.5}"#).unwrap();
        let config = RecallConfig::load_from_path(dir.path());
        assert_eq!(config.min_score, 0.5);
    }

    #[test]
    fn kg_decay_config_defaults() {
        let c = RecallConfig::default();
        assert!(c.kg_decay.enabled);
        assert_eq!(c.kg_decay.entity_half_life_days, 90.0);
        assert_eq!(c.kg_decay.relationship_half_life_days, 90.0);
        assert_eq!(c.kg_decay.min_confidence, 0.01);
        assert_eq!(c.kg_decay.skip_recent_hours, 24);
    }

    #[test]
    fn kg_decay_partial_override() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config");
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(
            path.join("recall_config.json"),
            r#"{"kg_decay": {"entity_half_life_days": 30.0}}"#,
        )
        .unwrap();
        let c = RecallConfig::load_from_path(dir.path());
        assert_eq!(c.kg_decay.entity_half_life_days, 30.0);
        // others remain default
        assert_eq!(c.kg_decay.relationship_half_life_days, 90.0);
        assert!(c.kg_decay.enabled);
    }

    #[test]
    fn default_conflict_resolver_interval_is_24() {
        let m = MemorySettings::default();
        assert_eq!(m.conflict_resolver_interval_hours, 24);
    }

    #[test]
    fn memory_settings_deserializes_partial() {
        let json = r#"{"conflictResolverIntervalHours": 6}"#;
        let m: MemorySettings = serde_json::from_str(json).unwrap();
        assert_eq!(m.conflict_resolver_interval_hours, 6);
        assert_eq!(
            m.corrections_abstractor_interval_hours, 24,
            "default preserved"
        );
    }

    #[test]
    fn memory_settings_deserializes_mmr_override() {
        // Inner MmrConfig has no `rename_all = "camelCase"`, so its fields
        // are read as snake_case (matching recall_config.json conventions).
        let json = r#"{
            "correctionsAbstractorIntervalHours": 24,
            "conflictResolverIntervalHours": 24,
            "mmr": { "enabled": false, "lambda": 0.5, "candidate_pool": 20 }
        }"#;
        let s: MemorySettings = serde_json::from_str(json).unwrap();
        assert!(s.mmr.is_some());
        let mmr = s.mmr.unwrap();
        assert!(!mmr.enabled);
        assert_eq!(mmr.lambda, 0.5);
        assert_eq!(mmr.candidate_pool, 20);
    }

    #[test]
    fn memory_settings_mmr_missing_is_none() {
        let json = r#"{
            "correctionsAbstractorIntervalHours": 24,
            "conflictResolverIntervalHours": 24
        }"#;
        let s: MemorySettings = serde_json::from_str(json).unwrap();
        assert!(s.mmr.is_none());
    }

    #[test]
    fn memory_settings_deserializes_rerank_override() {
        // Inner RerankConfig has no `rename_all`, so its fields are read
        // as snake_case (matching recall_config.json conventions).
        let json = r#"{
            "correctionsAbstractorIntervalHours": 24,
            "conflictResolverIntervalHours": 24,
            "rerank": {
                "enabled": true,
                "model_id": "BAAI/bge-reranker-base",
                "candidate_pool": 15,
                "top_k_after": 8,
                "score_threshold": 0.5
            }
        }"#;
        let s: MemorySettings = serde_json::from_str(json).unwrap();
        assert!(s.rerank.is_some());
        let r = s.rerank.unwrap();
        assert!(r.enabled);
        assert_eq!(r.model_id, "BAAI/bge-reranker-base");
        assert_eq!(r.candidate_pool, 15);
        assert_eq!(r.top_k_after, 8);
        assert_eq!(r.score_threshold, 0.5);
    }

    #[test]
    fn memory_settings_rerank_missing_is_none() {
        let json = r#"{
            "correctionsAbstractorIntervalHours": 24,
            "conflictResolverIntervalHours": 24
        }"#;
        let s: MemorySettings = serde_json::from_str(json).unwrap();
        assert!(s.rerank.is_none());
    }

    #[test]
    fn intent_router_config_defaults() {
        let c = IntentRouterConfig::default();
        assert_eq!(c.k, 5);
        assert!((c.confidence_threshold - 0.55).abs() < 1e-9);
    }

    #[test]
    fn recall_config_includes_intent_router_default() {
        let c = RecallConfig::default();
        assert_eq!(c.intent_router.k, 5);
        assert!((c.intent_router.confidence_threshold - 0.55).abs() < 1e-9);
    }

    #[test]
    fn memory_settings_intent_router_missing_is_none() {
        let json = r#"{
            "correctionsAbstractorIntervalHours": 24,
            "conflictResolverIntervalHours": 24
        }"#;
        let s: MemorySettings = serde_json::from_str(json).unwrap();
        assert!(s.intent_router.is_none());
    }

    #[test]
    fn memory_settings_deserializes_intent_router_override() {
        // `MemorySettings` has `rename_all = "camelCase"` so the outer key
        // is `intentRouter`. Inner `IntentRouterConfig` has no rename_all,
        // so its fields stay snake_case (matching recall_config.json).
        let json = r#"{
            "correctionsAbstractorIntervalHours": 24,
            "conflictResolverIntervalHours": 24,
            "intentRouter": { "k": 7, "confidence_threshold": 0.7 }
        }"#;
        let s: MemorySettings = serde_json::from_str(json).unwrap();
        assert!(s.intent_router.is_some());
        let r = s.intent_router.unwrap();
        assert_eq!(r.k, 7);
        assert!((r.confidence_threshold - 0.7).abs() < 1e-9);
    }

    #[test]
    fn rerank_config_defaults_are_safe() {
        // MEM-007 contract: enabling triggers a 280 MB download, so the
        // default must be off.
        let c = RecallConfig::default();
        assert!(!c.rerank.enabled, "rerank disabled by default");
        assert_eq!(c.rerank.model_id, "BAAI/bge-reranker-base");
        assert_eq!(c.rerank.candidate_pool, 20);
        assert_eq!(c.rerank.top_k_after, 10);
        assert_eq!(c.rerank.score_threshold, 0.0);
    }
}
