// ============================================================================
// RECALL CONFIG
// Configurable recall priority engine with compiled defaults and JSON merge.
// Missing file → defaults, corrupted file → defaults, partial file → deep merge.
// The config file is NEVER auto-created or modified by the system.
// ============================================================================

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

// ============================================================================
// Types
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
    pub mid_session_recall: MidSessionRecallConfig,
}

impl Default for RecallConfig {
    fn default() -> Self {
        let category_weights = HashMap::from([
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
            mid_session_recall: MidSessionRecallConfig::default(),
        }
    }
}

// ============================================================================
// Loading
// ============================================================================

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

// ============================================================================
// Deep Merge
// ============================================================================

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
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn default_config() {
        let config = RecallConfig::default();

        assert_eq!(config.category_weights.len(), 9);
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
        assert_eq!(config.category_weights.len(), 9);
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
        assert_eq!(config.category_weights.len(), 9);
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
}
