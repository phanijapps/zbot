# Model text input — design

**Goal:** Replace dropdown-based model selection in Settings > Advanced and the
per-agent edit panel with free-text inputs, so users can type any model
identifier a provider exposes without waiting for it to be added to
`providers[].models[]` or `models.json`. Remove the backend capability gates
that currently silently refuse user-typed models.

**Branch:** `enhancements`

---

## Scope

Three surfaces change, all in `apps/ui/src/features/`:

| Surface | File | Fields converted |
|---|---|---|
| Settings > Advanced | `settings/WebSettingsPanel.tsx` | Orchestrator model · Distillation model · Multimodal model |
| Agent edit | `agent/AgentEditPanel.tsx` | Model |
| (Agents > Schedules > Agent picker) | `agent/WebAgentsPanel.tsx` | **No change.** Picks among existing agents, not models. |

Provider pickers remain dropdowns everywhere. Providers carry API key + base
URL + rate limits and must be configured in the Providers UI — they are a true
referential link, not a free-form identifier.

---

## UI component — `ModelTextInput`

One shared component under `apps/ui/src/features/shared/modelTextInput/`.
Reused by all three call sites.

### Behavior

- `<input type="text">` styled with the existing `.form-input` class.
- On focus or typing, a floating suggestion list renders below the input.
  Suggestions come from the currently selected provider's `providers[].models[]`.
- Typing a value not in the suggestion list is accepted verbatim. No
  validation, no warning.
- Empty string means "use the provider default," same semantics the old
  dropdown's empty `<option value="">` carried.
- Changing the parent `providerId` does **not** wipe the model field — the
  user's text stays. A small "Reset to default" link next to the field clears
  to the new provider's first model.
- Keyboard: `↓/↑` move highlight, `Enter` selects highlighted entry, `Esc` or
  outside click closes the list. `aria-autocomplete="list"`, `aria-controls`,
  `aria-activedescendant` tie the listbox to the input for screen readers.

### Styling

Suggestion list uses the theme tokens already in the project:

```
background: var(--card)
border: 1px solid var(--border)
border-radius: var(--radius-md)
box-shadow: 0 8px 24px rgba(0,0,0,0.08)
item hover / highlight: var(--muted)
item focused color: var(--foreground)
```

No native `<datalist>` — browser-default styling ignores our tokens and looks
off against the rest of the forms.

### Interface

```ts
interface ModelTextInputProps {
  /** Current model value (empty string means provider default). */
  value: string;
  onChange(next: string): void;
  /** Models to show as suggestions — usually providers.find(p=>p.id===providerId).models. */
  suggestions: string[];
  /** Placeholder when empty. */
  placeholder?: string;
  /** HTML id for label association. */
  id?: string;
  disabled?: boolean;
}
```

Parent owns provider state and recomputes `suggestions` when the selected
provider changes. Component never fetches or knows about providers directly.

---

## Backend — drop capability gating (Option A)

The `ModelRegistry` already tolerates unknown models by returning a fallback
`ModelProfile`. The user-facing issue is that several call sites gate behavior
on `has_capability()`, which silently turns off features when the user types a
model not in the registry.

Three edits.

### 1. `gateway/gateway-services/src/models.rs` — adjust fallback

```rust
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
        input: 200_000,
        output: Some(64_000),
    },
    embedding: None,
};
```

Numbers are the user's spec — conservative for unknown models but large
enough for typical cloud models not in the curated list.

### 2. `gateway/gateway-execution/src/invoke/executor.rs:315` — remove thinking auto-disable

```rust
// delete:
if thinking_enabled
    && !registry.has_capability(&agent.model, Capability::Thinking)
{
    tracing::warn!(
        "thinking_enabled but model lacks thinking capability — disabling \
         model={}",
        agent.model
    );
    thinking_enabled = false;
}
```

When `agent.thinking_enabled == true`, pass the reasoning payload through to
the LLM client regardless of what the registry says. If the provider rejects
with 400 / 422, the existing LLM error path surfaces it via `tool_error` WS
event.

### 3. Audit + remove vision capability gates

Grep for `Capability::Vision` and any other `has_capability(..., Vision)`
call sites in `gateway/` and `runtime/`. For each, remove the gate and let
the call proceed; provider errors bubble up through the same path as
thinking.

Expected locations (to be confirmed during implementation):
- `runtime/agent-tools/src/tools/execution/multimodal_analyze.rs` (or
  equivalent) — likely gates on vision.
- `gateway/gateway-execution` — may have a preflight that refuses vision
  calls against non-vision models.

### What stays

- `ModelRegistry` itself, used by the UI's `ModelChip` component to show
  context-window hints when the model IS in the registry. Graceful when
  absent — chip just doesn't render the metadata badges.
- `models.json` as a local override. Power users can still declare
  capabilities for a specific model if they want the UI hints.

---

## Error surfacing

No new plumbing. User types a model that can't do vision into the
Multimodal slot, and attaches an image → executor sends the vision
payload → provider returns 400 / `"model doesn't support vision"` →
LLM client `Err` → executor emits `tool_error` → UI renders a red LLM
error banner in the assistant bubble, same as today's existing error
states. The user sees the provider's message verbatim and knows
immediately which model they picked is wrong.

---

## Test plan

**Rust:**

- Unit: `models.rs` — assert fallback profile matches `(input: 200_000,
  output: Some(64_000), tools: true, vision: false, thinking: false)`.
- Unit: `executor.rs` — mock registry returning
  `thinking: false` for some model id; assert executor still sends
  reasoning params when `agent.thinking_enabled == true`.
- Unit: `multimodal_analyze` (or equivalent) — assert the tool invokes for
  a model with `vision: false` in the registry (image attached path).

**TypeScript (vitest + React Testing Library):**

- `ModelTextInput.test.tsx`:
  - typing "xyz-model" updates value
  - blank value persists
  - suggestion list appears on focus, typing filters it
  - keyboard nav (↓/Enter/Esc) works
- `WebSettingsPanel.test.tsx` — existing test updated: assert orchestrator
  model field is a text input, not a select.
- `AgentEditPanel.test.tsx` — existing test updated likewise.

**e2e:** no additions. The existing research-v2 Mode UI + Mode Full specs
already exercise `execution.orchestrator.model` through the full path.

---

## Rollout / compatibility

- **Stored config shapes unchanged.** `providerId` + `model` stay
  `{ providerId: string, model: string }`. The UI change is presentational
  only.
- **Existing agents + settings load unchanged.** Values already saved are
  just strings the text input renders by default.
- **No migration.** Users land with their current model selections
  displayed in text boxes pre-filled from settings.json / agents.json.

---

## Out of scope

- Per-agent capability checkboxes (Option C from brainstorm). If the
  bare-provider-error UX turns out painful, we revisit with a capabilities
  strip later.
- Editing `providers[].models[]` inline from Settings > Advanced. That's
  still done via the Providers UI.
- Fetching model lists from providers (`GET /v1/models` against OpenAI-
  compatible endpoints) to auto-populate suggestions. Nice-to-have, not
  blocking.
