# observatory-graph-completeness execution evidence

User authorization: 2026-10-03, “push the current branch changes to remote raise a PR and then pull changes from develop branch. Start a new branch and starte executing all specs”. This authorizes the four reviewed scopes and their existing plans. Both spec-stage reviews report Clean in ../rig-hooks-session-graph-review.md.

Implementation checkout: /home/videogamer/projects/agentzero-rig-hooks-session-graph
Branch: feat/rig-hooks-session-graph
Base target: origin/develop (user-selected); pull --ff-only reported already up to date.
Carried-forward published work: b867a18a, PR #274.

No acceptance criterion is marked complete by approval alone. Runtime/provider settings and live vault remain outside fixture execution.

## Execution evidence (run b53af4e8, feat/desktop-session-completion branch)

### T1 — store boundary pagination (SHIPPED, wave 1)
- Four paged trait methods (`list_entities_paged`, `list_relationships_paged`, `list_all_entities_paged`, `list_all_relationships_paged`) + `search_entities_paged`, `search_all_entities_paged`, `get_neighbors_full_paged`; exact same-scope totals from one consistent read (windowed `COUNT(*) OVER ()` for entities; single sidecar-lock dedup pipeline for relationships).
- Deterministic order `mention_count DESC, agent_id, id` (unique tiebreaker; most-mentioned-first page preserved).
- Construction tests `stores/zbot-engram-adapter/tests/graph_pagination.rs`: 1200/900 fixture, exact-once enumeration, filter/total agreement, live-view mutation, empty scope, bounded offsets, missing-endpoint edges rejected at write time (stored data cannot dangle — unresolved endpoints are a pagination-ordering phenomenon for T3).
- Gates: adapter suite green (18 binaries), clippy clean.

### T2 — HTTP surface (SHIPPED, wave 2)
- Every `/api/graph/*` route guarded by SameOrigin + hardened LoopbackBind (incl. per-agent/aggregate stats, subgraph with `max_hops` 1–4, reindex, ingest, ingest-progress); LAN bind denies fail-closed with throttled method/path/peer denial logging (sessions.rs `log_guard_denial`).
- Exact totals + `offset` echo + `next_offset` exhaustion on lists/search/neighbors; new `GET /api/graph/all/search` and `GET /api/graph/:agent_id/entities/:entity_id`; `ward_id` kept and documented; parameter validation (limit 1–1000, offset ≤1e6, q ≤256).
- Projection: names clip at a Unicode boundary (≤1024 chars), over-budget properties/rows omitted with per-row `projection_truncated`, pages shorten to the 2 MiB budget without skipping rows.
- Tests `gateway/src/http/graph_pagination_tests.rs` (6, all green): the three pre-EXECUTE red stubs (exact totals, offset-advanced exhaustion, LAN 403-before-read) + full-route guard sweep + invalid-parameter matrix + projection truncation without count loss.
- Gates: gateway suite green except the documented pre-existing saved_surfaces baseline; consumer crates green; UI suite green except the intentional T3 red stub; clippy clean.

### T3/T4 — pending (wave 3/4)
Resume at CODE-IMPLEMENTATION wave 3: progressive UI loading + search (the red stub in graph-hooks.test.ts is the entry point), then the renderer benchmark + styling/a11y. Red stub file references and the plan's Tests lists carry the full obligations.

### T3 — UI progressive loading + search (SHIPPED, wave 3)
- useGraphData traverses by next_offset to exhaustion, merges by qualified (agent_id,id) identity (no duplicates), cancels on scope change, and exposes truthful state: live totals, stale (changed totals / no-progress page) with refresh offer, admission caps (50k/100k/64MiB) with a partial notice, loopback-only denial, unresolved endpoints counted (never silently dropped).
- useGraphSearch hits the server search endpoints (per-agent + aggregate) so results reach beyond the loaded pages; selecting a hit opens the detail panel.
- ObservatoryPage renders loaded/available counts and every partial state; graph strings render inert (markup fixture test). 18 hook tests + 13 page tests; full UI suite 1474 passed; build clean.

### T4 — renderer benchmark (MEASURED; swap pending owner ratification)
- Dev-only benchmark route (never in the production bundle) with a seeded 17k/5k synthetic scene; Playwright harness persists benchmark.md (host/CPU/browser recorded).
- Verdict: D3 fails AC5 input latency (p95 1687ms vs 150ms); cosmos.gl 3.4.2 passes everything (103ms first view, 17ms p95, 125MiB, no growth). Production adoption of cosmos.gl behind a narrow boundary (selection/detail/labels/zoom/fit preserved, GPU-unavailable fallback per AC7) awaits the owner's ratification of the dependency-tier change; see benchmark.md.
