# Persistence Layer Tech Debt — Registry & Fix Plan

## Why this doc exists

The medium-term plan is to replace `knowledge.db` (currently SQLite + `sqlite-vec` + FTS5) with **SurrealDB** (memory + knowledge graph in one embedded store). `conversations.db` will stay on SQLite indefinitely.

This doc tracks every piece of tech debt that stands between "today" and "switch-flip ready" — the state where adding SurrealDB is a single new crate plus a config switch, with no edits required in `gateway-execution/`, `services/*`, or business logic.

It is also the canonical home for persistence-layer tech-debt items that aren't migration-related. Treat it as a **living document**: check items off as they ship, add new items when discovered.

## How to use this doc

- **Inventory section** — flat list of every debt item, grouped by category. Each item has a stable ID (`TD-NNN`), severity, file:line refs, why it's debt, and a concrete fix approach.
- **Phased fix plan** — sequenced phases, each closing one or more inventory items.
- **Status field** on each item: `pending` / `in-progress` / `done`. Update as work happens.
- **PR linking** — when an item ships, replace `pending` with `done — PR #N`.
- **Adding items** — see the "How to add an item" section at the bottom.

## Severity & scope conventions

| Symbol | Meaning |
|---|---|
| 🔴 Critical | Real bug, or hard blocker for SurrealDB readiness |
| 🟠 High | Significant abstraction debt; on the critical path |
| 🟡 Medium | Mechanical cleanup; easy win |
| 🟢 Low | Stylistic; do alongside related work, not on its own |

Scope tags:
- **[K]** = knowledge-side (memory + KG; will move to SurrealDB)
- **[C]** = conversations-side (stays SQLite forever)
- **[B]** = bug, independent of migration
- **[Both]** = touches both halves

---

## Inventory

### Real bugs

#### TD-001 ✅ [B] `archiver.rs` archives now wrapped in a transaction
- **Location:** `gateway/gateway-execution/src/archiver.rs:186-210`
- **Resolution:** Wrapped the three writes (`DELETE FROM messages`, `DELETE FROM execution_logs`, `UPDATE sessions SET archived = 1`) in a single `with_connection` block bounded by `conn.unchecked_transaction()` and `tx.commit()`. Added regression test `archive_session_atomically_deletes_and_marks` that seeds a session with messages + logs and verifies all three writes succeed end-to-end.
- **Status:** done — PR #73 (commit `dbcebea`)

### Abstraction-shape debt — knowledge side (critical path for SurrealDB)

#### TD-010 ✅ [K] `KnowledgeGraphStore` trait extracted (Phase 1 done)
- **Location:** `services/knowledge-graph/src/storage.rs` (concrete `GraphStorage` retained as the SQLite impl backing).
- **Resolution:** `KnowledgeGraphStore` trait now lives in new `zero-stores` interface crate with all 14 methods (entity CRUD, aliases & resolution, relationships, bulk ingest, read paths, maintenance). New `zero-stores-sqlite` adapter crate implements the trait by wrapping `Arc<GraphStorage>` and bridging sync rusqlite → async via `spawn_blocking`. New `zero-stores-conformance` crate holds cross-impl scenarios (one scaffold scenario; more added incrementally). `AppState` exposes `kg_store: Option<Arc<dyn KnowledgeGraphStore>>` alongside the existing `graph_service`. One HTTP handler (`search_entities`) migrated as proof of pattern. Three atomicity bugs found and fixed in `GraphStorage` along the way: `delete_entity_by_id` (commit 324b573), `store_knowledge` (commit cc59cde) — both wrapped in `unchecked_transaction()` to honor the trait contract.
- **Status:** done — Phase 1 implementation plan executed across 14 tasks on `feature/phase1-kg-store-extraction`

#### TD-011 ✅ [K] `CausalEdgeStore` rusqlite leak removed
- **Location:** `services/knowledge-graph/src/causal.rs` — `pub fn new(db: Arc<KnowledgeDatabase>)` (was: `Arc<Mutex<rusqlite::Connection>>`).
- **Resolution:** Constructor now takes `Arc<KnowledgeDatabase>`. Struct field changed from `Arc<Mutex<rusqlite::Connection>>` to `Arc<KnowledgeDatabase>`. Method bodies use `self.db.with_connection(|conn| ...)` instead of `self.conn.lock()`. All 4 existing causal-edge tests pass; no behavioural change. No public callers exist outside the test module — `CausalEdgeStore` is constructed only in tests today, so updating callers reduces to fixing the test fixture.
- **Status:** done — commit `93a75bd`

#### TD-012 🟠 [K] `gateway-execution/sleep/*` bypasses persistence abstractions
- **Locations:** All in `gateway/gateway-execution/src/sleep/`
  - `synthesizer.rs` (~12 stmts; `kg_*`, `memory_facts`, `kg_episodes`, `session_episodes`)
  - `kg_backfill.rs` (~11 stmts; `kg_entities`, `kg_relationships`, `kg_compactions`)
  - `embedding_reindex.rs` (~9 stmts; `vec0` reindex pipeline)
  - `orphan_archiver.rs` (~8 stmts; `kg_*` cleanup)
  - `pattern_extractor.rs` (~9 stmts; cross-DB read)
- **Why debt:** Roughly 60 statements touching knowledge-side tables directly via raw `rusqlite::Connection`. None of these would survive a SurrealDB swap unless they all route through `KnowledgeGraphStore` and `MemoryFactStore`.
- **Fix:** After TD-010 lands, route each file's reads/writes through the appropriate store trait. Reindex is the trickiest — `vec0`-specific schema rebuild becomes a SurrealDB-specific schema rebuild — so hide it behind `KnowledgeGraphStore::reindex_embeddings(new_dim)` (or equivalent) so each impl owns its physical layout.
- **Per-file decisions:**
  - `orphan_archiver.rs` ✅ — read path migrated (Phase 3a, PR #76); write path migrated (Phase 3b, PR #77). Together: file uses no raw rusqlite in production logic.
  - `kg_backfill.rs` ⏸ **intentionally NOT migrated** — one-shot bootstrap that runs once at daemon startup to retrofit pre-existing rows with new schema fields. Forcing it through the trait would mean adding ~6 new methods (list-with-missing-property, batch-update-properties, compaction marker check/set) just for this single bootstrap concern. When SurrealDB launches, it'll start with the new schema and won't need the same backfill — each impl handles its own schema-evolution story. Marking as decision-explicit rather than pending. (Phase 3c)
  - `embedding_reindex.rs` ✅ — Phase 3d. Orchestration moved to `stores/zero-stores-sqlite/src/reindex.rs`; `SqliteKgStore::reindex_embeddings` now drives it via a new `with_embedding_client` constructor. The gateway-side `sleep/embedding_reindex.rs` stays as a thin re-export wrapper so the two existing progress-aware callers (`state::reconcile_embeddings_at_boot` and the `/api/embeddings/configure` SSE handler) keep emitting per-table `Health::Reindexing` events to the UI's `EmbeddingProgressModal` — the trait surface itself is intentionally fire-and-report (no progress callback) so it stays portable across SQLite / SurrealDB.
  - `synthesizer.rs` 🛑 deferred — needs `MemoryFactStore` relocated from `framework/zero-core` to `stores/zero-stores` (separate workstream) plus new methods (multi-session aggregation, cosine dedup over vec0). Belongs in Phase 5+.
  - `pattern_extractor.rs` 🛑 deferred — cross-DB read (knowledge.db + conversations.db) plus LLM orchestration. Needs `ProcedureStore` + `EpisodeStore` traits, not part of Phase 3 scope. Phase 5+.
- **Status:** in progress — orphan_archiver complete (Phase 3a/3b); embedding_reindex done (Phase 3d); kg_backfill explicitly opted out (3c); synthesizer + pattern_extractor deferred to Phase 5+

#### TD-013 ✅ [K] `VectorIndex` folded into store traits
- **Location:** `gateway/gateway-database/src/vector_index.rs:15-32`
- **Resolution:** Decided in `memory-bank/future-state/persistence-readiness-design.md` (Section: Trait surface). **No public `VectorIndex` trait** in `zero-stores`. Vector ops are part of `MemoryFactStore::recall` and `KnowledgeGraphStore::resolve_entity`. SQLite impl keeps `SqliteVecIndex` internally as an implementation detail; SurrealDB impl uses HNSW indexes inline on records.
- **Status:** resolved by design — fix lands as part of TD-010 / TD-012 implementation

#### TD-014 🟠 [K] Knowledge-side ops in `services/execution-state/repository.rs` co-mingle with conversations ops
- **Location:** `services/execution-state/src/repository.rs` (entire file — ~99 fns, ~81 stmts). Touches `sessions`, `agent_executions`, `messages` (conversations.db) **and** `memory_facts`, `kg_relationships`, `kg_entities`, `recall_log` (knowledge.db). Note also the developer-acknowledged pain at line 426: `"the with_connection trait hands us &Connection, not &mut"`.
- **Why debt:** A single repo straddles the two databases that are about to diverge. Once knowledge.db moves to SurrealDB, every method in this file has to know which backend to talk to — turning the repo into a manual coordinator.
- **Fix:** Split into a leaner `ExecutionStateStore` (sessions/executions/messages — stays SQLite) and remove all direct knowledge-side table access from this file, replacing with calls into `KnowledgeGraphStore` + `MemoryFactStore`.
- **Status:** pending (depends on TD-010)

### Abstraction-shape debt — conversations side (NOT critical for migration)

#### TD-020 🟡 [C] `DbProvider` / `StateDbProvider` traits are SQLite-shaped
- **Locations:**
  - `services/api-logs/src/repository.rs:17-21` — `DbProvider`
  - `services/execution-state/src/repository.rs:14-17` — `StateDbProvider`
- **What:** Both define `fn with_connection<F, R>(&self, f: F) -> Result<R, String> where F: FnOnce(&Connection) -> Result<R, rusqlite::Error>`. The closure parameter is a raw rusqlite `&Connection`; the inner error type is `rusqlite::Error`. The trait *is* SQLite.
- **Why debt:** Even though conversations stays SQLite, this shape blocks any cross-cutting work (observability decorators, swapping pool implementations, in-memory test doubles, etc).
- **Fix:** Reshape to method-per-operation traits — `LogStore::append`, `LogStore::query_by_session`, `ExecutionStateStore::insert_message`, etc. Internal pool can stay rusqlite; the contract stops leaking.
- **Status:** pending (does not block SurrealDB switch; do in Phase 6)

#### TD-021 🟢 [C] `ConversationRepository` is concrete — no trait
- **Location:** `gateway/gateway-database/src/repository.rs:37`. Used as `Arc<ConversationRepository>` at `gateway/src/state.rs:63`, `gateway-execution/src/runner/core.rs`, delegation callbacks, HTTP handlers.
- **Why debt:** Same hygiene argument as TD-020. Conversations stays SQLite, so not blocking.
- **Fix:** Extract `ConversationStore` trait. Defer until trait shapes for the knowledge side are settled, so the shape is consistent.
- **Status:** pending (defer to Phase 6)

#### TD-022 🟢 [C] `OutboxRepository` is concrete — no trait
- **Location:** `gateway/gateway-bridge/src/outbox.rs` (~12 stmts of raw rusqlite).
- **Fix:** Extract `OutboxStore` trait. Independent of SurrealDB plan.
- **Status:** pending (defer to Phase 6)

#### TD-023 ⏳ [Both] `AppState` factory pattern established; `graph_service` retirement deferred
- **Location:** `gateway/src/state/mod.rs:63-75` — concrete persistence types alongside trait objects.
- **Progress (Phase 5):** Factory pattern landed at `gateway/src/state/persistence_factory.rs`. `kg_store` construction goes through `build_kg_store_from_storage()`; the canonical `build_kg_store(knowledge_db, embedding_client)` entrypoint is gated until `graph_service` retires (it's where the SurrealDB config-driven branch will live). When SurrealDB support lands, this is where the branch goes — consumers don't need to change because they hold `Arc<dyn KnowledgeGraphStore>`.
- **Deferred:** Retirement of `graph_service: Option<Arc<GraphService>>` (the parallel concrete field on `AppState`). Migrating its dozens of consumers — HTTP handlers in `gateway/src/http/graph.rs`, sleep jobs, distillation, etc. — is a multi-PR workstream.
- **Status:** factory pattern done; `graph_service` retirement deferred to a future workstream

### Dialect-portability debt

#### TD-030 ✅ [K] `datetime('now')` knowledge-side callsites replaced with Rust timestamps
- **Locations cleared (Phase 2):**
  - `gateway/gateway-database/src/memory_repository.rs:473, 628, 667, 1103, 1156, 1172, 1202` — 7 callsites (commit `e0d89fe`)
  - `gateway/gateway-database/src/procedure_repository.rs:247, 258, 272` — 3 callsites (commit `67a6dd2`)
  - `gateway/gateway-database/src/episode_repository.rs:235` — 1 relative-time callsite using `chrono::Duration::days(14)` (commit `29a1b02`)
- **Resolution:** All knowledge-side runtime SQL callsites now bind a `chrono::Utc::now().to_rfc3339()` parameter instead of using SQLite's `datetime('now')`. Relative-time math (e.g., `'-14 days'`) is computed via `chrono::Duration::days(14)` and bound as a parameter. The SQLite impl behaves identically; the contract no longer assumes the DB has a clock — needed for the future SurrealDB swap.
- **Out of scope (deliberately retained):** `knowledge_schema.rs:31, 736` (schema bootstrap upsert + test fixture — impl-internal schema territory; addressed in TD-032 if/when revisited). Conversations-side callsites (`schema.rs`, `repository.rs`, `outbox.rs`, `archiver.rs`) — conversations stays SQLite forever per the design, so portability concerns don't apply.
- **Follow-up tracked separately:** TD-042 captures the `julianday('now')` callsite at `memory_repository.rs:629` (semantic change required, not a mechanical substitution).
- **Status:** done — Phase 2

#### TD-031 ✅ [K] `INSERT OR REPLACE` / `INSERT OR IGNORE` semantics already at trait level (closed by Phase 1)
- **Locations audited (Phase 2):** All `INSERT OR REPLACE` / `INSERT OR IGNORE` callsites in the workspace are inside impl crates (`gateway-database`, `services/knowledge-graph`, `services/execution-state`). Specifically: `recall_log_repository.rs`, `schema.rs`, `knowledge_schema.rs`, `memory_repository.rs:1102` (embedding_cache), `causal.rs`, `kg_episode_repository.rs` (×3), `storage.rs` (×3 alias inserts), `execution-state/repository.rs:534` (temp-table trick).
- **Resolution:** Phase 1's trait surface (`KnowledgeGraphStore`, `MemoryFactStore` in `stores/zero-stores`) already exposes upsert vocabulary — `upsert_entity`, `upsert_relationship`, `add_alias`, `save_fact`, etc. The SQLite-specific `INSERT OR …` syntax lives entirely inside the SQLite impl crate and is not visible to any consumer crate. The future SurrealDB impl will use SurrealDB's record-upsert semantics in its own crate — no contract change needed.
- **Status:** done by Phase 1 — audit confirmed in Phase 2 (no code changes required)

#### TD-032 ✅ [K] Schema bootstrap is per-impl, idempotent, no cross-version migration in scope
- **Locations:** `gateway/gateway-database/migrations/{v23_wiki_fts.sql, v24_global_scope_backfill.sql}`, plus inline schema strings in `gateway/gateway-database/src/schema.rs` and `knowledge_schema.rs`.
- **Resolution direction (per design doc):** Each impl crate has a private `bootstrap_schema()` function called once at startup. SQLite impl runs idempotent `CREATE TABLE IF NOT EXISTS …`. SurrealDB impl runs idempotent `DEFINE TABLE … IF NOT EXISTS`. **No `Migrator` trait in `zero-stores`.** Cross-version data migration and SQLite→SurrealDB data migration are explicitly out of this design's scope — those become a future `zero-stores-migrate` crate when actually needed.
- **Phase 4 outcome:** Added `stores/zero-stores-sqlite/src/bootstrap.rs` with `bootstrap_schema()` as the canonical hook point for the SQLite backend. Today it delegates to `gateway-database`'s `KnowledgeDatabase::new` (which runs the bootstrap as a constructor side effect) — pattern established without churning 1000+ lines of schema DDL. When SurrealDB lands, its bootstrap mirrors this in `stores/zero-stores-surreal/src/bootstrap.rs`. Full DDL relocation deferred until shapes need to diverge between backends.
- **Status:** ✅ done (Phase 4) — pattern established, full schema relocation deferred until proven necessary

#### TD-033 🟢 [Both] Hardcoded table-name string literals scattered across crates
- **Locations:** ~90 in `services/knowledge-graph/src/storage.rs`, ~40 in `services/execution-state/src/repository.rs`, ~12 in `gateway/gateway-bridge/src/outbox.rs`.
- **Why debt (mild):** Even after trait abstraction, stray `"memory_facts"` literals are coupling reminders. Not a swap blocker — once those callsites route through the store trait, the literals live inside the impl where they belong.
- **Fix:** No standalone fix. Resolves naturally as TD-012, TD-014, TD-020 land.
- **Status:** absorbed into other items

### Code smell (low priority)

#### TD-040 🟢 [K] Dynamic SQL via `format!()` in `embedding_reindex.rs`
- **Location (historical):** `gateway/gateway-execution/src/sleep/embedding_reindex.rs:127-218` — five `format!()` SQL builders interpolating fields of `ReindexTarget`.
- **Current location:** `stores/zero-stores-sqlite/src/reindex.rs` — same code, same const-driven safety, just lives in the right crate now.
- **What:** **Not** a SQL injection — `REINDEX_TARGETS` is a `&'static [ReindexTarget]` const; all interpolated fields are `&'static str` baked into the binary. Stylistic only.
- **Status:** ✅ moved with TD-012 Phase 3d. Orchestration relocated to `zero-stores-sqlite` where it belongs (SQLite-impl-internal).

#### TD-041 🟢 [Both] Mixed parameter binding styles
- **What:** `?`, `?1`/`?2`/…, and named `:foo` params are all in use across the codebase.
- **Fix:** No urgency. Standardize on numbered (`?1`) opportunistically as files are touched for other work.
- **Status:** opportunistic

#### TD-042 ✅ [K] `julianday('now')` date-arithmetic at memory_repository.rs:629
- **Location:** `gateway/gateway-database/src/memory_repository.rs:629` — `julianday('now') - julianday(updated_at) > ?2` (in `decay_stale_facts`).
- **Why debt:** `julianday()` is a SQLite-specific time function. Like `datetime('now')`, it bakes a clock assumption into the SQL — but unlike `datetime('now')`, it can't be replaced by a single bound parameter. The fix requires a semantic change: pre-compute a threshold timestamp in Rust (`Utc::now() - Duration::days(N)`) and rewrite the WHERE clause to compare `updated_at < ?threshold`.
- **Phase 4 fix:** `decay_stale_facts` now computes `cutoff = (Utc::now() - chrono::Duration::days(older_than_days as i64)).to_rfc3339()` in Rust and binds it as `?3` in `WHERE updated_at < ?3`. Params reordered: `?1` decay_factor, `?2` now (for `updated_at` write), `?3` cutoff. Same flavor as Phase 2's `datetime('now')` cleanup — keeps the SQL portable across SQLite and SurrealDB.
- **Status:** ✅ done (Phase 4)

---

## Phased fix plan

Each phase produces value standalone — none of them require finishing the next.

### Phase 0 — Real bugs (do anytime, independent)
**Closes:** TD-001
- One PR, one transaction wrapper, one regression test covering partial-failure (drop the connection between deletes and assert no half-archived state).

### Phase 1 — Knowledge-side trait shape (the unblock)
**Closes:** TD-010, TD-011
**Settles:** TD-013 design decision
- Define `KnowledgeGraphStore` trait. Decide its home: new `zero-knowledge-store` interface crate vs. trait in existing `services/knowledge-graph/`. Recommend the former because Phase 5 wants a clean factory boundary.
- Move existing `GraphStorage` into a `zero-knowledge-store-sqlite` impl (or feature-gate inside `services/knowledge-graph/`).
- Fix `CausalEdgeStore::new` to take a store handle, not a `Connection`.
- Resolve TD-013: pick (a) keep `VectorIndex` separate or (b) fold into the store traits. Recommend (b); document the choice in `memory-bank/decisions.md`.
- **This is the longest pole.** Without it, TD-012 and TD-014 are blocked.

### Phase 2 — Knowledge-side dialect cleanup ✅ done
**Closed:** TD-030, TD-031 (knowledge-side callsites only)
- Replaced `datetime('now')` with Rust-side `chrono::Utc::now()` parameter binding across `memory_repository.rs` (7), `procedure_repository.rs` (3), and `episode_repository.rs` (1) — 11 callsites total.
- TD-031 was already addressed by Phase 1's trait surface; Phase 2 audit confirmed all remaining `INSERT OR …` callsites are impl-internal.
- Follow-up TD-042 (deferred `julianday('now')` callsite) tracked separately for Phase 4.

### Phase 3 — Route shadow SQL through traits
**Closes:** TD-012, TD-014
- All 6 `gateway-execution/sleep/*` files route through `KnowledgeGraphStore` and `MemoryFactStore`.
- `services/execution-state/repository.rs` knowledge-side ops migrate to the new traits.
- Most code touched, but each file is independent — can be done as 6 small PRs.

### Phase 4 — Schema bootstrap per impl (no migration system)
**Closes:** TD-032
- Move existing SQLite inline migrations into `zero-stores-sqlite/src/bootstrap.rs` (idempotent `CREATE TABLE IF NOT EXISTS …`).
- SurrealDB impl ships `zero-stores-surreal/src/bootstrap.rs` (idempotent `DEFINE TABLE … IF NOT EXISTS`).
- **No `Migrator` trait in `zero-stores`.** Cross-version migration is explicitly out of scope — when actually needed, build a separate `zero-stores-migrate` crate.

### Phase 5 — Switch wiring
**Closes:** TD-023
- `AppState` fields become `Arc<dyn …>`.
- `PersistenceFactory::new(config) -> AppStores` reads config and constructs the chosen knowledge-store impl.
- After this lands, adding SurrealDB is a new crate plus a config switch.

### Phase 6 (optional, deferred) — Conversations-side hygiene
**Closes:** TD-020, TD-021, TD-022
- Reshape `DbProvider` / `StateDbProvider` to method-per-op traits.
- Extract `ConversationStore`, `OutboxStore`.
- Strictly hygiene, not on the SurrealDB critical path. Can land after the swap.

---

## What we are deliberately NOT doing

To keep the registry honest and prevent scope creep:

- **Not touching the two-DB split.** `conversations.db` + `knowledge.db` is intentional and good. SurrealDB simply replaces `knowledge.db`. Don't merge them.
- **Not migrating any data yet.** This registry is about getting *new code* SurrealDB-ready. A one-time `knowledge.db → SurrealDB` migration tool is a separate workstream.
- **Not abstracting conversations storage** beyond Phase 6's hygiene cleanup. SQLite is the future-state for that subsystem.
- **Not introducing `sqlx`, `sea-orm`, or another ORM.** SurrealDB's Rust client speaks records-not-SQL; rusqlite stays for the SQLite impl.
- **Not building a "compatibility layer" between SQLite knowledge.db and SurrealDB.** Per-backend impls only. Dual-write / fall-back is explicitly out of scope.

---

## Reading guide

- **Architecture overview:** `memory-bank/architecture.md`
- **Design decisions log:** `memory-bank/decisions.md` (record any non-obvious decisions about persistence boundaries here — particularly the TD-013 outcome)
- **SurrealDB future state:** to be added under `memory-bank/future-state/` once Phase 1 design is settled
- **General code-health scan:** `memory-bank/sonar_scan_report.md`

---

## How to add an item

1. Pick the next free `TD-NNN` ID within the relevant block (real bugs in 001–009, knowledge-side traits in 010–019, conversations-side traits in 020–029, dialect in 030–039, code smells in 040+).
2. Fill out: location (file:line), what it is, why it's debt, the fix approach, severity, scope tag, status.
3. If it slots into an existing phase, list it there. If not, add a new phase and explain why it doesn't fit existing ones.
4. Commit the doc change as part of the PR that introduces the related work, not as a doc-only commit. The registry should track reality.
