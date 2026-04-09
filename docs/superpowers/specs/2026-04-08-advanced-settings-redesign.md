# Advanced Settings Redesign — Design Spec

## Problem

The Settings > Advanced page is a single long scrollable column with 3 stacked cards (Execution, Orchestrator with nested Distillation, Multimodal). The Orchestrator card alone is 155 lines of JSX. Distillation settings are buried inside it, and a backend bug prevented them from saving (now fixed).

## Solution

Replace the vertical stack with a **Command Center** — a 2x2 grid of compact module cards, each representing an AI subsystem with its own color identity. Values are displayed as read-only badges; clicking a value turns it into an inline editor. Distillation is promoted to its own card.

## Design

### Grid Layout

```
┌─────────────────────────┐  ┌─────────────────────────┐
│ 🧠 ORCHESTRATOR         │  │ 💎 DISTILLATION          │
│ indigo accent            │  │ purple accent             │
│ Provider, Model,         │  │ Provider, Model           │
│ Temp, Tokens, Thinking   │  │ (inherits if empty)       │
└─────────────────────────┘  └─────────────────────────┘
┌─────────────────────────┐  ┌─────────────────────────┐
│ 👁️ MULTIMODAL           │  │ ⚡ EXECUTION             │
│ teal accent              │  │ orange accent             │
│ Provider, Model,         │  │ Max Parallel Agents       │
│ Temp, Tokens             │  │ Setup Wizard button       │
└─────────────────────────┘  └─────────────────────────┘
```

CSS: `display: grid; grid-template-columns: 1fr 1fr; gap: var(--spacing-3);`
Responsive: `@media (max-width: 768px) { grid-template-columns: 1fr; }`

### Card Anatomy

Each card has:
1. **Accent bar** — 2px gradient at top edge, card-specific color
2. **Header** — Icon (emoji or lucide) + uppercase label + one-line subtitle
3. **Model display area** — Dark inset panel showing current MODEL and PROVIDER as key-value pairs
4. **Stat pills** — Row of compact metric boxes (Temp, Tokens, Thinking toggle) with label above, value below
5. **No save button by default** — appears only when editing

### Color Identity Per Card

| Card | Accent Gradient | Label Color |
|------|----------------|-------------|
| Orchestrator | indigo (#818cf8 → #6366f1) | #818cf8 |
| Distillation | purple (#c084fc → #a855f7) | #c084fc |
| Multimodal | teal (#2dd4bf → #14b8a6) | #2dd4bf |
| Execution | orange (#fb923c → #f97316) | #fb923c |

### Card Contents

**Orchestrator:**
- Provider (dropdown)
- Model (dropdown, filtered by provider)
- Temperature (number input, 0-2, default 0.7)
- Max Output Tokens (number input, default 16384)
- Thinking Mode (toggle, default ON)

**Distillation:**
- Provider (dropdown, placeholder "Inherit from Orchestrator")
- Model (dropdown, placeholder "Inherit from Orchestrator")
- Help text: "Override to use a cheaper model for memory extraction"

**Multimodal:**
- Provider (dropdown)
- Model (dropdown, filtered by provider)
- Temperature (number input, 0-2, default 0.3)
- Max Output Tokens (number input, default 4096)

**Execution:**
- Max Parallel Agents (number input, 1-10, default 2)
- Setup Wizard button (styled to match card accent)

### Inline Edit Behavior

1. **Default state:** Values shown as styled text (model name, provider name, numbers)
2. **Click a value:** It transforms into a dropdown/input in-place. The card shows a subtle "editing" state (slightly brighter border).
3. **Save button:** Appears at bottom of card when any value is changed. Clicking saves just that card's settings.
4. **Cancel:** Clicking outside or pressing Escape reverts to display mode.
5. **Provider → Model cascade:** Selecting a provider filters the model dropdown to that provider's models (same as current behavior).

### What Gets Removed

- Help box at bottom of page
- Verbose subtitle paragraphs under card headings
- The nested Distillation sub-section inside Orchestrator (now its own card)
- Extra vertical spacing between form fields

### What Stays

- Same settings data model (ExecutionSettings)
- Same save API (`PUT /api/settings/execution`)
- Same provider/model dropdown data source
- Status messages (restart required, save confirmation)

## Scope

### In Scope
- Restructure Advanced tab content in `WebSettingsPanel.tsx` (lines 572-905)
- New CSS for Command Center grid and card styling
- Promote Distillation to its own card
- Inline edit behavior (click to edit, save per card)
- Responsive fallback to single column

### Out of Scope
- Other settings tabs (Providers, General, Logging)
- Backend API changes (distillation fix already committed)
- New settings fields
- Provider management (add/remove providers)

## Files to Modify

| File | Change |
|------|--------|
| `apps/ui/src/styles/components.css` | Add Command Center card styles |
| `apps/ui/src/features/settings/WebSettingsPanel.tsx:572-905` | Replace Advanced tab content with grid layout |
