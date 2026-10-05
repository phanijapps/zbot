# Spec: Complete and styled graph exploration

- **Status:** Implementing
- **Owner:** @videogamer
- **Plan:** [`plan.md`](plan.md)
- **Constrained by:** Existing KnowledgeGraphStore/Engram adapter and Observatory contracts (current implementation); [RFC-0021](../../rfc/0021-conversation-first-desktop-agent.md) desktop local-caller posture
- **Brief:** docs/product/briefs/rig-hooks-session-graph.md
- **Discovery:** none
- **Contract:** [graph-exploration.yaml](../../../contracts/openapi/graph-exploration.yaml)
- **Shape:** mixed
- **Mode:** full (dependency, interface, security, or user-facing structural change)

> **Spec contract:** Objective, Boundaries, Testing Strategy, and Acceptance Criteria define delivery. The implementation matches this contract or changes it in the same reviewed work.

## Objective

The Observatory lets users explore and search the entire selected agent/all-agent graph beyond 200 entities and 500 relationships. It shows truthful loaded/available counts, stable linked entities and a readable selected neighborhood, with coherent zbot styling and responsive bounded loading. The complete data-loading contract and measured renderer choice both serve this outcome.

## Boundaries

### Always do

- Use the actual KnowledgeGraphStore/Engram adapter boundary and current Observatory components; preserve existing entity detail/search/navigation.
- Show scope, loading progress and partial/filtered state explicitly; stop stale work on scope change.
- Use approved shared tokens and benchmark the current renderer against cosmos.gl before choosing the smallest adequate rendering change.
- Treat the Observatory graph as a loopback-only surface per RFC-0021's desktop posture: on a LAN-bound gateway the UI shows a truthful loopback-only denial state (not a generic failure or retry) for graph reads.

### Ask first

- Replace graph persistence, introduce a remote graph service, change graph semantics or add dependencies beyond a measured renderer choice.
- Increase benchmark memory/data budgets or claim support above the verified scale.

### Never do

- Fix completeness by changing only a page-size constant or renderer.
- Download unbounded data, invent endpoints, hide partial views, or create a second knowledge store.

## Testing Strategy

TDD store/HTTP contract tests prove pagination, scoped exact totals, complete traversal without duplicates and search beyond the first page. UI tests cover scope changes, failure/retry and endpoint reconciliation; end-to-end exploration tests load and inspect a later-page entity/edge. Goal-based browser benchmarks compare identical synthetic data and report hardware/browser. Visual/manual review covers selected neighborhood, dense overview, legend, panels, keyboard alternative and reduced motion against shared zbot tokens.

## Acceptance Criteria

- [ ] **AC1 — Pagination: entity and relationship pages return exact available totals for the same scope/filter, deterministic pagination (stable order with a unique tiebreaker: `mention_count DESC, agent_id, id`), a next offset or explicit exhausted state, and bounded page size. Invalid parameters fail; unchanged datasets enumerate all rows exactly once. Every route under `/api/graph/*` — the paged exploration reads and also per-agent stats, aggregate stats, subgraph (bounded `max_hops` 1–4), and the reindex/ingest mutations — uses SameOrigin plus the hardened proven LoopbackBind (including its Host-locality check) before any store access; denied, missing-proof, and native LAN callers receive sanitized 403 with zero store reads, and denials are logged (method, path, peer; rate-limited, no payload). Each page/count is read consistently; cross-request traversal is explicitly a live view. A no-progress page — one whose returned rows are all already-merged qualified IDs — or changed totals invalidates completeness and offers refresh; unchanged totals never prove a frozen snapshot.**
- [ ] **AC2 — Completeness: both per-agent and all-agent views traverse beyond 200 entities and 500 relationships. Every rendered edge has loaded source/target identities; unresolved endpoints are queued/resolved or counted explicitly rather than silently discarded. Agent-local IDs are qualified to avoid cross-agent collision.**
- [ ] **AC3 — Truthful state: loaded and available entity/relationship counts distinguish loading, complete, filtered, partial/error, stale scope, and loopback-only denial. UI can resume a failed page without repeating rows; changed totals and all-duplicate (no-progress) pages surface an incomplete state with a refresh offer, and resume adds no duplicate edges. Projected entity names are ≤1024 characters, properties ≤8 KiB UTF-8/depth four/64 entries per container, rows ≤16 KiB and pages ≤2 MiB. Truncation/omission is flagged per row without losing entity/edge counts; UI memory admission caps are 50000 entities, 100000 edges and 64 MiB serialized merged data, with a visible partial state and server-backed search/inspection after a cap. Scope change aborts requests and simulation work and cannot display the old scope as current. Graph strings (entity names, property values) render as inert text everywhere — graph labels, detail panel, and the accessible list alternative — proven by a markup-bearing fixture (e.g. a name containing `<img src=x onerror=alert(1)>`) that renders literally.**
- [ ] **AC4 — Global search and inspection: search queries the selected persisted scope, finds an entity beyond the initial page, and focuses/loads its relevant neighborhood and detail. Zoom, pan, fit, selection, filtering and refresh remain operable.**
- [ ] **AC5 — Scale evidence: on a recorded desktop Chromium host, a synthetic 17000-entity/5000-edge scene completes loading and first usable view within 5 seconds locally, and p95 input-to-visible feedback is ≤150 ms across 30 pan/zoom/select interactions after warmup. Record peak JS heap ≤256 MiB and no monotonic growth over ten mount/unmount cycles. A 50000-entity stress fixture reports supported limits; it is not silently described as fully rendered if sampled.** (stress fixture deferred: graph-stress-and-latepage-evidence)
- [ ] **AC6 — Styling: restrained token-based background, type legend, focused readable labels and emphasized selected relationships keep the neighborhood distinguishable at desktop/narrow widths. Detail/search controls remain readable at 1280×800 and 390px; dense overview uses selective labels/aggregation with explicit coverage. Screenshot review measures 4.5:1 text and 3:1 necessary controls/focus.**
- [ ] **AC7 — Accessibility and fallback: keyboard users can search, select an entity and inspect relationships through an accessible list/detail alternative; reduced motion pauses simulation/animated transitions. GPU-unavailable, renderer-error, and loopback-only-denial (403) states retain scoped search/inspection or disclose the boundary truthfully without claiming complete visual coverage.**
- [ ] **AC8 — Evidence: tests include ≥1200 entities and ≥900 relationships for pagination (including a relationship whose endpoint entity is absent — queued/counted, never silently dropped — and an empty-results scope) and a searchable late-page target, per-agent/all-agent/filter cases, missing endpoint, mutation-during-traversal (changed totals), failed page and cancellation. A spy-store test proves no `/api/graph/*` route reaches the store unguarded. API construction checks and renderer benchmark/visual evidence are retained.**

## Assumptions

- Technical: graph-hooks.ts requests 200/500 once; cross-agent graph HTTP total is page length and no offset reaches the adapter (verified source reads).
- Technical: live aggregate counts were 16693 entities/4728 relationships; benchmark uses generated data at comparable scale, never user content.
- Product: graph completeness and UX styling with cosmos.gl as reference are user-confirmed on 2026-10-03; renderer selection remains measurement-driven.
- Design/process: experience-design pack is absent; apps/ui/ARCHITECTURE.md and current approved desktop palette ground styling. Numeric benchmark thresholds are explicit proposed engineering criteria for this Draft, not a claim of observed performance.

## Data exposure and projection

The local graph surface reuses gateway::http::SameOrigin and the hardened sessions::LoopbackBind (including its Host-locality/rebinding check) before every `/api/graph/*` store access — the paged reads modeled below and the sibling stats, subgraph (bounded `max_hops` 1–4), reindex and ingest routes — including Origin-less native callers. Bind proof missing or non-loopback and mismatched Origin fail closed. Agent IDs select a local graph scope; they are not an authentication credential. This work adds no remote graph authorization scheme. User-visible denial uses a fixed message and cannot expose backend errors; denials are logged with method, path and peer address.

Projected names clip at a Unicode boundary; over-budget/deep properties are omitted as {}. projection_truncated=true identifies either loss. Stored data is untouched, and search matches the full persisted name before projection. Property strings cap at 4096 characters; containers cap at 64 entries and four nesting levels. Pages may contain fewer rows than limit to meet the byte budget; next_offset advances by rows actually returned. No row is dropped from pagination because its properties were omitted. Unknown field types/nonfinite numbers receive the same flagged omission. Graph strings are escaped display data. Scope/counts remain exact even when display fields are projected.

RFC-0011 is Draft context, not a governing decision for this delivery. Current store ownership is preserved without depending on acceptance of that RFC.
