# Brief: Provenance-aware context compaction

- **Slug:** `provenance-aware-context-compaction`
- **Received:** 2026-09-14
- **Owner:** phanijapps
- **Status:** Draft
- **Shape:** A (outcome brief; no story list)

## Outcome

Agent executions keep model context within a usable token budget without losing
the ability to identify and reload material tool evidence. The
provenance-aware compaction middleware is the default policy, while the current
recency-only middleware remains selectable through a feature flag for
side-by-side comparison and immediate rollback.

## Appetite

One sprint.

## Rabbit holes

- Isolate the work to context-editing middleware. Do not redesign durable
  memory, recall, conversation persistence, skills, plans, gateway APIs, or
  provider integrations.

## Scope / Non-goals

**In scope:**

- A new middleware implementation that retains zbot's existing structural
  safety properties and adds compact, typed provenance for cleared tool output.
- A runtime feature flag that selects the new implementation by default and
  selects the existing middleware for control experiments or rollback.
- Construction and recovery tests that exercise each selection path.
- Observable selection and compaction decisions suitable for comparing both
  policies in test or controlled runtime environments.

**Non-goals:**

- Replacing zbot's durable semantic memory, retrieval, conversation storage,
  or checkpoint formats.
- Changing the semantics or format of existing skills, plans, tool protocols,
  or gateway endpoints.
- Building an experiment dashboard, remote flag service, or per-user rollout
  system.
- Reworking prompt summarization beyond ensuring the selected middleware
  continues to compose with the existing pipeline.

## Success measures

- The default execution path selects the provenance-aware middleware and the
  control flag selects the existing recency-only behavior.
- A cleared result retains enough typed provenance for an agent/runtime path to
  identify its original tool call and durable source without retaining the raw
  payload in model context.
- Existing context-editing guarantees—recent-result retention, exclusion rules,
  call/result coherence, and skill-resource cascade—hold for both policies.
- Tests demonstrate a deterministic policy selection and recovery outcome.

## Risks and constraints

- **Policy drift:** two independently evolving implementations could diverge
  unintentionally. Mitigate with a shared configuration contract and paired
  tests for retained behavior.
- **False provenance confidence:** a pointer that cannot be resolved later is
  worse than an explicit unavailable result. Mitigate with an explicit
  availability state and no fabricated payload.
- **Hidden scope expansion:** memory or persistence changes could turn this
  into a multi-layer redesign. Keep all changes in the existing context-editing
  middleware boundary and adapt only its immediate builder wiring.
- **Default regression:** the new path becomes default before trace evidence is
  collected. Retain the current path behind a local feature flag and make the
  effective selection observable.

## Proposed shippable cut

### Slice 1 — Dual-policy context editing

Ship the provenance-aware middleware, a shared policy-selection configuration
with the new policy as default, the existing middleware as the flag-selected
control, and focused construction/recovery tests. This is independently
deployable and reversible without changing persistent formats or public APIs.

## Spec map

| Spec | Status |
| --- | --- |
| `provenance-aware-context-compaction` | <auto> |
