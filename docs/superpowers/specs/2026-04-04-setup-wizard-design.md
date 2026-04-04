# First-Time Setup Wizard

## Goal

Provide a guided onboarding experience for new z-Bot users that configures their root agent identity, LLM providers, skills, MCP servers, and agent model assignments in a single linear flow.

## Trigger & Routing

- New `/setup` route, lazy-loaded via React Router
- **Auto-redirect**: On app load, if `setupComplete === false` (from `GET /api/settings/execution`) AND no providers exist, redirect to `/setup`
- **Post-wizard**: Set `setupComplete = true`, redirect to Chat (`/`)
- **Re-run**: "Re-run Setup Wizard" button in Settings > Advanced tab navigates to `/setup`. When providers already exist, wizard pre-fills current config instead of starting blank.
- **Skip**: Every step except Step 2 (Providers) has a "Skip" link. Step 2 requires at least 1 verified provider. User can also skip the entire wizard from Step 1 via a "Skip setup" link in the footer.

## Wizard Steps

### Step 1: Name Your Agent

**Purpose**: Set the root agent's identity.

**UI**:
- 2x2 grid of preset personality cards using `.card--interactive` with emoji, name, and tagline
- Presets: **Brahmi** ("Witty, resourceful, always has a plan"), **JohnnyLever** ("Energetic, creative, makes work fun"), **z-Bot** ("Professional, focused, gets things done"), **Custom...** ("Choose your own name")
- Below the grid: editable `.form-input` pre-filled from selected preset
- Clicking a preset highlights it and fills the input. Typing freely activates the "Custom" card.

**Validation**: Name required, 1-50 characters. Next button disabled if empty.

**Data produced**: `{ agentName: string }`

### Step 2: Connect Providers

**Purpose**: Add and verify at least one LLM provider.

**UI**:
- Grid of provider preset cards (from existing `providerPresets.ts`). Featured presets (OpenAI, Anthropic, Ollama) shown first.
- Already-added presets shown dimmed with "added" label
- Clicking an unadded preset expands an inline form below the grid: API key `.form-input` + "Test & Add" `.btn--primary`
- Added providers shown in a list above the grid: name, model count, `.badge--success` "verified", `.badge--primary` "default", remove `.btn--ghost`
- First verified provider becomes default. User can click another provider's row to change default.
- Ollama preset (noApiKey) skips the key field and tests immediately.

**API calls during this step**:
- `transport.createProvider(...)` — creates provider in providers.json
- `transport.testProviderById(id)` — tests connection, discovers models, persists verified status
- `transport.deleteProvider(id)` — on remove

**Providers are persisted immediately** (not deferred to Launch) because:
1. Model discovery needs a real API call
2. Step 5 needs real provider IDs and discovered model lists
3. If user abandons mid-wizard, they keep configured providers

**Validation**: At least 1 verified provider required. Next button disabled otherwise.

**Data produced**: Providers are already persisted. Wizard state tracks `{ defaultProviderId: string, providers: ProviderResponse[] }` for downstream steps.

### Step 3: Enable Skills

**Purpose**: Choose which installed skills are available to agents.

**UI**:
- Skill list using `.skill-toggle` rows grouped by category
- Each row: skill name, description, toggle switch
- Recommended skills pre-checked: `coding`, `doc`, `duckduckgo-search`
- Category headers with "Select all" / "Deselect all" toggle
- Skippable — empty selection is valid

**Data source**: `transport.listSkills()` (or equivalent — reads from skills directory)

**Data produced**: `{ enabledSkillIds: string[] }`

### Step 4: MCP Servers

**Purpose**: Configure tool servers, filling in API keys for services that need them.

**UI**:
- Two sections:
  - **Ready to use**: Servers with no API key requirements (e.g., `time`). Shown as `.card` rows with enabled toggle. Pre-enabled.
  - **Requires API key**: Servers needing keys (e.g., `brave-search`, `github`). Shown with a `.form-input` for the key and a hint about where to find it. Disabled by default — enabling requires filling in the key.
- Each server row: name, description, transport type `.meta-chip`, enabled toggle, key input (if needed)
- Skippable

**Data source**: Default MCP template shipped in `gateway/templates/default_mcps.json` — a copy of the standard MCP config with all API keys/secrets blanked from env vars. The wizard reads this template, NOT the user's live `mcps.json`.

**Data produced**: `{ mcpConfigs: McpServerConfig[] }` — only servers the user enabled, with their keys filled in.

### Step 5: Configure Agents

**Purpose**: Set model assignments for root and specialist agents.

**UI**:
- **Global default card** (`.card` with accent border):
  - Provider: `select.form-select` populated from Step 2 verified providers
  - Model: `select.form-select` populated from selected provider's discovered models (enriched with registry capabilities)
  - Temperature: `input[type=number].form-input` (default 0.7, step 0.1, range 0-2)
  - Max Output Tokens: `input[type=number].form-input` (default 4096, step 1024)
- **Agent list** below:
  - Root agent first: shows user's name from Step 1 + `.badge--primary` "root" badge
  - Then all default specialists: code-agent, research-agent, writing-agent, planner-agent, data-analyst
  - Each row shows effective config summary (provider · model · temp · tokens)
  - "Customize" `.btn--outline.btn--sm` expands inline override fields (same 4 fields as global)
  - "Reset to default" link collapses back to inherited config
  - Customized agents get a `.badge--warning` "customized" badge
- Model dropdown updates dynamically when provider changes (shows that provider's discovered models)

**Data produced**: `{ globalDefault: { providerId, model, temperature, maxTokens }, agentOverrides: Record<agentId, { providerId?, model?, temperature?, maxTokens? }> }`

### Step 6: Review & Launch

**Purpose**: Summarize all choices and submit.

**UI**:
- Summary sections in `.card` containers:
  - **Agent Identity**: Name from Step 1
  - **Providers**: Count + names, which is default
  - **Skills**: Count of enabled skills, list of names
  - **MCP Servers**: Count of enabled servers, list of names
  - **Agent Configuration**: Global default summary, list of overridden agents with their custom configs
- Each section collapsible with `.settings-toggle-btn` pattern
- "Launch" `.btn--primary.btn--lg` at bottom
- "Back" link to return to previous steps

**On Launch**:
1. Rename root agent: `transport.updateAgent(rootId, { name: agentName, displayName: agentName })`
2. Update each agent's config: `transport.updateAgent(id, { providerId, model, temperature, maxTokens })` — applies global default or per-agent override
3. Save MCP configs: `transport.createMcp(config)` for each enabled server with user-provided keys
4. Skills are globally available once installed — no per-agent assignment needed. The wizard's skill selection in Step 3 is informational for the review summary; skills are already on disk and loaded at runtime.
5. Set `setupComplete = true`: `transport.updateExecutionSettings({ ...current, setupComplete: true })`
6. Redirect to `/` (Chat)

**Error handling**: If any API call fails, show `.alert--error` inline with the failed item. Don't redirect. User can retry or go back.

## Gateway Changes

### ExecutionSettings

Add `setupComplete` field:

```rust
pub struct ExecutionSettings {
    pub max_parallel_agents: u32,    // existing
    pub setup_complete: bool,        // NEW — default: false
}
```

Serialized as `setupComplete` in JSON (existing camelCase convention).

### Setup Status Endpoint

New lightweight endpoint for the redirect check:

```
GET /api/setup/status
Response: { "setupComplete": bool, "hasProviders": bool }
```

This avoids loading the full provider list on every app load just to check if setup is needed. The endpoint reads `setupComplete` from settings and does a quick `provider_service.list()` count check.

### Default MCP Template

Ship `gateway/templates/default_mcps.json` — a sanitized copy of the standard MCP server configs with:
- All env var values containing keys/secrets set to empty string `""`
- Server structure preserved (command, args, description, transport type)
- `enabled: false` for all servers (wizard enables them based on user choice)

New endpoint:
```
GET /api/setup/mcp-defaults
Response: McpServerConfig[]
```

Returns the template configs. Only used by the wizard.

## UI Architecture

### File Structure

```
apps/ui/src/features/setup/
├── SetupWizard.tsx          # Main container — step state machine, useReducer for wizard state
├── SetupGuard.tsx           # Route guard component — checks setup status, redirects if needed
├── steps/
│   ├── NameStep.tsx         # Step 1 — preset cards + name input
│   ├── ProvidersStep.tsx    # Step 2 — provider preset grid, inline add, test
│   ├── SkillsStep.tsx       # Step 3 — skill toggle list
│   ├── McpStep.tsx          # Step 4 — MCP server config with key inputs
│   ├── AgentsStep.tsx       # Step 5 — global default + per-agent overrides
│   └── ReviewStep.tsx       # Step 6 — summary + launch
├── components/
│   ├── WizardNav.tsx        # Back/Next/Skip footer bar
│   └── StepIndicator.tsx    # Progress dots (1-6)
└── presets.ts               # Name preset data (Brahmi, JohnnyLever, z-Bot)
```

### Wizard State

Managed via `useReducer` in `SetupWizard.tsx`:

```typescript
interface WizardState {
  currentStep: 1 | 2 | 3 | 4 | 5 | 6;
  agentName: string;
  namePreset: string | null;
  providers: ProviderResponse[];          // refreshed from API after Step 2
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
```

### Routing Integration

In `App.tsx`:
- Add `<Route path="/setup" element={<SetupWizard />} />`
- Wrap existing routes with `<SetupGuard>` — checks `GET /api/setup/status` on mount, redirects to `/setup` if needed

`SetupGuard` caches the check result in session storage so it doesn't re-check on every navigation.

### CSS

All wizard components use existing design system classes. New CSS added to `components.css` under a `/* Setup Wizard */` section:

```
.setup-wizard               # Full-page wizard container (centered, max-width)
.setup-wizard__header        # Step title + subtitle area
.setup-wizard__body          # Step content area (scrollable)
.setup-wizard__footer        # Navigation bar (back/next/skip)

.step-indicator              # Progress dots container
.step-indicator__dot         # Individual dot
.step-indicator__dot--active # Current step
.step-indicator__dot--done   # Completed step

.name-preset-grid            # 2x2 grid for name presets
.name-preset                 # Individual preset card
.name-preset--selected       # Selected state
.name-preset__emoji          # Emoji display
.name-preset__name           # Preset name
.name-preset__tagline        # Preset description

.provider-add-grid           # Provider preset selection grid
.provider-add-card           # Individual provider preset
.provider-add-card--added    # Already added (dimmed)
.provider-add-form           # Inline add form (expanded)
.provider-added-list         # List of added providers
.provider-added-row          # Individual added provider row

.skill-category              # Skill category group
.skill-category__header      # Category header with toggle-all

.mcp-section                 # MCP server section (ready / needs-key)
.mcp-row                     # Individual MCP server row
.mcp-row__key-input          # API key input area

.agent-global-card           # Global default config card
.agent-list                  # Agent configuration list
.agent-row                   # Individual agent row
.agent-row--customized       # Customized state (accent border)
.agent-row__summary          # Collapsed summary line
.agent-row__fields           # Expanded override fields

.review-section              # Summary section card
.review-section__header      # Section header (collapsible)
.review-section__body        # Section content
.review-item                 # Individual summary item
```

No inline styles. All spacing via design tokens. All colors via semantic variables.
