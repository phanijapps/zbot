//! Memory subsystem configuration types. Owned by gateway-memory crate;
//! re-exported through gateway-services for backward compat.

pub mod llm_factory;
pub mod recall;
pub mod services;
pub mod sleep;
pub mod util;

pub use llm_factory::{LlmClientConfig, MemoryLlmFactory};
pub use services::{MemoryServices, MemoryServicesConfig};
pub use util::{parse_llm_json, strip_code_fence};

pub use recall::query_gate::{
    GateResponse, LlmQueryGate, QueryGate, QueryGateLlm, RetrievalDecision,
};
pub use recall::scored_item::{
    intent_boost, rrf_merge, GoalLite, ItemKind, Provenance, ScoredItem,
};
pub use recall::MemoryRecall;
pub use sleep::belief_contradiction_detector::{
    BeliefContradictionConfig, BeliefContradictionDetector, ContradictionDetectionStats,
    ContradictionJudgeLlm, ContradictionJudgeResponse, JudgeDecision, LlmContradictionJudge,
};
pub use sleep::belief_propagator::{BeliefPropagationStats, BeliefPropagator};
pub use sleep::belief_synthesizer::{
    BeliefSynthesisLlm, BeliefSynthesisStats, BeliefSynthesizer, LlmBeliefSynthesizer,
    SynthesisLlmResponse,
};
pub use sleep::compactor::{CompactionStats, Compactor, PairwiseVerifier};
pub use sleep::conflict_resolver::{
    ConflictJudgeLlm, ConflictResolver, ConflictResponse, ConflictStats,
};
pub use sleep::corrections_abstractor::{
    AbstractionLlm, AbstractionResponse, AbstractionStats, CorrectionsAbstractor,
};
pub use sleep::decay::{DecayConfig, DecayEngine, KgDecayStats, PruneCandidate};
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
    /// Self-RAG retrieval-gate configuration — controls whether a small LLM
    /// pre-step decides skip/direct/split before the hybrid search runs.
    /// Disabled by default; opt-in via `enabled: true`.
    #[serde(default)]
    pub query_gate: QueryGateConfig,
    /// Belief Network synthesizer configuration. Phase B-1 of the
    /// reflective memory roadmap — opt-in (disabled by default).
    #[serde(default)]
    pub belief_network: BeliefNetworkConfig,
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
            query_gate: QueryGateConfig::default(),
            belief_network: BeliefNetworkConfig::default(),
        }
    }
}

/// Belief Network configuration. Controls both the `BeliefSynthesizer`
/// (Phase B-1) and the `BeliefContradictionDetector` (Phase B-2)
/// sleep-time workers — a single block governs the whole reflective-memory
/// pillar so operators flip one master switch.
///
/// Disabled by default — operators opt in by setting `enabled: true`.
/// Throttled by `interval_hours` (default 24) to keep LLM cost bounded.
/// B-2 additions (`neighborhood_prefix_depth`, `contradiction_budget_per_cycle`)
/// only kick in when contradiction detection runs, which still gates on
/// the shared `enabled` flag.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BeliefNetworkConfig {
    /// Master switch — covers B-1 synthesis, B-2 detection, and B-3
    /// confidence propagation. Default: `false`.
    #[serde(default)]
    pub enabled: bool,
    /// Minimum hours between belief / contradiction cycles. Default: 24.
    #[serde(default = "default_belief_network_interval_hours")]
    pub interval_hours: u32,
    /// Phase B-2: how many dot-separated subject components define a
    /// contradiction-detection "neighborhood". `1` = top-level prefix
    /// (e.g. `user`); `2` = first two levels (`user.dietary`).
    /// Default: `1`.
    #[serde(default = "default_neighborhood_prefix_depth")]
    pub neighborhood_prefix_depth: usize,
    /// Phase B-2: maximum LLM judge calls per detection cycle. Pairs
    /// beyond the cap are skipped this cycle and may be picked up later.
    /// Default: 20.
    #[serde(default = "default_contradiction_budget_per_cycle")]
    pub contradiction_budget_per_cycle: usize,
    /// Phase B-3: threshold for fact-confidence-drop propagation. The
    /// DecayEngine fires `belief_invalidate` on a fact when EITHER its
    /// new confidence falls below this floor (and was above it before)
    /// OR the single-cycle drop exceeds this value. Default: `0.3`.
    #[serde(default = "default_fact_confidence_drop_threshold")]
    pub fact_confidence_drop_threshold: f64,
}

pub fn default_belief_network_interval_hours() -> u32 {
    24
}

pub fn default_neighborhood_prefix_depth() -> usize {
    1
}

pub fn default_contradiction_budget_per_cycle() -> usize {
    20
}

pub fn default_fact_confidence_drop_threshold() -> f64 {
    0.3
}

impl Default for BeliefNetworkConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            interval_hours: default_belief_network_interval_hours(),
            neighborhood_prefix_depth: default_neighborhood_prefix_depth(),
            contradiction_budget_per_cycle: default_contradiction_budget_per_cycle(),
            fact_confidence_drop_threshold: default_fact_confidence_drop_threshold(),
        }
    }
}

// ============================================================================
// QUERY GATE CONFIG
// Self-RAG retrieval gate — small LLM call that decides skip/direct/split
// before the hybrid search runs. Reduces signal dilution on multi-topic queries.
// ============================================================================

/// Configuration for the Self-RAG retrieval gate.
///
/// When `enabled: true`, `MemoryRecall::recall` runs a small LLM call before
/// the hybrid search. The LLM returns one of three decisions:
///
/// - `skip` — context already suffices; no hybrid search.
/// - `direct` — single-topic query; use the reformulated query for hybrid search.
/// - `split` — multi-topic input; run hybrid search per subquery and dedup-merge.
///
/// Always-inject corrections (driven from the bootstrap path) are unaffected
/// by the gate decision. The gate scopes only the hybrid-search portion of recall.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryGateConfig {
    /// Master switch. Default: `false` (opt-in).
    #[serde(default)]
    pub enabled: bool,
    /// LLM model identifier. `None` = use the distillation/default model
    /// resolved by `MemoryLlmFactory`.
    #[serde(default)]
    pub model_id: Option<String>,
    /// Maximum number of subqueries accepted from a Split decision. Excess
    /// subqueries are truncated to the first N.
    #[serde(default = "default_max_subqueries")]
    pub max_subqueries: usize,
    /// Maximum character length of any single subquery. Longer subqueries
    /// are truncated.
    #[serde(default = "default_max_subquery_len")]
    pub max_subquery_len: usize,
    /// LLM call timeout in milliseconds. Kept for future use by the
    /// production gate impl; the trait surface accepts the value verbatim.
    #[serde(default = "default_query_gate_timeout_ms")]
    pub timeout_ms: u64,
}

pub fn default_max_subqueries() -> usize {
    4
}
pub fn default_max_subquery_len() -> usize {
    200
}
pub fn default_query_gate_timeout_ms() -> u64 {
    3000
}

impl Default for QueryGateConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            model_id: None,
            max_subqueries: default_max_subqueries(),
            max_subquery_len: default_max_subquery_len(),
            timeout_ms: default_query_gate_timeout_ms(),
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
    fn query_gate_default_is_disabled() {
        let cfg = QueryGateConfig::default();
        assert!(!cfg.enabled, "query gate must default to disabled");
        assert_eq!(cfg.max_subqueries, 4);
        assert_eq!(cfg.max_subquery_len, 200);
        assert_eq!(cfg.timeout_ms, 3000);
        assert!(cfg.model_id.is_none());
    }

    #[test]
    fn memory_settings_default_query_gate_disabled() {
        let m = MemorySettings::default();
        assert!(!m.query_gate.enabled);
    }

    #[test]
    fn query_gate_deserializes_camel_case() {
        let json = r#"{
            "enabled": true,
            "modelId": "gpt-4o-mini",
            "maxSubqueries": 6,
            "maxSubqueryLen": 120,
            "timeoutMs": 5000
        }"#;
        let cfg: QueryGateConfig = serde_json::from_str(json).unwrap();
        assert!(cfg.enabled);
        assert_eq!(cfg.model_id.as_deref(), Some("gpt-4o-mini"));
        assert_eq!(cfg.max_subqueries, 6);
        assert_eq!(cfg.max_subquery_len, 120);
        assert_eq!(cfg.timeout_ms, 5000);
    }

    #[test]
    fn belief_network_default_values() {
        let cfg = BeliefNetworkConfig::default();
        assert!(!cfg.enabled, "belief network must default to disabled");
        assert_eq!(cfg.interval_hours, 24);
        assert_eq!(cfg.neighborhood_prefix_depth, 1);
        assert_eq!(cfg.contradiction_budget_per_cycle, 20);
        assert!(
            (cfg.fact_confidence_drop_threshold - 0.3).abs() < 1e-9,
            "B-3 threshold defaults to 0.3"
        );
    }

    #[test]
    fn belief_network_legacy_json_back_compat() {
        // Existing settings.json from B-1 users only carries `enabled` +
        // `intervalHours`. Missing B-2 / B-3 fields must fall back to
        // defaults without failing deserialization.
        let json = r#"{"enabled": true, "intervalHours": 24}"#;
        let cfg: BeliefNetworkConfig = serde_json::from_str(json).unwrap();
        assert!(cfg.enabled);
        assert_eq!(cfg.interval_hours, 24);
        assert_eq!(cfg.neighborhood_prefix_depth, 1);
        assert_eq!(cfg.contradiction_budget_per_cycle, 20);
        assert!((cfg.fact_confidence_drop_threshold - 0.3).abs() < 1e-9);
    }

    #[test]
    fn belief_network_b3_only_legacy_json_keeps_b3_default() {
        // A settings.json that knows B-1 + B-2 but predates B-3 must
        // still parse and fall through to the 0.3 default for the new
        // field.
        let json = r#"{
            "enabled": true,
            "intervalHours": 24,
            "neighborhoodPrefixDepth": 2,
            "contradictionBudgetPerCycle": 50
        }"#;
        let cfg: BeliefNetworkConfig = serde_json::from_str(json).unwrap();
        assert!((cfg.fact_confidence_drop_threshold - 0.3).abs() < 1e-9);
    }

    #[test]
    fn belief_network_full_json_round_trips() {
        let json = r#"{
            "enabled": true,
            "intervalHours": 12,
            "neighborhoodPrefixDepth": 2,
            "contradictionBudgetPerCycle": 50,
            "factConfidenceDropThreshold": 0.45
        }"#;
        let cfg: BeliefNetworkConfig = serde_json::from_str(json).unwrap();
        assert!(cfg.enabled);
        assert_eq!(cfg.interval_hours, 12);
        assert_eq!(cfg.neighborhood_prefix_depth, 2);
        assert_eq!(cfg.contradiction_budget_per_cycle, 50);
        assert!((cfg.fact_confidence_drop_threshold - 0.45).abs() < 1e-9);
    }

    #[test]
    fn memory_settings_with_query_gate_block_round_trips() {
        let json = r#"{
            "queryGate": { "enabled": true, "maxSubqueries": 3 }
        }"#;
        let m: MemorySettings = serde_json::from_str(json).unwrap();
        assert!(m.query_gate.enabled);
        assert_eq!(m.query_gate.max_subqueries, 3);
        // unspecified fields keep defaults
        assert_eq!(m.query_gate.max_subquery_len, 200);
        assert_eq!(m.query_gate.timeout_ms, 3000);
        assert!(m.query_gate.model_id.is_none());
    }
}
