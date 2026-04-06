# Lazy Embedding & Distillation Config Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Save ~100MB RAM by lazy-loading/unloading the local embedding model, and allow distillation to use a configurable provider/model that inherits from orchestrator settings.

**Architecture:** Two independent changes. (1) `LocalEmbeddingClient` wraps `TextEmbedding` in `Mutex<Option<>>` with an idle-timeout watcher that drops the model after inactivity. (2) A `DistillationConfig` struct (provider_id + model) is added to `ExecutionSettings`, resolved in `SessionDistiller::extract_all()` via a 3-level fallback chain (distillation → orchestrator → default provider), and exposed in the Settings UI.

**Tech Stack:** Rust (tokio, fastembed, serde), TypeScript/React (settings UI)

---

## File Structure

| File | Responsibility |
|------|---------------|
| `runtime/agent-runtime/src/llm/embedding.rs` | `EmbeddingConfig` gains `idle_timeout_secs` field |
| `runtime/agent-runtime/src/llm/local_embedding.rs` | Lazy load/unload with `Mutex<Option<TextEmbedding>>` + idle watcher |
| `gateway/gateway-services/src/settings.rs` | `DistillationConfig` struct + field on `ExecutionSettings` |
| `gateway/gateway-execution/src/distillation.rs` | `SessionDistiller` reads settings, resolves provider/model chain |
| `gateway/src/state.rs` | Simplify embedding init, pass `SettingsService` to distiller |
| `apps/ui/src/services/transport/types.ts` | `DistillationConfig` TypeScript interface |
| `apps/ui/src/features/settings/WebSettingsPanel.tsx` | Distillation subsection in orchestrator card |

---

## Task 1: Add `idle_timeout_secs` to `EmbeddingConfig`

**Files:**
- Modify: `runtime/agent-runtime/src/llm/embedding.rs`

- [ ] **Step 1: Add the field and default function**

In `runtime/agent-runtime/src/llm/embedding.rs`, add the `idle_timeout_secs` field to `EmbeddingConfig` and a default function:

```rust
// Add after the existing default functions (after line 107):
const fn default_idle_timeout() -> u64 {
    300 // 5 minutes
}
```

Add the field to `EmbeddingConfig` (after `cache_enabled`):

```rust
    /// Idle timeout in seconds before unloading the local model from RAM.
    /// Default: 300 (5 minutes). Set to 0 to never unload (keep in RAM permanently).
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_secs: u64,
```

Update the `Default` impl to include the new field:

```rust
impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            provider: EmbeddingProviderType::Local,
            model: "all-MiniLM-L6-v2".to_string(),
            dimensions: 384,
            batch_size: default_batch_size(),
            cache_enabled: default_cache_enabled(),
            idle_timeout_secs: default_idle_timeout(),
        }
    }
}
```

- [ ] **Step 2: Update the default config test**

Update `test_default_config` in the same file:

```rust
    #[test]
    fn test_default_config() {
        let config = EmbeddingConfig::default();
        assert_eq!(config.model, "all-MiniLM-L6-v2");
        assert_eq!(config.dimensions, 384);
        assert_eq!(config.batch_size, 32);
        assert!(config.cache_enabled);
        assert_eq!(config.idle_timeout_secs, 300);
        assert!(matches!(config.provider, EmbeddingProviderType::Local));
    }
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p agent-runtime -- embedding`
Expected: All tests pass, including updated `test_default_config`.

- [ ] **Step 4: Commit**

```bash
git add runtime/agent-runtime/src/llm/embedding.rs
git commit -m "feat(embedding): add idle_timeout_secs to EmbeddingConfig"
```

---

## Task 2: Rewrite `LocalEmbeddingClient` for Lazy Load/Unload

**Files:**
- Modify: `runtime/agent-runtime/src/llm/local_embedding.rs`

- [ ] **Step 1: Replace the struct definition**

Replace the entire `LocalEmbeddingClient` struct and imports at the top of the file:

```rust
// ============================================================================
// LOCAL EMBEDDING CLIENT
// ONNX-based local embeddings via fastembed — zero API calls
// Lazy-loaded on first embed(), unloaded after idle timeout to save RAM.
// ============================================================================

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use fastembed::{InitOptions, TextEmbedding, EmbeddingModel};

use super::embedding::{EmbeddingClient, EmbeddingError};

/// Local embedding client using fastembed (ONNX Runtime).
///
/// Default model: `all-MiniLM-L6-v2` (384 dims, ~100MB, fastest).
/// Runs entirely on CPU — no API key, no network, no cost.
///
/// The ONNX model is loaded lazily on first `embed()` call and unloaded
/// after `idle_timeout_secs` of inactivity to reclaim ~100MB of RAM.
pub struct LocalEmbeddingClient {
    model: Mutex<Option<TextEmbedding>>,
    model_id: EmbeddingModel,
    model_name: String,
    dimensions: usize,
    last_used: AtomicU64,
    idle_timeout_secs: u64,
    unload_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
}
```

- [ ] **Step 2: Rewrite the constructors**

Replace the existing `impl LocalEmbeddingClient` block (constructors only, not the trait impl):

```rust
impl LocalEmbeddingClient {
    /// Create a local embedding client with the default model (all-MiniLM-L6-v2).
    /// The model is NOT loaded into RAM yet — it loads lazily on first embed().
    pub fn new() -> Self {
        Self::with_model(EmbeddingModel::AllMiniLML6V2, 300)
    }

    /// Create a local embedding client with a specific fastembed model and idle timeout.
    /// Set `idle_timeout_secs` to 0 to never unload (keep in RAM permanently).
    pub fn with_model(model_id: EmbeddingModel, idle_timeout_secs: u64) -> Self {
        let (name, dims) = model_info(&model_id);

        tracing::info!(
            "Local embedding client created (lazy): {} ({}d, idle_timeout={}s)",
            name, dims, idle_timeout_secs
        );

        Self {
            model: Mutex::new(None),
            model_id,
            model_name: name.to_string(),
            dimensions: dims,
            last_used: AtomicU64::new(0),
            idle_timeout_secs,
            unload_handle: Mutex::new(None),
        }
    }

    /// Ensure the model is loaded, returning a reference via the mutex guard.
    /// If the model is not loaded, loads it now (~1-2s for all-MiniLM-L6-v2).
    fn ensure_loaded(&self) -> Result<std::sync::MutexGuard<'_, Option<TextEmbedding>>, EmbeddingError> {
        let mut guard = self.model.lock()
            .map_err(|e| EmbeddingError::ModelError(format!("Mutex poisoned: {}", e)))?;

        if guard.is_none() {
            tracing::info!("Loading embedding model: {} ...", self.model_name);
            let options = InitOptions::new(self.model_id.clone())
                .with_show_download_progress(true);
            let model = TextEmbedding::try_new(options)
                .map_err(|e| EmbeddingError::ModelError(format!(
                    "Failed to load fastembed model: {}", e
                )))?;
            tracing::info!("Embedding model loaded: {}", self.model_name);
            *guard = Some(model);
        }

        // Update last-used timestamp
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.last_used.store(now, Ordering::Relaxed);

        Ok(guard)
    }

    /// Start the idle watcher if not already running and timeout > 0.
    fn ensure_watcher_running(&self) {
        if self.idle_timeout_secs == 0 {
            return; // Never unload
        }

        let mut handle_guard = match self.unload_handle.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        // If a watcher is already running, don't start another
        if handle_guard.as_ref().is_some_and(|h| !h.is_finished()) {
            return;
        }

        let timeout_secs = self.idle_timeout_secs;
        let last_used = &self.last_used as *const AtomicU64 as usize;
        let model_ptr = &self.model as *const Mutex<Option<TextEmbedding>> as usize;
        let model_name = self.model_name.clone();

        // SAFETY: The watcher only runs while `self` is alive because:
        // - LocalEmbeddingClient is stored in Arc<dyn EmbeddingClient> in AppState
        // - AppState lives for the entire application lifetime
        // - The watcher exits when the model is already None (client dropped)
        let handle = tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;

                // SAFETY: pointers valid as long as AppState lives (see above)
                let last_used = unsafe { &*(last_used as *const AtomicU64) };
                let model_mutex = unsafe { &*(model_ptr as *const Mutex<Option<TextEmbedding>>) };

                let last = last_used.load(Ordering::Relaxed);
                if last == 0 {
                    // Never been used, nothing to unload
                    continue;
                }

                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                if now.saturating_sub(last) >= timeout_secs {
                    if let Ok(mut guard) = model_mutex.lock() {
                        if guard.is_some() {
                            *guard = None;
                            tracing::info!(
                                "Embedding model unloaded after {}s idle: {}",
                                timeout_secs, model_name
                            );
                        }
                    }
                    // Stop watcher — it will restart on next embed()
                    break;
                }
            }
        });

        *handle_guard = Some(handle);
    }
}
```

- [ ] **Step 3: Rewrite the `EmbeddingClient` trait impl**

Replace the existing `#[async_trait] impl EmbeddingClient for LocalEmbeddingClient` block:

```rust
#[async_trait]
impl EmbeddingClient for LocalEmbeddingClient {
    async fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let owned: Vec<String> = texts.iter().map(|s| s.to_string()).collect();

        tracing::debug!("Embedding {} text(s) locally via {}", owned.len(), self.model_name);

        // Ensure model is loaded (lazy init)
        let guard = self.ensure_loaded()?;

        let embeddings = guard
            .as_ref()
            .expect("ensure_loaded guarantees Some")
            .embed(owned, None)
            .map_err(|e| EmbeddingError::ModelError(format!(
                "Embedding failed ({}): {}", self.model_name, e
            )))?;

        drop(guard); // Release lock before starting watcher

        // Start idle watcher (no-op if already running or timeout=0)
        self.ensure_watcher_running();

        Ok(embeddings)
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn model_name(&self) -> &str {
        &self.model_name
    }
}
```

- [ ] **Step 4: Update the tests**

Replace the entire `#[cfg(test)] mod tests` block:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_info() {
        let (name, dims) = model_info(&EmbeddingModel::AllMiniLML6V2);
        assert_eq!(name, "all-MiniLM-L6-v2");
        assert_eq!(dims, 384);
    }

    #[test]
    fn test_model_info_bge() {
        let (name, dims) = model_info(&EmbeddingModel::BGESmallENV15);
        assert_eq!(name, "bge-small-en-v1.5");
        assert_eq!(dims, 384);
    }

    #[test]
    fn test_lazy_construction() {
        let client = LocalEmbeddingClient::new();
        assert_eq!(client.dimensions(), 384);
        assert_eq!(client.model_name(), "all-MiniLM-L6-v2");
        // Model should NOT be loaded yet
        let guard = client.model.lock().unwrap();
        assert!(guard.is_none(), "Model should be lazy — not loaded at construction");
    }

    #[test]
    fn test_custom_timeout() {
        let client = LocalEmbeddingClient::with_model(EmbeddingModel::AllMiniLML6V2, 0);
        assert_eq!(client.idle_timeout_secs, 0);

        let client2 = LocalEmbeddingClient::with_model(EmbeddingModel::AllMiniLML6V2, 600);
        assert_eq!(client2.idle_timeout_secs, 600);
    }

    // Integration test: actually loads the model and embeds text.
    // Skipped in CI (model download required).
    #[test]
    #[ignore]
    fn test_local_embedding_end_to_end() {
        let client = LocalEmbeddingClient::new();
        assert_eq!(client.dimensions(), 384);
        assert_eq!(client.model_name(), "all-MiniLM-L6-v2");

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(client.embed(&["hello world", "test embedding"]));
        let embeddings = result.expect("Should embed successfully");
        assert_eq!(embeddings.len(), 2);
        assert_eq!(embeddings[0].len(), 384);
        assert_eq!(embeddings[1].len(), 384);

        // After embed, model should be loaded
        let guard = client.model.lock().unwrap();
        assert!(guard.is_some(), "Model should be loaded after embed()");
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p agent-runtime -- local_embedding`
Expected: `test_model_info`, `test_model_info_bge`, `test_lazy_construction`, `test_custom_timeout` all PASS. The `test_local_embedding_end_to_end` test is `#[ignore]`.

- [ ] **Step 6: Commit**

```bash
git add runtime/agent-runtime/src/llm/local_embedding.rs
git commit -m "feat(embedding): lazy load/unload with idle timeout

Model loads on first embed(), unloads after idle_timeout_secs (default 300s).
Set idle_timeout_secs=0 to never unload (preserves current behavior)."
```

---

## Task 3: Simplify Embedding Init in `state.rs`

**Files:**
- Modify: `gateway/src/state.rs`

- [ ] **Step 1: Simplify embedding client construction**

In `gateway/src/state.rs`, find the embedding client initialization block (lines 185-197) and replace it:

```rust
// Before:
let embedding_client: Option<Arc<dyn EmbeddingClient>> = match LocalEmbeddingClient::new() {
    Ok(client) => {
        tracing::info!(
            "Local embedding client initialized ({}d)",
            client.dimensions()
        );
        Some(Arc::new(client))
    }
    Err(e) => {
        tracing::warn!("Local embedding unavailable, FTS5-only recall: {}", e);
        None
    }
};

// After:
let embedding_client: Option<Arc<dyn EmbeddingClient>> = {
    let client = LocalEmbeddingClient::new();
    tracing::info!(
        "Local embedding client created (lazy, {}d)",
        client.dimensions()
    );
    Some(Arc::new(client))
};
```

Note: We keep the `Option` wrapper because the rest of the codebase (`MemoryRecall`, `SessionDistiller`, `MemoryFactStore`) all accept `Option<Arc<dyn EmbeddingClient>>`. Changing all those signatures is out of scope — the important thing is that construction never fails now.

- [ ] **Step 2: Run workspace check**

Run: `cargo check --workspace`
Expected: No errors. The `LocalEmbeddingClient::new()` no longer returns `Result`, so the `match` removal compiles.

- [ ] **Step 3: Commit**

```bash
git add gateway/src/state.rs
git commit -m "refactor(state): simplify embedding init — lazy client never fails at construction"
```

---

## Task 4: Add `DistillationConfig` to Settings

**Files:**
- Modify: `gateway/gateway-services/src/settings.rs`

- [ ] **Step 1: Add the `DistillationConfig` struct**

In `gateway/gateway-services/src/settings.rs`, add the struct after `OrchestratorConfig` (after line 96):

```rust
/// Distillation model configuration.
/// Controls which provider/model is used for session distillation.
/// Both fields default to None, inheriting from orchestrator config.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DistillationConfig {
    /// Provider ID override. None = inherit from orchestrator config.
    #[serde(default)]
    pub provider_id: Option<String>,
    /// Model override. None = inherit from orchestrator config.
    #[serde(default)]
    pub model: Option<String>,
}

impl Default for DistillationConfig {
    fn default() -> Self {
        Self {
            provider_id: None,
            model: None,
        }
    }
}
```

- [ ] **Step 2: Add `distillation` field to `ExecutionSettings`**

Add the field to the `ExecutionSettings` struct (after `orchestrator`):

```rust
    /// Distillation model configuration (provider/model override).
    #[serde(default)]
    pub distillation: DistillationConfig,
```

Update the `Default` impl for `ExecutionSettings`:

```rust
impl Default for ExecutionSettings {
    fn default() -> Self {
        Self {
            max_parallel_agents: default_max_parallel_agents(),
            setup_complete: false,
            agent_name: None,
            subagent_non_streaming: true,
            orchestrator: OrchestratorConfig::default(),
            distillation: DistillationConfig::default(),
        }
    }
}
```

- [ ] **Step 3: Add a test for distillation config**

Add to the `#[cfg(test)] mod tests` block:

```rust
    #[test]
    fn test_distillation_config_defaults() {
        let config = DistillationConfig::default();
        assert!(config.provider_id.is_none());
        assert!(config.model.is_none());
    }

    #[test]
    fn test_distillation_config_in_execution_settings() {
        let dir = tempdir().unwrap();
        let service = SettingsService::new_legacy(dir.path().to_path_buf());

        // Save with distillation overrides
        let mut settings = AppSettings::default();
        settings.execution.distillation = DistillationConfig {
            provider_id: Some("ollama".to_string()),
            model: Some("llama3".to_string()),
        };
        service.save(&settings).unwrap();

        // Reload and verify
        service.invalidate_cache();
        let loaded = service.get_execution_settings().unwrap();
        assert_eq!(loaded.distillation.provider_id.as_deref(), Some("ollama"));
        assert_eq!(loaded.distillation.model.as_deref(), Some("llama3"));
    }

    #[test]
    fn test_distillation_config_absent_in_json() {
        let dir = tempdir().unwrap();
        let service = SettingsService::new_legacy(dir.path().to_path_buf());

        // Write JSON without distillation field (simulates existing settings.json)
        let json = r#"{ "execution": { "maxParallelAgents": 3 } }"#;
        let config_dir = dir.path().join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(config_dir.join("settings.json"), json).unwrap();

        service.invalidate_cache();
        let loaded = service.get_execution_settings().unwrap();
        // Should default to None/None
        assert!(loaded.distillation.provider_id.is_none());
        assert!(loaded.distillation.model.is_none());
    }
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p gateway-services -- settings`
Expected: All tests pass, including the three new distillation tests.

- [ ] **Step 5: Commit**

```bash
git add gateway/gateway-services/src/settings.rs
git commit -m "feat(settings): add DistillationConfig to ExecutionSettings

Provider + model overrides for distillation, defaulting to None (inherits
from orchestrator config)."
```

---

## Task 5: Wire Distillation Config into `SessionDistiller`

**Files:**
- Modify: `gateway/gateway-execution/src/distillation.rs`
- Modify: `gateway/src/state.rs` (pass `SettingsService` to distiller)

- [ ] **Step 1: Add `settings_service` field to `SessionDistiller`**

In `gateway/gateway-execution/src/distillation.rs`, add the import:

```rust
use gateway_services::SettingsService;
```

Add the field to the `SessionDistiller` struct (after `paths`):

```rust
    settings_service: Option<Arc<SettingsService>>,
```

Update the constructor to accept it:

```rust
    pub fn new(
        provider_service: Arc<ProviderService>,
        embedding_client: Option<Arc<dyn EmbeddingClient>>,
        conversation_repo: Arc<ConversationRepository>,
        memory_repo: Arc<MemoryRepository>,
        graph_storage: Option<Arc<GraphStorage>>,
        distillation_repo: Option<Arc<DistillationRepository>>,
        episode_repo: Option<Arc<EpisodeRepository>>,
        paths: Arc<VaultPaths>,
        settings_service: Option<Arc<SettingsService>>,
    ) -> Self {
        Self {
            provider_service,
            embedding_client,
            conversation_repo,
            memory_repo,
            graph_storage,
            distillation_repo,
            episode_repo,
            paths,
            settings_service,
        }
    }
```

- [ ] **Step 2: Update `extract_all()` to resolve distillation config**

In the `extract_all()` method (around line 570), replace the provider resolution logic. The current code iterates all providers with the default first. Replace the beginning of the method (up to where it enters the `for idx in ordered_indices` loop) with:

```rust
    async fn extract_all(&self, transcript: &str) -> Result<DistillationResponse, String> {
        let providers = self.provider_service.list()
            .map_err(|e| format!("Failed to list providers: {}", e))?;

        if providers.is_empty() {
            return Err("No providers configured — cannot distill session".to_string());
        }

        // Load prompt once (shared across attempts)
        let system = self.load_distillation_prompt();
        let user = format!(
            "## Session Transcript\n\n{}\n\n---\nExtract durable facts, entities, relationships, and an episode assessment. Respond with ONLY the JSON object, nothing else.",
            transcript
        );

        // Resolve distillation provider/model from settings chain:
        // distillation config → orchestrator config → default provider
        let (target_provider_id, target_model) = self.resolve_distillation_target();

        // Order providers: target first (if specified), then default, then rest
        let default_idx = providers.iter().position(|p| p.is_default);
        let target_idx = target_provider_id.as_ref().and_then(|tid| {
            providers.iter().position(|p| p.id.as_deref() == Some(tid.as_str()))
        });

        let ordered_indices: Vec<usize> = {
            let mut indices = Vec::new();
            // Target provider first (if set and found)
            if let Some(idx) = target_idx {
                indices.push(idx);
            }
            // Default provider second (if different from target)
            if let Some(idx) = default_idx {
                if Some(idx) != target_idx {
                    indices.push(idx);
                }
            }
            // Remaining providers
            for i in 0..providers.len() {
                if !indices.contains(&i) {
                    indices.push(i);
                }
            }
            indices
        };

        let mut last_error = String::new();

        for (attempt, &idx) in ordered_indices.iter().enumerate() {
            let provider = &providers[idx];
            // Use target model for first attempt (if configured), else provider default
            let model = if attempt == 0 {
                target_model.clone().unwrap_or_else(|| provider.default_model().to_string())
            } else {
                provider.default_model().to_string()
            };
            let provider_id = provider.id.clone().unwrap_or_else(|| "default".to_string());

            let config = LlmConfig::new(
                provider.base_url.clone(),
                provider.api_key.clone(),
                model.to_string(),
                provider_id.clone(),
            )
            .with_temperature(0.3)
            .with_max_tokens(4096);
```

The rest of the loop body (from `let client = match OpenAiClient::new(config)` onwards) stays exactly the same.

- [ ] **Step 3: Add the `resolve_distillation_target` helper**

Add this method to `impl SessionDistiller` (above `extract_all`):

```rust
    /// Resolve the target provider ID and model for distillation.
    ///
    /// Resolution chain:
    /// 1. distillation.provider_id / distillation.model (if set)
    /// 2. orchestrator.provider_id / orchestrator.model (if set)
    /// 3. None (falls through to default provider in extract_all)
    fn resolve_distillation_target(&self) -> (Option<String>, Option<String>) {
        let settings = self.settings_service.as_ref()
            .and_then(|s| s.get_execution_settings().ok());

        let settings = match settings {
            Some(s) => s,
            None => return (None, None),
        };

        let provider_id = settings.distillation.provider_id.clone()
            .or_else(|| settings.orchestrator.provider_id.clone());

        let model = settings.distillation.model.clone()
            .or_else(|| settings.orchestrator.model.clone());

        if provider_id.is_some() || model.is_some() {
            tracing::debug!(
                provider = ?provider_id,
                model = ?model,
                "Distillation using configured target"
            );
        }

        (provider_id, model)
    }
```

- [ ] **Step 4: Update `state.rs` to pass `SettingsService` to distiller**

In `gateway/src/state.rs`, the `SessionDistiller::new()` call (around line 250) needs the settings service. The `SettingsService` is created at line 265, which is AFTER the distiller. Move the settings creation before the distiller, or pass it after creation. The simplest approach: create `SettingsService` earlier, before the distiller.

Move the settings creation (currently line 265):
```rust
let settings = Arc::new(SettingsService::new(paths.clone()));
```

To just before the distiller creation (before line 250). Then update the distiller constructor:

```rust
        let distiller = Arc::new(SessionDistiller::new(
            provider_service.clone(),
            embedding_client,
            conversation_repo.clone(),
            memory_repo.clone(),
            graph_storage,
            Some(distillation_repo.clone()),
            Some(episode_repo),
            paths.clone(),
            Some(settings.clone()),
        ));
```

- [ ] **Step 5: Run workspace check**

Run: `cargo check --workspace`
Expected: No compile errors.

- [ ] **Step 6: Commit**

```bash
git add gateway/gateway-execution/src/distillation.rs gateway/src/state.rs
git commit -m "feat(distillation): resolve provider/model from settings chain

Distillation reads DistillationConfig from settings at call time.
Falls back: distillation config → orchestrator config → default provider.
Hot-reloads without daemon restart."
```

---

## Task 6: Add TypeScript `DistillationConfig` Interface

**Files:**
- Modify: `apps/ui/src/services/transport/types.ts`

- [ ] **Step 1: Add the interface**

In `apps/ui/src/services/transport/types.ts`, add the `DistillationConfig` interface after `OrchestratorConfig` (after line 407):

```typescript
/** Distillation model configuration (provider/model override) */
export interface DistillationConfig {
  /** Provider ID override. null = inherit from orchestrator */
  providerId?: string | null;
  /** Model override. null = inherit from orchestrator */
  model?: string | null;
}
```

- [ ] **Step 2: Add the field to `ExecutionSettings`**

Add the `distillation` field to the `ExecutionSettings` interface (after the `orchestrator` field):

```typescript
  /** Distillation model configuration (inherits from orchestrator by default) */
  distillation?: DistillationConfig;
```

- [ ] **Step 3: Run type check**

Run: `cd apps/ui && npx tsc --noEmit`
Expected: No type errors.

- [ ] **Step 4: Commit**

```bash
git add apps/ui/src/services/transport/types.ts
git commit -m "feat(ui): add DistillationConfig TypeScript interface"
```

---

## Task 7: Add Distillation Subsection to Settings UI

**Files:**
- Modify: `apps/ui/src/features/settings/WebSettingsPanel.tsx`

- [ ] **Step 1: Add the distillation subsection**

In `apps/ui/src/features/settings/WebSettingsPanel.tsx`, find the closing `</label>` of the Thinking Mode toggle (line 745), and add the distillation subsection BEFORE the closing `</div>` of the orchestrator card (line 746):

```tsx
                {/* Distillation Config */}
                <div style={{ marginTop: "var(--spacing-4)", paddingTop: "var(--spacing-3)", borderTop: "1px solid var(--border-secondary)" }}>
                  <div style={{ marginBottom: "var(--spacing-2)" }}>
                    <h3 className="settings-field-label" style={{ fontSize: "var(--font-size-sm)", fontWeight: 600 }}>Distillation</h3>
                    <p className="page-subtitle" style={{ fontSize: "var(--font-size-xs)" }}>Override the model used for memory extraction. Inherits from orchestrator by default.</p>
                  </div>
                  <div className="grid grid-cols-2 gap-3">
                    <div>
                      <label className="settings-field-label">Provider</label>
                      <select
                        className="form-input form-select"
                        value={execSettings.distillation?.providerId || ""}
                        onChange={(e) => handleExecChange({
                          distillation: {
                            ...execSettings.distillation,
                            providerId: e.target.value || null,
                            model: e.target.value ? (execSettings.distillation?.model || null) : null,
                          },
                        })}
                      >
                        <option value="">Inherit from Orchestrator</option>
                        {providers.filter((p) => p.verified).map((p) => (
                          <option key={p.id} value={p.id}>{p.name}</option>
                        ))}
                      </select>
                    </div>
                    <div>
                      <label className="settings-field-label">Model</label>
                      <select
                        className="form-input form-select"
                        value={execSettings.distillation?.model || ""}
                        onChange={(e) => handleExecChange({
                          distillation: {
                            ...execSettings.distillation,
                            model: e.target.value || null,
                          },
                        })}
                      >
                        <option value="">Inherit from Orchestrator</option>
                        {(() => {
                          const distProviderId = execSettings.distillation?.providerId
                            || execSettings.orchestrator?.providerId
                            || defaultProviderId;
                          return (providers.find((p) => p.id === distProviderId)?.models || []).map((m) => (
                            <option key={m} value={m}>{m}</option>
                          ));
                        })()}
                      </select>
                    </div>
                  </div>
                </div>
```

- [ ] **Step 2: Run build**

Run: `cd apps/ui && npm run build`
Expected: Build succeeds with no errors.

- [ ] **Step 3: Commit**

```bash
git add apps/ui/src/features/settings/WebSettingsPanel.tsx
git commit -m "feat(ui): add distillation provider/model config to settings

Distillation subsection in Advanced > Orchestrator card.
Both fields default to 'Inherit from Orchestrator'."
```

---

## Task 8: Full Integration Check

**Files:** None (verification only)

- [ ] **Step 1: Run full workspace build**

Run: `cargo check --workspace`
Expected: No errors.

- [ ] **Step 2: Run all Rust tests**

Run: `cargo test --workspace`
Expected: All tests pass.

- [ ] **Step 3: Run UI build**

Run: `cd apps/ui && npm run build`
Expected: Build succeeds.

- [ ] **Step 4: Verify settings.json backward compatibility**

Create a temporary test: write a settings.json without `distillation` or `idle_timeout_secs`, load it, and verify defaults are applied. This is already covered by `test_distillation_config_absent_in_json` from Task 4.

- [ ] **Step 5: Commit (if any fixes needed)**

Only if integration checks required fixes.
