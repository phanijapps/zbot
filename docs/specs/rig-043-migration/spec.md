# Spec: Rig 0.43.0 execution parity

- **Status:** Implementing
- **Owner:** @videogamer
- **Plan:** [`plan.md`](plan.md)
- **Constrained by:** [Rig-only execution](../rig-only-execution/spec.md); existing engine-hook semantics
- **Brief:** docs/product/briefs/rig-hooks-session-graph.md
- **Discovery:** none
- **Contract:** none (internal adapter migration)
- **Shape:** integration
- **Mode:** full (dependency, interface, security, or user-facing structural change)

> **Spec contract:** Objective, Boundaries, Testing Strategy, and Acceptance Criteria define delivery. The implementation matches this contract or changes it in the same reviewed work.

## Objective

All local agent executions use the published Rig 0.43.0 runtime while preserving configured providers, validated intent decisions, tool policy, streaming, MCP, cancellation, terminal answers, and reloadable session state. The upgrade exposes the release’s hook callbacks through zbot’s existing runtime boundary without creating a second executor.

## Boundaries

### Always do

- Use the actual published 0.43.0 crate contract and retain actor-filtered tools and protected policies.
- Preserve the working Ollama/z.ai intent completion, reasoning settings and 5000-token user configuration; verification uses isolated settings.
- Preserve ordered before-tool veto, chained after-tool shaping, live context editing/recall and typed provider errors.

### Ask first

- Change user-visible execution semantics, storage schemas, provider configuration, or token budgets.
- Add a framework, provider replacement, or dependencies outside the Rig release components required for this migration.

### Never do

- Add an alternate execution engine, fallback executor, or duplicate model/tool policy layer.
- Silently treat absent usage counters as zero or repair a disallowed tool into an executable one.

## Testing Strategy

TDD regression fixtures verify provider/error mapping, ordered veto/result transformations, terminal respond arguments, MCP lifecycle, snapshot/recall and cancellation. Goal-based checks verify resolved release crates, workspace compilation and clippy because dependency wiring is observable in Cargo metadata. Isolated end-to-end Chat and delegated Research runs verify stream/non-stream parity, final persistence and reload across gateway/runtime boundaries; bounded opt-in provider checks verify actual Ollama and z.ai intent behavior without changing the live vault.

## Acceptance Criteria

- [ ] **AC1 — Release identity: Cargo metadata resolves the supported Rig facade/components to exact 0.43.0 release versions, with no stale 0.39.0 Git pin in the execution dependency closure.**
- [ ] **AC2 — Sole runtime: root, continuation, delegated Chat/Research and A2A ingress construct the Rig engine through the common builder. Unresolvable configuration fails explicitly.**
- [ ] **AC3 — Provider preservation: model/provider IDs, maximum output, reasoning parameters, structured/fenced JSON validation and bounded correction behavior survive the migration. Invalid/provider errors retain their typed classification.**
- [ ] **AC4 — Tool policy: denied/disallowed calls cause no side effect; valid results preserve raw versus model-visible values, per-call identity, before-tool veto ordering and after-tool replacement chaining.**
- [ ] **AC5 — Context: steering acknowledgements, recall deduplication, compaction snapshots and authoritative preamble changes reach the next model request without duplicating history.**
- [ ] **AC6 — Streaming and settlement: text, reasoning, tool outcomes and terminal respond arguments produce the same durable final answer; a cancelled/error run is not reported completed, including after reload.**
- [ ] **AC7 — MCP: admitted tools are discoverable and callable, forbidden tools remain unavailable, and owned connections close on success, error and cancellation.**
- [ ] **AC8 — Parity evidence: runtime adapter regressions and isolated gateway journeys pass; bounded live intent checks for configured Ollama and z.ai succeed or report a specific external prerequisite without being counted as a pass.**

## Assumptions

- Technical: current pin/version is in runtime/agent-runtime/Cargo.toml and Cargo.lock; 0.43.0 source is commit 654567eb64274fca00cab86cdd32c86b9913769e (published crate metadata).
- Technical: facade/component and hook changes are verified against https://github.com/0xPlaygrounds/rig/blob/654567eb64274fca00cab86cdd32c86b9913769e/crates/rig-agent/src/agent/hook.rs; the concepts page is conceptual guidance.
- Product: scope, simplicity, existing working intent behavior and prioritization are confirmed by the user on 2026-10-03.
- Process: this draft is for review, not code authorization (new-spec and docs/CONVENTIONS.md).
