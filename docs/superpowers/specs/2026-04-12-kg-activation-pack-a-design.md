# KG Activation Pack A — Design Spec

**Date:** 2026-04-12
**Branch target:** `feature/kg-activation-pack-a` (off `main`)
**Scope:** Minimal, high-ROI fixes that make the knowledge graph visibly dense and actively used. Packs B and C (first-class recall lane, reciprocal graph→facts synthesis) are deferred to separate specs.

---

## Problem Statement

Investigation on 2026-04-12 (see conversation transcript) found the knowledge graph is built but structurally underused:

1. **Sparse graph** — 561 entities, 28 relationships, 544 orphans (97% disconnected) in the current Hindu Mahasabha research ward. Root cause: `WardArtifactIndexer::index_one_file()` explicitly emits `relationships: vec![]` (a follow-up TODO never completed).
2. **Dormant `graph_query` tool** — Registered and wired to root + subagents when `graph_storage` is present, but zero mention in any system-prompt shard. Agents have no behavioral instruction to use it.
3. **Shallow session-start injection** — `recall_with_graph()` pulls 1-hop neighbors only. Multi-hop traversal gated by `config.graph_traversal.enabled`, which is off by default.

Pack A removes these three blockers with low-risk, locally scoped changes. No `recall.rs` scoring refactor. No new fact-synthesis pipeline.

---

## Goals

- **G1** — Ward Artifact Indexer emits relationships from structured JSON, turning orphan entities into a connected graph. Target: orphan ratio ≤ 30% on the Hindu Mahasabha fixture ward after re-index.
- **G2** — Agents receive explicit instruction on when and how to call `graph_query`. Target: ≥1 `graph_query` call per rich research session (≥5 tool calls total) in post-deployment smoke tests.
- **G3** — Multi-hop graph context (depth=2) available by default at session start and delegation. No config flag flip required by users.
- **G4** — Existing DB heals via idempotent re-index; no data loss.

Non-goals (explicitly deferred to Pack B/C):
- Promoting graph context to a scored, budget-governed recall lane alongside facts.
- Synthesizing `memory_facts` rows from graph traversals.
- Verifying or fixing Phase 5 `EntityMention` micro-recall firings (Pack B).
- LLM-assisted relationship extraction for arbitrary JSON schemas (Pack C hybrid).

---

## Fix 1 — Teach agents the `graph_query` tool

### Changes

**File: `gateway/templates/shards/tooling_skills.md`**

Add a new section titled `### graph_query — explore entity relationships` with:

- One-sentence description: "Explore the knowledge graph of entities and relationships accumulated from prior sessions, ward artifacts, and tool results."
- Three action descriptions (`search`, `neighbors`, `context`) with signatures copied from `runtime/agent-tools/src/tools/graph_query.rs`.
- **Trigger list** (when to call):
  1. User mentions a named entity (person, org, location, document, tool) you don't already have context for → `graph_query(action="search", query=name)`
  2. You need to understand how two or more entities relate → `graph_query(action="neighbors", entity_name=X, depth=2)`
  3. Broad domain exploration at task start → `graph_query(action="context", topic=...)`
- **Do NOT call** guidance: don't use `graph_query` for simple fact lookup (use `memory(action="recall")` instead); don't chain more than 2 consecutive graph_query calls — if you're still lost, delegate.

**File: `gateway/templates/shards/memory_learning.md`**

Add 2 examples alongside existing `memory(recall)` / `memory(save_fact)` examples:

```
# User asks: "what do you know about V.D. Savarkar?"
graph_query(action="search", query="Savarkar")
# → returns entity with aliases, mention_count, neighbors snippet

# Before delegating a research task on a named figure
graph_query(action="neighbors", entity_name="Hindu Mahasabha", depth=2)
# → returns 2-hop subgraph; include relevant findings in delegation task
```

### Validation

- Unit test: shard file parses without errors in `template_loader`.
- Smoke test: run a research session against the Hindu Mahasabha ward with a user prompt that mentions an entity; grep session transcript (`messages` table, role=assistant, tool_calls JSON) for `graph_query`. Expect ≥1 call.

---

## Fix 2 — Ward Artifact Indexer relationship extraction

### Approach: schema-driven, resolver-integrated, idempotent

The indexer already classifies JSON files into three shapes: `NamedObjectArray`, `DatedObjectArray`, `NamedObjectMap` (see `gateway/gateway-execution/src/ward_artifact_indexer.rs`). We extend `index_one_file()` to populate `ExtractedKnowledge::relationships` using field-name heuristics.

### Extraction rules (field-name → relationship)

Applied after entity extraction for each parsed object. Every target is resolved via `EntityResolver::resolve()` so existing entities merge rather than duplicate.

| Source entity | Field in JSON | Target entity type | Relationship |
|---|---|---|---|
| any | `location` (string) | `location` | `held_at` (for events) / `located_in` (for orgs, people) |
| any | `organization` (string) | `organization` | `member_of` |
| any | `role` (string) | `role` | `held_role` |
| any | `founder` (string) | `person` | `founder_of` (reversed: person → org) |
| any | `founded_in` / `founded_at` (string) | `location` | `located_in` |
| event | `participants` (array<string>) | `person` | `participant` (reversed: person → event) |
| event | `date` / `year` | `time_period` | `during` |
| any | `author` | `person` | `author_of` (reversed) |
| any | `born_in` | `location` | `born_in` |
| any | `died_in` | `location` | `died_in` |

**Direction**: Use existing `canonicalize_relationship` conventions from `services/knowledge-graph/src/service.rs` (active voice, source performs action on target). For inverted fields (e.g., `founder` appears on org but maps to `person --founder_of--> org`), explicitly swap source/target before emitting.

**Type inference for unresolvable targets**: If the resolver creates a new target entity because no match exists, infer type from the field name (`location` → Location, `organization` → Organization, etc.). Default to `Concept` only when no rule applies.

**Unknown fields**: Ignored. No LLM fallback in Pack A.

### Idempotency

- Entity writes: `EntityResolver` cascade handles dedup; aliases accumulate.
- Relationship writes: `UNIQUE(source_entity_id, target_entity_id, relationship_type)` already enforced at the storage layer. On conflict, bump `mention_count`, update `last_seen_at`, append source_episode_ids.
- Re-running the indexer over the same file is safe and boosts mention counts; no duplicate rows.

### Re-index path for existing DB (DB remediation)

Add a `force_reindex: bool` flag to `WardArtifactIndexer::run()`:

- When `false` (default): existing behavior — skip files whose `content_hash` matches an existing `kg_episodes` row.
- When `true`: process all files regardless, but still upsert episodes (no duplicate episode rows thanks to `UNIQUE(content_hash, source_type)`).

One-time trigger: add an admin HTTP endpoint `POST /api/admin/reindex-graph` that iterates all wards and calls `run(force_reindex=true)`. User invokes once after deployment; idempotent, safe to re-run.

No startup auto-backfill (avoids surprising boot-time cost). No destructive DB wipe.

### Files touched

- `gateway/gateway-execution/src/ward_artifact_indexer.rs` — extraction rules + `force_reindex` flag. Extract rule engine into a private helper module `relationship_rules` if file exceeds 500 lines after changes.
- `gateway/src/http/admin.rs` (new, or existing admin module) — `POST /api/admin/reindex-graph` handler.
- `gateway/gateway-execution/src/ward_artifact_indexer.rs` tests — fixture-based unit tests for each rule.

### Validation

- Unit tests, one per rule above, asserting the emitted `(source_type, target_type, relationship)` triple.
- Idempotency test: run indexer twice on the same fixture, assert relationship count unchanged and `mention_count` incremented.
- Integration test: full run against a synthetic 3-file ward (timeline.json, people.json, organizations.json); assert orphan ratio < 30%.
- Post-deployment manual check: run `/api/admin/reindex-graph` on the Hindu Mahasabha ward; query `SELECT COUNT(*) FROM kg_relationships` — expect growth from 28 to ≥ 200.

---

## Fix 6 — Multi-hop graph traversal on by default

### Changes

**File: `gateway/gateway-services/src/recall_config.rs`**

Update `RecallConfig::default()`:

```rust
graph_traversal: GraphTraversalConfig {
    enabled: true,        // was false
    depth: 2,             // was 1
    max_neighbors_per_hop: 5,  // unchanged
    ..Default::default()
}
```

Keep `max_recall_tokens` unchanged — existing token budget already caps graph context at 2000 chars, which accommodates 2-hop without blowup.

### Migration for existing users

`RecallConfig` is loaded via `recall_config.json` with fallback to defaults. Users who have a persisted config file keep their current values; new installs get the new defaults. Add a one-line migration helper: if a loaded config has `graph_traversal.enabled = false` AND no user-set override marker, flip to true. Simplest implementation: on config load, if the field is literally absent from JSON, use new default.

### Validation

- Unit test: `RecallConfig::default()` returns `enabled=true, depth=2`.
- Integration test: session-start recall on a ward with a 2-hop path returns entities at depth 2 in the graph section.

---

## Testing Strategy

| Layer | Tests |
|---|---|
| Unit | Per-rule extraction tests (Fix 2), config default test (Fix 6), shard parse test (Fix 1) |
| Integration | Idempotent re-index test (Fix 2), 2-hop traversal test (Fix 6), synthetic-ward orphan-ratio test (Fix 2) |
| Smoke (manual, post-deploy) | Run research session on Hindu Mahasabha ward, verify `graph_query` is called and 2-hop context appears in session-start system message |

All tests use `#[tokio::test]` where async is needed. No new test dependencies.

## Code quality requirements (from project rules)

- Cognitive complexity ≤ 15 per function (SonarQube rust:S3776)
- `cargo fmt --all` clean, `cargo clippy --all-targets -- -D warnings` clean
- No `unwrap()` in production code paths; acceptable in tests
- Relationship rule dispatch: if the match arm grows past 10 arms, extract each rule into a named helper

---

## Out of Scope

- Graph-as-scored-recall-lane (Pack B).
- Reciprocal graph→facts synthesis (Pack C).
- LLM-assisted relationship extraction for non-conforming JSON (Pack C hybrid option).
- Observatory UI changes.
- Phase 5 `EntityMention` micro-recall audit (Pack B).
- Entity/relationship pruning, decay, or TTL.

---

## Risks & Mitigations

| Risk | Mitigation |
|---|---|
| Schema-driven rules miss unusual ward artifacts | Document the rules in `memory-bank/components/memory-layer/knowledge-graph.md`; users can file an issue with example JSON. Pack C hybrid is the escalation. |
| Bad relationship direction leaks past `canonicalize_relationship` | Reuse existing canonicalizer; add direction assertions in unit tests. |
| Multi-hop traversal explodes token budget on dense graphs | Existing 2000-char cap on graph section still enforced; depth=2 × neighbors=5 worst case is 25 nodes, well within budget. |
| `force_reindex=true` floods DB with writes on large deployments | Writes are batched per ward; resolver dedups; relationship upsert is conflict-friendly. Document as "run during low-traffic window". |
| Agents overuse `graph_query`, racking up latency | Shard includes explicit "don't chain more than 2 consecutive graph_query calls" guidance. |

---

## Success Criteria

Pack A is done when:

1. Hindu Mahasabha ward, post-reindex: orphan ratio ≤ 30%, relationship count ≥ 200.
2. New research session on that ward: session-start system message contains 2-hop graph context; transcript contains ≥1 `graph_query` tool call.
3. All new tests pass; `cargo clippy --all-targets -- -D warnings` clean.
4. `memory-bank/components/memory-layer/knowledge-graph.md` updated with the new extraction rules and `force_reindex` admin endpoint.
