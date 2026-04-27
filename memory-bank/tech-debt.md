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

#### TD-001 🔴 [B] `archiver.rs` archives without a transaction
- **Location:** `gateway/gateway-execution/src/archiver.rs:187-210`
- **What:** `SessionArchiver::archive_session` runs three independent statements in three separate `with_connection` calls — `DELETE FROM messages`, `DELETE FROM execution_logs`, `UPDATE sessions SET archived = 1` — with no outer transaction. Compressed JSONL file is written to disk *before* the deletes.
- **Risk:** A crash between calls leaves the session in a partial state — messages gone but session not flagged archived; or flagged archived but logs still present. The next archive sweep can compound the corruption (re-archive an already-half-archived session).
- **Fix:** Wrap all three statements in one `with_connection` block bounded by `BEGIN`/`COMMIT`. Ideally push the whole operation behind a `ConversationStore::archive_session(id)` trait method so the impl owns the transaction (this is a natural fit for Phase 6).
- **Status:** pending

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
- **Status:** pending (depends on TD-010)

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

#### TD-023 🟡 [Both] `AppState` holds concrete persistence types
- **Location:** `gateway/src/state.rs:63-75` — `pub conversations: Arc<ConversationRepository>`, `pub knowledge_db: Arc<KnowledgeDatabase>`, `pub log_service: Arc<LogService<DatabaseManager>>`, `pub state_service: Arc<StateService<DatabaseManager>>`.
- **Why debt:** The "switch" that picks a backend has to live in `AppState::new`. Today there's nothing to switch — concrete types are baked in.
- **Fix:** After traits exist (TD-010, TD-014, optionally TD-020+), change `AppState` fields to `Arc<dyn ...>` and add a `PersistenceFactory::new(config)` that constructs the chosen impls.
- **Status:** pending (last step — do after relevant traits exist)

### Dialect-portability debt

#### TD-030 🟡 [K] `datetime('now')` and `datetime('now', '-N days')` time math in SQL strings
- **Total occurrences:** 41 across the workspace (grep `datetime('now'` over `gateway/`, `services/`, `runtime/`, `framework/`).
- **Knowledge-side callsites (must be cleaned before SurrealDB):**
  - `gateway/gateway-database/src/memory_repository.rs:49,79,110,136`
  - `gateway/gateway-database/src/knowledge_schema.rs:32`
  - `gateway/gateway-database/src/procedure_repository.rs:51,55`
  - `services/knowledge-graph/src/storage.rs` (multiple)
  - `services/execution-state/src/repository.rs` (knowledge-side ops only)
- **Conversations-side callsites (can stay):**
  - `gateway/gateway-database/src/repository.rs:22`
  - `gateway/gateway-bridge/src/outbox.rs` (4×)
- **Fix:** For knowledge-side callsites, generate timestamps in Rust via `chrono::Utc::now().to_rfc3339()` and `chrono::Utc::now() - chrono::Duration::days(14)`, then pass as bound parameters. The SQLite impl behaves identically; the contract no longer assumes SQLite-flavoured time math.
- **Status:** pending

#### TD-031 🟡 [K] `INSERT OR REPLACE` / `INSERT OR IGNORE` syntax in SQL not in trait
- **Locations** (grep `INSERT OR (REPLACE|IGNORE)` over Rust sources):
  - `gateway/gateway-database/src/schema.rs` — `schema_version` upsert
  - `gateway/gateway-database/src/memory_repository.rs` — multiple upserts
  - `gateway/gateway-database/src/recall_log_repository.rs`
  - `gateway/gateway-database/src/kg_episode_repository.rs`
  - `gateway/gateway-database/src/knowledge_schema.rs`
  - `services/knowledge-graph/src/storage.rs` (2×)
  - `services/execution-state/src/repository.rs` (1×)
  - `gateway/gateway-execution/src/archiver.rs` (1×)
  - `gateway/gateway-execution/tests/session_state_tests.rs` (test fixture)
- **Fix:** Express upsert intent at the trait level — `upsert_entity`, `insert_if_absent_episode`, etc. Move SQLite-specific syntax into the impl. SurrealDB's record upsert semantics differ; the contract should be backend-agnostic.
- **Status:** pending

#### TD-032 🟡 [K] Schema bootstrap is per-impl, idempotent, no cross-version migration in scope
- **Locations:** `gateway/gateway-database/migrations/{v23_wiki_fts.sql, v24_global_scope_backfill.sql}`, plus inline schema strings in `gateway/gateway-database/src/schema.rs` and `knowledge_schema.rs`.
- **Resolution direction (per design doc):** Each impl crate has a private `bootstrap_schema()` function called once at startup. SQLite impl runs idempotent `CREATE TABLE IF NOT EXISTS …`. SurrealDB impl runs idempotent `DEFINE TABLE … IF NOT EXISTS`. **No `Migrator` trait in `zero-stores`.** Cross-version data migration and SQLite→SurrealDB data migration are explicitly out of this design's scope — those become a future `zero-stores-migrate` crate when actually needed.
- **Fix:** Refactor existing inline migrations into `zero-stores-sqlite/src/bootstrap.rs`. Add `zero-stores-surreal/src/bootstrap.rs` for SurrealDB's `DEFINE …` calls when that impl is added.
- **Status:** pending

#### TD-033 🟢 [Both] Hardcoded table-name string literals scattered across crates
- **Locations:** ~90 in `services/knowledge-graph/src/storage.rs`, ~40 in `services/execution-state/src/repository.rs`, ~12 in `gateway/gateway-bridge/src/outbox.rs`.
- **Why debt (mild):** Even after trait abstraction, stray `"memory_facts"` literals are coupling reminders. Not a swap blocker — once those callsites route through the store trait, the literals live inside the impl where they belong.
- **Fix:** No standalone fix. Resolves naturally as TD-012, TD-014, TD-020 land.
- **Status:** absorbed into other items

### Code smell (low priority)

#### TD-040 🟢 [K] Dynamic SQL via `format!()` in `embedding_reindex.rs`
- **Location:** `gateway/gateway-execution/src/sleep/embedding_reindex.rs:127-218` — five `format!()` SQL builders interpolating fields of `ReindexTarget`.
- **What:** **Not** a SQL injection — `REINDEX_TARGETS` is a `&'static [ReindexTarget]` const at line 64; all interpolated fields are `&'static str` baked into the binary. Stylistic only.
- **Fix:** Resolves naturally when reindex moves behind `KnowledgeGraphStore::reindex_embeddings` (TD-012). No standalone fix needed.
- **Status:** absorbed into TD-012

#### TD-041 🟢 [Both] Mixed parameter binding styles
- **What:** `?`, `?1`/`?2`/…, and named `:foo` params are all in use across the codebase.
- **Fix:** No urgency. Standardize on numbered (`?1`) opportunistically as files are touched for other work.
- **Status:** opportunistic

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

### Phase 2 — Knowledge-side dialect cleanup (small, parallel-safe)
**Closes:** TD-030, TD-031 (knowledge-side callsites only)
- Replace `datetime('now')` with Rust-side `chrono::Utc::now()` strings.
- Hide `INSERT OR REPLACE` / `INSERT OR IGNORE` behind trait methods.
- Can run in parallel with Phase 1 once trait shapes are decided.

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
