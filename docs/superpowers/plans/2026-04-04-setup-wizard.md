# Setup Wizard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a 6-step first-time setup wizard that configures agent identity, providers, skills, MCP servers, and model assignments.

**Architecture:** New `/setup` route with `SetupGuard` redirect logic. Gateway gets `setupComplete` flag in ExecutionSettings, a `/api/setup/status` endpoint, and a `/api/setup/mcp-defaults` endpoint serving a sanitized MCP template. Wizard UI is a standalone feature folder with `useReducer` state management, one component per step, all using existing design system classes (no inline styles).

**Tech Stack:** React + TypeScript (UI), Rust/Axum (gateway), existing transport layer, existing CSS design system (BEM + theme tokens)

---

## File Structure

### Gateway (Rust)

| File | Action | Responsibility |
|------|--------|----------------|
| `gateway/gateway-services/src/settings.rs` | Modify | Add `setup_complete: bool` to `ExecutionSettings` |
| `gateway/src/http/settings.rs` | Modify | Add `setup_complete` to update request/response |
| `gateway/src/http/setup.rs` | Create | `GET /api/setup/status`, `GET /api/setup/mcp-defaults` |
| `gateway/src/http/mod.rs` | Modify | Register setup routes |
| `gateway/templates/default_mcps.json` | Create | Sanitized MCP template (no API keys) |

### UI (TypeScript + React)

| File | Action | Responsibility |
|------|--------|----------------|
| `apps/ui/src/features/setup/presets.ts` | Create | Name preset data |
| `apps/ui/src/features/setup/components/StepIndicator.tsx` | Create | Progress dots (1-6) |
| `apps/ui/src/features/setup/components/WizardNav.tsx` | Create | Back/Next/Skip footer |
| `apps/ui/src/features/setup/steps/NameStep.tsx` | Create | Step 1 — preset cards + name input |
| `apps/ui/src/features/setup/steps/ProvidersStep.tsx` | Create | Step 2 — provider add/test/remove |
| `apps/ui/src/features/setup/steps/SkillsStep.tsx` | Create | Step 3 — skill toggle list |
| `apps/ui/src/features/setup/steps/McpStep.tsx` | Create | Step 4 — MCP server config |
| `apps/ui/src/features/setup/steps/AgentsStep.tsx` | Create | Step 5 — global default + overrides |
| `apps/ui/src/features/setup/steps/ReviewStep.tsx` | Create | Step 6 — summary + launch |
| `apps/ui/src/features/setup/SetupWizard.tsx` | Create | Main container, useReducer, step routing |
| `apps/ui/src/features/setup/SetupGuard.tsx` | Create | Route guard — check status, redirect |
| `apps/ui/src/features/setup/index.ts` | Create | Barrel exports |
| `apps/ui/src/styles/components.css` | Modify | Add setup wizard CSS classes |
| `apps/ui/src/services/transport/types.ts` | Modify | Add `setupComplete` to ExecutionSettings, add SetupStatus type |
| `apps/ui/src/services/transport/interface.ts` | Modify | Add `getSetupStatus()`, `getMcpDefaults()` |
| `apps/ui/src/services/transport/http.ts` | Modify | Implement new transport methods |
| `apps/ui/src/App.tsx` | Modify | Add `/setup` route, wrap with SetupGuard |
| `apps/ui/src/features/settings/WebSettingsPanel.tsx` | Modify | Add "Re-run Setup" button in Advanced tab |

---

### Task 1: Gateway — Add `setupComplete` to ExecutionSettings

**Files:**
- Modify: `gateway/gateway-services/src/settings.rs:32-50`
- Modify: `gateway/src/http/settings.rs:278-295`
- Modify: `apps/ui/src/services/transport/types.ts`
- Modify: `apps/ui/src/features/settings/WebSettingsPanel.tsx`

- [ ] **Step 1: Add `setup_complete` field to `ExecutionSettings`**

In `gateway/gateway-services/src/settings.rs`, add the field to the struct:

```rust
/// Execution settings for controlling agent concurrency and delegation behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionSettings {
    /// Maximum number of subagents that can run in parallel across all sessions.
    #[serde(default = "default_max_parallel_agents")]
    pub max_parallel_agents: u32,
    /// Whether the first-time setup wizard has been completed.
    #[serde(default)]
    pub setup_complete: bool,
}

impl Default for ExecutionSettings {
    fn default() -> Self {
        Self {
            max_parallel_agents: default_max_parallel_agents(),
            setup_complete: false,
        }
    }
}
```

- [ ] **Step 2: Update `UpdateExecutionSettingsRequest` in HTTP settings**

In `gateway/src/http/settings.rs`, add `setup_complete` to the request struct and the `From` impl:

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateExecutionSettingsRequest {
    #[serde(default = "default_max_parallel")]
    pub max_parallel_agents: u32,
    #[serde(default)]
    pub setup_complete: bool,
}

impl From<UpdateExecutionSettingsRequest> for ExecutionSettings {
    fn from(req: UpdateExecutionSettingsRequest) -> Self {
        ExecutionSettings {
            max_parallel_agents: req.max_parallel_agents,
            setup_complete: req.setup_complete,
        }
    }
}
```

- [ ] **Step 3: Update UI transport type**

In `apps/ui/src/services/transport/types.ts`, add `setupComplete` to `ExecutionSettings`:

```typescript
export interface ExecutionSettings {
  maxParallelAgents: number;
  setupComplete: boolean;
}
```

- [ ] **Step 4: Update Advanced tab to show setupComplete (and add Re-run button)**

In `apps/ui/src/features/settings/WebSettingsPanel.tsx`, inside the Advanced tab's execution card, after the max parallel agents input, add:

```tsx
<div className="flex items-center gap-3" style={{ marginTop: "var(--spacing-4)" }}>
  <button
    className="btn btn--outline btn--sm"
    onClick={() => window.location.href = "/setup"}
  >
    Re-run Setup Wizard
  </button>
</div>
```

- [ ] **Step 5: Build and verify**

Run:
```bash
cargo build -p gateway 2>&1 | grep "^error"
cd apps/ui && npm run build 2>&1 | tail -5
```
Expected: No errors.

- [ ] **Step 6: Commit**

```bash
git add gateway/gateway-services/src/settings.rs gateway/src/http/settings.rs \
  apps/ui/src/services/transport/types.ts apps/ui/src/features/settings/WebSettingsPanel.tsx
git commit -m "feat: add setupComplete to ExecutionSettings, re-run button in Advanced tab"
```

---

### Task 2: Gateway — Setup Status and MCP Defaults Endpoints

**Files:**
- Create: `gateway/src/http/setup.rs`
- Modify: `gateway/src/http/mod.rs`
- Create: `gateway/templates/default_mcps.json`

- [ ] **Step 1: Create the default MCP template**

Create `gateway/templates/default_mcps.json` with sanitized configs (all API keys blanked, all disabled):

```json
[
  {
    "type": "stdio",
    "id": "time",
    "name": "Time",
    "description": "Get current time and timezone information",
    "command": "uvx",
    "args": ["mcp-server-time"],
    "env": {},
    "enabled": false
  },
  {
    "type": "stdio",
    "id": "github",
    "name": "GitHub",
    "description": "Access GitHub repositories, issues, and pull requests",
    "command": "npx",
    "args": ["-y", "@modelcontextprotocol/server-github"],
    "env": {
      "GITHUB_PERSONAL_ACCESS_TOKEN": ""
    },
    "enabled": false
  },
  {
    "type": "stdio",
    "id": "brave-search",
    "name": "Brave Search",
    "description": "Search the web using Brave Search API",
    "command": "npx",
    "args": ["-y", "@brave/brave-search-mcp-server", "--transport", "stdio"],
    "env": {
      "BRAVE_API_KEY": ""
    },
    "enabled": false
  },
  {
    "type": "stdio",
    "id": "google-maps",
    "name": "Google Maps",
    "description": "Location search, directions, and place details",
    "command": "npx",
    "args": ["-y", "@modelcontextprotocol/server-google-maps"],
    "env": {
      "GOOGLE_MAPS_API_KEY": ""
    },
    "enabled": false
  },
  {
    "type": "stdio",
    "id": "sequential-thinking",
    "name": "Sequential Thinking",
    "description": "Step-by-step reasoning and problem decomposition",
    "command": "npx",
    "args": ["-y", "@modelcontextprotocol/server-sequential-thinking"],
    "env": {},
    "enabled": false
  }
]
```

- [ ] **Step 2: Create the setup HTTP module**

Create `gateway/src/http/setup.rs`:

```rust
//! # Setup Endpoints
//!
//! Lightweight endpoints for the first-time setup wizard.

use crate::state::AppState;
use axum::{extract::State, http::StatusCode, Json};
use serde::Serialize;

/// GET /api/setup/status — lightweight check for setup redirect logic.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupStatus {
    pub setup_complete: bool,
    pub has_providers: bool,
}

pub async fn get_setup_status(
    State(state): State<AppState>,
) -> Result<Json<SetupStatus>, StatusCode> {
    let setup_complete = state
        .settings
        .get_execution_settings()
        .map(|s| s.setup_complete)
        .unwrap_or(false);

    let has_providers = state
        .provider_service
        .list()
        .map(|providers| !providers.is_empty())
        .unwrap_or(false);

    Ok(Json(SetupStatus {
        setup_complete,
        has_providers,
    }))
}

/// GET /api/setup/mcp-defaults — sanitized MCP template for the wizard.
pub async fn get_mcp_defaults() -> Json<serde_json::Value> {
    let template = gateway_templates::Templates::get("default_mcps.json")
        .map(|f| {
            serde_json::from_slice(&f.data).unwrap_or_else(|_| serde_json::json!([]))
        })
        .unwrap_or_else(|| serde_json::json!([]));

    Json(template)
}
```

- [ ] **Step 3: Register setup routes in mod.rs**

In `gateway/src/http/mod.rs`, add:

```rust
mod setup;
```

And in the router chain, add after the settings routes:

```rust
        // Setup wizard endpoints
        .route("/api/setup/status", get(setup::get_setup_status))
        .route("/api/setup/mcp-defaults", get(setup::get_mcp_defaults))
```

- [ ] **Step 4: Build and verify**

Run:
```bash
cargo build -p gateway 2>&1 | grep "^error"
```
Expected: No errors.

- [ ] **Step 5: Commit**

```bash
git add gateway/src/http/setup.rs gateway/src/http/mod.rs gateway/templates/default_mcps.json
git commit -m "feat: add setup status and MCP defaults endpoints"
```

---

### Task 3: UI Transport — Add Setup API Methods

**Files:**
- Modify: `apps/ui/src/services/transport/types.ts`
- Modify: `apps/ui/src/services/transport/interface.ts`
- Modify: `apps/ui/src/services/transport/http.ts`
- Modify: `apps/ui/src/services/transport/index.ts`

- [ ] **Step 1: Add types**

In `apps/ui/src/services/transport/types.ts`, add after `ExecutionSettingsResponse`:

```typescript
/** Setup wizard status check */
export interface SetupStatus {
  setupComplete: boolean;
  hasProviders: boolean;
}
```

- [ ] **Step 2: Add interface methods**

In `apps/ui/src/services/transport/interface.ts`, add to the imports:

```typescript
  SetupStatus,
```

Add to the `Transport` interface, after `updateExecutionSettings`:

```typescript
  // =========================================================================
  // Setup Wizard Operations
  // =========================================================================

  /** Check if first-time setup is needed */
  getSetupStatus(): Promise<TransportResult<SetupStatus>>;

  /** Get sanitized MCP server templates for wizard */
  getMcpDefaults(): Promise<TransportResult<McpServerConfig[]>>;
```

- [ ] **Step 3: Implement in HTTP transport**

In `apps/ui/src/services/transport/http.ts`, add the import for `SetupStatus` and implement:

```typescript
  async getSetupStatus(): Promise<TransportResult<SetupStatus>> {
    return this.get<SetupStatus>("/api/setup/status");
  }

  async getMcpDefaults(): Promise<TransportResult<McpServerConfig[]>> {
    return this.get<McpServerConfig[]>("/api/setup/mcp-defaults");
  }
```

- [ ] **Step 4: Export new type from barrel**

In `apps/ui/src/services/transport/index.ts`, add `SetupStatus` to the type exports.

- [ ] **Step 5: Build and verify**

Run:
```bash
cd apps/ui && npm run build 2>&1 | tail -5
```
Expected: Build succeeds.

- [ ] **Step 6: Commit**

```bash
git add apps/ui/src/services/transport/
git commit -m "feat: add setup status and MCP defaults to transport layer"
```

---

### Task 4: CSS — Setup Wizard Styles

**Files:**
- Modify: `apps/ui/src/styles/components.css`

- [ ] **Step 1: Add setup wizard CSS classes**

Append to the end of `apps/ui/src/styles/components.css`:

```css
/* ============================================================================
   SETUP WIZARD
   First-time setup flow — full-page, centered, step-based
   ============================================================================ */

.setup-wizard {
  display: flex;
  flex-direction: column;
  align-items: center;
  min-height: 100vh;
  padding: var(--spacing-8) var(--spacing-4);
  overflow-y: auto;
}

.setup-wizard__container {
  width: 100%;
  max-width: 640px;
}

.setup-wizard__header {
  text-align: center;
  margin-bottom: var(--spacing-6);
}

.setup-wizard__title {
  font-family: var(--font-display);
  font-size: var(--text-2xl);
  font-weight: 600;
  color: var(--foreground);
  margin: 0 0 var(--spacing-1);
}

.setup-wizard__subtitle {
  font-size: var(--text-sm);
  color: var(--muted-foreground);
  margin: 0;
}

.setup-wizard__body {
  margin-bottom: var(--spacing-6);
}

.setup-wizard__footer {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding-top: var(--spacing-4);
  border-top: 1px solid var(--border);
}

.setup-wizard__skip {
  font-size: var(--text-xs);
  color: var(--muted-foreground);
  text-decoration: underline;
  cursor: pointer;
  background: none;
  border: none;
  padding: 0;
}

.setup-wizard__skip:hover {
  color: var(--foreground);
}

/* Step Indicator */

.step-indicator {
  display: flex;
  justify-content: center;
  gap: var(--spacing-2);
  margin-bottom: var(--spacing-6);
}

.step-indicator__dot {
  width: 8px;
  height: 8px;
  border-radius: var(--radius-full);
  background: var(--border);
  transition: background 0.2s ease;
}

.step-indicator__dot--active {
  background: var(--primary);
}

.step-indicator__dot--done {
  background: var(--primary);
  opacity: 0.6;
}

/* Name Presets */

.name-preset-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: var(--spacing-3);
  margin-bottom: var(--spacing-5);
}

.name-preset {
  display: flex;
  flex-direction: column;
  padding: var(--spacing-4);
  background: var(--card);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  cursor: pointer;
  transition: border-color 0.15s ease, box-shadow 0.15s ease;
}

.name-preset:hover {
  border-color: var(--muted-foreground);
}

.name-preset--selected {
  border-color: var(--primary);
  box-shadow: 0 0 0 1px var(--primary);
}

.name-preset__emoji {
  font-size: var(--text-2xl);
  margin-bottom: var(--spacing-1);
}

.name-preset__name {
  font-weight: 700;
  font-size: var(--text-base);
  color: var(--foreground);
}

.name-preset__tagline {
  font-size: var(--text-xs);
  color: var(--muted-foreground);
  margin-top: var(--spacing-0-5);
}

/* Provider Add Grid */

.provider-add-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: var(--spacing-2);
  margin-bottom: var(--spacing-4);
}

.provider-add-card {
  padding: var(--spacing-3);
  background: var(--card);
  border: 2px dashed var(--border);
  border-radius: var(--radius-md);
  cursor: pointer;
  text-align: center;
  transition: border-color 0.15s ease;
}

.provider-add-card:hover {
  border-color: var(--muted-foreground);
}

.provider-add-card--added {
  opacity: 0.4;
  pointer-events: none;
  border-style: solid;
}

.provider-add-card__name {
  font-weight: 600;
  font-size: var(--text-sm);
  color: var(--foreground);
}

.provider-add-card__hint {
  font-size: var(--text-xs);
  color: var(--muted-foreground);
}

.provider-add-form {
  background: var(--card);
  border: 1px solid var(--primary);
  border-radius: var(--radius-md);
  padding: var(--spacing-4);
  margin-bottom: var(--spacing-4);
}

.provider-added-list {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
  margin-bottom: var(--spacing-4);
}

.provider-added-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--spacing-3) var(--spacing-4);
  background: var(--card);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
}

.provider-added-row--verified {
  border-color: var(--success);
  border-color: color-mix(in srgb, var(--success) 40%, transparent);
}

.provider-added-row__info {
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
}

.provider-added-row__dot {
  width: 8px;
  height: 8px;
  border-radius: var(--radius-full);
  background: var(--success);
}

.provider-added-row__name {
  font-weight: 600;
  font-size: var(--text-sm);
}

.provider-added-row__models {
  font-size: var(--text-xs);
  color: var(--muted-foreground);
}

.provider-added-row__actions {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
}

/* Skill Category */

.skill-category {
  margin-bottom: var(--spacing-4);
}

.skill-category__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding-bottom: var(--spacing-2);
  margin-bottom: var(--spacing-2);
  border-bottom: 1px solid var(--border);
}

.skill-category__name {
  font-weight: 600;
  font-size: var(--text-sm);
  color: var(--foreground);
  text-transform: capitalize;
}

.skill-category__toggle {
  font-size: var(--text-xs);
  color: var(--primary);
  cursor: pointer;
  background: none;
  border: none;
  padding: 0;
}

/* MCP Sections */

.mcp-section {
  margin-bottom: var(--spacing-5);
}

.mcp-section__title {
  font-weight: 600;
  font-size: var(--text-sm);
  color: var(--foreground);
  margin-bottom: var(--spacing-3);
}

.mcp-row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  padding: var(--spacing-3) var(--spacing-4);
  background: var(--card);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  margin-bottom: var(--spacing-2);
}

.mcp-row__info {
  flex: 1;
  min-width: 0;
}

.mcp-row__name {
  font-weight: 600;
  font-size: var(--text-sm);
  color: var(--foreground);
}

.mcp-row__desc {
  font-size: var(--text-xs);
  color: var(--muted-foreground);
  margin-top: var(--spacing-0-5);
}

.mcp-row__key-input {
  margin-top: var(--spacing-2);
}

.mcp-row__key-hint {
  font-size: var(--text-xs);
  color: var(--muted-foreground);
  margin-top: var(--spacing-0-5);
}

.mcp-row__toggle {
  flex-shrink: 0;
  margin-left: var(--spacing-3);
}

/* Agent Config */

.agent-global-card {
  background: var(--card);
  border: 2px solid var(--primary);
  border-radius: var(--radius-lg);
  padding: var(--spacing-4);
  margin-bottom: var(--spacing-5);
}

.agent-global-card__label {
  display: inline-block;
  font-size: var(--text-xs);
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  background: var(--primary);
  color: var(--primary-foreground);
  padding: var(--spacing-0-5) var(--spacing-2);
  border-radius: var(--radius-sm);
  margin-bottom: var(--spacing-3);
}

.agent-global-card__fields {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: var(--spacing-3);
}

.agent-list {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
}

.agent-row {
  background: var(--card);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  padding: var(--spacing-3) var(--spacing-4);
}

.agent-row--customized {
  border-color: var(--primary);
}

.agent-row__summary {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.agent-row__info {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
}

.agent-row__name {
  font-weight: 600;
  font-size: var(--text-sm);
}

.agent-row__config {
  font-size: var(--text-xs);
  color: var(--muted-foreground);
}

.agent-row__fields {
  display: grid;
  grid-template-columns: 1fr 1fr 1fr 1fr;
  gap: var(--spacing-2);
  margin-top: var(--spacing-3);
  padding-top: var(--spacing-3);
  border-top: 1px solid var(--border);
}

.agent-row__field-label {
  font-size: var(--text-xs);
  color: var(--muted-foreground);
  margin-bottom: var(--spacing-0-5);
}

/* Review Section */

.review-section {
  background: var(--card);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  margin-bottom: var(--spacing-3);
  overflow: hidden;
}

.review-section__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--spacing-3) var(--spacing-4);
  cursor: pointer;
  transition: background 0.1s ease;
}

.review-section__header:hover {
  background: var(--accent);
}

.review-section__title {
  font-weight: 600;
  font-size: var(--text-sm);
}

.review-section__count {
  font-size: var(--text-xs);
  color: var(--muted-foreground);
}

.review-section__body {
  padding: 0 var(--spacing-4) var(--spacing-4);
}

.review-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--spacing-2) 0;
  font-size: var(--text-sm);
  border-bottom: 1px solid var(--border);
}

.review-item:last-child {
  border-bottom: none;
}

.review-item__label {
  color: var(--muted-foreground);
}

.review-item__value {
  font-weight: 500;
  color: var(--foreground);
}
```

- [ ] **Step 2: Build and verify**

Run:
```bash
cd apps/ui && npm run build 2>&1 | tail -5
```
Expected: Build succeeds.

- [ ] **Step 3: Commit**

```bash
git add apps/ui/src/styles/components.css
git commit -m "feat: add setup wizard CSS classes to design system"
```

---

### Task 5: UI — Name Presets and Shared Components

**Files:**
- Create: `apps/ui/src/features/setup/presets.ts`
- Create: `apps/ui/src/features/setup/components/StepIndicator.tsx`
- Create: `apps/ui/src/features/setup/components/WizardNav.tsx`

- [ ] **Step 1: Create name presets data**

Create `apps/ui/src/features/setup/presets.ts`:

```typescript
export interface NamePreset {
  id: string;
  name: string;
  emoji: string;
  tagline: string;
}

export const NAME_PRESETS: NamePreset[] = [
  { id: "brahmi", name: "Brahmi", emoji: "\uD83C\uDFAD", tagline: "Witty, resourceful, always has a plan" },
  { id: "johnnylever", name: "JohnnyLever", emoji: "\uD83D\uDE02", tagline: "Energetic, creative, makes work fun" },
  { id: "zbot", name: "z-Bot", emoji: "\uD83E\uDD16", tagline: "Professional, focused, gets things done" },
  { id: "custom", name: "Custom...", emoji: "\u2728", tagline: "Choose your own name" },
];
```

- [ ] **Step 2: Create StepIndicator component**

Create `apps/ui/src/features/setup/components/StepIndicator.tsx`:

```tsx
const TOTAL_STEPS = 6;

interface StepIndicatorProps {
  currentStep: number;
}

export function StepIndicator({ currentStep }: StepIndicatorProps) {
  return (
    <div className="step-indicator">
      {Array.from({ length: TOTAL_STEPS }, (_, i) => {
        const step = i + 1;
        let className = "step-indicator__dot";
        if (step === currentStep) className += " step-indicator__dot--active";
        else if (step < currentStep) className += " step-indicator__dot--done";
        return <div key={step} className={className} />;
      })}
    </div>
  );
}
```

- [ ] **Step 3: Create WizardNav component**

Create `apps/ui/src/features/setup/components/WizardNav.tsx`:

```tsx
interface WizardNavProps {
  currentStep: number;
  canNext: boolean;
  isLoading?: boolean;
  nextLabel?: string;
  onBack: () => void;
  onNext: () => void;
  onSkip?: () => void;
}

export function WizardNav({
  currentStep,
  canNext,
  isLoading,
  nextLabel,
  onBack,
  onNext,
  onSkip,
}: WizardNavProps) {
  return (
    <div className="setup-wizard__footer">
      <div>
        {currentStep > 1 ? (
          <button className="btn btn--ghost btn--sm" onClick={onBack} disabled={isLoading}>
            &larr; Back
          </button>
        ) : onSkip ? (
          <button className="setup-wizard__skip" onClick={onSkip}>
            Skip setup
          </button>
        ) : (
          <div />
        )}
      </div>
      <div className="flex items-center gap-3">
        {onSkip && currentStep > 1 && (
          <button className="setup-wizard__skip" onClick={onSkip}>
            Skip
          </button>
        )}
        <button
          className="btn btn--primary btn--sm"
          onClick={onNext}
          disabled={!canNext || isLoading}
        >
          {isLoading ? "..." : nextLabel || "Next \u2192"}
        </button>
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Commit**

```bash
git add apps/ui/src/features/setup/
git commit -m "feat: add setup wizard presets, StepIndicator, WizardNav components"
```

---

### Task 6: UI — Step 1 (Name) and Step 2 (Providers)

**Files:**
- Create: `apps/ui/src/features/setup/steps/NameStep.tsx`
- Create: `apps/ui/src/features/setup/steps/ProvidersStep.tsx`

- [ ] **Step 1: Create NameStep**

Create `apps/ui/src/features/setup/steps/NameStep.tsx`:

```tsx
import { NAME_PRESETS, type NamePreset } from "../presets";

interface NameStepProps {
  agentName: string;
  namePreset: string | null;
  onChange: (name: string, presetId: string | null) => void;
}

export function NameStep({ agentName, namePreset, onChange }: NameStepProps) {
  const handlePresetClick = (preset: NamePreset) => {
    if (preset.id === "custom") {
      onChange("", "custom");
    } else {
      onChange(preset.name, preset.id);
    }
  };

  return (
    <div>
      <div className="name-preset-grid">
        {NAME_PRESETS.map((preset) => (
          <div
            key={preset.id}
            className={`name-preset ${namePreset === preset.id ? "name-preset--selected" : ""}`}
            onClick={() => handlePresetClick(preset)}
          >
            <span className="name-preset__emoji">{preset.emoji}</span>
            <span className="name-preset__name">{preset.name}</span>
            <span className="name-preset__tagline">{preset.tagline}</span>
          </div>
        ))}
      </div>

      <div className="form-group">
        <label className="form-label">Agent Name</label>
        <input
          className="form-input"
          value={agentName}
          onChange={(e) => {
            const val = e.target.value.slice(0, 50);
            const matchingPreset = NAME_PRESETS.find((p) => p.name === val && p.id !== "custom");
            onChange(val, matchingPreset?.id || "custom");
          }}
          placeholder="Enter a name..."
          maxLength={50}
        />
        <p className="settings-hint">
          Click a preset above or type your own name. You can always change this later.
        </p>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Create ProvidersStep**

Create `apps/ui/src/features/setup/steps/ProvidersStep.tsx`:

```tsx
import { useState } from "react";
import { Loader2 } from "lucide-react";
import { getTransport } from "@/services/transport";
import type { ProviderResponse } from "@/services/transport";
import { PROVIDER_PRESETS, type ProviderPreset } from "@/features/settings/providerPresets";

interface ProvidersStepProps {
  providers: ProviderResponse[];
  defaultProviderId: string;
  onProvidersChanged: (providers: ProviderResponse[], defaultId: string) => void;
}

export function ProvidersStep({ providers, defaultProviderId, onProvidersChanged }: ProvidersStepProps) {
  const [expandedPreset, setExpandedPreset] = useState<string | null>(null);
  const [apiKey, setApiKey] = useState("");
  const [isTesting, setIsTesting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const addedBaseUrls = new Set(providers.map((p) => p.baseUrl.replace(/\/+$/, "")));

  const handleAddProvider = async (preset: ProviderPreset) => {
    if (preset.noApiKey) {
      await testAndAdd(preset, "ollama");
      return;
    }
    setExpandedPreset(preset.name);
    setApiKey("");
    setError(null);
  };

  const testAndAdd = async (preset: ProviderPreset, key: string) => {
    setIsTesting(true);
    setError(null);
    try {
      const transport = await getTransport();
      const models = preset.models.split(",").map((m) => m.trim()).filter(Boolean);
      const createResult = await transport.createProvider({
        name: preset.name,
        description: `${preset.name} API`,
        apiKey: key,
        baseUrl: preset.baseUrl,
        models,
      });
      if (!createResult.success || !createResult.data) {
        setError(createResult.error || "Failed to create provider");
        setIsTesting(false);
        return;
      }
      const id = createResult.data.id!;
      const testResult = await transport.testProviderById(id);
      if (!testResult.success || !testResult.data?.success) {
        await transport.deleteProvider(id);
        setError(testResult.data?.message || "Connection test failed");
        setIsTesting(false);
        return;
      }
      // Refresh provider list
      const listResult = await transport.listProviders();
      if (listResult.success && listResult.data) {
        const newProviders = listResult.data;
        const newDefault = newProviders.length === 1 ? id : defaultProviderId || id;
        onProvidersChanged(newProviders, newDefault);
      }
      setExpandedPreset(null);
      setApiKey("");
    } catch {
      setError("Something went wrong. Please try again.");
    } finally {
      setIsTesting(false);
    }
  };

  const handleRemove = async (id: string) => {
    try {
      const transport = await getTransport();
      await transport.deleteProvider(id);
      const listResult = await transport.listProviders();
      if (listResult.success && listResult.data) {
        const remaining = listResult.data;
        const newDefault = id === defaultProviderId
          ? remaining[0]?.id || ""
          : defaultProviderId;
        onProvidersChanged(remaining, newDefault);
      }
    } catch { /* ignore */ }
  };

  const handleSetDefault = (id: string) => {
    onProvidersChanged(providers, id);
  };

  return (
    <div>
      {/* Added providers */}
      {providers.length > 0 && (
        <div className="provider-added-list">
          {providers.map((p) => (
            <div key={p.id} className={`provider-added-row ${p.verified ? "provider-added-row--verified" : ""}`}>
              <div className="provider-added-row__info">
                {p.verified && <div className="provider-added-row__dot" />}
                <div>
                  <div className="provider-added-row__name">{p.name}</div>
                  <div className="provider-added-row__models">
                    {p.models.slice(0, 3).join(", ")}{p.models.length > 3 ? ` + ${p.models.length - 3} more` : ""}
                  </div>
                </div>
              </div>
              <div className="provider-added-row__actions">
                {p.verified && <span className="badge badge--success badge--xs">verified</span>}
                {p.id === defaultProviderId
                  ? <span className="badge badge--primary badge--xs">default</span>
                  : <button className="btn btn--ghost btn--sm" onClick={() => handleSetDefault(p.id!)}>set default</button>}
                <button className="btn btn--ghost btn--sm" onClick={() => handleRemove(p.id!)}>remove</button>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Preset grid */}
      <div className="settings-field-label">Add a provider</div>
      <div className="provider-add-grid">
        {PROVIDER_PRESETS.map((preset) => {
          const isAdded = addedBaseUrls.has(preset.baseUrl.replace(/\/+$/, ""));
          return (
            <div
              key={preset.name}
              className={`provider-add-card ${isAdded ? "provider-add-card--added" : ""}`}
              onClick={() => !isAdded && handleAddProvider(preset)}
            >
              <div className="provider-add-card__name">{preset.name}</div>
              <div className="provider-add-card__hint">
                {isAdded ? "added" : preset.noApiKey ? "no key needed" : preset.apiKeyPlaceholder}
              </div>
            </div>
          );
        })}
      </div>

      {/* Inline add form */}
      {expandedPreset && (
        <div className="provider-add-form">
          <div className="settings-field-label">Add {expandedPreset}</div>
          <div className="flex gap-2">
            <input
              className="form-input flex-1"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              placeholder={PROVIDER_PRESETS.find((p) => p.name === expandedPreset)?.apiKeyPlaceholder}
              type="password"
              autoFocus
            />
            <button
              className="btn btn--primary btn--sm"
              onClick={() => {
                const preset = PROVIDER_PRESETS.find((p) => p.name === expandedPreset);
                if (preset && apiKey.trim()) testAndAdd(preset, apiKey.trim());
              }}
              disabled={!apiKey.trim() || isTesting}
            >
              {isTesting ? <Loader2 className="loading-spinner__icon" /> : "Test & Add"}
            </button>
          </div>
          {error && <div className="alert alert--error" style={{ marginTop: "var(--spacing-2)" }}>{error}</div>}
          <p className="settings-hint">
            {PROVIDER_PRESETS.find((p) => p.name === expandedPreset)?.apiKeyHint}
          </p>
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 3: Build and verify**

Run:
```bash
cd apps/ui && npm run build 2>&1 | tail -5
```
Expected: Build succeeds (components not yet wired to routes, but should compile).

- [ ] **Step 4: Commit**

```bash
git add apps/ui/src/features/setup/steps/NameStep.tsx apps/ui/src/features/setup/steps/ProvidersStep.tsx
git commit -m "feat: add NameStep and ProvidersStep components"
```

---

### Task 7: UI — Step 3 (Skills) and Step 4 (MCP)

**Files:**
- Create: `apps/ui/src/features/setup/steps/SkillsStep.tsx`
- Create: `apps/ui/src/features/setup/steps/McpStep.tsx`

- [ ] **Step 1: Create SkillsStep**

Create `apps/ui/src/features/setup/steps/SkillsStep.tsx`:

```tsx
import { useEffect, useState } from "react";
import { Loader2 } from "lucide-react";
import { getTransport } from "@/services/transport";
import type { SkillResponse } from "@/services/transport";

const RECOMMENDED_SKILLS = ["coding", "doc", "duckduckgo-search"];

interface SkillsStepProps {
  enabledSkillIds: string[];
  onChange: (ids: string[]) => void;
}

export function SkillsStep({ enabledSkillIds, onChange }: SkillsStepProps) {
  const [skills, setSkills] = useState<SkillResponse[]>([]);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    const load = async () => {
      try {
        const transport = await getTransport();
        const result = await transport.listSkills();
        if (result.success && result.data) {
          setSkills(result.data);
          // Pre-select recommended on first load if nothing selected
          if (enabledSkillIds.length === 0) {
            const recommended = result.data
              .filter((s) => RECOMMENDED_SKILLS.includes(s.name))
              .map((s) => s.id);
            if (recommended.length > 0) onChange(recommended);
          }
        }
      } finally {
        setIsLoading(false);
      }
    };
    load();
  }, []);

  if (isLoading) {
    return <div className="settings-loading"><Loader2 className="loading-spinner__icon" /></div>;
  }

  // Group by category
  const byCategory = skills.reduce<Record<string, SkillResponse[]>>((acc, skill) => {
    const cat = skill.category || "other";
    (acc[cat] = acc[cat] || []).push(skill);
    return acc;
  }, {});

  const toggleSkill = (id: string) => {
    if (enabledSkillIds.includes(id)) {
      onChange(enabledSkillIds.filter((s) => s !== id));
    } else {
      onChange([...enabledSkillIds, id]);
    }
  };

  const toggleCategory = (categorySkills: SkillResponse[]) => {
    const ids = categorySkills.map((s) => s.id);
    const allSelected = ids.every((id) => enabledSkillIds.includes(id));
    if (allSelected) {
      onChange(enabledSkillIds.filter((id) => !ids.includes(id)));
    } else {
      onChange([...new Set([...enabledSkillIds, ...ids])]);
    }
  };

  return (
    <div>
      {Object.entries(byCategory).map(([category, categorySkills]) => {
        const allSelected = categorySkills.every((s) => enabledSkillIds.includes(s.id));
        return (
          <div key={category} className="skill-category">
            <div className="skill-category__header">
              <span className="skill-category__name">{category}</span>
              <button
                className="skill-category__toggle"
                onClick={() => toggleCategory(categorySkills)}
              >
                {allSelected ? "Deselect all" : "Select all"}
              </button>
            </div>
            {categorySkills.map((skill) => (
              <div
                key={skill.id}
                className={`skill-toggle ${enabledSkillIds.includes(skill.id) ? "skill-toggle--on" : ""}`}
                onClick={() => toggleSkill(skill.id)}
              >
                <div className="skill-toggle__info">
                  <div className="skill-toggle__name">{skill.displayName || skill.name}</div>
                  <div className="skill-toggle__desc">{skill.description}</div>
                </div>
              </div>
            ))}
          </div>
        );
      })}
      {skills.length === 0 && (
        <p className="settings-hint">No skills installed. You can add skills later.</p>
      )}
    </div>
  );
}
```

- [ ] **Step 2: Create McpStep**

Create `apps/ui/src/features/setup/steps/McpStep.tsx`:

```tsx
import { useEffect, useState } from "react";
import { Loader2 } from "lucide-react";
import { getTransport } from "@/services/transport";
import type { McpServerConfig } from "@/services/transport";

interface McpStepProps {
  mcpConfigs: McpServerConfig[];
  onChange: (configs: McpServerConfig[]) => void;
}

export function McpStep({ mcpConfigs, onChange }: McpStepProps) {
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    const load = async () => {
      try {
        const transport = await getTransport();
        const result = await transport.getMcpDefaults();
        if (result.success && result.data && mcpConfigs.length === 0) {
          // Pre-enable keyless servers
          const configs = result.data.map((c: McpServerConfig) => ({
            ...c,
            enabled: !hasEnvKeys(c),
          }));
          onChange(configs);
        }
      } finally {
        setIsLoading(false);
      }
    };
    load();
  }, []);

  if (isLoading) {
    return <div className="settings-loading"><Loader2 className="loading-spinner__icon" /></div>;
  }

  const keyless = mcpConfigs.filter((c) => !hasEnvKeys(c));
  const needsKey = mcpConfigs.filter((c) => hasEnvKeys(c));

  const toggleServer = (id: string) => {
    onChange(
      mcpConfigs.map((c) =>
        c.id === id ? { ...c, enabled: !c.enabled } : c
      )
    );
  };

  const updateEnvKey = (serverId: string, envKey: string, value: string) => {
    onChange(
      mcpConfigs.map((c) =>
        c.id === serverId
          ? { ...c, env: { ...c.env, [envKey]: value } }
          : c
      )
    );
  };

  return (
    <div>
      {keyless.length > 0 && (
        <div className="mcp-section">
          <div className="mcp-section__title">Ready to use</div>
          {keyless.map((server) => (
            <div key={server.id} className="mcp-row">
              <div className="mcp-row__info">
                <div className="mcp-row__name">{server.name}</div>
                <div className="mcp-row__desc">{server.description}</div>
              </div>
              <div className="mcp-row__toggle">
                <div
                  className={`toggle-switch ${server.enabled ? "toggle-switch--on" : "toggle-switch--off"}`}
                  onClick={() => toggleServer(server.id!)}
                />
              </div>
            </div>
          ))}
        </div>
      )}

      {needsKey.length > 0 && (
        <div className="mcp-section">
          <div className="mcp-section__title">Requires API key</div>
          {needsKey.map((server) => {
            const envKeys = getEmptyEnvKeys(server);
            return (
              <div key={server.id} className="mcp-row">
                <div className="mcp-row__info">
                  <div className="mcp-row__name">{server.name}</div>
                  <div className="mcp-row__desc">{server.description}</div>
                  {envKeys.map((envKey) => (
                    <div key={envKey} className="mcp-row__key-input">
                      <input
                        className="form-input"
                        placeholder={envKey}
                        type="password"
                        value={server.env?.[envKey] || ""}
                        onChange={(e) => updateEnvKey(server.id!, envKey, e.target.value)}
                      />
                    </div>
                  ))}
                </div>
                <div className="mcp-row__toggle">
                  <div
                    className={`toggle-switch ${server.enabled ? "toggle-switch--on" : "toggle-switch--off"}`}
                    onClick={() => toggleServer(server.id!)}
                  />
                </div>
              </div>
            );
          })}
        </div>
      )}

      {mcpConfigs.length === 0 && (
        <p className="settings-hint">No MCP server templates available.</p>
      )}
    </div>
  );
}

function hasEnvKeys(config: McpServerConfig): boolean {
  if (!config.env) return false;
  return Object.values(config.env).some((v) => v === "" || v === undefined);
}

function getEmptyEnvKeys(config: McpServerConfig): string[] {
  if (!config.env) return [];
  return Object.entries(config.env)
    .filter(([, v]) => v === "" || v === undefined)
    .map(([k]) => k);
}
```

- [ ] **Step 3: Build and verify**

Run:
```bash
cd apps/ui && npm run build 2>&1 | tail -5
```
Expected: Build succeeds.

- [ ] **Step 4: Commit**

```bash
git add apps/ui/src/features/setup/steps/SkillsStep.tsx apps/ui/src/features/setup/steps/McpStep.tsx
git commit -m "feat: add SkillsStep and McpStep components"
```

---

### Task 8: UI — Step 5 (Agents) and Step 6 (Review)

**Files:**
- Create: `apps/ui/src/features/setup/steps/AgentsStep.tsx`
- Create: `apps/ui/src/features/setup/steps/ReviewStep.tsx`

- [ ] **Step 1: Create AgentsStep**

Create `apps/ui/src/features/setup/steps/AgentsStep.tsx`:

```tsx
import { useEffect, useState } from "react";
import { Loader2 } from "lucide-react";
import { getTransport } from "@/services/transport";
import type { ProviderResponse, AgentResponse } from "@/services/transport";

interface GlobalDefault {
  providerId: string;
  model: string;
  temperature: number;
  maxTokens: number;
}

interface AgentOverride {
  providerId?: string;
  model?: string;
  temperature?: number;
  maxTokens?: number;
}

interface AgentsStepProps {
  providers: ProviderResponse[];
  defaultProviderId: string;
  agentName: string;
  globalDefault: GlobalDefault;
  agentOverrides: Record<string, AgentOverride>;
  onGlobalChange: (defaults: GlobalDefault) => void;
  onOverrideChange: (overrides: Record<string, AgentOverride>) => void;
}

export function AgentsStep({
  providers,
  defaultProviderId,
  agentName,
  globalDefault,
  agentOverrides,
  onGlobalChange,
  onOverrideChange,
}: AgentsStepProps) {
  const [agents, setAgents] = useState<AgentResponse[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [expandedAgent, setExpandedAgent] = useState<string | null>(null);

  useEffect(() => {
    const load = async () => {
      try {
        const transport = await getTransport();
        const result = await transport.listAgents();
        if (result.success && result.data) {
          setAgents(result.data);
          // Initialize global default from default provider if not set
          if (!globalDefault.providerId && defaultProviderId) {
            const provider = providers.find((p) => p.id === defaultProviderId);
            onGlobalChange({
              providerId: defaultProviderId,
              model: provider?.models[0] || "",
              temperature: 0.7,
              maxTokens: 4096,
            });
          }
        }
      } finally {
        setIsLoading(false);
      }
    };
    load();
  }, []);

  if (isLoading) {
    return <div className="settings-loading"><Loader2 className="loading-spinner__icon" /></div>;
  }

  const selectedProvider = providers.find((p) => p.id === globalDefault.providerId);
  const globalModels = selectedProvider?.models || [];

  const getEffectiveConfig = (agentId: string) => {
    const override = agentOverrides[agentId];
    if (!override) return globalDefault;
    return {
      providerId: override.providerId || globalDefault.providerId,
      model: override.model || globalDefault.model,
      temperature: override.temperature ?? globalDefault.temperature,
      maxTokens: override.maxTokens ?? globalDefault.maxTokens,
    };
  };

  const getProviderModels = (providerId: string) => {
    return providers.find((p) => p.id === providerId)?.models || [];
  };

  const getProviderName = (providerId: string) => {
    return providers.find((p) => p.id === providerId)?.name || providerId;
  };

  const handleOverride = (agentId: string, field: string, value: string | number) => {
    const current = agentOverrides[agentId] || {};
    onOverrideChange({
      ...agentOverrides,
      [agentId]: { ...current, [field]: value },
    });
  };

  const handleResetOverride = (agentId: string) => {
    const { [agentId]: _, ...rest } = agentOverrides;
    onOverrideChange(rest);
    setExpandedAgent(null);
  };

  // Sort: root agent first, then alphabetical
  const rootAgent = agents.find((a) => a.agentType === "root" || a.name === "root");
  const specialists = agents.filter((a) => a !== rootAgent).sort((a, b) => a.name.localeCompare(b.name));
  const sortedAgents = rootAgent ? [rootAgent, ...specialists] : specialists;

  return (
    <div>
      {/* Global default card */}
      <div className="agent-global-card">
        <div className="agent-global-card__label">Default for all</div>
        <div className="agent-global-card__fields">
          <div className="form-group">
            <label className="form-label">Provider</label>
            <select
              className="form-input form-select"
              value={globalDefault.providerId}
              onChange={(e) => {
                const pid = e.target.value;
                const models = getProviderModels(pid);
                onGlobalChange({ ...globalDefault, providerId: pid, model: models[0] || "" });
              }}
            >
              {providers.map((p) => (
                <option key={p.id} value={p.id}>{p.name}</option>
              ))}
            </select>
          </div>
          <div className="form-group">
            <label className="form-label">Model</label>
            <select
              className="form-input form-select"
              value={globalDefault.model}
              onChange={(e) => onGlobalChange({ ...globalDefault, model: e.target.value })}
            >
              {globalModels.map((m) => (
                <option key={m} value={m}>{m}</option>
              ))}
            </select>
          </div>
          <div className="form-group">
            <label className="form-label">Temperature</label>
            <input
              className="form-input"
              type="number"
              value={globalDefault.temperature}
              onChange={(e) => onGlobalChange({ ...globalDefault, temperature: parseFloat(e.target.value) || 0 })}
              min={0} max={2} step={0.1}
            />
          </div>
          <div className="form-group">
            <label className="form-label">Max Output Tokens</label>
            <input
              className="form-input"
              type="number"
              value={globalDefault.maxTokens}
              onChange={(e) => onGlobalChange({ ...globalDefault, maxTokens: parseInt(e.target.value) || 4096 })}
              min={256} step={1024}
            />
          </div>
        </div>
      </div>

      {/* Agent list */}
      <div className="settings-field-label">Agents</div>
      <div className="agent-list">
        {sortedAgents.map((agent) => {
          const isRoot = agent === rootAgent;
          const isCustomized = !!agentOverrides[agent.id];
          const isExpanded = expandedAgent === agent.id;
          const effective = getEffectiveConfig(agent.id);
          const overrideModels = isCustomized && agentOverrides[agent.id]?.providerId
            ? getProviderModels(agentOverrides[agent.id].providerId!)
            : globalModels;

          return (
            <div key={agent.id} className={`agent-row ${isCustomized ? "agent-row--customized" : ""}`}>
              <div className="agent-row__summary">
                <div className="agent-row__info">
                  <span className="agent-row__name">{isRoot ? agentName : agent.displayName || agent.name}</span>
                  {isRoot && <span className="badge badge--primary badge--xs">root</span>}
                  {isCustomized && <span className="badge badge--warning badge--xs">customized</span>}
                </div>
                {!isExpanded ? (
                  <div className="flex items-center gap-2">
                    <span className="agent-row__config">
                      {getProviderName(effective.providerId)} &middot; {effective.model} &middot; {effective.temperature} &middot; {effective.maxTokens}
                    </span>
                    <button className="btn btn--outline btn--sm" onClick={() => setExpandedAgent(agent.id)}>
                      Customize
                    </button>
                  </div>
                ) : (
                  <button className="btn btn--ghost btn--sm" onClick={() => handleResetOverride(agent.id)}>
                    Reset to default
                  </button>
                )}
              </div>
              {isExpanded && (
                <div className="agent-row__fields">
                  <div>
                    <div className="agent-row__field-label">Provider</div>
                    <select
                      className="form-input form-select"
                      value={agentOverrides[agent.id]?.providerId || globalDefault.providerId}
                      onChange={(e) => handleOverride(agent.id, "providerId", e.target.value)}
                    >
                      {providers.map((p) => (
                        <option key={p.id} value={p.id}>{p.name}</option>
                      ))}
                    </select>
                  </div>
                  <div>
                    <div className="agent-row__field-label">Model</div>
                    <select
                      className="form-input form-select"
                      value={agentOverrides[agent.id]?.model || globalDefault.model}
                      onChange={(e) => handleOverride(agent.id, "model", e.target.value)}
                    >
                      {overrideModels.map((m) => (
                        <option key={m} value={m}>{m}</option>
                      ))}
                    </select>
                  </div>
                  <div>
                    <div className="agent-row__field-label">Temp</div>
                    <input
                      className="form-input"
                      type="number"
                      value={agentOverrides[agent.id]?.temperature ?? globalDefault.temperature}
                      onChange={(e) => handleOverride(agent.id, "temperature", parseFloat(e.target.value) || 0)}
                      min={0} max={2} step={0.1}
                    />
                  </div>
                  <div>
                    <div className="agent-row__field-label">Tokens</div>
                    <input
                      className="form-input"
                      type="number"
                      value={agentOverrides[agent.id]?.maxTokens ?? globalDefault.maxTokens}
                      onChange={(e) => handleOverride(agent.id, "maxTokens", parseInt(e.target.value) || 4096)}
                      min={256} step={1024}
                    />
                  </div>
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Create ReviewStep**

Create `apps/ui/src/features/setup/steps/ReviewStep.tsx`:

```tsx
import { useState } from "react";
import { ChevronDown, ChevronRight, Loader2 } from "lucide-react";
import { getTransport } from "@/services/transport";
import type { ProviderResponse, McpServerConfig } from "@/services/transport";

interface GlobalDefault {
  providerId: string;
  model: string;
  temperature: number;
  maxTokens: number;
}

interface AgentOverride {
  providerId?: string;
  model?: string;
  temperature?: number;
  maxTokens?: number;
}

interface ReviewStepProps {
  agentName: string;
  providers: ProviderResponse[];
  defaultProviderId: string;
  enabledSkillIds: string[];
  mcpConfigs: McpServerConfig[];
  globalDefault: GlobalDefault;
  agentOverrides: Record<string, AgentOverride>;
  onLaunchComplete: () => void;
}

export function ReviewStep({
  agentName,
  providers,
  defaultProviderId,
  enabledSkillIds,
  mcpConfigs,
  globalDefault,
  agentOverrides,
  onLaunchComplete,
}: ReviewStepProps) {
  const [isLaunching, setIsLaunching] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [openSections, setOpenSections] = useState<Set<string>>(new Set(["identity", "providers", "agents"]));

  const toggleSection = (id: string) => {
    const next = new Set(openSections);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    setOpenSections(next);
  };

  const enabledMcps = mcpConfigs.filter((c) => c.enabled);
  const overrideCount = Object.keys(agentOverrides).length;

  const handleLaunch = async () => {
    setIsLaunching(true);
    setError(null);
    try {
      const transport = await getTransport();

      // 1. Get all agents to find root
      const agentsResult = await transport.listAgents();
      if (!agentsResult.success || !agentsResult.data) {
        throw new Error("Failed to load agents");
      }
      const agents = agentsResult.data;
      const rootAgent = agents.find((a) => a.agentType === "root" || a.name === "root");

      // 2. Rename root agent
      if (rootAgent) {
        await transport.updateAgent(rootAgent.id, {
          name: agentName.toLowerCase().replace(/\s+/g, "-"),
          displayName: agentName,
        });
      }

      // 3. Set default provider
      if (defaultProviderId) {
        await transport.setDefaultProvider(defaultProviderId);
      }

      // 4. Update agent configs
      for (const agent of agents) {
        const override = agentOverrides[agent.id];
        const config = override
          ? {
              providerId: override.providerId || globalDefault.providerId,
              model: override.model || globalDefault.model,
              temperature: override.temperature ?? globalDefault.temperature,
              maxTokens: override.maxTokens ?? globalDefault.maxTokens,
            }
          : globalDefault;

        await transport.updateAgent(agent.id, {
          providerId: config.providerId,
          model: config.model,
          temperature: config.temperature,
          maxTokens: config.maxTokens,
        });
      }

      // 5. Create enabled MCP servers
      for (const mcp of enabledMcps) {
        await transport.createMcp({
          type: mcp.type,
          id: mcp.id,
          name: mcp.name,
          description: mcp.description,
          command: mcp.command,
          args: mcp.args,
          env: mcp.env,
          url: mcp.url,
          headers: mcp.headers,
          enabled: true,
        });
      }

      // 6. Mark setup complete
      const execResult = await transport.getExecutionSettings();
      const currentExec = execResult.data || { maxParallelAgents: 2, setupComplete: false };
      await transport.updateExecutionSettings({
        ...currentExec,
        setupComplete: true,
      });

      onLaunchComplete();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Something went wrong");
    } finally {
      setIsLaunching(false);
    }
  };

  const getProviderName = (id: string) => providers.find((p) => p.id === id)?.name || id;

  return (
    <div>
      {/* Identity */}
      <Section id="identity" title="Agent Identity" count={agentName} open={openSections} onToggle={toggleSection}>
        <div className="review-item">
          <span className="review-item__label">Name</span>
          <span className="review-item__value">{agentName}</span>
        </div>
      </Section>

      {/* Providers */}
      <Section id="providers" title="Providers" count={`${providers.length} connected`} open={openSections} onToggle={toggleSection}>
        {providers.map((p) => (
          <div key={p.id} className="review-item">
            <span className="review-item__label">{p.name}</span>
            <span className="review-item__value">
              {p.models.length} models
              {p.id === defaultProviderId ? " (default)" : ""}
            </span>
          </div>
        ))}
      </Section>

      {/* Skills */}
      <Section id="skills" title="Skills" count={`${enabledSkillIds.length} enabled`} open={openSections} onToggle={toggleSection}>
        {enabledSkillIds.length > 0 ? (
          <p className="settings-hint">{enabledSkillIds.join(", ")}</p>
        ) : (
          <p className="settings-hint">No skills selected</p>
        )}
      </Section>

      {/* MCP Servers */}
      <Section id="mcps" title="MCP Servers" count={`${enabledMcps.length} enabled`} open={openSections} onToggle={toggleSection}>
        {enabledMcps.length > 0 ? (
          enabledMcps.map((m) => (
            <div key={m.id} className="review-item">
              <span className="review-item__label">{m.name}</span>
              <span className="review-item__value">{m.type}</span>
            </div>
          ))
        ) : (
          <p className="settings-hint">No MCP servers enabled</p>
        )}
      </Section>

      {/* Agents */}
      <Section id="agents" title="Agent Config" count={overrideCount > 0 ? `${overrideCount} customized` : "all default"} open={openSections} onToggle={toggleSection}>
        <div className="review-item">
          <span className="review-item__label">Default</span>
          <span className="review-item__value">
            {getProviderName(globalDefault.providerId)} / {globalDefault.model} / {globalDefault.temperature} / {globalDefault.maxTokens}
          </span>
        </div>
        {Object.entries(agentOverrides).map(([agentId, override]) => (
          <div key={agentId} className="review-item">
            <span className="review-item__label">{agentId}</span>
            <span className="review-item__value">
              {getProviderName(override.providerId || globalDefault.providerId)} / {override.model || globalDefault.model}
            </span>
          </div>
        ))}
      </Section>

      {error && <div className="alert alert--error">{error}</div>}

      <button
        className="btn btn--primary btn--lg"
        onClick={handleLaunch}
        disabled={isLaunching}
        style={{ width: "100%", marginTop: "var(--spacing-4)" }}
      >
        {isLaunching ? <><Loader2 className="loading-spinner__icon" /> Launching...</> : "Launch"}
      </button>
    </div>
  );
}

function Section({ id, title, count, open, onToggle, children }: {
  id: string; title: string; count: string;
  open: Set<string>; onToggle: (id: string) => void;
  children: React.ReactNode;
}) {
  const isOpen = open.has(id);
  return (
    <div className="review-section">
      <div className="review-section__header" onClick={() => onToggle(id)}>
        <span className="review-section__title">{title}</span>
        <div className="flex items-center gap-2">
          <span className="review-section__count">{count}</span>
          {isOpen ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        </div>
      </div>
      {isOpen && <div className="review-section__body">{children}</div>}
    </div>
  );
}
```

- [ ] **Step 3: Build and verify**

Run:
```bash
cd apps/ui && npm run build 2>&1 | tail -5
```
Expected: Build succeeds.

- [ ] **Step 4: Commit**

```bash
git add apps/ui/src/features/setup/steps/AgentsStep.tsx apps/ui/src/features/setup/steps/ReviewStep.tsx
git commit -m "feat: add AgentsStep and ReviewStep components"
```

---

### Task 9: UI — SetupWizard Container, SetupGuard, and Routing

**Files:**
- Create: `apps/ui/src/features/setup/SetupWizard.tsx`
- Create: `apps/ui/src/features/setup/SetupGuard.tsx`
- Create: `apps/ui/src/features/setup/index.ts`
- Modify: `apps/ui/src/App.tsx`

- [ ] **Step 1: Create SetupWizard**

Create `apps/ui/src/features/setup/SetupWizard.tsx`:

```tsx
import { useReducer, useCallback } from "react";
import { useNavigate } from "react-router-dom";
import type { ProviderResponse, McpServerConfig } from "@/services/transport";
import { StepIndicator } from "./components/StepIndicator";
import { WizardNav } from "./components/WizardNav";
import { NameStep } from "./steps/NameStep";
import { ProvidersStep } from "./steps/ProvidersStep";
import { SkillsStep } from "./steps/SkillsStep";
import { McpStep } from "./steps/McpStep";
import { AgentsStep } from "./steps/AgentsStep";
import { ReviewStep } from "./steps/ReviewStep";

interface WizardState {
  currentStep: number;
  agentName: string;
  namePreset: string | null;
  providers: ProviderResponse[];
  defaultProviderId: string;
  enabledSkillIds: string[];
  mcpConfigs: McpServerConfig[];
  globalDefault: {
    providerId: string;
    model: string;
    temperature: number;
    maxTokens: number;
  };
  agentOverrides: Record<string, {
    providerId?: string;
    model?: string;
    temperature?: number;
    maxTokens?: number;
  }>;
}

type WizardAction =
  | { type: "SET_STEP"; step: number }
  | { type: "SET_NAME"; name: string; preset: string | null }
  | { type: "SET_PROVIDERS"; providers: ProviderResponse[]; defaultId: string }
  | { type: "SET_SKILLS"; ids: string[] }
  | { type: "SET_MCPS"; configs: McpServerConfig[] }
  | { type: "SET_GLOBAL_DEFAULT"; defaults: WizardState["globalDefault"] }
  | { type: "SET_OVERRIDES"; overrides: WizardState["agentOverrides"] };

function reducer(state: WizardState, action: WizardAction): WizardState {
  switch (action.type) {
    case "SET_STEP": return { ...state, currentStep: action.step };
    case "SET_NAME": return { ...state, agentName: action.name, namePreset: action.preset };
    case "SET_PROVIDERS": return { ...state, providers: action.providers, defaultProviderId: action.defaultId };
    case "SET_SKILLS": return { ...state, enabledSkillIds: action.ids };
    case "SET_MCPS": return { ...state, mcpConfigs: action.configs };
    case "SET_GLOBAL_DEFAULT": return { ...state, globalDefault: action.defaults };
    case "SET_OVERRIDES": return { ...state, agentOverrides: action.overrides };
    default: return state;
  }
}

const initialState: WizardState = {
  currentStep: 1,
  agentName: "Brahmi",
  namePreset: "brahmi",
  providers: [],
  defaultProviderId: "",
  enabledSkillIds: [],
  mcpConfigs: [],
  globalDefault: { providerId: "", model: "", temperature: 0.7, maxTokens: 4096 },
  agentOverrides: {},
};

const STEP_TITLES: Record<number, { title: string; subtitle: string }> = {
  1: { title: "What should we call your agent?", subtitle: "Pick a personality or choose your own name." },
  2: { title: "Connect your AI providers", subtitle: "Add at least one provider to power your agents." },
  3: { title: "Enable skills", subtitle: "Choose which skills your agents can use." },
  4: { title: "Configure tool servers", subtitle: "Connect external tools and services via MCP." },
  5: { title: "Configure your agents", subtitle: "Set a default model, then customize individual agents." },
  6: { title: "Review & Launch", subtitle: "Everything looks good? Hit launch to get started." },
};

export function SetupWizard() {
  const [state, dispatch] = useReducer(reducer, initialState);
  const navigate = useNavigate();

  const canNext = (): boolean => {
    switch (state.currentStep) {
      case 1: return state.agentName.trim().length > 0;
      case 2: return state.providers.some((p) => p.verified);
      case 3: return true; // skippable
      case 4: return true; // skippable
      case 5: return !!state.globalDefault.providerId && !!state.globalDefault.model;
      case 6: return true;
      default: return false;
    }
  };

  const handleNext = () => {
    if (state.currentStep < 6) {
      dispatch({ type: "SET_STEP", step: state.currentStep + 1 });
    }
  };

  const handleBack = () => {
    if (state.currentStep > 1) {
      dispatch({ type: "SET_STEP", step: state.currentStep - 1 });
    }
  };

  const handleSkip = useCallback(() => {
    navigate("/");
  }, [navigate]);

  const handleLaunchComplete = useCallback(() => {
    sessionStorage.setItem("setupComplete", "true");
    navigate("/");
  }, [navigate]);

  const isSkippable = state.currentStep === 3 || state.currentStep === 4;
  const stepInfo = STEP_TITLES[state.currentStep];

  return (
    <div className="setup-wizard">
      <div className="setup-wizard__container">
        <StepIndicator currentStep={state.currentStep} />

        <div className="setup-wizard__header">
          <h2 className="setup-wizard__title">{stepInfo.title}</h2>
          <p className="setup-wizard__subtitle">{stepInfo.subtitle}</p>
        </div>

        <div className="setup-wizard__body">
          {state.currentStep === 1 && (
            <NameStep
              agentName={state.agentName}
              namePreset={state.namePreset}
              onChange={(name, preset) => dispatch({ type: "SET_NAME", name, preset })}
            />
          )}
          {state.currentStep === 2 && (
            <ProvidersStep
              providers={state.providers}
              defaultProviderId={state.defaultProviderId}
              onProvidersChanged={(providers, defaultId) =>
                dispatch({ type: "SET_PROVIDERS", providers, defaultId })
              }
            />
          )}
          {state.currentStep === 3 && (
            <SkillsStep
              enabledSkillIds={state.enabledSkillIds}
              onChange={(ids) => dispatch({ type: "SET_SKILLS", ids })}
            />
          )}
          {state.currentStep === 4 && (
            <McpStep
              mcpConfigs={state.mcpConfigs}
              onChange={(configs) => dispatch({ type: "SET_MCPS", configs })}
            />
          )}
          {state.currentStep === 5 && (
            <AgentsStep
              providers={state.providers}
              defaultProviderId={state.defaultProviderId}
              agentName={state.agentName}
              globalDefault={state.globalDefault}
              agentOverrides={state.agentOverrides}
              onGlobalChange={(defaults) => dispatch({ type: "SET_GLOBAL_DEFAULT", defaults })}
              onOverrideChange={(overrides) => dispatch({ type: "SET_OVERRIDES", overrides })}
            />
          )}
          {state.currentStep === 6 && (
            <ReviewStep
              agentName={state.agentName}
              providers={state.providers}
              defaultProviderId={state.defaultProviderId}
              enabledSkillIds={state.enabledSkillIds}
              mcpConfigs={state.mcpConfigs}
              globalDefault={state.globalDefault}
              agentOverrides={state.agentOverrides}
              onLaunchComplete={handleLaunchComplete}
            />
          )}
        </div>

        {state.currentStep < 6 && (
          <WizardNav
            currentStep={state.currentStep}
            canNext={canNext()}
            onBack={handleBack}
            onNext={handleNext}
            onSkip={isSkippable || state.currentStep === 1 ? handleSkip : undefined}
          />
        )}
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Create SetupGuard**

Create `apps/ui/src/features/setup/SetupGuard.tsx`:

```tsx
import { useEffect, useState } from "react";
import { useNavigate, useLocation } from "react-router-dom";
import { Loader2 } from "lucide-react";
import { getTransport } from "@/services/transport";

interface SetupGuardProps {
  children: React.ReactNode;
}

export function SetupGuard({ children }: SetupGuardProps) {
  const [isChecking, setIsChecking] = useState(true);
  const navigate = useNavigate();
  const location = useLocation();

  useEffect(() => {
    // Skip check if already on /setup
    if (location.pathname === "/setup") {
      setIsChecking(false);
      return;
    }

    // Check session cache first
    const cached = sessionStorage.getItem("setupComplete");
    if (cached === "true") {
      setIsChecking(false);
      return;
    }

    const check = async () => {
      try {
        const transport = await getTransport();
        const result = await transport.getSetupStatus();
        if (result.success && result.data) {
          if (result.data.setupComplete || result.data.hasProviders) {
            sessionStorage.setItem("setupComplete", "true");
          } else {
            navigate("/setup", { replace: true });
            return;
          }
        }
      } catch {
        // If check fails, don't block — let user proceed
      }
      setIsChecking(false);
    };
    check();
  }, [navigate, location.pathname]);

  if (isChecking) {
    return (
      <div className="loading-spinner">
        <Loader2 className="loading-spinner__icon" />
      </div>
    );
  }

  return <>{children}</>;
}
```

- [ ] **Step 3: Create barrel export**

Create `apps/ui/src/features/setup/index.ts`:

```typescript
export { SetupWizard } from "./SetupWizard";
export { SetupGuard } from "./SetupGuard";
```

- [ ] **Step 4: Update App.tsx routing**

In `apps/ui/src/App.tsx`, add imports:

```typescript
import { SetupWizard, SetupGuard } from "./features/setup";
```

Add the `/setup` route BEFORE the `WebAppShell` (it renders without sidebar):

```tsx
      <Routes>
        {/* Setup wizard — renders without app shell */}
        <Route path="/setup" element={<SetupWizard />} />

        {/* Main app with sidebar */}
        <Route path="/*" element={
          <SetupGuard>
            <WebAppShell connectionStatus={connectionStatus}>
              <Routes>
                <Route path="/" element={<WebChatPanel />} />
                <Route path="/dashboard" element={<WebOpsDashboard />} />
                <Route path="/logs" element={<WebLogsPanel />} />
                <Route path="/memory" element={<WebMemoryPanel />} />
                <Route path="/observatory" element={<ObservatoryPage />} />
                <Route path="/agents" element={<WebAgentsPanel />} />
                <Route path="/integrations" element={<WebIntegrationsPanel />} />
                <Route path="/settings" element={<WebSettingsPanel />} />
                <Route path="/chat" element={<Navigate to="/" replace />} />
                <Route path="/providers" element={<Navigate to="/settings" replace />} />
                <Route path="/skills" element={<Navigate to="/agents?tab=skills" replace />} />
                <Route path="/hooks" element={<Navigate to="/agents?tab=schedules" replace />} />
                <Route path="/connectors" element={<Navigate to="/integrations?tab=plugins" replace />} />
                <Route path="/mcps" element={<Navigate to="/integrations" replace />} />
              </Routes>
            </WebAppShell>
          </SetupGuard>
        } />
      </Routes>
```

- [ ] **Step 5: Build and verify end-to-end**

Run:
```bash
cd apps/ui && npm run build 2>&1 | tail -5
cargo build -p gateway 2>&1 | grep "^error"
```
Expected: Both succeed with no errors.

- [ ] **Step 6: Commit**

```bash
git add apps/ui/src/features/setup/ apps/ui/src/App.tsx
git commit -m "feat: add SetupWizard, SetupGuard, wire routing for first-time setup"
```

---

### Task 10: Manual Testing Checklist

- [ ] **Step 1: Reset setup state for testing**

Delete existing providers and reset setupComplete:

```bash
# Backup first
cp ~/Documents/zbot/config/settings.json ~/Documents/zbot/config/settings.json.bak
cp ~/Documents/zbot/config/providers.json ~/Documents/zbot/config/providers.json.bak

# Clear providers to trigger first-run
echo "[]" > ~/Documents/zbot/config/providers.json

# Reset setupComplete
python3 -c "
import json
with open('$HOME/Documents/zbot/config/settings.json') as f: d=json.load(f)
d.setdefault('execution',{})['setupComplete']=False
with open('$HOME/Documents/zbot/config/settings.json','w') as f: json.dump(d,f,indent=2)
"
```

- [ ] **Step 2: Start the daemon and open the UI**

```bash
# Start daemon
cargo run -p gateway

# Open browser
open http://localhost:18791
```

Expected: Auto-redirects to `/setup` showing Step 1.

- [ ] **Step 3: Walk through all 6 steps**

1. Step 1: Select "Brahmi", verify name fills. Change to custom, verify input works. Click Next.
2. Step 2: Add a provider (e.g., Ollama if running locally). Verify "Test & Add" works. Verify it shows as verified. Click Next.
3. Step 3: Toggle some skills. Verify recommended are pre-checked. Click Next.
4. Step 4: Verify keyless servers shown as ready. Verify key servers have empty input. Click Next.
5. Step 5: Verify global default populated from Step 2 provider. Click "Customize" on an agent. Verify override works. Click Next.
6. Step 6: Verify summary correct. Click Launch.

Expected: Redirects to Chat. Root agent shows with chosen name.

- [ ] **Step 4: Verify setup doesn't re-trigger**

Refresh the browser page.

Expected: Stays on Chat, does NOT redirect to `/setup`.

- [ ] **Step 5: Verify re-run from Settings**

Navigate to Settings > Advanced tab. Click "Re-run Setup Wizard".

Expected: Opens `/setup` with current config pre-filled (providers already shown).

- [ ] **Step 6: Restore backups if needed**

```bash
cp ~/Documents/zbot/config/settings.json.bak ~/Documents/zbot/config/settings.json
cp ~/Documents/zbot/config/providers.json.bak ~/Documents/zbot/config/providers.json
```
