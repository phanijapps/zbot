# Memory Policies — Backlog Feature

## Goal

Let users create, edit, and manage **policy facts** via the Memory UI — high-priority corrections/rules that the brain always follows. No code changes, no prompt edits. Just add a policy in the UI and every future session respects it.

## What It Is

A policy is a memory fact with:
- `category: "correction"` (highest recall priority, 1.5x weight)
- `confidence: 1.0`
- `ward_id: "__global__"` (or ward-scoped)
- `mention_count: 5` (high boost)

Policies surface at the top of every recall as "Rules (ALWAYS follow)."

## UI: Memory Page → Policies Tab

Add a "Policies" tab to the existing Memory page (`/memory`). Shows:

### Policy List
- Each policy: content, scope (global/ward-scoped), created date, active toggle
- Edit inline — click to modify content
- Delete with confirmation
- Toggle active/inactive (sets confidence to 0.0 when inactive, 1.0 when active)

### Add Policy Form
- Content textarea (the rule)
- Scope dropdown: Global / specific ward
- Priority: Critical (confidence 1.0) / Important (0.9) / Guidance (0.8)
- "Save Policy" button

### Example Policies
Pre-populated suggestions (user can enable):
- "Always delegate to research-agent for factual data — never rely on LLM training data"
- "Keep all code files under 3KB — split into modules if larger"
- "Use duckduckgo-search + light-panda-browser for web research, not raw shell curl"
- "For ADHD educational content: 3-minute sections, visual, interactive, no walls of text"

## API

### Endpoints
```
GET    /api/policies              — list all policies
POST   /api/policies              — create policy
PUT    /api/policies/:id          — update policy
DELETE /api/policies/:id          — delete policy
PATCH  /api/policies/:id/toggle   — toggle active/inactive
```

### Under the Hood
Policies are just memory facts with `category = "correction"`. The API is a thin wrapper:
- Create: `upsert_memory_fact` with category=correction, confidence=1.0, ward_id=__global__, mention_count=5
- Toggle: update confidence (1.0 ↔ 0.0)
- Delete: remove from memory_facts

No new tables. No schema changes. Uses existing infrastructure.

## Implementation Estimate

| Component | Files | Effort |
|-----------|-------|--------|
| Gateway API endpoints | `gateway/src/http/policies.rs` | Low |
| Transport types + methods | `apps/ui/src/services/transport/` | Low |
| Policies tab UI | `apps/ui/src/features/memory/PoliciesTab.tsx` | Medium |
| Wire into Memory page | `apps/ui/src/features/memory/` | Low |

## Priority

Medium — the manual SQL insert works for now. This is a UX improvement for managing policies without touching the database directly.
