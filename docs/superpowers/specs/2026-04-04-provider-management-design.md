# Provider Management Redesign — Rate Limiting, Model Capabilities & UI

## Problem Statement

1. **429/connection failures** — multiple agents hit the same provider concurrently with no rate limiting. Z.AI drops connections, subagents crash, sessions fail.
2. **max_tokens crashes** — agent config has `maxTokens: 100000` but model only supports 65536. No clamping.
3. **providers.json is flat** — models are a `string[]` with no capabilities, no token limits. The system guesses from the bundled registry.
4. **UI shows models as dumb chips** — no capabilities, no limits, no rate limit config.
5. **No auto-discovery** — "Test Connection" finds model IDs but doesn't enrich with capabilities.

## Design Overview

### Data Model

`providers.json` evolves from flat model list to enriched model configs:

```json
{
  "id": "z.ai",
  "name": "Z.AI",
  "apiKey": "...",
  "baseUrl": "https://api.z.ai/api/coding/paas/v4",
  "verified": true,
  "isDefault": true,
  "defaultModel": "glm-5.1",
  "rateLimits": {
    "requestsPerMinute": 30,
    "concurrentRequests": 2
  },
  "models": {
    "glm-5.1": {
      "capabilities": { "text": true, "tools": true, "vision": false, "thinking": false, "embeddings": false },
      "maxInput": 128000,
      "maxOutput": 16384,
      "source": "registry"
    },
    "glm-4.7": {
      "capabilities": { "text": true, "tools": true, "vision": true, "thinking": false, "embeddings": false },
      "maxInput": 128000,
      "maxOutput": 8192,
      "source": "registry"
    }
  }
}
```

Key fields:
- `models` changes from `string[]` to `Record<string, ModelConfig>`
- Each model has `capabilities`, `maxInput`, `maxOutput`, `source`
- `source`: `"registry"` (from bundled), `"discovered"` (from API test), `"user"` (manual override)
- User overrides (`source: "user"`) survive "Test Connection" re-runs
- `rateLimits`: per-provider throttle config

### Backward Compatibility

On load, if `models` is a `string[]` (old format), auto-migrate:
1. For each model string, look up in bundled registry
2. Convert to `Record<string, ModelConfig>` with registry data
3. Save back to `providers.json`

This is a one-time silent migration. No user action needed.

---

## Change 1: Provider Struct (Backend)

**File:** `gateway/gateway-services/src/providers.rs`

### New types

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimits {
    #[serde(default = "default_rpm")]
    pub requests_per_minute: u32,
    #[serde(default = "default_concurrent")]
    pub concurrent_requests: u32,
}

fn default_rpm() -> u32 { 60 }
fn default_concurrent() -> u32 { 4 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub capabilities: ModelCapabilities,
    #[serde(default)]
    pub max_input: Option<u64>,
    #[serde(default)]
    pub max_output: Option<u64>,
    #[serde(default = "default_source")]
    pub source: String,
}

fn default_source() -> String { "registry".to_string() }
```

### Provider struct changes

```rust
pub struct Provider {
    // ... existing fields ...

    /// Per-model configuration with capabilities and token limits.
    /// Replaces the old `models: Vec<String>`.
    #[serde(default)]
    pub models: ProviderModels,

    /// Rate limiting configuration.
    #[serde(default)]
    pub rate_limits: Option<RateLimits>,

    // Remove: pub models: Vec<String>  (replaced by ProviderModels)
}

/// Supports both old format (string array) and new format (model configs).
/// Deserializes from either, always serializes as the new format.
#[derive(Debug, Clone)]
pub enum ProviderModels {
    Legacy(Vec<String>),
    Enriched(HashMap<String, ModelConfig>),
}
```

Custom serde for `ProviderModels` to handle both formats transparently.

### Helper methods

```rust
impl Provider {
    /// Get model IDs (works with both legacy and enriched).
    pub fn model_ids(&self) -> Vec<String> { ... }

    /// Get model config (falls back to registry for legacy format).
    pub fn model_config(&self, model_id: &str, registry: &ModelRegistry) -> ModelConfig { ... }

    /// Get effective max_output for a model (for clamping agent max_tokens).
    pub fn effective_max_output(&self, model_id: &str, registry: &ModelRegistry) -> u64 { ... }

    /// Enrich models from registry (called after test/discovery).
    pub fn enrich_models(&mut self, registry: &ModelRegistry) { ... }
}
```

---

## Change 2: Rate Limiter (Backend)

**File:** `runtime/agent-runtime/src/llm/rate_limiter.rs` (new)

### Per-provider shared rate limiter

```rust
use std::sync::Arc;
use tokio::sync::{Semaphore, Mutex};
use std::time::{Instant, Duration};

/// Per-provider rate limiter shared across all executors.
pub struct ProviderRateLimiter {
    /// Concurrency semaphore — max N simultaneous requests.
    concurrency: Arc<Semaphore>,
    /// Sliding window rate counter — max N requests per minute.
    window: Arc<Mutex<SlidingWindow>>,
    /// Config
    rpm: u32,
}

struct SlidingWindow {
    timestamps: Vec<Instant>,
    max_per_minute: u32,
}

impl ProviderRateLimiter {
    pub fn new(concurrent: u32, rpm: u32) -> Self { ... }

    /// Acquire a slot. Waits if rate limited. Never fails — only waits.
    pub async fn acquire(&self) -> RateLimitGuard { ... }

    /// Called when a 429 is received — halve the rpm and log.
    pub fn on_rate_limited(&self) { ... }
}

/// RAII guard that releases concurrency semaphore on drop.
pub struct RateLimitGuard { ... }
```

### Registry of rate limiters

```rust
/// Global registry of per-provider rate limiters.
/// Created at runner startup, shared across all executors.
pub struct RateLimiterRegistry {
    limiters: RwLock<HashMap<String, Arc<ProviderRateLimiter>>>,
}

impl RateLimiterRegistry {
    pub fn get_or_create(&self, provider_id: &str, config: &RateLimits) -> Arc<ProviderRateLimiter> { ... }
}
```

### Integration point

In `gateway/gateway-execution/src/invoke/executor.rs`, wrap the LLM client:

```rust
// Current stack:
// OpenAiClient → RetryingLlmClient → ThrottledLlmClient

// New stack:
// OpenAiClient → RetryingLlmClient → RateLimitedLlmClient(per-provider) → ThrottledLlmClient(deprecated)
```

Or simpler: replace `ThrottledLlmClient` with the new `ProviderRateLimiter`. The rate limiter is acquired before each `chat()` / `chat_stream()` call.

---

## Change 3: max_tokens Clamping (Backend)

**File:** `gateway/gateway-execution/src/invoke/executor.rs`

After resolving `executor_config.max_tokens = agent.max_tokens`, add clamping:

```rust
// Clamp max_tokens to model's actual output limit
let model_max_output = provider.effective_max_output(&agent.model, &model_registry);
if executor_config.max_tokens as u64 > model_max_output {
    tracing::warn!(
        agent = %agent.id,
        model = %agent.model,
        requested = executor_config.max_tokens,
        clamped_to = model_max_output,
        "Clamped max_tokens to model's output limit"
    );
    executor_config.max_tokens = model_max_output as u32;
}
```

This prevents the `100000 > 65536` crash. The agent config says "I want 100K" but the system says "this model supports 65K, using 65K."

---

## Change 4: Test Connection Enhancement (Backend)

**File:** `gateway/gateway-services/src/providers.rs` (`test()` method)

After discovering model IDs from `/models`, enrich each model:

```rust
pub async fn test_and_enrich(&mut self, registry: &ModelRegistry) -> ProviderTestResult {
    // 1. Hit /models endpoint → get model IDs
    let model_ids = self.discover_models().await?;

    // 2. For each model, check registry for capabilities + limits
    let mut enriched = HashMap::new();
    for model_id in &model_ids {
        let profile = registry.get(model_id);  // Returns fallback if unknown
        let config = ModelConfig {
            capabilities: profile.capabilities.clone(),
            max_input: Some(profile.context.input),
            max_output: profile.context.output,
            source: if registry.has(model_id) { "registry" } else { "discovered" }.to_string(),
        };

        // Don't overwrite user overrides
        if let Some(existing) = self.models.get(model_id) {
            if existing.source == "user" {
                enriched.insert(model_id.clone(), existing.clone());
                continue;
            }
        }

        enriched.insert(model_id.clone(), config);
    }

    // 3. Update provider's models
    self.models = ProviderModels::Enriched(enriched);

    // 4. Save to disk
    // (caller persists via provider_service.update())

    ProviderTestResult { success: true, message: "Connected", models: model_ids }
}
```

---

## Change 5: Bundled Registry as Starter File

**File:** `gateway/gateway-templates/src/lib.rs`

On first run, copy `models_registry.json` to `config/models.json`:

```rust
let models_path = config_dir.join("models.json");
if !models_path.exists() {
    if let Some(bundled) = Templates::get("models_registry.json") {
        let _ = std::fs::write(&models_path, &bundled.data);
        tracing::info!("Created config/models.json from bundled registry");
    }
}
```

The `ModelRegistry::load()` already reads `config/models.json` as local overrides. Now it always exists (copied from bundled on first run). Power users edit it directly.

---

## Change 6: Expand Bundled Registry

**File:** `gateway/templates/models_registry.json`

Add/verify entries for all major providers. Current: 52 models. Target: ~80+ covering:

| Provider | Models to add/verify |
|----------|---------------------|
| OpenAI | gpt-4o, gpt-4o-mini, gpt-4.1, gpt-4.1-mini, o3, o3-mini, o4-mini |
| Anthropic | claude-sonnet-4-6, claude-opus-4-6, claude-haiku-4-5 |
| Z.AI / GLM | glm-5.1, glm-5, glm-5-turbo, glm-4.7, glm-4.6, glm-4.5 |
| DeepSeek | deepseek-chat, deepseek-reasoner |
| Ollama Cloud | minimax-m2.7:cloud, qwen3.5:397b-cloud, nemotron-3-super:cloud |
| Google | gemini-2.5-pro, gemini-2.5-flash |
| Mistral | mistral-large, mistral-medium, codestral |
| Alibaba | qwen-max, qwen-plus, qwen-turbo, qwen-vl-max |
| OpenRouter | Pass-through — uses underlying model IDs |

Each entry needs: `name`, `provider`, `capabilities` (all 7 flags), `context` (input + output), `embedding` (if applicable).

---

## Change 7: UI Updates

### Provider Types (frontend)

```typescript
interface ProviderResponse {
  id?: string;
  name: string;
  description: string;
  apiKey: string;
  baseUrl: string;
  models: Record<string, ModelConfig> | string[];  // backward compat
  defaultModel?: string;
  verified?: boolean;
  isDefault?: boolean;
  rateLimits?: RateLimits;
  createdAt?: string;
}

interface ModelConfig {
  capabilities: ModelCapabilities;
  maxInput?: number;
  maxOutput?: number;
  source: "registry" | "discovered" | "user";
}

interface RateLimits {
  requestsPerMinute: number;
  concurrentRequests: number;
}
```

### Provider Card (grid)

- Show aggregated capability badges from all models (union of capabilities)
- Badge colors: green for text/tools, blue for vision, purple for thinking, orange for embeddings

### Provider Detail Slideover

- **New section:** Rate Limits with inputs for RPM and concurrent
- **Models section:** Each model row shows: name, capability badges, token limits (compact)
- Model rows are clickable → opens Model Editor

### Model Editor (new sub-panel or modal)

- Capability toggles (text, tools, vision, thinking, embeddings, voice)
- Token limit inputs (maxInput, maxOutput) pre-filled from registry
- Source indicator ("From bundled registry" / "User override")
- Embedding config (dimensions, maxDimensions) — conditional on embeddings toggle
- Save marks `source: "user"` — survives re-test

### Edit Mode

- Rate limits section editable alongside name/apiKey/baseUrl
- Models manageable: add (text input), remove (X button), edit (click → Model Editor)
- "Test Connection" button triggers auto-enrichment and refreshes model list

---

## Change 8: API Endpoints

### Existing (modify response shape)

- `GET /api/providers` — returns providers with enriched `models: Record<string, ModelConfig>`
- `GET /api/providers/:id` — same
- `PUT /api/providers/:id` — accepts new fields (rateLimits, enriched models)
- `POST /api/providers/:id/test` — returns enriched models in response

### New

- `GET /api/models/registry` — returns the full model registry (for UI model editor pre-fill)
- `PUT /api/models/registry/:modelId` — update a model in the local registry override (`config/models.json`)

---

## Implementation Phases

### Phase 1: Backend Data Model + Rate Limiter (highest impact)
1. New types: `RateLimits`, `ModelConfig`, `ProviderModels`
2. Provider struct migration (backward compat serde)
3. `ProviderRateLimiter` with concurrency + sliding window
4. `RateLimiterRegistry` shared across executors
5. Wire rate limiter into executor builder
6. max_tokens clamping
7. Test connection enrichment

### Phase 2: Registry Expansion + Starter File
1. Expand `models_registry.json` to ~80 models
2. Copy to `config/models.json` on first run
3. API endpoints for registry access

### Phase 3: UI
1. Provider types update (frontend)
2. Provider card capability badges
3. Provider detail — rate limits section
4. Provider detail — enriched model rows
5. Model Editor sub-panel
6. Edit mode — rate limits + model management
7. Test Connection UI refresh with enriched data

---

## Expected Impact

| Problem | Fix |
|---------|-----|
| 429/connection crashes | Per-provider rate limiter waits instead of failing |
| max_tokens > model limit | Automatic clamping with warning log |
| Unknown model capabilities | Registry enrichment on test + user override |
| Flat providers.json | Enriched model configs with capabilities + limits |
| Dumb model chips in UI | Capability badges, token limits, model editor |
| No rate limit config | UI section with RPM + concurrent inputs |
