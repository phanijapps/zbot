# Provider UI — Rate Limits, Model Capabilities & Token Limits

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Update the provider settings UI to show rate limits, model capabilities with toggle editing, and token limits — matching the enriched backend data model.

**Architecture:** Update TypeScript types to match new Provider backend fields. Enhance ProviderCard with capability badges. Add rate limits section to ProviderSlideover. Add model detail editor with capability toggles and token limit inputs. All changes in `apps/ui/src/`.

**Tech Stack:** React, TypeScript, Lucide icons, existing CSS variables

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `apps/ui/src/services/transport/types.ts` | Modify | Add RateLimits, ModelConfig types, update ProviderResponse |
| `apps/ui/src/features/settings/ProviderCard.tsx` | Modify | Show aggregated capability badges |
| `apps/ui/src/features/settings/ProviderSlideover.tsx` | Modify | Add rate limits section, enriched model rows, model editor |
| `apps/ui/src/services/transport/http.ts` | Modify | Update provider API types if needed |

---

### Task 0: Enrich Provider API Response with Model Capabilities

**Files:**
- Modify: `gateway/src/http/providers.rs`

The API returns providers.json as-is. When `modelConfigs` is empty (old format), enrich the response with capabilities from the model registry.

- [ ] **Step 1: Add enrichment helper**

Add to `providers.rs`:

```rust
use gateway_services::models::ModelRegistry;
use gateway_services::providers::{ModelConfig, Provider};
use std::collections::HashMap;

/// Enrich a provider's model list with capabilities from the model registry.
/// Only populates modelConfigs if it's empty (doesn't overwrite user data).
fn enrich_provider(provider: &mut Provider, registry: &ModelRegistry) {
    if provider.model_configs.is_some() {
        return; // Already enriched or user-configured
    }

    let mut configs = HashMap::new();
    for model_id in &provider.models {
        let profile = registry.get(model_id);
        configs.insert(model_id.clone(), ModelConfig {
            capabilities: profile.capabilities.clone(),
            max_input: Some(profile.context.input),
            max_output: profile.context.output,
            source: "registry".to_string(),
        });
    }

    if !configs.is_empty() {
        provider.model_configs = Some(configs);
    }
}
```

- [ ] **Step 2: Apply enrichment in list_providers handler**

Change the `list_providers` handler:

```rust
async fn list_providers(State(state): State<AppState>) -> impl IntoResponse {
    match state.provider_service.list() {
        Ok(mut providers) => {
            for p in &mut providers {
                enrich_provider(p, &state.model_registry);
            }
            Json(providers).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}
```

- [ ] **Step 3: Apply enrichment in get_provider handler**

```rust
async fn get_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.provider_service.get(&id) {
        Ok(mut provider) => {
            enrich_provider(&mut provider, &state.model_registry);
            Json(provider).into_response()
        }
        Err(e) => (StatusCode::NOT_FOUND, e).into_response(),
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p gateway`
Expected: All pass

- [ ] **Step 5: Commit**

```bash
git add gateway/src/http/providers.rs
git commit -m "feat: enrich provider API response with model capabilities from registry"
```

---

### Task 1: Update TypeScript Types

**Files:**
- Modify: `apps/ui/src/services/transport/types.ts`

- [ ] **Step 1: Add new types after ModelCapabilities**

After the existing `ModelCapabilities` interface (around line 247), add:

```typescript
export interface RateLimits {
  requestsPerMinute: number;
  concurrentRequests: number;
}

export interface ModelConfig {
  capabilities: ModelCapabilities;
  maxInput?: number;
  maxOutput?: number;
  source: "registry" | "discovered" | "user";
}
```

- [ ] **Step 2: Update ProviderResponse**

Add to the `ProviderResponse` interface:

```typescript
export interface ProviderResponse {
  id?: string;
  name: string;
  description: string;
  apiKey: string;
  baseUrl: string;
  models: string[];
  embeddingModels?: string[];
  defaultModel?: string;
  verified?: boolean;
  isDefault?: boolean;
  createdAt?: string;
  // New fields
  rateLimits?: RateLimits;
  modelConfigs?: Record<string, ModelConfig>;
}
```

- [ ] **Step 3: Update CreateProviderRequest and UpdateProviderRequest**

Add to both:

```typescript
  rateLimits?: RateLimits;
  modelConfigs?: Record<string, ModelConfig>;
```

- [ ] **Step 4: Run build**

Run: `cd apps/ui && npm run build`
Expected: Build succeeds (new fields are optional — no breaking changes)

- [ ] **Step 5: Commit**

```bash
git add apps/ui/src/services/transport/types.ts
git commit -m "feat(ui): add RateLimits, ModelConfig types to provider interfaces"
```

---

### Task 2: Provider Card — Capability Badges

**Files:**
- Modify: `apps/ui/src/features/settings/ProviderCard.tsx`

- [ ] **Step 1: Add capability badge aggregation**

Add a helper function before the component:

```typescript
/** Aggregate capabilities across all models for a provider */
function getProviderCapabilities(
  provider: ProviderResponse,
  modelRegistry: ModelRegistryResponse,
): { tools: boolean; vision: boolean; thinking: boolean; embeddings: boolean } {
  const caps = { tools: false, vision: false, thinking: false, embeddings: false };

  // Check modelConfigs (enriched) first
  if (provider.modelConfigs) {
    for (const config of Object.values(provider.modelConfigs)) {
      if (config.capabilities.tools) caps.tools = true;
      if (config.capabilities.vision) caps.vision = true;
      if (config.capabilities.thinking) caps.thinking = true;
      if (config.capabilities.embeddings) caps.embeddings = true;
    }
    return caps;
  }

  // Fallback: check model registry
  for (const modelId of provider.models) {
    const profile = modelRegistry[modelId];
    if (profile?.capabilities) {
      if (profile.capabilities.tools) caps.tools = true;
      if (profile.capabilities.vision) caps.vision = true;
      if (profile.capabilities.thinking) caps.thinking = true;
      if (profile.capabilities.embeddings) caps.embeddings = true;
    }
  }
  return caps;
}
```

- [ ] **Step 2: Render capability badges in the card**

In the component JSX, after the models section (after the `provider-card__models` div), add:

```tsx
      {/* Capability badges */}
      {(() => {
        const caps = getProviderCapabilities(provider, modelRegistry);
        const badges = [];
        if (caps.tools) badges.push({ label: "Tools", icon: "🔧" });
        if (caps.vision) badges.push({ label: "Vision", icon: "👁" });
        if (caps.thinking) badges.push({ label: "Thinking", icon: "🧠" });
        if (caps.embeddings) badges.push({ label: "Embeddings", icon: "📐" });
        if (badges.length === 0) return null;
        return (
          <div className="provider-card__capabilities">
            {badges.map((b) => (
              <span key={b.label} className="provider-card__cap-badge">{b.icon} {b.label}</span>
            ))}
          </div>
        );
      })()}
```

- [ ] **Step 3: Add CSS**

Add to the settings stylesheet (find the file with `.provider-card` styles — likely in `apps/ui/src/features/settings/` or a global CSS file):

```css
.provider-card__capabilities {
  display: flex;
  gap: 4px;
  flex-wrap: wrap;
  margin-top: 8px;
}

.provider-card__cap-badge {
  font-size: 10px;
  padding: 1px 6px;
  border-radius: 3px;
  background: var(--surface-2, #1a2e1a);
  color: var(--success, #4a9);
}
```

Find where other `.provider-card` styles are defined and add there.

- [ ] **Step 4: Run build**

Run: `cd apps/ui && npm run build`
Expected: Build succeeds

- [ ] **Step 5: Commit**

```bash
git add apps/ui/src/features/settings/ProviderCard.tsx
git commit -m "feat(ui): provider card shows aggregated capability badges"
```

---

### Task 3: Provider Slideover — Rate Limits Section

**Files:**
- Modify: `apps/ui/src/features/settings/ProviderSlideover.tsx`

- [ ] **Step 1: Add rate limits to EditForm**

Update the `EditForm` interface:

```typescript
interface EditForm {
  name: string;
  description: string;
  apiKey: string;
  baseUrl: string;
  models: string[];
  defaultModel: string;
  // New
  rateLimitsRpm: string;        // stored as string for input binding
  rateLimitsConcurrent: string;  // stored as string for input binding
}
```

Update the form initialization (in the useEffect that sets form from provider/preset) to include:

```typescript
rateLimitsRpm: String(provider?.rateLimits?.requestsPerMinute ?? 60),
rateLimitsConcurrent: String(provider?.rateLimits?.concurrentRequests ?? 3),
```

- [ ] **Step 2: Add rate limits section to view mode**

In the JSX, after the API key section and before the models section, add the rate limits display:

```tsx
      {/* Rate Limits */}
      <div className="slideover__section">
        <div className="slideover__section-title">⚡ Rate Limits</div>
        <div className="slideover__rate-limits">
          <div className="slideover__rate-limit-item">
            <span className="slideover__rate-limit-label">Requests/min</span>
            <span className="slideover__rate-limit-value">
              {provider?.rateLimits?.requestsPerMinute ?? 60}
            </span>
          </div>
          <div className="slideover__rate-limit-item">
            <span className="slideover__rate-limit-label">Concurrent</span>
            <span className="slideover__rate-limit-value">
              {provider?.rateLimits?.concurrentRequests ?? 3}
            </span>
          </div>
        </div>
      </div>
```

- [ ] **Step 3: Add rate limits inputs to edit mode**

In the edit form section, add after baseUrl input:

```tsx
      {/* Rate Limits (edit mode) */}
      <div className="slideover__field-group">
        <div className="slideover__field">
          <label className="slideover__label">Requests per minute</label>
          <input
            className="slideover__input"
            type="number"
            min="1"
            max="1000"
            value={form.rateLimitsRpm}
            onChange={(e) => handleFormChange({ rateLimitsRpm: e.target.value })}
          />
        </div>
        <div className="slideover__field">
          <label className="slideover__label">Max concurrent</label>
          <input
            className="slideover__input"
            type="number"
            min="1"
            max="20"
            value={form.rateLimitsConcurrent}
            onChange={(e) => handleFormChange({ rateLimitsConcurrent: e.target.value })}
          />
        </div>
      </div>
```

- [ ] **Step 4: Include rate limits in save payload**

In the save handler (handleSave function), when building the request body, include:

```typescript
rateLimits: {
  requestsPerMinute: parseInt(form.rateLimitsRpm) || 60,
  concurrentRequests: parseInt(form.rateLimitsConcurrent) || 3,
},
```

- [ ] **Step 5: Add CSS for rate limits**

```css
.slideover__rate-limits {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
}

.slideover__rate-limit-item {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.slideover__rate-limit-label {
  font-size: 10px;
  color: var(--text-muted);
  text-transform: uppercase;
}

.slideover__rate-limit-value {
  font-size: 16px;
  font-weight: 600;
  color: var(--success, #4a9);
}

.slideover__field-group {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
}
```

- [ ] **Step 6: Run build**

Run: `cd apps/ui && npm run build`
Expected: Build succeeds

- [ ] **Step 7: Commit**

```bash
git add apps/ui/src/features/settings/ProviderSlideover.tsx
git commit -m "feat(ui): rate limits section in provider detail — view + edit mode"
```

---

### Task 4: Enhanced Model Rows with Capabilities

**Files:**
- Modify: `apps/ui/src/features/settings/ProviderSlideover.tsx`

- [ ] **Step 1: Update model list rendering in view mode**

Find where models are rendered as chips in view mode. Replace with enriched rows:

```tsx
      {/* Models */}
      <div className="slideover__section">
        <div className="slideover__section-title">
          Models ({provider?.models.length ?? 0})
        </div>
        {provider?.models.map((modelId) => {
          const config = provider.modelConfigs?.[modelId];
          const registryProfile = modelRegistry[modelId];
          const caps = config?.capabilities ?? registryProfile?.capabilities;
          const maxIn = config?.maxInput ?? registryProfile?.context?.input;
          const maxOut = config?.maxOutput ?? registryProfile?.context?.output;
          const isDefault = modelId === (provider.defaultModel || provider.models[0]);

          return (
            <div key={modelId} className="slideover__model-row">
              <div className="slideover__model-header">
                <span className="slideover__model-name">{modelId}</span>
                {isDefault && <span className="badge badge--success badge--sm">default</span>}
              </div>
              <div className="slideover__model-meta">
                {caps?.tools && <span className="slideover__cap-tag">tools</span>}
                {caps?.vision && <span className="slideover__cap-tag slideover__cap-tag--vision">vision</span>}
                {caps?.thinking && <span className="slideover__cap-tag slideover__cap-tag--thinking">thinking</span>}
                {caps?.embeddings && <span className="slideover__cap-tag slideover__cap-tag--embed">embeddings</span>}
                {maxIn && (
                  <span className="slideover__model-context">
                    {Math.round(maxIn / 1000)}K in
                    {maxOut ? ` / ${Math.round(maxOut / 1000)}K out` : ""}
                  </span>
                )}
              </div>
            </div>
          );
        })}
      </div>
```

- [ ] **Step 2: Add CSS for model rows**

```css
.slideover__model-row {
  background: var(--surface-1, #0d1b2a);
  border: 1px solid var(--border, #334);
  border-radius: 6px;
  padding: 10px 12px;
  margin-bottom: 6px;
}

.slideover__model-header {
  display: flex;
  align-items: center;
  gap: 6px;
}

.slideover__model-name {
  font-size: 13px;
  font-weight: 500;
  color: var(--text-primary);
}

.slideover__model-meta {
  display: flex;
  gap: 4px;
  align-items: center;
  margin-top: 4px;
  flex-wrap: wrap;
}

.slideover__cap-tag {
  font-size: 10px;
  padding: 1px 5px;
  border-radius: 3px;
  background: var(--surface-success, #1a2e1a);
  color: var(--success, #4a9);
}

.slideover__cap-tag--vision {
  background: var(--surface-info, #1a1a2e);
  color: var(--info, #49a);
}

.slideover__cap-tag--thinking {
  background: #2a1a2e;
  color: #a49;
}

.slideover__cap-tag--embed {
  background: #2a2a1a;
  color: #a94;
}

.slideover__model-context {
  font-size: 10px;
  color: var(--text-muted, #555);
}

.badge--sm {
  font-size: 9px;
  padding: 1px 5px;
}
```

- [ ] **Step 3: Run build**

Run: `cd apps/ui && npm run build`
Expected: Build succeeds

- [ ] **Step 4: Commit**

```bash
git add apps/ui/src/features/settings/ProviderSlideover.tsx
git commit -m "feat(ui): enriched model rows with capability badges and token limits"
```

---

### Task 5: Build and Verify

- [ ] **Step 1: Full UI build**

Run: `cd apps/ui && npm run build`
Expected: Clean build, no TypeScript errors

- [ ] **Step 2: Verify dist output**

Run: `ls -la dist/assets/ | head -5`
Expected: Fresh JS/CSS bundles
