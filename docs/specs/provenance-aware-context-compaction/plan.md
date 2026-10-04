# Plan: Provenance-aware context compaction

- **Spec:** [`spec.md`](spec.md)
- **Status:** Executing

> **Plan contract:** this is the implementation strategy. Unlike the spec, this
> document is allowed to change as learning occurs while its Status is
> `Drafting` or `Executing`.

## Approach

Keep the existing `ContextEditingMiddleware` unchanged as the control policy.
Introduce a sibling provenance-aware middleware in the existing
`runtime/agent-runtime/src/middleware/` boundary, reusing the established
selection/cascade helpers wherever possible. Add a small policy enum and a
local runtime-flag parser at the existing gateway builder seam, implementing
the canonical selection contract in AC1–AC3. Test shared invariants once and
policy-specific output and selection separately.

## Constraints

- The brief confines the change to context-editing middleware and its immediate
  builder wiring.
- The existing `ContextEditingMiddleware` remains behaviorally unchanged for
  `legacy` control runs.
- No persistent schema, gateway API, external service, dependency, or new
  top-level module is introduced.

## Construction tests

**Integration tests:** a real execution/recovery fixture runs once per policy
and asserts skills, plan state, effect scope, and coherent tool history.

**Manual verification:** none; the runtime flag is exercised through the
documented environment-variable construction tests.

## Design (LLD)

### Design decisions

- `ContextEditingPolicy` has two closed variants: `ProvenanceAware` and
  `Legacy`. Its parser implements the canonical runtime-flag values, default,
  and malformed-value behavior in AC1–AC3. Traces to: AC1–AC3.
- `ProvenanceAwareContextEditingMiddleware` reuses the existing eligibility,
  exclusion, retained-result, and cascade rules. It changes only the
  replacement representation for selected results. Traces to: AC4–AC7.
- A provenance placeholder is bounded and content-free: tool name, call ID,
  optional message ID, and the `identified`/`unidentified` state defined by
  AC7. It is not a second memory store or a retrieval protocol. Traces to:
  AC6–AC7.

### Component / module decomposition

- `runtime/agent-runtime/src/middleware/context_editing.rs`: retain legacy
  behavior and extract shared candidate-selection helpers only when this avoids
  semantic duplication.
- `runtime/agent-runtime/src/middleware/provenance_context_editing.rs`: hold
  the new middleware and compact-record rendering/reference-state logic.
- `runtime/agent-runtime/src/middleware/config.rs`: hold the closed policy
  enum/parser if configuration types already live there.
- `gateway/gateway-execution/src/invoke/builder.rs`: resolve the local flag and
  install exactly one policy in the existing pipeline.
- Existing middleware and gateway-execution test modules: cover selection and
  behavior without a new test harness layer.

### State & control flow

```text
context window known
      │
      ▼
resolve runtime policy according to AC1–AC3
      │
      ▼
selected middleware identifies eligible historical tool results
      │
      ▼
legacy: existing placeholder       provenance-aware: bounded provenance record
```

### Failure, edge cases & resilience

- A malformed runtime-flag value never disables context editing or silently
  selects legacy; it follows AC3.
- Missing message identity becomes an explicit absent field in the compact
  record, not an invented identity.
- The reference state never claims that a source can be reloaded; raw payload
  is never fabricated or reconstructed.
- The flag is read once while the pipeline is built, so one execution has one
  deterministic policy.

### Quality attributes (NFRs)

- Compact records have a bounded, fixed field set and contain no result
  payload, keeping compaction token savings predictable. Traces to: AC6.
- Logs/diagnostics contain policy/count/identifier metadata only, never raw
  cleared content. Traces to: AC3, AC9.

## Tasks

### T1: Policy selection is deterministic and default-safe

**Depends on:** none

**Touches:** `runtime/agent-runtime/src/middleware/config.rs`, `gateway/gateway-execution/src/invoke/builder.rs`, `gateway/gateway-execution/src/invoke/builder_tests.rs`

**Tests:**

- TDD: verify each configuration case in AC1–AC3 selects the required policy
  and invalid input emits the required warning.
- stub: draft (uncompiled) — spec authoring does not yet establish the concrete
  configuration-module exports or builder test harness; materialize the red
  table-driven parser and pipeline-construction stubs during work-loop PLAN.
- Goal-based: the pipeline contains exactly the selected context-editing
  middleware and retains existing plan/summarization ordering (AC1–AC3).

**Approach:**

- Add the closed policy type and parser at the existing configuration boundary.
- Resolve it once in the existing pipeline builder and install the appropriate
  middleware without changing threshold/retention policy.
- Emit the effective-policy diagnostic through the existing tracing surface.

**Done when:** parser and pipeline construction tests prove default, legacy,
and invalid-flag behavior.

### T2: Provenance-aware clearing preserves context-editing invariants

**Depends on:** T1

**Touches:** `runtime/agent-runtime/src/middleware/context_editing.rs`, `runtime/agent-runtime/src/middleware/provenance_context_editing.rs`, `runtime/agent-runtime/src/middleware/mod.rs`

**Tests:**

- TDD: selected old results become content-free provenance records while the
  configured newest results remain untouched (AC4, AC6).
- TDD: below-threshold context remains byte-for-byte unchanged; above-threshold
  context compacts eligible results or emits the explicit no-eligible-result
  diagnostic required by AC5.
- TDD: excluded tools, linked tool calls, and skill-resource cascades match
  legacy selection behavior (AC4).
- TDD: missing source metadata produces explicit `unidentified` reference state
  rather than copied or invented content (AC7).
- stub: draft (uncompiled) — the compact-record type and sibling middleware do
  not exist until T1 selects their concrete module shape; materialize red
  selection, cascade, and content-free rendering stubs during work-loop PLAN.

**Approach:**

- Reuse/extract only the legacy eligibility and cascade selection logic needed
  to prevent policy drift.
- Implement bounded provenance rendering and explicit identified/unidentified
  reference-state handling in the sibling middleware.
- Preserve canonical message ordering and tool protocol linkage.

**Done when:** focused middleware tests prove all provenance outputs are
content-free and legacy invariants remain intact.

### T3: Both policies survive execution and recovery

**Depends on:** T1, T2

**Touches:** `gateway/gateway-execution/tests/rig_tool_contracts/context.rs`, `gateway/gateway-execution/src/invoke/builder_tests.rs`

**Tests:**

- Goal-based integration: run the existing compaction/recovery fixture under
  `ProvenanceAware` and `Legacy`; each preserves skills, plan state, effect
  scope, retained results, and coherent message history (AC8).
- Goal-based integration: inspect diagnostics from each path to prove selected
  policy, one content-free record per compaction decision, and aggregate count
  are observable without cleared raw content (AC9).

**Approach:**

- Parameterize the existing fixture rather than creating a second execution
  harness.
- Add assertions for default selection, explicit legacy selection, compact
  record shape, and content-free diagnostics.

**Done when:** both policy runs pass the recovery contract and diagnostics
assertions.

## Rollout

- **Delivery:** provenance-aware policy is the default. Use the explicit local
  control defined in AC2 for a controlled comparison or rollback.
- **Infrastructure:** none.
- **External-system integration:** none.
- **Deployment sequencing:** land policy parser/wiring before the new
  middleware; retain the legacy path for the full evaluation period.

## Risks

- Shared-helper extraction can alter legacy behavior. Paired legacy regression
  tests must run before accepting the refactor.
- Environment variables can leak across parallel tests. Tests must isolate or
  serialize environment mutation and restore it after each case.
- A provenance record can consume nearly as many tokens as a small result if it
  grows freely. Keep the AC6–AC7 fixed field set and test its content contract.

## Changelog

- 2026-09-14: initial plan; provenance-aware middleware is default and legacy
  behavior is retained as a local runtime feature-flag control.
