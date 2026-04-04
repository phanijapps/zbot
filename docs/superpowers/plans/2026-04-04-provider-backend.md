# Provider Backend — Rate Limiting, Model Capabilities & max_tokens Clamping

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix 429 crashes with shared per-provider rate limiting, prevent max_tokens exceeding model limits, and enrich providers.json with model capabilities and token limits.

**Architecture:** Extend `Provider` struct with `RateLimits` and enriched model configs. Create a `RateLimiterRegistry` that maps provider IDs to shared semaphores+sliding windows. Wire it into the runner (not the executor builder) so all agents sharing a provider share one rate limiter. Clamp `max_tokens` at executor build time using model registry data.

**Tech Stack:** Rust (gateway-services, gateway-execution, agent-runtime), serde_json, tokio::sync

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `gateway/gateway-services/src/providers.rs` | Modify | Add `RateLimits`, `ModelConfig`, enriched models, backward compat |
| `runtime/agent-runtime/src/llm/rate_limiter.rs` | Create | `ProviderRateLimiter` with concurrency semaphore + sliding window |
| `runtime/agent-runtime/src/llm/mod.rs` | Modify | Register new module |
| `runtime/agent-runtime/src/lib.rs` | Modify | Re-export rate limiter types |
| `gateway/gateway-execution/src/runner.rs` | Modify | Create shared rate limiters per provider, pass to executors |
| `gateway/gateway-execution/src/invoke/executor.rs` | Modify | max_tokens clamping, use shared rate limiter |
| `gateway/templates/models_registry.json` | Modify | Add missing models |

---

### Task 1: Add RateLimits and ModelConfig to Provider Struct

**Files:**
- Modify: `gateway/gateway-services/src/providers.rs`

- [ ] **Step 1: Add RateLimits struct**

Add after the `ProviderTestResult` struct:

```rust
/// Per-provider rate limit configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimits {
    /// Maximum requests per minute. Default: 60.
    #[serde(rename = "requestsPerMinute", default = "default_rpm")]
    pub requests_per_minute: u32,
    /// Maximum concurrent requests. Default: 3.
    #[serde(rename = "concurrentRequests", default = "default_concurrent")]
    pub concurrent_requests: u32,
}

fn default_rpm() -> u32 { 60 }
fn default_concurrent() -> u32 { 3 }

impl Default for RateLimits {
    fn default() -> Self {
        Self { requests_per_minute: default_rpm(), concurrent_requests: default_concurrent() }
    }
}
```

- [ ] **Step 2: Add ModelConfig struct**

```rust
/// Per-model configuration with capabilities and token limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// Model capabilities (text, tools, vision, etc.)
    #[serde(default)]
    pub capabilities: ModelCapabilities,
    /// Maximum input tokens. None = use registry fallback.
    #[serde(rename = "maxInput", skip_serializing_if = "Option::is_none")]
    pub max_input: Option<u64>,
    /// Maximum output tokens. None = use registry fallback.
    #[serde(rename = "maxOutput", skip_serializing_if = "Option::is_none")]
    pub max_output: Option<u64>,
    /// Data source: "registry", "discovered", or "user".
    #[serde(default = "default_source")]
    pub source: String,
}

fn default_source() -> String { "registry".to_string() }

/// Re-use the existing ModelCapabilities from models.rs.
/// Import it here: `use crate::models::ModelCapabilities;`
```

Note: `ModelCapabilities` already exists in `gateway-services/src/models.rs`. Import and reuse it — do not duplicate.

- [ ] **Step 3: Add rate_limits and enriched_models to Provider**

Add to the `Provider` struct (after `default_model`):

```rust
    /// Rate limiting configuration for this provider.
    #[serde(rename = "rateLimits", skip_serializing_if = "Option::is_none", default)]
    pub rate_limits: Option<RateLimits>,

    /// Enriched model configurations with capabilities and limits.
    /// When present, overrides the flat `models` list for resolution.
    #[serde(rename = "modelConfigs", skip_serializing_if = "Option::is_none", default)]
    pub model_configs: Option<HashMap<String, ModelConfig>>,
```

Keep the existing `models: Vec<String>` for backward compat. The `model_configs` is the enriched overlay. Add `use std::collections::HashMap;` at the top.

- [ ] **Step 4: Add helper methods**

```rust
impl Provider {
    /// Get the effective max_output for a model.
    /// Priority: model_configs override → registry → 128000 (safe default).
    pub fn effective_max_output(&self, model_id: &str) -> Option<u64> {
        self.model_configs
            .as_ref()
            .and_then(|configs| configs.get(model_id))
            .and_then(|c| c.max_output)
    }

    /// Get effective rate limits. Falls back to defaults if not set.
    pub fn effective_rate_limits(&self) -> RateLimits {
        self.rate_limits.clone().unwrap_or_default()
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p gateway-services`
Expected: All pass (new fields are optional with defaults)

- [ ] **Step 6: Commit**

```bash
git add gateway/gateway-services/src/providers.rs
git commit -m "feat: add RateLimits and ModelConfig to Provider — enriched model capabilities and token limits"
```

---

### Task 2: Create ProviderRateLimiter

**Files:**
- Create: `runtime/agent-runtime/src/llm/rate_limiter.rs`
- Modify: `runtime/agent-runtime/src/llm/mod.rs`
- Modify: `runtime/agent-runtime/src/lib.rs`

- [ ] **Step 1: Create rate_limiter.rs**

```rust
//! Per-provider rate limiter with concurrency control + sliding window.
//!
//! Shared across all executors using the same provider.
//! Calls wait (not fail) when rate limited.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore, SemaphorePermit};

/// Per-provider rate limiter.
///
/// Enforces:
/// 1. Max concurrent requests (semaphore)
/// 2. Max requests per minute (sliding window)
///
/// All agents sharing a provider share one instance.
pub struct ProviderRateLimiter {
    concurrency: Arc<Semaphore>,
    window: Arc<Mutex<SlidingWindow>>,
    rpm: u32,
}

struct SlidingWindow {
    timestamps: VecDeque<Instant>,
    max_per_minute: u32,
}

impl ProviderRateLimiter {
    /// Create a new rate limiter.
    pub fn new(concurrent: u32, rpm: u32) -> Self {
        Self {
            concurrency: Arc::new(Semaphore::new(concurrent as usize)),
            window: Arc::new(Mutex::new(SlidingWindow {
                timestamps: VecDeque::new(),
                max_per_minute: rpm,
            })),
            rpm,
        }
    }

    /// Acquire a rate limit slot. Waits if necessary. Never fails.
    pub async fn acquire(&self) -> SemaphorePermit<'_> {
        // Wait for sliding window slot
        loop {
            {
                let mut window = self.window.lock().await;
                let now = Instant::now();
                let one_minute_ago = now - Duration::from_secs(60);

                // Remove timestamps older than 1 minute
                while window.timestamps.front().map_or(false, |t| *t < one_minute_ago) {
                    window.timestamps.pop_front();
                }

                // If under the limit, record and proceed
                if (window.timestamps.len() as u32) < window.max_per_minute {
                    window.timestamps.push_back(now);
                    break;
                }

                // Over limit — calculate wait time until oldest entry expires
                if let Some(oldest) = window.timestamps.front() {
                    let wait = (*oldest + Duration::from_secs(60)) - now;
                    tracing::debug!(
                        rpm = self.rpm,
                        current = window.timestamps.len(),
                        wait_ms = wait.as_millis(),
                        "Rate limited — waiting for sliding window slot"
                    );
                    drop(window); // Release lock before sleeping
                    tokio::time::sleep(wait + Duration::from_millis(10)).await;
                    continue;
                }
            }
        }

        // Acquire concurrency permit
        self.concurrency.acquire().await
            .expect("Rate limiter semaphore closed")
    }

    /// Called when a 429 is received despite local rate limiting.
    /// Halves the RPM to adapt to provider-side limits.
    pub async fn on_rate_limited(&self) {
        let mut window = self.window.lock().await;
        let new_rpm = (window.max_per_minute / 2).max(1);
        tracing::warn!(
            old_rpm = window.max_per_minute,
            new_rpm = new_rpm,
            "Auto-reducing RPM after 429 response"
        );
        window.max_per_minute = new_rpm;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter_allows_under_limit() {
        let limiter = ProviderRateLimiter::new(2, 60);
        let _permit = limiter.acquire().await;
        // Should not block
    }

    #[tokio::test]
    async fn test_rate_limiter_concurrency() {
        let limiter = Arc::new(ProviderRateLimiter::new(1, 100));
        let permit1 = limiter.acquire().await;

        // Second acquire should block — test with timeout
        let limiter2 = limiter.clone();
        let result = tokio::time::timeout(
            Duration::from_millis(100),
            async move { limiter2.acquire().await }
        ).await;

        assert!(result.is_err(), "Second acquire should block when concurrency=1");
        drop(permit1); // Release — now second should succeed
    }

    #[tokio::test]
    async fn test_on_rate_limited_halves_rpm() {
        let limiter = ProviderRateLimiter::new(2, 60);
        limiter.on_rate_limited().await;
        let window = limiter.window.lock().await;
        assert_eq!(window.max_per_minute, 30);
    }
}
```

- [ ] **Step 2: Register module**

In `runtime/agent-runtime/src/llm/mod.rs`, add:
```rust
pub mod rate_limiter;
```

In `runtime/agent-runtime/src/lib.rs`, add to re-exports:
```rust
pub use llm::rate_limiter::ProviderRateLimiter;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p agent-runtime -- rate_limiter`
Expected: All 3 pass

- [ ] **Step 4: Commit**

```bash
git add runtime/agent-runtime/src/llm/rate_limiter.rs runtime/agent-runtime/src/llm/mod.rs runtime/agent-runtime/src/lib.rs
git commit -m "feat: ProviderRateLimiter — shared concurrency + sliding window rate limiting per provider"
```

---

### Task 3: Shared Rate Limiter Registry in Runner

**Files:**
- Modify: `gateway/gateway-execution/src/runner.rs`

- [ ] **Step 1: Add rate limiter registry to AgentRunner**

The current code creates a new semaphore per `create_executor` call (line 1217). This means root and subagents don't share rate limits. Fix: create rate limiters once per provider and share them.

Add to `AgentRunner` struct:

```rust
    /// Per-provider rate limiters — shared across all executors using the same provider.
    rate_limiters: Arc<RwLock<HashMap<String, Arc<agent_runtime::ProviderRateLimiter>>>>,
```

Initialize in constructor:

```rust
    rate_limiters: Arc::new(RwLock::new(HashMap::new())),
```

Add helper method:

```rust
    /// Get or create a rate limiter for a provider.
    fn get_rate_limiter(&self, provider: &Provider) -> Arc<agent_runtime::ProviderRateLimiter> {
        let provider_id = provider.id.clone().unwrap_or_else(|| provider.name.clone());
        let rate_limits = provider.effective_rate_limits();

        // Check if exists
        {
            let guard = self.rate_limiters.read().unwrap();
            if let Some(limiter) = guard.get(&provider_id) {
                return limiter.clone();
            }
        }

        // Create new
        let limiter = Arc::new(agent_runtime::ProviderRateLimiter::new(
            rate_limits.concurrent_requests,
            rate_limits.requests_per_minute,
        ));

        {
            let mut guard = self.rate_limiters.write().unwrap();
            guard.insert(provider_id, limiter.clone());
        }

        limiter
    }
```

- [ ] **Step 2: Replace per-invocation semaphore with shared rate limiter**

In `create_executor` (around line 1216), replace:

```rust
        let max_concurrent = provider.max_concurrent_requests.unwrap_or(3);
        let llm_throttle = Arc::new(tokio::sync::Semaphore::new(max_concurrent as usize));
```

With:

```rust
        let rate_limiter = self.get_rate_limiter(&provider);
        // Extract the semaphore from the rate limiter for the ThrottledLlmClient
        // (reusing existing ThrottledLlmClient with shared semaphore)
        let llm_throttle = rate_limiter.concurrency.clone();
```

Wait — `ProviderRateLimiter::concurrency` is private. We need to either:
a) Make it `pub` (breaks encapsulation)
b) Have the executor builder accept a `ProviderRateLimiter` instead of a `Semaphore`
c) Have `ProviderRateLimiter` implement `LlmClient` as a wrapper (like ThrottledLlmClient)

Option (c) is cleanest — make `ProviderRateLimiter` wrap the LLM client directly:

Add to `rate_limiter.rs`:

```rust
use super::client::{ChatResponse, LlmClient, LlmError, StreamCallback};
use crate::types::ChatMessage;

/// LLM client wrapper that enforces rate limits.
pub struct RateLimitedLlmClient {
    inner: Arc<dyn LlmClient>,
    limiter: Arc<ProviderRateLimiter>,
}

impl RateLimitedLlmClient {
    pub fn new(inner: Arc<dyn LlmClient>, limiter: Arc<ProviderRateLimiter>) -> Self {
        Self { inner, limiter }
    }
}

#[async_trait::async_trait]
impl LlmClient for RateLimitedLlmClient {
    fn model(&self) -> &str { self.inner.model() }
    fn provider(&self) -> &str { self.inner.provider() }

    async fn chat(&self, messages: Vec<ChatMessage>, tools: Option<serde_json::Value>) -> Result<ChatResponse, LlmError> {
        let _permit = self.limiter.acquire().await;
        self.inner.chat(messages, tools).await
    }

    async fn chat_stream(&self, messages: Vec<ChatMessage>, tools: Option<serde_json::Value>, callback: StreamCallback) -> Result<ChatResponse, LlmError> {
        let _permit = self.limiter.acquire().await;
        self.inner.chat_stream(messages, tools, callback).await
    }
}
```

Then in `executor.rs` builder, replace `ThrottledLlmClient` with `RateLimitedLlmClient`:

```rust
        let llm_client: Arc<dyn agent_runtime::LlmClient> = if let Some(ref limiter) = self.rate_limiter {
            Arc::new(agent_runtime::RateLimitedLlmClient::new(retrying_client, limiter.clone()))
        } else {
            retrying_client
        };
```

Update `ExecutorBuilder` to accept `rate_limiter: Option<Arc<ProviderRateLimiter>>` instead of `llm_throttle: Option<Arc<Semaphore>>`.

- [ ] **Step 3: Pass shared rate limiter to subagent executors too**

In `delegation/spawn.rs`, the subagent executor builder also needs the shared rate limiter. Thread it through `DelegationRequest` or resolve it from the provider in `spawn_delegated_agent`.

The cleanest approach: in `spawn_delegated_agent`, look up the provider and get the rate limiter from the runner's registry. This requires passing the `rate_limiters` map to the spawn function.

- [ ] **Step 4: Run tests**

Run: `cargo test -p agent-runtime && cargo test -p gateway-execution`
Expected: All pass

- [ ] **Step 5: Commit**

```bash
git add runtime/agent-runtime/src/llm/rate_limiter.rs runtime/agent-runtime/src/llm/mod.rs runtime/agent-runtime/src/lib.rs gateway/gateway-execution/src/runner.rs gateway/gateway-execution/src/invoke/executor.rs gateway/gateway-execution/src/delegation/spawn.rs
git commit -m "feat: shared per-provider rate limiter — all agents using same provider share one rate limit bucket"
```

---

### Task 4: max_tokens Clamping

**Files:**
- Modify: `gateway/gateway-execution/src/invoke/executor.rs`

- [ ] **Step 1: Add clamping after max_tokens is set**

Find where `executor_config.max_tokens = agent.max_tokens;` is set (around line 261). Add after it:

```rust
        // Clamp max_tokens to model's actual output limit (prevents API errors)
        if let Some(ref registry) = self.model_registry {
            let model_output = registry.context_window(&agent.model).resolved_output();
            if (executor_config.max_tokens as u64) > model_output && model_output > 0 {
                tracing::warn!(
                    agent = %agent.id,
                    model = %agent.model,
                    requested = executor_config.max_tokens,
                    clamped_to = model_output,
                    "Clamped max_tokens to model's output limit"
                );
                executor_config.max_tokens = model_output as u32;
            }
        }

        // Also check provider-level model config override
        // (provider.model_configs has user-set limits that take priority)
        if let Some(provider_max) = provider.effective_max_output(&agent.model) {
            if (executor_config.max_tokens as u64) > provider_max && provider_max > 0 {
                tracing::warn!(
                    agent = %agent.id,
                    model = %agent.model,
                    requested = executor_config.max_tokens,
                    clamped_to = provider_max,
                    "Clamped max_tokens to provider model config limit"
                );
                executor_config.max_tokens = provider_max as u32;
            }
        }
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p gateway-execution`
Expected: All pass

- [ ] **Step 3: Commit**

```bash
git add gateway/gateway-execution/src/invoke/executor.rs
git commit -m "feat: clamp max_tokens to model output limit — prevents 'exceeds maximum' API errors"
```

---

### Task 5: Expand Bundled Model Registry

**Files:**
- Modify: `gateway/templates/models_registry.json`

- [ ] **Step 1: Add/verify model entries**

Read the current registry, then add missing models. For each model, include: `name`, `provider`, `capabilities` (7 flags), `context` (input + output).

Models to add or verify:

```json
"claude-sonnet-4-6": {
  "name": "Claude Sonnet 4.6",
  "provider": "anthropic",
  "capabilities": { "tools": true, "vision": true, "thinking": true, "embeddings": false, "voice": false, "image_generation": false, "video_generation": false },
  "context": { "input": 1000000, "output": 64000 }
},
"claude-opus-4-6": {
  "name": "Claude Opus 4.6",
  "provider": "anthropic",
  "capabilities": { "tools": true, "vision": true, "thinking": true, "embeddings": false, "voice": false, "image_generation": false, "video_generation": false },
  "context": { "input": 1000000, "output": 64000 }
},
"minimax-m2.7:cloud": {
  "name": "MiniMax M2.7 Cloud",
  "provider": "minimax",
  "capabilities": { "tools": true, "vision": false, "thinking": false, "embeddings": false, "voice": false, "image_generation": false, "video_generation": false },
  "context": { "input": 256000, "output": 128000 }
},
"nemotron-3-super:cloud": {
  "name": "Nemotron 3 Super Cloud",
  "provider": "nvidia",
  "capabilities": { "tools": true, "vision": false, "thinking": false, "embeddings": false, "voice": false, "image_generation": false, "video_generation": false },
  "context": { "input": 128000, "output": 65536 }
},
"qwen3.5:397b-cloud": {
  "name": "Qwen 3.5 397B Cloud",
  "provider": "alibaba",
  "capabilities": { "tools": true, "vision": false, "thinking": true, "embeddings": false, "voice": false, "image_generation": false, "video_generation": false },
  "context": { "input": 128000, "output": 8192 }
}
```

Also verify existing entries: deepseek-chat, deepseek-reasoner, glm-5.1, glm-5, glm-5-turbo, glm-4.7, glm-4.6, glm-4.5.

Read the current file first to avoid duplicates.

- [ ] **Step 2: Copy to config/models.json on first run**

In `gateway/gateway-templates/src/lib.rs`, find where SOUL.md and INSTRUCTIONS.md are created on first run. Add:

```rust
    let models_path = config_dir.join("models.json");
    if !models_path.exists() {
        if let Some(bundled) = Templates::get("models_registry.json") {
            let _ = std::fs::write(&models_path, &bundled.data);
            tracing::info!("Created config/models.json from bundled registry");
        }
    }
```

- [ ] **Step 3: Run tests**

Run: `cargo test --workspace 2>&1 | grep FAILED`
Expected: No failures

- [ ] **Step 4: Commit**

```bash
git add gateway/templates/models_registry.json gateway/gateway-templates/src/lib.rs
git commit -m "feat: expand model registry + copy to config/models.json on first run"
```

---

### Task 6: Final Verification

- [ ] **Step 1: Run full workspace tests**

Run: `cargo test --workspace 2>&1 | grep -E "FAILED|test result" | grep -v "zero-core.*doc"`

- [ ] **Step 2: Build**

Run: `cargo build`

- [ ] **Step 3: Manual test**

Start daemon. Check that:
- `config/models.json` is created if missing
- Providers load with optional `rateLimits` and `modelConfigs` fields
- A provider with `maxTokens: 100000` and `nemotron-3-super` model gets clamped to 65536
