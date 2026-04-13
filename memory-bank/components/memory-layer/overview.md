# Memory Layer — v22 Overview

## Purpose

The memory layer gives every agent long-term, cross-session recall and a
compact working set of what matters right now. It turns raw transcripts,
distillations, and ingested documents into structured facts, a typed
knowledge graph, ward-scoped wikis, reusable procedures, and tracked goals —
all retrievable through one unified scored API.

## Two-database split

Memory is split across two SQLite files so operational churn never drags on
long-term durability:

- **`conversations.db`** — operational state (sessions, agent executions,
  messages, execution logs, artifacts, bridge outbox, recall log,
  distillation runs). Pruned aggressively as sessions complete.
  Schema: `gateway/gateway-database/src/schema.rs:357`.
- **`knowledge.db`** — long-term memory (facts, KG entities/aliases/
  relationships, wiki, procedures, session episodes, goals, compactions,
  ingestion episodes + payloads, embedding cache). `sqlite-vec` loaded;
  contains all five `vec0` virtual tables. Portable — back it up without
  touching `conversations.db`.
  Schema: `gateway/gateway-database/src/knowledge_schema.rs:24`.

Both DBs carry schema version 22 (`SCHEMA_VERSION` constants in the same
files). `knowledge_schema.rs` is applied idempotently on daemon boot: there
are no migrations, only `CREATE TABLE IF NOT EXISTS`. Routing to
`KnowledgeDatabase` vs the conversations pool is enforced by the repository
layer.

## Six cognitive layers

### Layer 0 — Facts
Atomic propositions keyed by `(agent_id, scope, ward_id, key)`. Stored in
`memory_facts` with an FTS5 shadow (`memory_facts_fts`) kept in sync via
three triggers, and embeddings in the `memory_facts_index` vec0 table.
Hybrid recall = BM25 (FTS) + cosine (vec0) merged per configured weights
(`MemoryRepository::search_memory_facts_hybrid`,
`gateway/gateway-database/src/memory_repository.rs:634`).
Contradicted facts are tombstoned via `contradicted_by`/`superseded_by`;
archival facts are copied to `memory_facts_archive` by
`MemoryRepository::archive_fact`.

### Layer 1 — Knowledge graph
Typed entities (`kg_entities`), directional relationships
(`kg_relationships`), surface-form aliases (`kg_aliases`), causal edges
(`kg_causal_edges`), and per-extraction provenance via `kg_episodes` +
`kg_episode_payloads`. Name embeddings live in `kg_name_index` (vec0).
The resolver cascade (`services/knowledge-graph/src/resolver.rs:36`)
dedups on write; the sleep-time compactor collapses near-duplicates on a
schedule.

### Layer 2 — Ward wiki
Compiled per-ward articles unique on `(ward_id, title)` with a vec0
index (`wiki_articles_index`) for similarity search. Written by the
wiki compiler, queried via `WikiRepository::search_by_similarity`
(`gateway/gateway-database/src/wiki_repository.rs`).

### Layer 3 — Procedures
Reusable multi-step workflows with JSON `steps`, success/failure
counters, and `avg_duration_ms`/`avg_token_cost` for cost-aware
selection. Backed by `procedures` + `procedures_index` (vec0),
`ProcedureRepository::search_by_similarity`.

### Layer 4 — Goals (Phase 3, new in v22)
Intent lifecycle with slot tracking: `kg_goals(state, slots,
filled_slots, parent_goal_id)`. Active goals drive intent-boost in
recall (1.3× multiplier when a recalled item's content contains an
unfilled slot name). See `GoalRepository::list_active`
(`gateway/gateway-database/src/goal_repository.rs:116`) and
`scored_item::intent_boost`
(`gateway/gateway-execution/src/recall/scored_item.rs:81`).

### Layer 5 — Sleep-time maintenance (Phase 4, new in v22)
Hourly tokio task: `Compactor` → `DecayEngine` → `Pruner`. Runs
per-agent, emits one `run_id` per cycle, records every merge and prune
in `kg_compactions`. See `SleepTimeWorker::start`
(`gateway/gateway-execution/src/sleep/worker.rs:28`).

## sqlite-vec: one similarity mechanism

Every vector search goes through a `vec0` virtual table. No hand-rolled
cosine loops in the recall path, no BLOB columns duplicating embeddings
on base rows (see the structural assertion at
`gateway/gateway-database/src/knowledge_schema.rs:434`). The five vec0
tables, all 384-dim:

| Table | Partner table |
|---|---|
| `kg_name_index` | `kg_entities` |
| `memory_facts_index` | `memory_facts` |
| `wiki_articles_index` | `ward_wiki_articles` |
| `procedures_index` | `procedures` |
| `session_episodes_index` | `session_episodes` |

Five `AFTER DELETE` triggers
(`gateway/gateway-database/src/knowledge_schema.rs:315`) keep vec0 in
lockstep with the base rows.

## Streaming ingestion pipeline

```
HTTP POST /api/memory/ingest
    │
    ▼
chunker      (ingest/chunker.rs:37 — paragraph-aware, 1000 tok ±100 overlap)
    │
    ▼
for each chunk:
    KgEpisodeRepository::upsert_pending   (content_hash dedup)
    KgEpisodeRepository::set_payload      (full chunk text)
    IngestionQueue::notify                (wake workers)
    │
    ▼                                     <-- HTTP returns 202 here
queue worker (ingest/queue.rs:64)
    │
    ▼
Extractor::process                        (ingest/extractor.rs:16)
    pass 1: entities
    pass 2: relationships (conditioned on entity list)
    GraphStorage::store_knowledge → resolver cascade
    │
    ▼
KgEpisodeRepository::mark_done | mark_failed
```

Backpressure (`ingest/backpressure.rs:28`) gates both global queue depth
(default 5000) and per-source pending count (default 500) before
enqueue. Violations return `Err` → HTTP 429.

## Unified scored recall

`MemoryRecall::recall_unified`
(`gateway/gateway-execution/src/recall/mod.rs:826`) pulls from five
sources, adapts each row into `ScoredItem` (`recall/adapters.rs`), applies
`intent_boost` against active goals, and fuses the ranked lists via
Reciprocal Rank Fusion (`k = 60.0`,
`recall/scored_item.rs:41`):

1. Facts — `MemoryRepository::search_memory_facts_hybrid` (BM25 + cosine)
2. Wiki — `WikiRepository::search_by_similarity` (ward-scoped)
3. Procedures — `ProcedureRepository::search_by_similarity`
4. Graph — `GraphStorage::search_entities_by_name_embedding` (ANN via
   `kg_name_index`)
5. Goals — projected directly from `GoalRepository::list_active`

Missing subsystems are silently skipped — the caller gets whatever
sources are wired.

## Sleep-time maintenance

`SleepTimeWorker` spawns one background tokio task per agent. Each cycle:

1. **Compactor** (`sleep/compactor.rs:57`) — for each of 5 entity types
   (Person, Organization, Location, Event, Concept), fetches duplicate
   candidates at cosine ≥ 0.92 and calls
   `GraphStorage::merge_entity_into` (up to 50 pairs per type).
   `PairwiseVerifier` trait is defined but not wired in Phase 4.
2. **DecayEngine** (`sleep/decay.rs:45`) — surfaces orphan entities with
   no relationships and `last_seen_at` older than `min_age_days` (default
   30). Archival entities are excluded at the SQL level.
3. **Pruner** (`sleep/pruner.rs:28`) — calls `GraphStorage::mark_pruned`,
   which sets `compressed_into = '__pruned__'` and deletes the row from
   `kg_name_index`. Recall queries filter on `compressed_into IS NULL`.

Cycles can be forced via `POST /api/memory/consolidate`
(`SleepTimeWorker::trigger`, `sleep/worker.rs:73`). One `run_id` per
cycle is recorded across all merge and prune rows in `kg_compactions`
for audit.

## Observability

- `GET /api/memory/stats` — counts per fact/entity/relationship/wiki/
  procedure; last sleep-time run summary.
- `GET /api/memory/health` — ingestion queue depth, status counts,
  consolidator liveness.
- `POST /api/memory/consolidate` — force an immediate sleep-time cycle.

Wired in `gateway/src/http/memory.rs`.

## Performance (baseline)

All three Phase 4 budgets met by ≥1000× margin on a consumer-grade dev
box. Full numbers in
`docs/memory-v2-performance-baseline.md`:

- Resolver p95: **2.15 ms** vs 20 ms budget (9.3× under).
- Reader p95 under ingest load: **140 µs** vs 200 ms budget (1400× under).
- Sleep-time cycle: well within the hourly interval even under load.

## Further reading

- [`data-model.md`](./data-model.md) — full v22 schema, table by table.
- [`knowledge-graph.md`](./knowledge-graph.md) — resolver cascade,
  compactor, pruner, merge semantics.
- [`backlog.md`](./backlog.md) — known gaps and deferred work.
- `docs/superpowers/specs/2026-04-12-memory-layer-redesign-design.md` —
  umbrella design spec (trust the code when the spec diverges).
