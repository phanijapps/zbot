# Model Capabilities Registry — Design Spec

## Problem

Models are stored as plain strings (`"glm-5.1"`, `"gpt-4o"`). The system has zero metadata about what any model can actually do. Context windows are hardcoded in two divergent functions: `get_model_context_window()` in `runtime/agent-runtime/src/middleware/token_counter.rs` and `get_context_window()` in `framework/zero-middleware/src/token_counter.rs`. These have different match logic and return different values for the same models.

The only "capability" flags are two booleans on the Agent struct (`thinking_enabled`, `voice_recording_enabled`) — per-agent settings with no model-level validation.

This means:
- You can enable thinking on a model that doesn't support it — silent misconfiguration
- Context windows for new models default to 8,192 unless you override at the provider level
- No way to know if a model supports vision, tools, embeddings, or generation
- When multimodal features come, there's no infrastructure to route inputs to capable models

## Goal

A model capabilities registry that:
1. Ships a bundled catalog of known models with their capabilities
2. Supports local overrides for custom/new models via `config/models.json`
3. Replaces both hardcoded context window lookups (runtime and framework)
4. Validates agent configurations against actual model capabilities
5. Surfaces capability info in UI model dropdowns
6. Provides a clean extension path for remote registry updates

## Non-Goals

- No dedicated model management UI panel (capabilities shown inline in existing dropdowns)
- No remote fetch mechanism (reserved for future)
- No cost-aware routing (pricing field reserved but null)
- No automatic model selection — agents still explicitly choose models
- No hot-reload of `config/models.json` — changes take effect on restart (v1 limitation)

## Model Data Schema

Each model entry is keyed by its model ID (the same string used in `provider.models` and `agent.model`):

```json
{
  "glm-5.1": {
    "name": "GLM-5.1",
    "provider": "zhipu",
    "capabilities": {
      "tools": true,
      "vision": true,
      "thinking": true,
      "embeddings": false,
      "voice": false,
      "imageGeneration": false,
      "videoGeneration": false
    },
    "context": {
      "input": 128000,
      "output": 8192
    },
    "embedding": null,
    "pricing": null
  }
}
```

**Note:** All example data above is illustrative. Real model data will be populated from actual vendor specifications during implementation.

### Field Definitions

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Human-readable display name |
| `provider` | string | Canonical vendor label for display/grouping ("openai", "zhipu", "anthropic", "deepseek", "google", "meta"). Not a lookup key — does not map to `Provider.id` or `Provider.name`. |
| `capabilities.tools` | bool | Supports function/tool calling |
| `capabilities.vision` | bool | Accepts image inputs |
| `capabilities.thinking` | bool | Has extended reasoning/chain-of-thought mode |
| `capabilities.embeddings` | bool | Produces vector embeddings (not a chat model) |
| `capabilities.voice` | bool | Accepts or produces audio |
| `capabilities.imageGeneration` | bool | Generates images (DALL-E style) |
| `capabilities.videoGeneration` | bool | Generates or accepts video |
| `context.input` | u64 | Max input tokens |
| `context.output` | u64 or null | Max output tokens. When `null`, resolved as `input` (same budget). Use `resolved_output()` helper. |
| `embedding` | object or null | Present only for embedding models |
| `embedding.dimensions` | u32 | Default embedding vector dimensions |
| `embedding.maxDimensions` | u32 or null | Max configurable dimensions |
| `pricing` | object or null | Reserved for future cost-aware routing. Not included in Rust struct yet. |

## Three-Layer Resolution

```
resolve_model("glm-5.1")
    |
    +-- 1. Local overrides: config/models.json      <-- user edits, highest priority
    |
    +-- 2. Bundled registry: models_registry.json    <-- embedded in binary
    |
    +-- 3. Unknown model default                     <-- conservative fallback
```

### Layer 1 — Local Overrides

File: `~/Documents/zbot/config/models.json`

- Same location as `providers.json`, `OS.md`
- Sparse — only models you want to override or add
- Merged on top of bundled registry by model ID
- **Error handling:** If the file exists but is malformed JSON, log a warning and fall back to bundled-only. A bad override file must not prevent the system from starting.

### Layer 2 — Bundled Registry

File: `gateway/templates/models_registry.json`

- Embedded via `rust-embed` (same mechanism as OS templates, shards)
- Community-maintained, updated with code releases
- Contains known models across major providers

### Layer 3 — Unknown Model Fallback

When a model ID isn't found in either layer, the registry returns a default profile. The fallback is stored as a field on `ModelRegistry` (not constructed on the fly) so `get()` can return a reference.

```json
{
  "name": "<model-id>",
  "provider": "unknown",
  "capabilities": {
    "tools": true,
    "vision": false,
    "thinking": false,
    "embeddings": false,
    "voice": false,
    "imageGeneration": false,
    "videoGeneration": false
  },
  "context": {
    "input": 8192,
    "output": 4096
  }
}
```

Conservative: assumes tool calling (most modern models have it), disables everything else, small context window.

### Future Layer 0 — Remote Fetch (not built now)

When implemented:
- `POST /api/models/sync` fetches from a URL
- Writes results into `config/models.json`
- Priority becomes: local override > remote cache > bundled > fallback

## Rust Data Model

New file: `gateway/gateway-services/src/models.rs`

All structs follow existing codebase patterns (`providers.rs`, `agents.rs`) with standard derives and serde renames for camelCase JSON:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelProfile {
    pub name: String,
    /// Canonical vendor label for display/grouping. Not a Provider.id lookup key.
    pub provider: String,
    pub capabilities: ModelCapabilities,
    pub context: ContextWindow,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding: Option<EmbeddingSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCapabilities {
    pub tools: bool,
    pub vision: bool,
    pub thinking: bool,
    pub embeddings: bool,
    pub voice: bool,
    pub image_generation: bool,
    pub video_generation: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextWindow {
    pub input: u64,
    pub output: Option<u64>,
}

impl ContextWindow {
    /// Resolve output token limit. Returns explicit output if set, otherwise input.
    pub fn resolved_output(&self) -> u64 {
        self.output.unwrap_or(self.input)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingSpec {
    pub dimensions: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_dimensions: Option<u32>,
}

/// Capability enum for programmatic checks via `has_capability()`.
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
```

## ModelRegistry Service

Same file: `gateway/gateway-services/src/models.rs`

```rust
pub struct ModelRegistry {
    models: HashMap<String, ModelProfile>,
    /// Pre-built fallback for unknown models. Stored so get() can return &ModelProfile.
    fallback: ModelProfile,
}

impl ModelRegistry {
    /// Load from bundled bytes + local override file, merge into single registry.
    /// `bundled_json` is passed in by the caller (from rust-embed Templates::get()),
    /// avoiding a circular dependency between gateway-services and gateway-templates.
    pub fn load(bundled_json: &[u8], config_dir: &Path) -> Self;

    /// Get a model profile. Returns &self.fallback if not found.
    pub fn get(&self, model_id: &str) -> &ModelProfile;

    /// Check a specific capability.
    pub fn has_capability(&self, model_id: &str, cap: Capability) -> bool;

    /// Get context window for a model (replaces get_model_context_window).
    pub fn context_window(&self, model_id: &str) -> &ContextWindow;

    /// List all known models (for API/UI).
    pub fn list(&self) -> Vec<(&str, &ModelProfile)>;
}
```

### Avoiding Circular Dependencies

The bundled `models_registry.json` is embedded via `rust-embed` in `gateway-templates`. The `ModelRegistry` lives in `gateway-services`. To avoid a circular dependency (`gateway-templates` already depends on `gateway-services`), the caller passes the bundled bytes into `ModelRegistry::load()`:

```rust
// In gateway/src/state.rs (initialization)
let bundled = gateway_templates::Templates::get("models_registry.json")
    .map(|f| f.data.to_vec())
    .unwrap_or_default();
let model_registry = Arc::new(ModelRegistry::load(&bundled, &paths.vault_dir()));
```

## Integration Points

### Injection Chain

The `ModelRegistry` is initialized in `AppState` and threaded through the execution pipeline:

```
state.rs (AppState)
  → RuntimeService (holds Arc<ModelRegistry>)
    → ExecutionRunner (passes to executor builder and delegation spawn)
      → ExecutorBuilder (new field: model_registry: Arc<ModelRegistry>)
      → spawn_delegated_agent() (receives Arc<ModelRegistry>)
```

### 1. Executor Builder (`invoke/executor.rs`)

**Before:**
```rust
executor_config.context_window_tokens = provider.context_window
    .unwrap_or_else(|| get_model_context_window(&agent.model) as u64);
```

**After:**
```rust
let model_profile = model_registry.get(&agent.model);
executor_config.context_window_tokens = provider.context_window
    .unwrap_or(model_profile.context.input);

// Validate thinking capability
if agent.thinking_enabled && !model_profile.capabilities.thinking {
    tracing::warn!(model = %agent.model, "thinking_enabled but model lacks thinking capability — disabling");
    llm_config = llm_config.with_thinking(false);
}
```

### 2. Summarization Middleware (`agent-runtime/src/middleware/summarization.rs`)

Currently uses the framework-level `get_context_window()`. After this change, the context window is already resolved in `ExecutorConfig.context_window_tokens` (set by the executor builder above). The summarization middleware reads from `ExecutorConfig`, so no direct registry access needed — it just gets the correct value automatically.

### 3. Intent Analysis Middleware (`middleware/intent_analysis.rs`)

Soft enhancement — when recommending agents for an execution graph, the middleware can cross-reference agent model capabilities with task requirements. Not a hard gate, just smarter recommendations.

### 4. Delegation Spawn (`delegation/spawn.rs`)

Validate that delegated agent's model supports `tools` capability (subagents always use tool calling). Log warning if not.

### 5. Token Counter — Consolidation

Both hardcoded context window functions are removed:
- `runtime/agent-runtime/src/middleware/token_counter.rs` → `get_model_context_window()` — removed
- `framework/zero-middleware/src/token_counter.rs` → `get_context_window()` — removed

All callers now get context window from `ExecutorConfig.context_window_tokens`, which is set by the executor builder from the registry.

## API Endpoints

New routes registered in `gateway/src/http/models.rs`, following the existing flat route pattern (like agents):

```
GET /api/models         → Full merged registry (bundled + local overrides)
GET /api/models/:id     → Single model profile (or 404)
```

No POST/PUT/DELETE — overrides are via `config/models.json` file for now.

## UI Changes

### TypeScript Types (`transport/types.ts`)

```typescript
export interface ModelProfile {
  name: string;
  provider: string;
  capabilities: ModelCapabilities;
  context: ContextWindow;
  embedding?: EmbeddingSpec;
}

export interface ModelCapabilities {
  tools: boolean;
  vision: boolean;
  thinking: boolean;
  embeddings: boolean;
  voice: boolean;
  imageGeneration: boolean;
  videoGeneration: boolean;
}

export interface ContextWindow {
  input: number;
  output: number | null;
}

export interface EmbeddingSpec {
  dimensions: number;
  maxDimensions?: number;
}

/** Full registry response: model ID → profile */
export type ModelRegistryResponse = Record<string, ModelProfile>;
```

### Agent Dropdowns (`AgentEditPanel.tsx`, `WebAgentsPanel.tsx`)

Model dropdowns fetch `GET /api/models` and join with provider's model list. Each model shows small capability badges:
- Wrench icon → tools
- Eye icon → vision
- Brain icon → thinking
- Speaker icon → voice

Unknown models (not in registry) show the model name with no badges.

### Provider Page (`WebIntegrationsPanel.tsx`)

When displaying a provider's model list, show the same capability badges next to each model name. Read-only — no editing of model capabilities on this page. Provider page keeps all existing functionality (API keys, base URL, models list, test connection, set default).

## What Gets Replaced

| Current | Replaced By |
|---------|-------------|
| `get_model_context_window()` in `agent-runtime/middleware/token_counter.rs` | `model_registry.context_window()` via `ExecutorConfig` |
| `get_context_window()` in `zero-middleware/token_counter.rs` | Same — reads from `ExecutorConfig.context_window_tokens` |
| `provider.context_window` as primary override | Still works as legacy fallback, but registry is preferred |
| `agent.thinking_enabled` without validation | Validated against `model.capabilities.thinking` |
| Hardcoded model names in match arms | Data-driven lookup from registry |

## What Does NOT Change

- `providers.json` structure — models stay as string arrays
- Agent config files — `model: "glm-5"` stays a string
- `provider.context_window` — still honored as override (provider knows best for their endpoint)
- Distillation — uses default model, no capability gating needed

## Known Limitations (v1)

- No hot-reload of `config/models.json` — changes take effect on restart
- No remote fetch — manual file edits for local overrides
- `pricing` field reserved in JSON but not in Rust struct yet

## Files Changed (Summary)

| Layer | Files |
|-------|-------|
| **New** | `gateway/gateway-services/src/models.rs` (structs + ModelRegistry service) |
| **New** | `gateway/templates/models_registry.json` (bundled catalog) |
| **New** | `gateway/src/http/models.rs` (API routes) |
| **Modified** | `gateway/src/state.rs` (initialize registry, add to AppState) |
| **Modified** | `gateway/gateway-execution/src/runner.rs` (thread registry to RuntimeService/ExecutionRunner) |
| **Modified** | `gateway/gateway-execution/src/invoke/executor.rs` (use registry for context window + capability validation) |
| **Modified** | `gateway/gateway-execution/src/delegation/spawn.rs` (capability validation, receives registry) |
| **Modified** | `runtime/agent-runtime/src/middleware/token_counter.rs` (remove `get_model_context_window()`) |
| **Modified** | `framework/zero-middleware/src/token_counter.rs` (remove `get_context_window()`) |
| **Modified** | `runtime/agent-runtime/src/middleware/summarization.rs` (use ExecutorConfig context window, not hardcoded lookup) |
| **Modified** | `apps/ui/src/services/transport/types.ts` (model profile TS types) |
| **Modified** | `apps/ui/src/features/agent/AgentEditPanel.tsx` (capability badges in dropdowns) |
| **Modified** | `apps/ui/src/features/agent/WebAgentsPanel.tsx` (capability badges in dropdowns) |
| **Modified** | `apps/ui/src/features/integrations/WebIntegrationsPanel.tsx` (capability badges in model list) |
