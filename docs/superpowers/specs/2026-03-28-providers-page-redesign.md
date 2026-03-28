# Providers Page Redesign — Design Spec

## Problem

The current Providers page was built as a developer tool. It fails non-technical users in 6 ways:

1. **No inline editing** — API key rotates? Delete and recreate the entire provider.
2. **Raw comma-separated models** — users type model IDs by hand with no help.
3. **No onboarding guidance** — blank form with no explanation of what a provider is.
4. **Doesn't use the design system** — 450 lines of inline Tailwind, ignores `.card`, `.btn`, `.form-input`, `.badge`, `.empty-state` from components.css.
5. **No model context** — tiny icons with no labels. Non-technical users don't know what they mean.
6. **No test-before-save** — create a provider, then separately test it. Bad key = broken provider you can't fix.

## Goal

A professional, friendly Providers page that:
- Guides new users through connecting their first AI provider in under 30 seconds
- Shows provider status, capabilities, and model info at a glance
- Supports editing (API key, base URL, models) without delete-and-recreate
- Uses the Warm Sand design system consistently (semantic CSS classes)
- Makes model capabilities visible and understandable to non-technical users

## Target User

Non-technical users who understand a bit of code but primarily want agents to simplify their work. They know what "OpenAI" and "API key" mean, but shouldn't need to know model IDs or base URLs.

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Empty state | Focused Top-3 + expand | Don't overwhelm. Show OpenAI, Anthropic, Ollama prominently. "Show more" for the rest. |
| Main layout | Card grid + slide-over | Cards give visual overview of all providers. Slide-over for detail without losing context. |
| Detail panel | View + Edit toggle | Prevents accidental edits. Explicit Edit → Save/Cancel flow is safer for non-technical users. |
| Add provider | Inline expand card | Fewest clicks for happy path. Click preset → type API key → Test & Connect. "Advanced" link for full options. |

## Page States

### State 1: Empty (no providers configured)

```
┌─────────────────────────────────────────────────────┐
│                                                     │
│                   Get Started                       │
│    Connect an AI provider so your agents can think. │
│    Most users start with one of these:              │
│                                                     │
│   ┌──────────┐  ┌──────────┐  ┌──────────┐        │
│   │  OpenAI   │  │ Anthropic│  │  Ollama  │        │
│   │ GPT-4o,  │  │ Claude   │  │ Free,    │        │
│   │ o4-mini  │  │ Sonnet 4 │  │ local    │        │
│   │[Connect] │  │[Connect] │  │[Connect] │        │
│   └──────────┘  └──────────┘  └──────────┘        │
│                                                     │
│     Show 6 more providers · Custom provider         │
│                                                     │
└─────────────────────────────────────────────────────┘
```

- Top-3 presets as large cards with name, key models, and Connect button
- "Show 6 more" expands to reveal: DeepSeek, Google Gemini, OpenRouter, Z.AI, Mistral, Ollama Cloud
- "Custom provider" opens the slide-over with a blank form
- Already-configured providers are filtered out by matching **base URL** (trailing-slash normalized). Name matching is secondary (case-insensitive). If all presets are configured, the preset section disappears and only "+ Add custom provider" remains.

### State 2: Inline Expand (after clicking a preset)

The expanded card renders below the preset grid as a separate section (avoids grid reflow issues). The preset cards above dim (opacity: 0.5, transition 200ms).

Expanded section shows:
- Provider name + pre-filled info (base URL, models)
- API key input (auto-focused)
- "Test & Connect" button — tests the connection, then saves if successful
- "Advanced" link opens the full slide-over (for editing base URL, models)
- "Cancel" collapses back to the preset grid

For Ollama specifically: no API key field needed (local), just "Connect" to test localhost:11434.

**Error handling**: If test fails, show an inline `.alert--error` below the API key input with the error message. Keep the form expanded. User can edit the key and retry. Network timeout shows "Could not reach provider — check the URL and try again."

### State 3: Main View (providers exist)

```
┌─────────────────────────────────────────────────────┐
│ Providers                     [+ Add Provider]      │
│ 3 connected · 1 active                              │
│                                                     │
│ ┌─────────────────────┐ ┌─────────────────────┐    │
│ │ Z.AI          ━━━━━ │ │ DeepSeek      ━━━━━ │    │
│ │ api.z.ai      Active│ │ api.deepseek.com    │    │
│ │ ● Connected         │ │ ● Connected         │    │
│ │ [glm-5.1] [glm-5]  │ │ [deepseek-chat]     │    │
│ │ +4 more             │ │ [deepseek-reasoner] │    │
│ └─────────────────────┘ └─────────────────────┘    │
│ ┌─────────────────────┐                             │
│ │ OpenRouter    ━━━━━ │                             │
│ │ openrouter.ai       │                             │
│ │ ○ Not tested        │                             │
│ │ [3 models]          │                             │
│ └─────────────────────┘                             │
│                                                     │
│     + Add another provider                          │
│                                                     │
└─────────────────────────────────────────────────────┘
```

- Responsive 2-column card grid: `md:grid-cols-2` (Tailwind breakpoint, 1-col below 768px)
- Active provider has `var(--primary)` border
- Each card shows: name, shortened base URL, status badge (Connected/Not tested), model chips (first 2-3 + "+N more")
- Click a card → opens slide-over detail panel
- "+ Add Provider" button in page header
- "+ Add another provider" link at bottom shows the preset selection (filtered)

**Status badge logic**: Uses `provider.verified` boolean. `true` → "Connected" (green badge). `false` or `undefined` → "Not tested" (warning badge). No timestamp for v1 — just the boolean.

### State 4: Slide-over Detail Panel

```
┌────────────────────────────────┐
│ Z.AI                    [Edit] │
│ ● Connected  ◆ Active     [✕] │
├────────────────────────────────┤
│ ● Connection verified    Test  │
├────────────────────────────────┤
│ API KEY                        │
│ 83c6••••••ca20                 │
│                                │
│ BASE URL                       │
│ api.z.ai/api/coding/paas/v4   │
│                                │
│ DEFAULT MODEL                  │
│ glm-5.1                        │
│                                │
│ MODELS (6)                     │
│ [glm-5.1 🔧👁🧠 128K]         │
│ [glm-5 🔧👁 128K]             │
│ [glm-4.7 🔧👁 128K]           │
│ [glm-5-turbo 🔧 128K]         │
│ [glm-4.6 🔧👁 128K]           │
│ [glm-4.5 🔧👁 128K]           │
├────────────────────────────────┤
│ Delete provider    Set as active│
└────────────────────────────────┘
```

**View mode (default):**
- Read-only display of all fields
- Status badge with verified/not-tested indicator
- Model chips with capability icons AND context window (e.g., "128K")
- "Test" button for on-demand connection test
- "Edit" button in header to enter edit mode
- Footer: "Delete provider" (opens confirmation Dialog) and "Set as active"

**Edit mode (after clicking Edit):**
- All fields become editable inputs
- API key shows full value with show/hide toggle
- Base URL is an editable text input
- Models become tag chips with X to remove + "+ Add model" button
- Default model becomes a Select dropdown (populated from current models)
- Header changes to: Save and Cancel buttons
- Inline validation (base URL format, API key not empty)

### State 5: Add Provider Slide-over (custom or "Advanced")

Same slide-over component in create mode:
- Pre-filled from preset if applicable
- API key hint with link to provider's key page (e.g., "Get key from platform.openai.com")
- Model tag chips pre-filled from preset, editable
- "Test & Connect" button (tests before saving)

## Slide-over Behavior

- **Width**: `var(--modal-lg-width)` (576px / 36rem)
- **Position**: Fixed, right edge, full height
- **Backdrop**: Semi-transparent overlay. Clicking backdrop closes in view mode. In edit mode, clicking backdrop triggers the unsaved-changes dialog.
- **Animation**: Slide in from right, 200ms ease-out. Backdrop fades in 150ms.
- **Close triggers**: X button, Escape key (in view mode), backdrop click (in view mode)
- **Focus**: Focus is trapped inside the slide-over while open. First focusable element receives focus on open. Focus returns to the triggering card on close.
- **Escape in edit mode**: Shows unsaved-changes confirmation dialog instead of closing.

## Dirty State Handling

When the user has unsaved changes in edit mode and attempts to leave (close slide-over, click different card, navigate away):

- Show a confirmation dialog (using existing `shared/ui/dialog.tsx`): "You have unsaved changes. Discard them?"
- Two buttons: "Discard" (closes without saving) and "Keep editing" (returns to edit mode)
- No auto-save — explicit Save/Cancel only

## Loading States

| Operation | UI Feedback |
|-----------|-------------|
| Page load (providers + models) | Full-page centered Loader2 spinner |
| Test connection (inline or slide-over) | Button shows Loader2 spinner, disabled. Result replaces spinner. |
| Save edits | "Save" button shows Loader2 spinner, disabled. On success: returns to view mode. On error: inline alert. |
| Delete provider | Confirmation Dialog. "Delete" button shows spinner during request. On success: slide-over closes, card removed. |
| Set as active | Button shows spinner. On success: card borders update, badge updates. |
| Create provider (Test & Connect) | Button shows spinner during test. On test success: auto-saves (brief spinner). On test fail: inline error. |

## Model Capability Display

Model chips in the detail panel and grid cards show:
- Model name
- Capability icons with **Tooltips** (using existing `shared/ui/tooltip.tsx`): Wrench → "Tool Calling", Eye → "Vision", Brain → "Thinking", Volume2 → "Voice"
- Context window size formatted as compact notation: 128000 → "128K", 200000 → "200K", 1048576 → "1M". Show input context only.

Use Lucide icons (Wrench, Eye, Brain, Volume2) — not emoji.

Unknown models (not in registry) show name only, no badges.

## Design System Migration

Replace all inline Tailwind with semantic CSS classes from components.css:

| Current (inline) | Replace with |
|-------------------|-------------|
| `className="flex h-full bg-[var(--background)]"` | `className="page"` |
| `className="bg-[var(--card)] rounded-xl p-4 card-shadow"` | `className="card card__padding"` |
| `className="bg-[var(--primary)] hover:bg-[var(--primary)]/90..."` | `className="btn btn--primary btn--sm"` |
| `className="w-full bg-[var(--background)] border..."` | `className="form-input"` |
| `className="text-xs text-[var(--muted-foreground)]..."` | `className="form-label"` or `className="settings-field-label"` |
| `className="inline-flex items-center px-2 py-0.5..."` | `className="badge"` or `className="badge badge--primary"` |
| Empty state container | `className="empty-state"` with `.empty-state__icon`, `.empty-state__title` |

New CSS classes to add to components.css:
- `.provider-card` — extends `.card--interactive` with status border
- `.provider-card--active` — `border-color: var(--primary)` (works in both light/dark via design tokens)
- `.provider-card__status` — badge positioning (absolute top-right)
- `.model-chip` — model tag with capability icons
- `.model-chip__capabilities` — icon container within chip
- `.provider-slideover` — slide-over panel, width `var(--modal-lg-width)`, fixed right
- `.provider-slideover__backdrop` — semi-transparent overlay
- `.inline-connect` — expanded preset form section

All new classes use design tokens (`var(--primary)`, `var(--card)`, etc.) — dark mode is automatically supported with no additional overrides needed.

## Shared Components to Reuse

| Component | Used For |
|-----------|----------|
| `shared/ui/tooltip.tsx` | Model capability icon tooltips |
| `shared/ui/dialog.tsx` | Delete confirmation, unsaved changes confirmation |
| `shared/ui/select.tsx` | Default model dropdown in edit mode |
| `shared/ui/badge.tsx` | Status badges (Connected, Not tested, Active) |
| `shared/ui/button.tsx` | All buttons (primary, ghost, destructive variants) |
| `shared/ui/input.tsx` | Form fields in edit mode |

## API Changes

### Existing endpoints (no changes needed)
- `PUT /api/providers/:id` — already supports partial updates (UpdateProviderRequest has all fields optional)
- `POST /api/providers/:id/test` — existing test endpoint (for configured providers)
- `POST /api/providers/test` — test inline without saving (for new providers)
- `POST /api/providers/:id/default` — set as active

**Recommended "Test & Connect" flow**: Use existing `POST /api/providers/test` to validate, then `POST /api/providers` to save. No backend changes needed.

## Component Structure

Start with separate files — the redesign is too complex for a single file (5 states, view/edit toggle, inline expand, slide-over):

```
apps/ui/src/features/integrations/
├── WebIntegrationsPanel.tsx      (page container, routing between states)
├── ProvidersEmptyState.tsx       (empty state with Top-3 presets + inline expand)
├── ProvidersGrid.tsx             (card grid of configured providers)
├── ProviderCard.tsx              (individual provider card)
├── ProviderSlideover.tsx         (slide-over: view/edit/create modes)
├── ModelChip.tsx                 (model tag with capability badges + tooltip)
└── providerPresets.ts            (preset data: name, baseUrl, models, apiKeyHint)
```

## Accessibility

- All interactive elements (cards, buttons, links) are focusable via keyboard
- Card grid uses `role="button"` + `tabIndex={0}` + `onKeyDown` (Enter/Space)
- Slide-over traps focus while open, returns focus to trigger on close
- Escape key closes slide-over (with dirty-state check in edit mode)
- Capability tooltips are keyboard-accessible via the existing Radix Tooltip component

## Files Changed

| File | Change |
|------|--------|
| `apps/ui/src/features/integrations/WebIntegrationsPanel.tsx` | Complete rewrite (page container) |
| `apps/ui/src/features/integrations/ProvidersEmptyState.tsx` | New: empty state + inline connect |
| `apps/ui/src/features/integrations/ProvidersGrid.tsx` | New: card grid |
| `apps/ui/src/features/integrations/ProviderCard.tsx` | New: individual provider card |
| `apps/ui/src/features/integrations/ProviderSlideover.tsx` | New: slide-over detail/edit/create |
| `apps/ui/src/features/integrations/ModelChip.tsx` | New: model tag with capability badges |
| `apps/ui/src/features/integrations/providerPresets.ts` | New: preset data (extracted from current inline array) |
| `apps/ui/src/styles/components.css` | Add `.provider-card`, `.model-chip`, `.provider-slideover`, `.inline-connect` classes |
| `apps/ui/src/services/transport/types.ts` | Already has ModelRegistryResponse (done) |
| `apps/ui/src/services/transport/http.ts` | Already has listModels() (done) |

## Non-Goals

- No drag-and-drop reordering of providers
- No bulk import/export of providers
- No provider usage analytics or cost tracking
- No real-time WebSocket status polling (test is on-demand)
- No `lastTestedAt` timestamp (uses boolean `verified` for v1)

## Known Limitations

- Model capability badges depend on the model being in the registry. Unknown models show name only.
- "Test & Connect" tests the connection but doesn't validate individual models.
- Status is based on `verified` boolean — no "last tested" timestamp in v1.
