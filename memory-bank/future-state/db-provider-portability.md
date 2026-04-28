# DB Provider Portability — Findings & Migration Guide

**Status:** Living document. Updated as each TD-023 retirement increment lands.
**Last updated:** 2026-04-28

## Goal

Make every persistence call route through the `KnowledgeGraphStore` /
`MemoryFactStore` traits so swapping the backend is a config change, not
a code change. The existing `feature/surrealdb-backend` work added a
SurrealDB 3.0 implementation alongside SQLite, but a chunk of legacy
concrete-typed callers still bypass the trait surface — those need to
be migrated for the swap to be truly seamless.

This doc captures (1) what's already portable, (2) what isn't and why,
and (3) the precise playbook for adding a third backend (e.g., Postgres,
DuckDB, a remote service).

---

## What's portable today

These call paths route through `Arc<dyn KnowledgeGraphStore>` /
`Arc<dyn MemoryFactStore>` and work on either backend:

| Surface | Trait method | Status |
|---|---|---|
| `GET /api/memory` | `MemoryFactStore::list_memory_facts` (None agent) + `count_all_facts(None)` | ✅ migrated |
| `GET /api/memory/:agent_id` | `list_memory_facts(Some(agent_id), …)` + `count_all_facts(Some(...))` | ✅ migrated |
| `GET /api/memory/:agent_id/facts/:fact_id` | `get_memory_fact_by_id` | ✅ migrated |
| `DELETE /api/memory/:agent_id/facts/:fact_id` | `delete_memory_fact` | ✅ migrated |
| All `/api/graph/*` read endpoints | `KnowledgeGraphStore::*` | ✅ migrated (TD-023 phase 6) |
| All `/api/embeddings/*` health endpoints | `KnowledgeGraphStore::vec_index_health` | ✅ migrated |
| Agent `memory.search` tool — hybrid recall | `MemoryFactStore::recall_facts` (Value-typed) | ✅ migrated |
| Agent `memory.save` tool — basic save | `MemoryFactStore::save_fact` | ✅ migrated |
| Sleep-time orphan archiver | `KnowledgeGraphStore::list_archivable_orphans` + `mark_entity_archival` | ✅ migrated |
| Embedding reindex | `KnowledgeGraphStore::reindex_embeddings` | ✅ migrated |

These all pass the conformance suite at `stores/zero-stores-conformance/`
on both SQLite and SurrealDB.

---

## What isn't portable (the TD-023 retirement scope)

These call sites still talk to the concrete `MemoryRepository` /
`GraphService` / `KnowledgeDatabase` types directly. When the user opts
into SurrealDB via the UI dropdown, these paths still hit SQLite — which
is the source of "I picked SurrealDB but X still shows old data."

### Phase B: chat-time read/write paths

| File | Lines | Concrete dependency | What it does | Needed trait method |
|---|---|---|---|---|
| `gateway-execution/src/distillation.rs` | 502, 520, 1253, 1387 | `memory_repo.upsert_memory_fact(MemoryFact)` | Distillation writes typed facts after a turn | `upsert_typed_fact(Value)` accepting full MemoryFact shape |
| `gateway-execution/src/distillation.rs` | 502 | `memory_repo.supersede_fact(old_id, new_id)` | Mark old fact as superseded by new | `supersede_fact(old_id, new_id)` |
| `gateway-execution/src/distillation.rs` | 1403, 1412 | `memory_repo.get_cached_embedding` / `cache_embedding` | Embedding cache by content hash | Out of scope — local cache, not data |
| `gateway-execution/src/recall/mod.rs` | 156 | `memory_repo.search_memory_facts_hybrid(query, agent, limit, ward, embed)` | Hybrid FTS + vector recall with ranking | `search_memory_facts_hybrid(query, agent, limit, ward, Option<&[f32]>)` |
| `gateway-execution/src/recall/adapters.rs` | (top-level imports) | typed `MemoryFact` struct + `ScoredFact` | Adapter shapes for recall ranking | Trait surface returns ranked Vec<Value>; adapter converts |

### Phase C: executor housekeeping

| File | Concrete dependency | What it does | Needed trait method |
|---|---|---|---|
| `gateway-execution/src/sleep/worker.rs` | `memory_repo` field on `SleepWorker` | Sleep-time consolidation orchestrator | Replace field type with `Arc<dyn MemoryFactStore>` |
| `gateway-execution/src/sleep/synthesizer.rs` | 256, 328 | `get_fact_embedding`, `upsert_memory_fact` | Synthesize new facts from clusters | Same as distillation |
| `gateway-execution/src/pruning.rs` | 80 | `memory_repo.archive_fact(fact_id)` | Mark stale facts archived | `archive_fact(fact_id)` |
| `gateway-execution/src/session_ctx/snapshot.rs` | 41, 171 | `Option<&Arc<MemoryRepository>>` | Read facts for session snapshot | Trait param |
| `gateway-execution/src/runner/{core,continuation_watcher,delegation_dispatcher,execution_stream,invoke_bootstrap}.rs` | `memory_repo` field threaded through 8+ structs | Carried for downstream subsystems | Replace field type with trait Arc |
| `gateway-execution/src/delegation/spawn.rs` | 62, 292, 393, 402 | `Option<Arc<MemoryRepository>>` | Subagent spawn passes repo to child | Trait Arc |
| `gateway-execution/src/invoke/micro_recall.rs` | 52 | `Option<Arc<MemoryRepository>>` field | Lightweight per-turn recall | Trait Arc |

### HTTP layer remainder

| File | Issue | Trait additions needed |
|---|---|---|
| `gateway/src/http/memory.rs::save_memory_fact` (POST /api/memory/save) | `memory_repo.upsert_memory_fact(MemoryFact)` | `upsert_typed_fact(Value)` — same as distillation |
| `gateway/src/http/memory.rs::search_memory_facts` (GET /:agent/search) | Hybrid search via `memory_repo` + `state.embedding_service` | `search_memory_facts_hybrid` — same as recall |
| `gateway/src/http/memory.rs::stats` | `memory_repo.aggregate_stats()` (already on trait via default-impl) | Wire `state.memory_store` |
| `gateway/src/http/memory.rs::health` | `memory_repo.health_metrics()` | Wire `state.memory_store` |
| `gateway/src/http/memory_search.rs` | Uses `state.knowledge_db` to instantiate hybrid search engine; not currently behind any trait | Larger refactor — `KnowledgeDatabase` is the SQLite handle; needs an abstraction |
| `gateway/src/http/ward_content.rs` | Same — `state.knowledge_db` direct access for ward-scoped reads across episode/wiki/procedure repos | Larger refactor — needs `EpisodeStore`, `WikiStore`, `ProcedureStore` traits |

---

## SurrealDB schema gaps (compared to SQLite `MemoryFact`)

The `memory_fact` table currently models a subset of the SQLite columns.
Migrating the typed-write paths requires extending the schema.

| Field | SQLite | SurrealDB | Notes |
|---|---|---|---|
| `id` | TEXT PK | RecordId String | ✅ |
| `session_id` | TEXT NULL | — | Add `DEFINE FIELD session_id ON memory_fact TYPE option<string>;` |
| `agent_id` | TEXT | string | ✅ |
| `scope` | TEXT | — | Add — used by ctx/session/global filtering |
| `category` (vs `fact_type`) | TEXT | string (`fact_type`) | Rename or add alias; SQLite uses `category`, Surreal currently uses `fact_type` |
| `key` | TEXT | — | Add — used by upsert dedup |
| `content` | TEXT | string | ✅ |
| `confidence` | REAL | float | ✅ |
| `mention_count` | INTEGER | — | Add `DEFINE FIELD mention_count ON memory_fact TYPE int DEFAULT 0;` |
| `source_summary` | TEXT NULL | — | Add `option<string>` |
| `embedding` | (separate table) | option<array<float>> | ✅ but on the row directly; SQLite splits to `vec_memory_facts_index` |
| `ward_id` | TEXT DEFAULT `__global__` | — | Add `DEFINE FIELD ward_id ON memory_fact TYPE string DEFAULT '__global__';` |
| `contradicted_by` | TEXT NULL | — | Add `option<string>` |
| `created_at` | TEXT (ISO8601) | datetime | ✅ |
| `updated_at` | TEXT (ISO8601) | datetime (`last_used_at`) | Rename or alias |
| `expires_at` | TEXT NULL | — | Add `option<datetime>` |
| `valid_from` | TEXT NULL | — | Add `option<datetime>` |
| `valid_until` | TEXT NULL | — | Add `option<datetime>` |
| `superseded_by` | TEXT NULL | — | Add `option<string>` |
| `pinned` | INTEGER (0/1) | — | Add `DEFINE FIELD pinned ON memory_fact TYPE bool DEFAULT false;` |
| `epistemic_class` | TEXT | — | Add — drives lifecycle behavior |

Once these land, the SQLite-side `MemoryFact` struct can serialize to a
JSON shape that the Surreal-side schema accepts as-is via UPSERT, and
both sides round-trip the same Value.

---

## Embedding cache — separate concern

`memory_repo.get_cached_embedding(hash, model)` and
`memory_repo.cache_embedding(hash, model, embedding)` are a per-content
embedding cache, not user-facing data. They exist to avoid re-embedding
the same string when the embedding model hasn't changed.

Recommendation: **leave on SQLite**, even when SurrealDB is selected for
KG + memory. The cache is local-only, ephemeral by nature, and the
existing SQLite implementation is fine. A separate trait
`EmbeddingCacheStore` could be introduced if a future deployment needs
to share the cache across processes.

---

## How to add a third backend (Postgres, DuckDB, etc.)

1. **Create a new crate**: `stores/zero-stores-postgres/` (or whatever),
   modeled on `stores/zero-stores-surreal/`. Cargo.toml depends on
   `zero-stores-traits`, `knowledge-graph` (types only), and the
   backend driver (e.g., `tokio-postgres`).

2. **Implement the schema**: write a `schema/<namespace>.sql` (or
   equivalent) that mirrors the SurrealDB schema in
   `stores/zero-stores-surreal/src/schema/memory_kg.surql`. Include all
   SQLite-side columns listed in the table above to ensure parity.

3. **Implement the traits**: `KnowledgeGraphStore` + `MemoryFactStore`.
   Use the existing SurrealDB or SQLite impls as references. Each method
   should round-trip a `Value` shape compatible with `MemoryFactResponse`
   (the HTTP response type) so handlers don't need backend-specific code.

4. **Conformance**: wire the new backend into
   `stores/zero-stores-conformance/`. Both SQLite and SurrealDB pass the
   suite — your impl should too. Failures surface real divergence.

5. **Cargo feature**: add `<backend-name>-backend` to
   `gateway/Cargo.toml`'s `[features]`, with `optional = true` on the
   dep. Forward in `apps/daemon/Cargo.toml`.

6. **Factory dispatch**: in
   `gateway/src/state/persistence_factory.rs`, add a
   `maybe_build_<backend>_pair` function modeled on
   `maybe_build_surreal_pair`. It reads
   `execution.featureFlags.<backend>_backend` from settings.json and
   builds the trait pair. Wire into `AppState::new`'s feature-gated
   branch.

7. **UI**: extend `apps/ui/src/features/settings/PersistenceCard.tsx`
   to include the new option in the dropdown. Reuse the same
   `featureFlags.<backend>_backend` save path.

8. **Recovery**: optional but recommended — create
   `stores/zero-stores-<backend>-recovery/` modeled on
   `zero-stores-surreal-recovery/`. CLI-invoked, not auto-recovery.

The key invariant: **everything goes through the trait surface**. If
your new backend can't satisfy a method, look at why the trait method
exists and whether the contract needs to relax. Don't expose the
backend's native handle to handlers — that locks the door on future
swaps and re-creates the TD-023 problem we're escaping.

---

## Migration ergonomics — lessons learned

Things that bit us during the SurrealDB integration, captured so the
next backend doesn't repeat them:

**SurrealDB 3.0 quirks** (in case you're on 2.x or 4.x and patterns differ):
- `Thing` was renamed to `RecordId`; both at `surrealdb::types::RecordId`.
- `RecordIdKey` is an enum; no `Display` impl — exhaustive match on `String`/`Number`/`Uuid`/Array/Object/Range.
- `Root` auth fields are `String` (owned), not `&'a str`.
- `SEARCH ANALYZER` was renamed `FULLTEXT ANALYZER`; FULLTEXT indexes accept one column max.
- `value` is a reserved keyword — use `data` for arbitrary metadata fields.
- Underscore-prefixed table names (`_meta`) are rejected in `SELECT FROM` — use a non-underscore name.
- `SCHEMAFULL` is strict: nested fields not declared in schema get rejected. Use `SCHEMALESS` for variable-shape metadata tables.
- Non-existent tables error on read in 3.0 (vs empty result in 2.0); handlers must catch `does not exist` on both `query()` and `take()`.
- Custom row structs deserializing via `take()` need `#[derive(SurrealValue)]` (not `serde::Deserialize`); annotate with `#[surreal(crate = "surrealdb::types")]` because the proc-macro defaults to the wrong path.
- UPSERT first-write field-increment idiom: `mention_count = (mention_count OR 0) + 1`.
- `duration::from_hours` (single underscore), not `duration::from::hours`.

**General trait-design lessons**:
- Returning `serde_json::Value` for typed structs avoids the dep-cycle headache (gateway-database depends on agent-runtime which depends on agent-tools; types in either crate can't easily reach the trait crate).
- Default trait impls returning safe values (empty Vec, false, None) keep mocks and partial implementations from blocking caller migration.
- Conformance tests catch backend divergence early. Two known SQLite gaps surfaced: `reindex_embeddings` requires an embedding client wired in (fixture issue), and `list_entities` leaks across agents (real bug). Both are TD follow-ups.
- The bridge from sync `AppState::new` to async `connect_surreal` uses
  `tokio::task::block_in_place(|| Handle::current().block_on(...))`.
  Works because the daemon is already inside a tokio runtime at startup;
  would not work in a non-tokio context.

---

## Phase status (as of 2026-04-28)

| Phase | Scope | Status |
|---|---|---|
| Foundation | New crate, schema, factory, AppState wiring, UI toggle | ✅ shipped (commits c3f9ebb…1915bdc) |
| A1 | `/api/memory` listing endpoints | ✅ shipped (commits fc812fc, a1f5ed6) |
| A2 | `/api/memory` save + search endpoints | ⏳ pending — needs `upsert_typed_fact` + `search_memory_facts_hybrid` trait methods |
| A3 | `/api/memory/stats` + `/api/memory/health` | ⏳ pending — methods exist as defaults; just rewire handlers |
| A4 | `memory_search.rs` + `ward_content.rs` | ⏳ blocked on EpisodeStore + WikiStore + ProcedureStore traits |
| B | distillation + recall (chat-time write/read) | ⏳ pending — needs SurrealDB schema extension first |
| C | sleep worker, pruning, runner field-type churn | ⏳ pending — large but mechanical once trait Arc passing is established |

---

## Recommended next-PR sizing

- **PR-1 (small, 1-2 hr):** A2 — upsert_typed_fact + handler migration. Unblocks UI fact saves on SurrealDB. Schema extension on SurrealDB side: add `scope`, `key`, `mention_count`, `source_summary`, `pinned`, `epistemic_class`, `valid_*`, `expires_at`, `superseded_by`, `contradicted_by`, `ward_id`. Conformance scenario for upsert round-trip.

- **PR-2 (medium, 2-3 hr):** A3 + B (recall) — wire stats/health to trait, migrate recall_facts_hybrid trait method + impls. Distillation upsert path follows trivially since A2 added the typed-write method.

- **PR-3 (large, 4-8 hr):** C — change struct field types in runner/sleep/pruning from `Arc<MemoryRepository>` to `Arc<dyn MemoryFactStore>`. Mechanical churn but touches ~10 files. Subagent-driven execution recommended.

- **PR-4 (large, separate workstream):** A4 — introduce `EpisodeStore`, `WikiStore`, `ProcedureStore` traits, implement on both backends. This is its own design discussion because the cross-repo hybrid search has real complexity.

After PR-3, the SurrealDB toggle is fully end-to-end for memory + KG.
After PR-4, the same is true for ward content.

---

## Phase D — full DDD/clean-architecture relocation

The previous phases retire concrete-typed call sites in favor of the
trait surface, but the trait *implementations* still live in the wrong
crate (`gateway/gateway-database/`) — and the SQLite-coupled types
(`MemoryFact`, `KnowledgeDatabase`, `VectorIndex`, etc.) leak from
`gateway-database` into the rest of the workspace, including pure
business-logic crates like `services/knowledge-graph/`.

The target architecture (DDD-style) is:

```
stores/                                   ← all persistence here
├── zero-stores-domain/                     pure-data structs (MemoryFact,
│                                            Entity, Relationship, Episode,
│                                            WikiArticle, Procedure, Goal, etc.)
├── zero-stores-traits/                     trait surface (FactStore,
│                                            KnowledgeGraphStore, EpisodeStore,
│                                            WikiStore, ProcedureStore,
│                                            DistillationStore, ConversationStore,
│                                            RecallLogStore, GoalStore, etc.)
├── zero-stores/                            re-exports + shared error/value types
├── zero-stores-sqlite/                     full SQLite impl (absorbs
│                                            gateway-database/ + the SQLite
│                                            half of services/knowledge-graph/)
├── zero-stores-surreal/                    full SurrealDB impl (parity)
└── zero-stores-postgres/                   future

services/
└── knowledge-graph/                        traversal, resolution, alias logic
                                             — NO storage code

gateway/                                   ← thin: HTTP + composition
├── src/state/                                DI container; one place that
│                                              picks store impls + wires Arcs
└── src/http/                                 handlers take Arc<dyn Store>
                                               only; no knowledge_db, no
                                               memory_repo concrete refs
```

`gateway-database/` and `services/knowledge-graph/storage/` are deleted.
`gateway/Cargo.toml` no longer depends on rusqlite, sqlite-vec, etc.

### Slice-by-slice plan (each slice is a green-build, mergeable PR)

| Slice | Description | Effort | Build invariant |
|---|---|---|---|
| **D1** | Domain crate: create `stores/zero-stores-domain/`, move `MemoryFact` + sibling pure-data structs into it. `gateway-database` re-exports the moved types so the 80 import sites compile unchanged. | 1-2 hr | All builds + tests pass without any consumer change |
| **D2** | Add `EpisodeStore` trait + SQLite impl + SurrealDB impl + conformance scenarios. Migrate `/api/wards/:ward/content` episode read paths through it. | 4-6 hr | UI ward content endpoint shape unchanged on either backend |
| **D3** | Same as D2 for `WikiStore`. Migrates `ward_content.rs` wiki reads. | 4-6 hr | UI wiki tab unchanged on either backend |
| **D4** | Same as D2 for `ProcedureStore`. Migrates recall-side procedure search. | 3-4 hr | Agent recall behaviour unchanged |
| **D5** | `RecallLogStore`, `DistillationStore`, `ConversationStore`, `GoalStore` traits + impls. These are smaller surface areas; can batch into one PR. | 4-6 hr | Distillation + conversation save unchanged |
| **D6** | Relocate the entire `services/knowledge-graph/storage/GraphStorage` into `stores/zero-stores-sqlite/kg/`. `services/knowledge-graph/` keeps only the traversal + resolution logic. Update the 30+ KG storage imports. | 6-8 hr | `cargo check --workspace` passes; conformance + KG tests pass |
| **D7** | Relocate the rest of `gateway-database` into `stores/zero-stores-sqlite/`. Keep `gateway-database` as a re-export shim during the transition (one-PR move, no consumer churn). | 4-6 hr | Workspace builds; `gateway-database` is now a thin facade |
| **D8** | Migrate the 80 import sites from `gateway_database::*` to `zero_stores_sqlite::*` (or `zero_stores_domain::*` for types). Subagent-driven mechanical churn. | 6-10 hr | Workspace builds at every commit; tests pass |
| **D9** | Delete the empty `gateway-database` crate. Remove rusqlite + sqlite-vec from `gateway/Cargo.toml`. Final clippy + fmt pass. | 1-2 hr | `cargo build` is clean; gateway has no SQLite-direct deps |

**Total: 33-50 hours of focused work.** Per-slice commits stay green and
mergeable, so the work can be paused and resumed at any boundary.

### Sizing for a subagent-driven sprint

The mechanical slices (D1, D7, D8, D9) are excellent subagent targets —
the change shape is predictable and the verification is `cargo build
--workspace`. Slices D2-D6 need more design judgment (trait surface
selection, conformance scenarios) and are best driven by a senior with
subagent assistance for the impl files.

Recommendation: open one branch per slice, merge sequentially. Don't
batch — the import-update slice (D8) is high-noise on diff and benefits
from being its own focused PR.
