// ============================================================================
// MODEL REGISTRY
// Model capabilities and metadata registry for the gateway.
// Three-layer resolution: local overrides > bundled registry > unknown fallback.
// ============================================================================

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

// ============================================================================
// Types
// ============================================================================

/// Profile for a single model — capabilities, context window, embedding spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelProfile {
    pub name: String,
    /// Canonical vendor label for display/grouping (e.g., "openai", "zhipu").
    /// Not a Provider.id lookup key — purely informational.
    pub provider: String,
    pub capabilities: ModelCapabilities,
    pub context: ContextWindow,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding: Option<EmbeddingSpec>,
}

/// Boolean capability flags for a model.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCapabilities {
    #[serde(default)]
    pub tools: bool,
    #[serde(default)]
    pub vision: bool,
    #[serde(default)]
    pub thinking: bool,
    #[serde(default)]
    pub embeddings: bool,
    #[serde(default)]
    pub voice: bool,
    #[serde(default)]
    pub image_generation: bool,
    #[serde(default)]
    pub video_generation: bool,
}

/// Input/output token limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextWindow {
    pub input: u64,
    /// Max output tokens. None means same as input.
    pub output: Option<u64>,
}

impl ContextWindow {
    /// Resolve output token limit. Returns explicit output if set, otherwise input.
    pub fn resolved_output(&self) -> u64 {
        self.output.unwrap_or(self.input)
    }
}

/// Embedding model parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingSpec {
    pub dimensions: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_dimensions: Option<u32>,
}

/// Named capability for programmatic checks via `has_capability()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    Tools,
    Vision,
    Thinking,
    Embeddings,
    Voice,
    ImageGeneration,
    VideoGeneration,
}

impl ModelCapabilities {
    /// Check whether a specific capability is enabled.
    pub fn has(&self, cap: Capability) -> bool {
        match cap {
            Capability::Tools => self.tools,
            Capability::Vision => self.vision,
            Capability::Thinking => self.thinking,
            Capability::Embeddings => self.embeddings,
            Capability::Voice => self.voice,
            Capability::ImageGeneration => self.image_generation,
            Capability::VideoGeneration => self.video_generation,
        }
    }
}

// ============================================================================
// Registry
// ============================================================================

/// Model capabilities registry with three-layer resolution.
///
/// Resolution order:
/// 1. Local overrides (config/models.json) — highest priority
/// 2. Bundled registry (embedded models_registry.json)
/// 3. Unknown model fallback — conservative defaults
pub struct ModelRegistry {
    models: HashMap<String, ModelProfile>,
    /// Pre-built fallback returned for unknown model IDs.
    fallback: ModelProfile,
}

impl ModelRegistry {
    /// Load registry from bundled JSON bytes + local override file.
    ///
    /// `bundled_json` comes from the caller (via rust-embed Templates::get())
    /// to avoid circular dependency between gateway-services and gateway-templates.
    pub fn load(bundled_json: &[u8], config_dir: &Path) -> Self {
        let fallback = ModelProfile {
            name: "Unknown Model".to_string(),
            provider: "unknown".to_string(),
            capabilities: ModelCapabilities {
                tools: true,
                vision: false,
                thinking: false,
                embeddings: false,
                voice: false,
                image_generation: false,
                video_generation: false,
            },
            context: ContextWindow {
                input: 256_000,
                output: Some(128_000),
            },
            embedding: None,
        };

        // Layer 2: Bundled registry
        let mut models: HashMap<String, ModelProfile> = if bundled_json.is_empty() {
            tracing::warn!("Bundled models registry is empty — using fallback only");
            HashMap::new()
        } else {
            match serde_json::from_slice(bundled_json) {
                Ok(m) => m,
                Err(e) => {
                    tracing::error!("Failed to parse bundled models registry: {}", e);
                    HashMap::new()
                }
            }
        };

        // Layer 1: Local overrides (merge on top)
        let local_path = config_dir.join("config").join("models.json");
        if local_path.exists() {
            match std::fs::read_to_string(&local_path) {
                Ok(content) => match serde_json::from_str::<HashMap<String, ModelProfile>>(&content) {
                    Ok(overrides) => {
                        let count = overrides.len();
                        models.extend(overrides);
                        tracing::info!("Loaded {} model override(s) from {}", count, local_path.display());
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Malformed models.json at {} — skipping local overrides: {}",
                            local_path.display(),
                            e
                        );
                    }
                },
                Err(e) => {
                    tracing::warn!("Cannot read {}: {}", local_path.display(), e);
                }
            }
        }

        tracing::info!("Model registry loaded: {} models", models.len());

        Self { models, fallback }
    }

    /// Get a model profile by ID. Returns the fallback profile for unknown models.
    pub fn get(&self, model_id: &str) -> &ModelProfile {
        self.models.get(model_id).unwrap_or(&self.fallback)
    }

    /// Check whether a model has a specific capability.
    pub fn has_capability(&self, model_id: &str, cap: Capability) -> bool {
        self.get(model_id).capabilities.has(cap)
    }

    /// Get context window for a model.
    pub fn context_window(&self, model_id: &str) -> &ContextWindow {
        &self.get(model_id).context
    }

    /// List all known models (for API/UI). Sorted by model ID.
    pub fn list(&self) -> Vec<(&str, &ModelProfile)> {
        let mut entries: Vec<_> = self.models.iter().map(|(k, v)| (k.as_str(), v)).collect();
        entries.sort_by_key(|(k, _)| *k);
        entries
    }

    /// Check if a model is known (exists in registry, not the fallback).
    pub fn is_known(&self, model_id: &str) -> bool {
        self.models.contains_key(model_id)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample_registry_json() -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "gpt-4o": {
                "name": "GPT-4o",
                "provider": "openai",
                "capabilities": {
                    "tools": true,
                    "vision": true,
                    "thinking": false,
                    "embeddings": false,
                    "voice": false,
                    "imageGeneration": false,
                    "videoGeneration": false
                },
                "context": {
                    "input": 128000,
                    "output": 16384
                }
            },
            "text-embedding-3-large": {
                "name": "Text Embedding 3 Large",
                "provider": "openai",
                "capabilities": {
                    "tools": false,
                    "vision": false,
                    "thinking": false,
                    "embeddings": true,
                    "voice": false,
                    "imageGeneration": false,
                    "videoGeneration": false
                },
                "context": {
                    "input": 8191,
                    "output": null
                },
                "embedding": {
                    "dimensions": 3072,
                    "maxDimensions": 3072
                }
            }
        }))
        .unwrap()
    }

    #[test]
    fn test_load_bundled() {
        let registry = ModelRegistry::load(&sample_registry_json(), &PathBuf::from("/nonexistent"));
        assert_eq!(registry.list().len(), 2);
    }

    #[test]
    fn test_get_known_model() {
        let registry = ModelRegistry::load(&sample_registry_json(), &PathBuf::from("/nonexistent"));
        let profile = registry.get("gpt-4o");
        assert_eq!(profile.name, "GPT-4o");
        assert!(profile.capabilities.tools);
        assert!(profile.capabilities.vision);
        assert!(!profile.capabilities.thinking);
        assert_eq!(profile.context.input, 128000);
        assert_eq!(profile.context.resolved_output(), 16384);
    }

    #[test]
    fn test_get_unknown_model_returns_fallback() {
        let registry = ModelRegistry::load(&sample_registry_json(), &PathBuf::from("/nonexistent"));
        let profile = registry.get("nonexistent-model");
        assert_eq!(profile.provider, "unknown");
        assert!(profile.capabilities.tools);
        assert!(!profile.capabilities.vision);
        assert_eq!(profile.context.input, 8192);
    }

    #[test]
    fn test_has_capability() {
        let registry = ModelRegistry::load(&sample_registry_json(), &PathBuf::from("/nonexistent"));
        assert!(registry.has_capability("gpt-4o", Capability::Tools));
        assert!(registry.has_capability("gpt-4o", Capability::Vision));
        assert!(!registry.has_capability("gpt-4o", Capability::Thinking));
        assert!(registry.has_capability("text-embedding-3-large", Capability::Embeddings));
    }

    #[test]
    fn test_context_window() {
        let registry = ModelRegistry::load(&sample_registry_json(), &PathBuf::from("/nonexistent"));
        let ctx = registry.context_window("gpt-4o");
        assert_eq!(ctx.input, 128000);
        assert_eq!(ctx.output, Some(16384));

        // Embedding model: output is None, resolved_output returns input
        let ctx = registry.context_window("text-embedding-3-large");
        assert_eq!(ctx.input, 8191);
        assert_eq!(ctx.output, None);
        assert_eq!(ctx.resolved_output(), 8191);
    }

    #[test]
    fn test_is_known() {
        let registry = ModelRegistry::load(&sample_registry_json(), &PathBuf::from("/nonexistent"));
        assert!(registry.is_known("gpt-4o"));
        assert!(!registry.is_known("nonexistent"));
    }

    #[test]
    fn test_empty_bundled() {
        let registry = ModelRegistry::load(&[], &PathBuf::from("/nonexistent"));
        assert_eq!(registry.list().len(), 0);
        // Still returns fallback for unknown
        let profile = registry.get("anything");
        assert_eq!(profile.provider, "unknown");
    }

    #[test]
    fn test_malformed_bundled() {
        let registry = ModelRegistry::load(b"not json", &PathBuf::from("/nonexistent"));
        assert_eq!(registry.list().len(), 0);
    }
}
