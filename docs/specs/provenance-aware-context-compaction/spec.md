# Spec: Provenance-aware context compaction

- **Status:** Implementing
- **Owner:** phanijapps
- **Plan:** [`plan.md`](plan.md)
- **Constrained by:** none
- **Brief:** `docs/product/briefs/provenance-aware-context-compaction.md`
- **Discovery:** none
- **Contract:** none
- **Shape:** service

> **Spec contract:** this document defines what "done" means. The implementing
> PR must match this spec, or update it. Verification must be derivable from it.

## Objective

zbot compacts oversized agent context using a provenance-aware middleware by
default. It replaces eligible historical tool-result payloads with a compact
record that identifies the original result and its reference state without
retaining the raw payload in model context. A local runtime feature flag
selects the existing recency-only middleware for controlled comparison and
immediate rollback, without changing durable storage, recall, skills, plans,
or gateway APIs.

## Boundaries

### Always do

- Keep the existing context-editing invariants: threshold-based activation,
  configured recent-result retention, excluded-tool retention, coherent
  tool-call/result history, and skill-resource cascade behavior.
- Make provenance-aware policy selection the default whenever context editing
  is enabled and no explicit control flag selects legacy behavior.
- Make the effective policy and each compaction decision observable through
  existing structured diagnostics without including cleared raw payloads.

### Ask first

- Changing the default policy after this spec has been approved.
- Adding a persisted settings field, public HTTP API, or remotely managed flag
  rather than the local runtime feature flag defined here.
- Expanding provenance into a durable memory, checkpoint, or retrieval schema.

### Never do

- Alter durable message, checkpoint, memory, knowledge-graph, skill, plan, or
  gateway API formats.
- Create a new top-level crate, module boundary, dependency, feature-flag
  service, experiment dashboard, or rollout system.
- Present a cleared result as available when its authoritative source cannot be
  resolved.

## Testing Strategy

- **TDD:** policy parsing/selection, compact-record construction, eligible
  result selection, exclusion handling, skill cascade, and
  identified/unidentified reference-state behavior are deterministic Rust logic
  with compact invariants.
- **Goal-based check:** gateway middleware-pipeline construction selects the
  provenance-aware implementation by default and the legacy implementation
  only when the local control flag explicitly requests it.
- **Goal-based integration check:** an execution/recovery fixture proves both
  selected policies preserve the existing transcript and skill/plan/effect
  scope contracts while the provenance policy removes raw historical payload
  from model-facing context.

## Acceptance Criteria

- [ ] When context editing is enabled and `ZBOT_CONTEXT_EDITING_POLICY` is
  absent, empty, or `provenance-aware`, the execution pipeline installs the
  provenance-aware middleware.
- [ ] When `ZBOT_CONTEXT_EDITING_POLICY=legacy`, the execution pipeline
  installs the existing `ContextEditingMiddleware` behavior without changing
  its selection, placeholder, exclusion, retained-result, or cascade rules.
- [ ] When `ZBOT_CONTEXT_EDITING_POLICY` has any other value, startup falls
  back to the provenance-aware policy and emits a structured warning that names
  the invalid value and the selected fallback without logging conversation or
  tool-result content.
- [ ] The provenance-aware middleware retains the configured newest eligible
  tool results, honors `exclude_tools`, preserves tool-call/result linkage,
  applies the same skill-resource cascade boundary as the legacy middleware,
  and makes no message edit below its configured token threshold.
- [ ] When model-visible context reaches or exceeds the configured token
  threshold and an eligible historical result exists, the provenance-aware
  middleware compacts at least the configured minimum reclaim amount or leaves
  the context unchanged with an explicit diagnostic explaining why no eligible
  result can be cleared.
- [ ] Every provenance-cleared result replaces raw model-visible payload with a
  bounded record containing the producing tool name, tool-call identifier,
  original message identifier when present, and an explicit reference state;
  the record never embeds raw result content.
- [ ] A provenance record has exactly one reference state: `identified` when
  the cleared result has a nonempty original message identifier, or
  `unidentified` when it does not. A reference state does not claim that an
  authoritative source is resolvable. An unidentified record does not
  synthesize, infer, or substitute result content.
- [ ] When a session is compacted and recovered under either selected policy,
  its loaded skills, plan state, effect scope, retained recent tool results,
  and coherent tool-call/result history remain available; under the
  provenance-aware policy, cleared old results remain compact records rather
  than raw payloads.
- [ ] The selected policy, every compaction decision's tool name, tool-call
  identifier, optional original message identifier, reference state, and
  aggregate compaction count are emitted through existing diagnostics; no raw
  cleared payload appears in logs or events.

## Assumptions

- Technical: `ContextEditingMiddleware` already performs thresholded,
  recency-based tool-result clearing with exclusion and skill-cascade support
  (source: Engram context for
  `ContextEditingMiddleware::process` and
  `ContextEditingMiddleware::find_tool_results_to_clear_with_cascade`,
  2026-09-14).
- Technical: the gateway creates the middleware pipeline in
  `gateway/gateway-execution/src/invoke/builder.rs` and configures context
  editing differently for chat and deep modes (source: Engram context for
  `build_runtime_middleware_pipeline`, 2026-09-14).
- Product: the new policy is default and the existing policy is selectable for
  side-by-side testing and rollback (source: user confirmation 2026-09-14).
- Product: scope remains isolated to context-editing middleware (source: user
  confirmation 2026-09-14).
- Process: the work has a one-sprint appetite and begins as a Draft spec and
  Drafting plan (source: user confirmation 2026-09-14).
