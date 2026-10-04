# Plan: Complete and styled graph exploration

- **Spec:** [`spec.md`](spec.md)
- **Status:** Approved

## Approach

First make the scoped store/API traversal truthful, then connect bounded progressive fetching and global search to the existing graph page. Benchmark D3 against cosmos.gl using identical generated scenes; choose a renderer only after measurements, and preserve selection/detail behavior with a narrow renderer boundary if required.

## Constraints

- The approved [brief](../../product/briefs/rig-hooks-session-graph.md) and the spec's Boundaries govern scope.
- Preserve existing uncommitted work. Spec authoring does not approve implementation or reset another workflow.
- Current branch is two commits behind `origin/main`; obtain a fresh isolated implementation base before code work. Do not stage/rebase the shared dirty checkout.

## Construction tests

HTTP schema validation and UI fixtures use generated graph data. Browser evidence reports dataset, browser, CPU/GPU/RAM and cold/warm conditions; timings measure network/data merge/render readiness separately. A fast visual rendering of incomplete pages fails the integration journey.

## Design (LLD)

### Interfaces & contracts

Graph-exploration OpenAPI defines paged existing entity/relationship endpoints and scoped search. `total` is an exact scope count, not page length. Rows carry agent ownership; response pagination is deterministic. Each page/count is one consistent read; stable offset traversal across requests remains a live view. Changed totals or no-progress pages offer refresh; no snapshot/revision service is introduced. Extend the existing store trait and Engram sidecar queries, not a separate store (AC1–4).

### State & control flow

Scope/filter key each fetch. Merge entities/edges by qualified ID, reconcile endpoints, and advance only after a successful page. Cancel stale fetches on scope changes. Search is server-backed across scope and loads selected neighbors. Rendering/label density is distinct from loaded coverage (AC2–4).

### Dependencies & integration

`GraphCanvas.tsx` starts as the D3 baseline. Compare cosmos.gl at a pinned npm version during the spike. A measured renderer switch uses the current React host/detail state, with disposal of GPU resources and an accessible list fallback; no second Observatory app (AC5–8).

## Tasks

### T1: Define pagination/count at the store boundary

**Depends on:** none

**Verification mode:** TDD

**Spec mapping:** AC1–2, AC8

**Tests:**
- Generated 1200/900 fixture enumerates per-agent and cross-agent pages; assert scoped totals, stable order, changed-total/no-progress refresh, endpoint ownership and negative limits.

**Approach:**
Update services/knowledge-graph/src/kg_trait/store.rs and stores/zbot-engram-adapter/src/stores/knowledge_graph.rs/sidecar. Identify other actual trait implementations and preserve conformance. Use bounded queries/counts and live-view semantics defined by the contract.

### T2: Expose the existing scoped HTTP graph surface accurately

**Depends on:** T1

**Verification mode:** TDD and contract-based HTTP integration

**Spec mapping:** AC1, AC3–4, AC8

**Tests:**
- HTTP pages/search/entity/neighbors validate contract fields and actual totals; invalid/filter/changed-total and oversized/deep-property cases. Mismatched Origin, LAN bind and missing bind proof return 403 before any spy-store read, including Origin-less callers.

**Approach:**
Update gateway/src/http/graph.rs, existing routes and gateway-served OpenAPI. Add the contract’s aggregate search and direct per-agent entity read for endpoint resolution; the existing route table lacks both. Paginate existing neighbor reads for truthful selected neighborhoods. Reuse SameOrigin and sessions::LoopbackBind, not a new guard. Existing responses gain additive pagination/projection fields. Bound names/properties/rows and dynamically shorten pages to the 2 MiB budget without skipping rows.

### T3: Load and search beyond the first page

**Depends on:** T2

**Verification mode:** TDD and browser integration

**Spec mapping:** AC2–4, AC8

**Tests:**
- UI delayed-page/scope change, failed-page resume, endpoint loading and late-page search-to-detail journeys; oversized projection labels and scene-admission cap retain truthful partial counts and server-backed inspection.

**Approach:**
Update observatory/graph-hooks.ts, ObservatoryPage.tsx and transport contracts. Do not turn an initial fetch limit into a completeness claim; surface progress and partial status.

### T4: Measure renderer and finish graph UX

**Depends on:** T3

**Verification mode:** Goal-based benchmark and visual/manual QA

**Spec mapping:** AC4–8

**Tests:**
- Benchmark D3 and cosmos.gl against the datasets, latency, heap and lifecycle thresholds in spec AC5. Retain measurements and accessible/GPU-error journeys.

**Approach:**
Record benchmark.md, pin a renderer only if the current implementation fails the criterion, and apply shared tokens/legend/focus labels/panel styling. Implement accessible scoped search/list/inspection and reduced motion, dispose simulation/GPU resources.

## Gates

- Run scoped Rust/UI checks named by the tasks, then the appropriate workspace build checks before implementation review.
- Run spec metadata and brief coverage lint, validate declared contracts, and check document links.
- Review spec and plan adversarially; apply findings and rerun affected gates. Security-boundary work also receives secure-design review.
- Keep per-task acceptance evidence in `verification.md` during implementation. No AC is checked merely because its plan exists.

## Risks

Cross-agent storage currently lacks correct aggregate offset/count semantics; avoid claiming snapshot consistency from simple offset paging under mutation. Thousands of SVG elements may exceed performance budgets; use measurement to select a renderer, not automatic package replacement.

## Rollout

Keep the current renderer until paging/search correctness passes. Publish measured limits and explicit fallback behavior. No vault data migration is included; additive HTTP fields and existing routes preserve callers.

## Changelog

- 2026-10-03: Draft delivery contract and construction strategy from the approved brief; no runtime implementation.
