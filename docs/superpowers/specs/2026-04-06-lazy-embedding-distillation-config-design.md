# Lazy Embedding Model Management & Distillation Config

**Date:** 2026-04-06
**Status:** Draft
**Scope:** Two independent improvements — (1) lazy load/unload of local embedding models to save RAM, (2) configurable distillation provider/model inheriting from orchestrator settings.

---

## 1. Lazy Embedding Model with Idle Unload

### Problem

The local fastembed ONNX model (`all-MiniLM-L6-v2`, ~100MB) is loaded eagerly at application startup and remains resident in RAM for the entire process lifetime. Embedding operations (memory recall, fact saving, contradiction detection) are infrequent and bursty — a cluster of operations at session start/end, then potentially hours of idle time. The model should be loaded on demand and unloaded after an idle period to reclaim memory.

### Design

#### Data Model

Replace the eagerly-loaded `TextEmbedding` in `LocalEmbeddingClient` with a lazy-init-with-cleanup pattern:

```rust
// runtime/agent-runtime/src/llm/local_embedding.rs
pub struct LocalEmbeddingClient {
    model: Mutex<Option<TextEmbedding>>,
    model_id: EmbeddingModel,           // stored to recreate on reload
    model_name: String,
    dimensions: usize,
    last_used: AtomicU64,               // epoch secs of last embed() call
    idle_timeout_secs: u64,             // 0 = never unload
    unload_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
}
```

Add `idle_timeout_secs` to `EmbeddingConfig`:

```rust
// runtime/agent-runtime/src/llm/embedding.rs
pub struct EmbeddingConfig {
    // ... existing fields ...
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_secs: u64,  // default 300 (5 min), 0 = never unload
}
```

#### Lifecycle

1. **Construction** (`new()` / `with_model()`): Does NOT load the ONNX model. Stores `model_id` and metadata only. Model field starts as `None`. Construction is infallible (no `Result` needed).
2. **First `embed()` call**: Acquires mutex, sees `None`, calls `TextEmbedding::try_new()` to load (~1-2s), embeds, updates `last_used`, starts the idle watcher if not already running.
3. **Subsequent `embed()` calls**: Model is `Some`, embed directly, update `last_used`.
4. **Idle watcher**: A `tokio::spawn` task that wakes every 60s, checks `now - last_used > idle_timeout_secs`, and if expired, acquires the mutex and sets `*guard = None` (dropping ONNX weights, reclaiming ~100MB). Watcher stops itself when model is already `None`.
5. **`idle_timeout_secs = 0`**: No watcher spawned. Model stays loaded permanently after first use (opt-in to current behavior).

#### Thread Safety

- `TextEmbedding` is `!Send` but never moves across threads — it lives behind `Mutex<Option<>>` and all operations happen under the lock on whichever thread acquires it.
- The idle watcher task only acquires the mutex to set it to `None` — no `Send` requirement on `TextEmbedding`.
- Concurrent embed + unload: watcher acquires lock, unloads; simultaneous `embed()` waits for lock, finds `None`, reloads. Brief contention, no correctness issue.

#### Startup Change (`state.rs`)

`LocalEmbeddingClient::new()` becomes infallible. The current `match` block:

```rust
// Before
let embedding_client: Option<Arc<dyn EmbeddingClient>> = match LocalEmbeddingClient::new() {
    Ok(client) => Some(Arc::new(client)),
    Err(e) => {
        tracing::warn!("Local embedding unavailable, FTS5-only recall: {}", e);
        None
    }
};
```

Simplifies to:

```rust
// After
let embedding_client: Arc<dyn EmbeddingClient> = Arc::new(LocalEmbeddingClient::new());
```

Failure handling moves to runtime: if the model can't load on first `embed()`, the error propagates and callers fall back to FTS5 as they do today.

#### Fallback Behavior

Existing FTS5 fallback remains unchanged. If `embed()` returns an error (model download fails, ONNX init fails), recall degrades to BM25-only search. This is the same behavior as today, just triggered at first use instead of startup.

---

## 2. Configurable Distillation Provider/Model

### Problem

Distillation currently uses the default provider's default model with hardcoded temperature (0.3) and max_tokens (4096). Users want to run distillation on a cheaper/faster model (e.g., Haiku) while the orchestrator uses a more capable model (e.g., Opus). The distillation model should be independently configurable, inheriting from orchestrator settings by default.

### Design

#### Data Model — Rust

```rust
// gateway/gateway-services/src/settings.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DistillationConfig {
    /// Provider ID override. None = inherit from orchestrator config.
    pub provider_id: Option<String>,
    /// Model override. None = inherit from orchestrator config.
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

Added to `ExecutionSettings`:

```rust
pub struct ExecutionSettings {
    pub max_parallel_agents: u32,
    pub setup_complete: bool,
    pub agent_name: Option<String>,
    pub subagent_non_streaming: bool,
    pub orchestrator: OrchestratorConfig,
    pub distillation: DistillationConfig,  // NEW
}
```

#### Data Model — TypeScript

```typescript
// apps/ui/src/services/transport/types.ts
export interface DistillationConfig {
  providerId?: string | null;
  model?: string | null;
}

export interface ExecutionSettings {
  // ... existing fields ...
  orchestrator?: OrchestratorConfig;
  distillation?: DistillationConfig;  // NEW
}
```

#### Resolution Chain

When `extract_all()` resolves which provider/model to use:

```
1. distillation.provider_id  → if set, use this provider
2. orchestrator.provider_id  → if set, use orchestrator's provider
3. default provider          → first provider marked is_default

1. distillation.model        → if set, use this model
2. orchestrator.model        → if set, use orchestrator's model
3. provider.default_model()  → provider's default model
```

Temperature (0.3) and max_tokens (4096) remain fixed — these are tuned for reliable JSON extraction and not user-configurable.

#### Distiller Integration

`SessionDistiller` gets a new field:

```rust
pub struct SessionDistiller {
    // ... existing fields ...
    settings_service: Arc<SettingsService>,  // NEW — to read distillation config
}
```

In `extract_all()`, before building the provider fallback chain:

```rust
// 1. Read distillation + orchestrator config
let exec_settings = self.settings_service.get_execution_settings();
let dist_config = exec_settings.map(|s| s.distillation).unwrap_or_default();
let orch_config = exec_settings.map(|s| s.orchestrator);

// 2. Resolve target provider
let target_provider_id = dist_config.provider_id
    .or_else(|| orch_config.as_ref().and_then(|o| o.provider_id.clone()));

// 3. Resolve target model
let target_model = dist_config.model
    .or_else(|| orch_config.as_ref().and_then(|o| o.model.clone()));

// 4. Build LlmConfig with resolved values, fallback chain for remaining providers
```

#### Hot-Reload

Settings are read at call time in `extract_all()`, not cached at construction. Changing the distillation provider/model in the UI takes effect on the next distillation without requiring a daemon restart.

#### UI

A "Distillation" subsection added below the existing orchestrator fields in the Advanced > Orchestrator card in `WebSettingsPanel.tsx`:

- **Provider dropdown**: Options are "Inherit from Orchestrator" (null) + all verified providers
- **Model dropdown**: Shows models from the selected provider. Hidden when provider is set to "Inherit from Orchestrator" and orchestrator also has no override.

Both fields default to "Inherit from Orchestrator" (null values).

---

## Files Changed

| File | Change |
|------|--------|
| `runtime/agent-runtime/src/llm/local_embedding.rs` | Lazy load/unload with `Mutex<Option<TextEmbedding>>`, idle watcher |
| `runtime/agent-runtime/src/llm/embedding.rs` | Add `idle_timeout_secs` to `EmbeddingConfig` |
| `gateway/src/state.rs` | Simplify embedding init (infallible), pass `SettingsService` to distiller |
| `gateway/gateway-execution/src/distillation.rs` | Resolve provider/model from settings chain in `extract_all()` |
| `gateway/gateway-services/src/settings.rs` | Add `DistillationConfig` struct + field on `ExecutionSettings` |
| `apps/ui/src/services/transport/types.ts` | Add `DistillationConfig` interface |
| `apps/ui/src/features/settings/WebSettingsPanel.tsx` | Distillation subsection in orchestrator card |

## Files NOT Changed

- `EmbeddingClient` trait — no signature changes
- `OpenAiEmbeddingClient` — stateless HTTP, unaffected
- `MemoryRecall`, `MemoryFactStore`, `EpisodeRepository` — consumers of `EmbeddingClient` trait, unaffected
- Distillation prompt or JSON parsing logic — unchanged
- `recall_config.rs` — unaffected

## Testing

- **Embedding lazy load**: Verify model is `None` after construction, `Some` after first `embed()`, `None` again after idle timeout expires.
- **Idle timeout = 0**: Verify model stays loaded permanently after first use.
- **Distillation config resolution**: Unit test the 3-level fallback chain (distillation → orchestrator → default provider).
- **UI**: Verify "Inherit from Orchestrator" is default, changing provider updates model list, save persists to settings.json.
