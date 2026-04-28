# SurrealDB 3.0 Backend — Design Spec

**Date:** 2026-04-27
**Status:** Design approved, awaiting implementation plan
**Predecessor:** `memory-bank/future-state/persistence-readiness-design.md` (trait-abstraction work, PRs #71–#83 — landed)
**Quality bar:** Clean Code + ~90% unit test coverage (non-negotiable; see §10)

---

## 1. Goal

Add a SurrealDB 3.0 implementation of `KnowledgeGraphStore` and `MemoryFactStore` behind the existing trait abstraction so the knowledge-graph + memory subsystems can run on SurrealDB while conversations stay on SQLite. Default backend stays SQLite; SurrealDB is opt-in until the conformance suite proves parity.

## 2. Non-Goals

- **Migration of existing SQLite knowledge.db data.** A migration crate is a separate, future workstream. First launch on the SurrealDB backend = empty DB, agent rebuilds from interactions.
- **Conversations migration.** Conversations stay on `conversations.db` SQLite for this release. The design reserves `NAMESPACE conversations` for a future, conditional migration but does not implement it.
- **Performance targets.** We don't gate the swap on benchmarks. Correctness via conformance is the bar.
- **Multi-process / cluster deployments.** Mode A is single-process embedded; Mode B (subprocess sidecar) is deliberately deferred.

---

## 3. Topology & Crate Layout

### Process model — Mode A (today)

One process. The gateway daemon spawns `Surreal<Any>` from
`surrealdb::engine::any::connect("rocksdb:///path/to/knowledge.surreal")` during startup. No subprocess, no IPC, no port. Same crash domain as the rest of the daemon. Tokio runtime is shared; SurrealDB's RocksDB engine runs on the daemon's blocking pool.

### Process model — Mode B (future, designed-in)

Same `Surreal<Any>` API, but constructed from
`connect("ws://127.0.0.1:18792")`. The only code that changes is the URL string and a new supervisor module that spawns/restarts the `surreal start` binary. Everything downstream (stores, traits, services) stays identical because `Surreal<Any>` is the universal type. Detailed migration checklist in §9.

### Namespaces

- `NAMESPACE memory_kg` — memory + KG today. Tables: `entity`, `relationship`, `entity_alias`, `memory_fact`, `wiki_doc`, etc.
- `NAMESPACE conversations` — **reserved**, not created until/unless conversations migrate. When triggered, becomes its own namespace with `conversation`, `message`, `event`, `agent_execution`. Keeps a clean rollback boundary; switching namespaces in queries is free.

### Crate layout

```
stores/
  zero-stores-traits/         (existing, untouched)
  zero-stores/                (existing, untouched)
  zero-stores-sqlite/         (existing, untouched — stays for conversations + fallback)
  zero-stores-surreal/        ← NEW: SurrealDB impls of KnowledgeGraphStore + MemoryFactStore
  zero-stores-surreal-recovery/ ← NEW: placeholder corruption-recovery crate (CLI-invoked)
  zero-stores-conformance/    (existing, grown to ~30 scenarios — runs against both impls)
```

`zero-stores-surreal` depends on `zero-stores-traits`, `surrealdb` (with `kv-rocksdb` feature), `knowledge-graph` (type definitions only). It does **not** depend on `gateway-database`, `gateway-services`, or any SQLite-specific crate.

### `AGENTS.md` requirement

`stores/zero-stores-surreal/AGENTS.md` is a Task 1 deliverable in the implementation plan. It captures every locked decision in this spec — topology, namespaces, config surface, declarative schema, lazy HNSW, idempotency rules — so future agents touching the crate don't have to reverse-engineer the choices.

---

## 4. Configuration & Construction

### Config surface (`settings.json`)

```json
{
  "persistence": {
    "knowledge_backend": "sqlite",
    "surreal": {
      "url": "rocksdb://$VAULT/data/knowledge.surreal",
      "namespace": "memory_kg",
      "database": "main",
      "credentials": null
    }
  }
}
```

| Field | Meaning |
|---|---|
| `knowledge_backend` | `"sqlite"` (default) or `"surreal"`. Single switch — flip + restart. |
| `surreal.url` | Connection string passed verbatim to `engine::any::connect()`. Scheme picks the engine. |
| `surreal.credentials` | `null` for Mode A. `{ username, password }` for Mode B. Designed in now so Mode B doesn't need a config schema change. |

URL schemes:
- `rocksdb://...` → embedded (Mode A)
- `ws://127.0.0.1:PORT` / `wss://...` → subprocess/remote (Mode B)
- `mem://` → in-memory (tests only)

`$VAULT` placeholder gets expanded against the user's vault root before `connect()` (same pattern as existing SQLite path resolution). On Pi/SSD setups, the user puts the SSD mount in `$VAULT` and the URL just works.

### Construction (single site)

```rust
// stores/zero-stores-surreal/src/connection.rs
pub async fn connect(cfg: &SurrealConfig) -> Result<Surreal<Any>> {
    let url = expand_vault_placeholder(&cfg.url)?;
    let db = surrealdb::engine::any::connect(&url).await?;
    if let Some(creds) = &cfg.credentials {
        db.signin(Root { username: &creds.username, password: &creds.password }).await?;
    }
    db.use_ns(&cfg.namespace).use_db(&cfg.database).await?;
    Ok(db)
}
```

This is the **only** place URL strings are interpreted. Mode B requires zero changes here.

### Lifetime

One `Surreal<Any>` handle lives in `AppState`, wrapped in `Arc`. Cloning is cheap (internal SDK channel). Both store impls (`SurrealKgStore`, `SurrealMemoryStore`) hold `Arc<Surreal<Any>>` and dispatch all queries through it. No per-operation reconnect, no pooling code — the SDK handles it.

### Startup failure policy

If `connect()` fails (corrupt DB, bad URL, port not listening in Mode B), the daemon **refuses to start** with a clear error. No silent fallback to SQLite — that masks data divergence. Recovery = manual config flip + the corruption-recovery placeholder crate (§6).

---

## 5. Schema & Migration

### Approach: declarative bootstrap

All `DEFINE NAMESPACE / TABLE / FIELD / INDEX` statements use `IF NOT EXISTS` and run on every startup. Idempotent — running them on a fully-initialized DB is a no-op. No numbered migration files, no `_migrations` tracking table.

For genuinely breaking changes (rare), a `_meta:version` record is read at bootstrap and upgrade closures run in sequence to bring the DB to current.

### Schema scope

The SurrealQL bootstrap mirrors the existing SQLite schema in `gateway-database/src/knowledge_schema.rs` and `migrations/v*.sql` **one-to-one**: same column names, same types, same defaults, same constraints. The illustrative sketch below is not the canonical source — the canonical SurrealQL file is derived directly from the existing SQLite DDL during implementation.

### Bootstrap flow

1. `connect()` succeeds → handle ready.
2. `apply_schema()` runs `DEFINE NAMESPACE memory_kg IF NOT EXISTS`, `USE NS memory_kg DB main`, then the SurrealQL bootstrap script via `db.query(include_str!("schema/memory_kg.surql"))`.
3. Read `_meta:version`. Absent → first launch, write current. Behind current → run upgrade closures. At current → skip.
4. Done. ~50 ms cold; effectively free warm.

### Layout

```
stores/zero-stores-surreal/
  src/
    connection.rs
    schema/
      mod.rs              (apply_schema entry point)
      memory_kg.surql     (table + index defs for memory_kg namespace)
      bootstrap.rs        (reads schema_version, applies upgrades)
    kg/
      mod.rs              (SurrealKgStore: impl KnowledgeGraphStore)
      entity.rs
      relationship.rs
      traverse.rs
      stats.rs
    memory/
      mod.rs              (SurrealMemoryStore: impl MemoryFactStore)
    types.rs              (Thing <-> EntityId conversions)
    error.rs
  AGENTS.md               (locked design decisions; see §3)
  Cargo.toml
```

### Illustrative SurrealQL sketch (non-canonical)

```sql
DEFINE TABLE entity SCHEMAFULL;
DEFINE FIELD agent_id        ON entity TYPE string;
DEFINE FIELD name            ON entity TYPE string;
DEFINE FIELD entity_type     ON entity TYPE string;
DEFINE FIELD mention_count   ON entity TYPE int DEFAULT 0;
DEFINE FIELD first_seen_at   ON entity TYPE datetime;
DEFINE FIELD epistemic_class ON entity TYPE string DEFAULT 'standard';
DEFINE FIELD embedding       ON entity TYPE option<array<float>>;
DEFINE INDEX entity_agent_name_type ON entity FIELDS agent_id, name, entity_type UNIQUE;

DEFINE TABLE relationship TYPE RELATION FROM entity TO entity SCHEMAFULL;
DEFINE FIELD agent_id          ON relationship TYPE string;
DEFINE FIELD relationship_type ON relationship TYPE string;
DEFINE FIELD mention_count     ON relationship TYPE int DEFAULT 0;
DEFINE INDEX rel_agent_type ON relationship FIELDS agent_id, relationship_type;

DEFINE ANALYZER ascii TOKENIZERS class FILTERS lowercase, ascii;
DEFINE INDEX entity_name_fts ON entity FIELDS name FULLTEXT ANALYZER ascii BM25;
```

(HNSW vector index is defined separately; see §6.)

### Migration of existing SQLite data

**Out of scope for this release.** No migration crate. First launch on the SurrealDB backend = empty DB. Switching backends in Settings shows a clear "this will start with empty knowledge graph + memory" warning.

---

## 6. Embeddings & HNSW Vector Index

### Constraint

SurrealDB's HNSW index requires `DIMENSION N` at index-define time. The existing system has runtime-changeable embedding dim (provider switch can move from 1024 → 1536 → 3072 mid-life).

### Approach: define-on-first-write, rebuild-on-change, idempotent across restarts

1. **First launch with no embeddings.** HNSW index is **not** defined. The `embedding` field exists as `option<array<float>>` but is unindexed. Vector search returns "no embeddings indexed".

2. **First embedding write.** Detect dim, write `_meta:embedding_config { dim: N }`, then issue `DEFINE INDEX entity_embedding_hnsw ON entity FIELDS embedding HNSW DIMENSION N DIST COSINE`. From here, vector queries hit the HNSW index.

3. **Subsequent restarts (dim known, index exists).** Bootstrap reads `_meta:embedding_config.dim` and issues `DEFINE INDEX entity_embedding_hnsw ON entity FIELDS embedding HNSW DIMENSION $dim DIST COSINE IF NOT EXISTS`. The `IF NOT EXISTS` makes this a no-op when the index already matches — **no rebuild on restart**.

4. **Embedding dim change** (provider swap):
   - Compare `new_dim` to `_meta:embedding_config.dim`. If equal → return `ReindexReport { rebuilt: false, ... }`. **No-op.**
   - If different → `REMOVE INDEX entity_embedding_hnsw ON entity`, clear stale embeddings (`UPDATE entity SET embedding = NONE WHERE array::len(embedding) != $new_dim`), update `_meta:embedding_config`, define new HNSW with new dim, schedule background re-embedding (existing job in `gateway-execution`).

5. **`vec_index_health()` trait method.** Reports HNSW index existence, current dim, row count. UI surfaces this on the embeddings panel exactly as today.

### Idempotency guarantee

- **Restart with matching dim:** zero rebuild, zero data movement. `IF NOT EXISTS` short-circuits; existing index is reused.
- **`reindex_embeddings(N)` when dim is already N:** returns immediately with `rebuilt: false`. Matches existing SQLite impl behavior.
- **The only triggers for an actual rebuild:** explicit dim change, or the corruption-recovery crate forcing one as part of recovery.

### Vector search query shape

```sql
SELECT *, vector::distance::knn() AS dist
FROM entity
WHERE embedding <|10,40|> $query_vec AND agent_id = $agent_id
ORDER BY dist;
```

`<|K,EF|>` is SurrealDB's KNN syntax (K results, EF candidate pool).

### FULLTEXT index for name search

Static — `DEFINE INDEX entity_name_fts ON entity FIELDS name FULLTEXT ANALYZER ascii BM25 IF NOT EXISTS` runs at bootstrap. Maps to existing FTS5 search behavior.

### Why not multi-dim coexistence

Provider switches in practice abandon old vectors (they were generated by a different model). Carrying multiple HNSW indexes per dim adds complexity for a transient state. Single-dim, rebuild-on-change is simpler and matches existing behavior.

---

## 7. Bootstrap, First Launch & Corruption Recovery

### First launch (no DB file)

1. `connect("rocksdb://$VAULT/data/knowledge.surreal")` — RocksDB engine creates the directory and an empty store. ~20 ms.
2. `apply_schema()` issues `DEFINE NAMESPACE memory_kg IF NOT EXISTS` and `DEFINE DATABASE main IF NOT EXISTS` (explicit, idempotent), then `use_ns("memory_kg").use_db("main")`.
3. Same `apply_schema()` call runs the bootstrap SurrealQL — all `DEFINE TABLE/FIELD/INDEX IF NOT EXISTS`. ~30 ms total.
4. `_meta:version` written with current schema version.
5. Daemon serves traffic. Empty memory + KG; agent rebuilds from interactions.

### Subsequent launches (DB exists, valid)

1. `connect()` opens the RocksDB directory.
2. `use_ns/use_db` selects existing namespace.
3. `apply_schema()` runs — every statement is idempotent.
4. `_meta:version` read; if behind, run upgrade closures; if at current, skip.
5. Done. ~50 ms cold (RocksDB cache warm).

### Corruption (RocksDB error on open or first health check)

Detection point: `connect()` or first `db.query("INFO FOR DB")` health probe at startup.

1. **Log loudly** — error level, full RocksDB error, path to DB directory.
2. **Refuse to start** — daemon exits non-zero with: "knowledge.surreal at $PATH appears corrupted. Run `agentzero recover-knowledge` to attempt repair, or move the directory aside to start fresh."
3. **No silent fallback** to SQLite or empty DB.

### Placeholder recovery crate

`stores/zero-stores-surreal-recovery/` exposes a single CLI-invoked function:

```rust
pub fn recover_knowledge_db(path: &Path) -> Result<RecoveryReport>
```

Initial impl does the simplest useful thing:

1. Try to open with RocksDB read-only.
2. On success → export entities/relationships to JSON sidecar.
3. Move corrupted directory aside (`knowledge.surreal.corrupted-{timestamp}`).
4. Return report naming the sidecar.

**Not wired into daemon startup.** A CLI subcommand the user invokes manually. The daemon failing to start is the trigger.

### Disk full

RocksDB returns IO errors on writes. Stores translate to `StoreError::Backend` (same shape as SQLite `SQLITE_FULL`). UI surfaces a banner. Agent request fails gracefully; daemon does not crash.

### Concurrent access (Mode A)

RocksDB enforces a process-level lock. Second daemon instance fails fast at `connect()` with "lock held by PID X" and exits cleanly. No corruption risk.

---

## 8. Trait Implementation Shape

The existing `KnowledgeGraphStore` (~25 methods) and `MemoryFactStore` (~10 methods) get full SurrealDB implementations. All callers — agent tools (`memory.search`, `query_graph`, `extract_knowledge`), HTTP handlers (`graph.rs`, `memory.rs`, `embeddings.rs`, `ward_content.rs`), executor jobs (`OrphanArchiver`, `EmbeddingReindexer`, `GraphIngestor`) — already consume the trait objects after PRs #71–#83. They don't change.

### Reads — direct `db.query()` with bound params

```rust
async fn get_entity(&self, id: &EntityId) -> StoreResult<Option<Entity>> {
    let mut resp = self.db
        .query("SELECT * FROM entity WHERE id = $id LIMIT 1")
        .bind(("id", id.to_thing()))
        .await
        .map_err(StoreError::backend)?;
    let row: Option<EntityRow> = resp.take(0).map_err(StoreError::backend)?;
    Ok(row.map(Into::into))
}
```

### Atomic writes — `BEGIN/COMMIT` blocks in a single query

```rust
async fn store_knowledge(&self, agent_id: &str, k: ExtractedKnowledge) -> StoreResult<StoreOutcome> {
    self.db.query(r#"
        BEGIN;
        FOR $e IN $entities {
            UPSERT entity SET
                agent_id = $agent_id,
                name = $e.name,
                entity_type = $e.entity_type,
                mention_count += 1
            WHERE agent_id = $agent_id AND name = $e.name AND entity_type = $e.entity_type;
        };
        FOR $r IN $rels {
            RELATE $r.from -> relationship -> $r.to SET
                agent_id = $agent_id,
                relationship_type = $r.relationship_type,
                mention_count += 1;
        };
        COMMIT;
    "#)
        .bind(("agent_id", agent_id))
        .bind(("entities", k.entities))
        .bind(("rels", k.relationships))
        .await?;
    // ... build StoreOutcome from response
}
```

The 3 transaction-safety bugs fixed in `GraphStorage` during Phase 1 (delete_entity, store_knowledge, archiver) all map to `BEGIN/COMMIT` blocks here — SurrealDB engine guarantees atomicity within a single query.

### Graph traversal — native graph operators

```rust
async fn traverse(&self, seed: &EntityId, max_hops: usize, limit: usize) -> StoreResult<Vec<TraversalHit>> {
    let mut resp = self.db
        .query("SELECT id, ->relationship->entity.* AS neighbors FROM $seed FETCH neighbors")
        .bind(("seed", seed.to_thing()))
        .await?;
    // ...
}
```

`->relationship->entity` replaces the manual recursive CTE used in SQLite. First-class query, no N+1 in app code.

### Vector search

```rust
async fn search_entities_by_embedding(&self, agent_id: &str, query_vec: &[f32], k: usize)
    -> StoreResult<Vec<EntityHit>>
{
    let mut resp = self.db
        .query(r#"
            SELECT *, vector::distance::knn() AS dist
            FROM entity
            WHERE embedding <|$k,40|> $vec AND agent_id = $agent_id
            ORDER BY dist
        "#)
        .bind(("vec", query_vec.to_vec()))
        .bind(("agent_id", agent_id))
        .bind(("k", k))
        .await?;
    // ...
}
```

### Type bridging

- `EntityId(String)` ↔ SurrealDB `Thing` — conversion in `types.rs`. `Thing` does not leak to callers.
- `chrono::DateTime<Utc>` ↔ SurrealDB `datetime` via serde. No manual conversion.
- `Vec<f32>` ↔ SurrealDB `array<float>` directly.

### Transaction note

SurrealDB transactions are `BEGIN/COMMIT` blocks inside a single `db.query()` call. There is no separate `tx.commit()` in the SDK. This is the SurrealDB equivalent of the per-call transaction wrapping we use in SQLite.

---

## 9. Mode A → Mode B Migration Path (designed-in, not built)

### Already designed in (today)

1. **`Surreal<Any>` engine-erased type** used everywhere. Switching `connect("rocksdb://...")` → `connect("ws://127.0.0.1:PORT")` returns the same type. Stores, traits, services, agent tools — none of them know which engine is underneath.
2. **URL-string config** in `settings.json`. The schema (`{ url, namespace, database, credentials }`) covers Mode B already. Mode A users have `credentials: null` and a `rocksdb://` URL; Mode B users flip both.
3. **Single construction site** in `connection.rs::connect()` (~10 lines). Already handles credentials path. Mode B requires zero changes here.
4. **No port assumptions in stores.** No code asks "what port" or "are we embedded or remote." A code-review block on adding any such check.
5. **No filesystem assumptions in stores.** Stores never touch the `knowledge.surreal` directory directly — only through the `Surreal<Any>` handle. Mode B (separate process owns the directory) doesn't break anything.
6. **Conformance crate** runs against `connect("mem://")` for tests. Same suite will run against `connect("ws://...")` when Mode B lands — no test rewrites, just a new factory.

### What Mode B will need to add (future work, ~2–3 days)

1. **Supervisor crate** at `gateway/gateway-surreal-supervisor/` (~150–300 LoC):
   - Spawns `surreal start --bind 127.0.0.1:PORT --user $u --password $p rocksdb://$VAULT/data/knowledge.surreal`.
   - Captures stdout/stderr to `$VAULT/logs/surreal.log`.
   - Restarts on crash with exponential backoff.
   - SIGTERM on daemon shutdown; SIGKILL after grace period.
2. **Health probe** before gateway serves HTTP — wait for `connect("ws://127.0.0.1:PORT")` + `db.query("INFO FOR DB")` to succeed. Timeout 5 s.
3. **Port allocation** — fixed port from settings, fall-forward on conflict. Localhost-only on all platforms (no UDS, even where supported).
4. **Credentials generation** — auto-generate per-install random root password, persist in `$VAULT/.surreal-credentials` (mode 0600 on Unix), pass to both supervisor and daemon's `connect()`.
5. **Config flag flip** — `surreal.url` changes from `rocksdb://...` to `ws://127.0.0.1:PORT`. UI exposes a "Run SurrealDB as separate process" toggle that flips both `url` and `credentials`.
6. **Conversations expansion** (further future) — when conversations migrate, `NAMESPACE conversations` is created in the same instance. `use_ns/use_db` switches per query. No topology change.

### Net migration cost when Mode B is triggered

New supervisor crate + UI toggle + ~5 lines of config. Daemon code, store code, agent tools, HTTP handlers — unchanged.

---

## 10. Quality Bar — Clean Code + ~90% Unit Test Coverage

**This is a hard requirement, not a nice-to-have.**

### Clean Code

- Every function under cognitive complexity 15 (per `.claude/rules/rust-code-quality.md`).
- No `unwrap()` outside tests/setup.
- No `as` casts where `TryFrom`/`TryInto` works.
- Each file has one clear responsibility (per `.claude/rules/typescript-complexity.md` philosophy applied to Rust).
- Module boundaries match data domain: `kg/entity.rs`, `kg/relationship.rs`, `kg/traverse.rs`, `memory/`, etc. — not "layered by technical concern."
- Files that change together live together.

### Test coverage target: ≥ 90% line coverage in `stores/zero-stores-surreal/` (per `cargo llvm-cov` line metric)

- Every `KnowledgeGraphStore` and `MemoryFactStore` trait method has at least 2 unit tests: happy path + at least one edge case (empty input, dim mismatch, missing entity, atomicity failure, agent isolation).
- Tests use `connect("mem://")` — in-memory engine, no file I/O, parallel-safe.
- Schema bootstrap tested independently: idempotency, version upgrade closure invocation, HNSW define-on-first-write.
- Type bridging (`Thing` ↔ `EntityId`, datetime, embeddings) has dedicated round-trip tests.
- Error mapping (`surrealdb::Error` → `StoreError`) is tested per error kind.
- The recovery crate has its own test suite covering: read-only open success, JSON sidecar export, corrupted-directory rename.

### Coverage measurement

`cargo llvm-cov --workspace --html` reports per-crate coverage. Implementation plan includes a coverage gate task: if `zero-stores-surreal` reports under 90% line coverage, the task is not done.

### Conformance scenarios as the parity bar

The ~30 conformance scenarios in §11 prove functional parity, not coverage. Both bars apply: 90% line coverage in unit tests **and** all conformance scenarios green on both backends.

---

## 11. Testing & Conformance

### Conformance crate shape (existing, fattens)

```rust
// stores/zero-stores-conformance/src/lib.rs
pub fn run_kg_suite<F, Fut>(factory: F) -> impl Future
where F: Fn() -> Fut, Fut: Future<Output = Arc<dyn KnowledgeGraphStore>>;

pub fn run_memory_suite<F, Fut>(factory: F) -> impl Future
where F: Fn() -> Fut, Fut: Future<Output = Arc<dyn MemoryFactStore>>;
```

Each backend imports and runs both suites against its own factory:

```rust
// stores/zero-stores-sqlite/tests/conformance.rs
#[tokio::test] async fn kg_suite() {
    zero_stores_conformance::run_kg_suite(|| async { build_sqlite_kg_store().await }).await;
}

// stores/zero-stores-surreal/tests/conformance.rs
#[tokio::test] async fn kg_suite() {
    zero_stores_conformance::run_kg_suite(|| async { build_surreal_kg_store_in_memory().await }).await;
}
```

Surreal version uses `connect("mem://")` — in-memory, no file I/O, parallel-safe.

### KnowledgeGraphStore scenarios (~20)

1. `entity_round_trip`
2. `entity_upsert_increments_mention_count`
3. `alias_resolution_returns_canonical_entity`
4. `alias_resolution_via_embedding_similarity`
5. `relationship_round_trip`
6. `delete_entity_cascades_to_relationships_atomically` (catches TD-001-class bug)
7. `store_knowledge_is_atomic_on_partial_failure`
8. `traverse_respects_max_hops`
9. `traverse_respects_limit`
10. `get_neighbors_direction_in_out_both`
11. `search_entities_by_name_fts_ranking`
12. `search_entities_by_embedding_returns_knn`
13. `reindex_embeddings_idempotent_when_dim_unchanged`
14. `reindex_embeddings_clears_and_rebuilds_on_dim_change`
15. `list_archivable_orphans_filters_correctly`
16. `mark_entity_archival_atomic`
17. `graph_stats_per_agent`
18. `vec_index_health_reports_truthfully`
19. `subgraph_bfs_centered_on_entity`
20. `cross_agent_isolation` (agent_id filter honored everywhere)

### MemoryFactStore scenarios (~10)

Store, get, delete, search round-trips; decay; agent isolation; embedding search; archive lifecycle.

### Gate

SurrealDB backend ships behind Cargo feature `surreal-backend` and `knowledge_backend = "surreal"` config until **all conformance scenarios pass on both backends and `cargo llvm-cov` reports ≥ 90% line coverage on `stores/zero-stores-surreal/`**. SQLite stays default. No user is silently moved.

### Smoke testing the live daemon

- `scripts/zai_rate_probe.py`-style harness adapted to flip `knowledge_backend` between runs and call the same agent-tool/HTTP-API surface. Quick parity check beyond unit + conformance suites.
- UI smoke: open Memory, Knowledge Graph, Wiki pages on the SurrealDB backend with seeded data → visual diff against SQLite.

### What conformance does *not* cover (and is fine)

- Performance — SurrealDB will be faster on graph traversal, slower on point lookups. We don't gate on benchmarks.
- Concurrent-write contention — Mode A is single-process; Mode B is the time to add multi-writer scenarios.
- SQLite-data migration — out of scope.

---

## 12. AppState & persistence_factory Wiring

### Today (after PRs #71–#83)

`AppState` already holds `kg_store: Arc<dyn KnowledgeGraphStore>` and `memory_store: Arc<dyn MemoryFactStore>` alongside the concrete SQLite repos. `persistence_factory` builds them from SQLite-backed `GraphStorage` / `GatewayMemoryFactStore`. All HTTP handlers, agent tools, and executor jobs already consume the trait objects.

### Change

`persistence_factory.rs` grows a backend selector:

```rust
// gateway/src/state/persistence_factory.rs
pub async fn build_kg_store(cfg: &PersistenceConfig, ...) -> Result<Arc<dyn KnowledgeGraphStore>> {
    match cfg.knowledge_backend {
        KnowledgeBackend::Sqlite => build_sqlite_kg_store(...).await,
        KnowledgeBackend::Surreal => build_surreal_kg_store(&cfg.surreal).await,
    }
}

async fn build_surreal_kg_store(cfg: &SurrealConfig) -> Result<Arc<dyn KnowledgeGraphStore>> {
    let db = zero_stores_surreal::connect(cfg).await?;
    zero_stores_surreal::schema::apply_schema(&db).await?;
    Ok(Arc::new(zero_stores_surreal::SurrealKgStore::new(db)))
}
```

Same shape for `build_memory_store`. Both Surreal stores share the **same** `Arc<Surreal<Any>>` handle — one connection, two store impls multiplexing.

### `AppState` lifecycle on Surreal backend

- `kg_store` and `memory_store` hold `Arc<SurrealKgStore>` / `Arc<SurrealMemoryStore>`, each wrapping the shared `Arc<Surreal<Any>>`.
- The legacy `graph_service`, `memory_repo`, `knowledge_db` fields on `AppState` (still present today for the few not-yet-migrated handlers) become **`Option<...>` and `None`** on Surreal backend. Any handler still consuming them is a known gap to close before Surreal is wired.
- Drop order: stores drop → last `Arc<Surreal<Any>>` drops → SDK closes connection → RocksDB releases file lock. No special teardown.

### What does *not* change

HTTP layer, agent tools, executor jobs, SessionInvoker, ExecutionRunner, OrphanArchiver, EmbeddingReindexer — all already trait-typed. They take `Arc<dyn KnowledgeGraphStore>` and call methods. Backend swap is transparent to them.

### Default & rollout

- `knowledge_backend` defaults to `"sqlite"`. No silent moves.
- `"surreal"` ships behind Cargo feature `surreal-backend` initially — SurrealDB SDK is not pulled into stable builds until the feature is default-on.
- Once conformance is green on both backends and 90% coverage is met, the feature becomes default-on (still opt-in via config).
- Settings UI: "Backend" dropdown under Advanced → Persistence. Switching = config write + daemon restart prompt. **No automatic data copy** — switching backends gives you an empty store on the new side, with a clear warning.

---

## 13. Out-of-Scope (explicit list)

- Migration of existing SQLite knowledge.db data
- Conversations migration (reserved as `NAMESPACE conversations`, not implemented)
- Mode B (subprocess sidecar) — designed-in, not built
- Performance benchmarking
- Multi-process / cluster deployments
- Auto-recovery loops on corruption (placeholder crate is CLI-invoked)

---

## 14. Risks & Mitigations

| Risk | Mitigation |
|---|---|
| SurrealDB 3.0 SDK churn before GA | Pin `surrealdb = "=3.0.x"` in Cargo.toml; pin RocksDB feature flags. Update deliberately. |
| HNSW index rebuild on every restart (perf foot-gun) | `IF NOT EXISTS` makes it idempotent (§6); explicitly tested in conformance scenario 13. |
| RocksDB lock conflicts during dev (two daemons) | Mode A relies on RocksDB's process-lock; daemon fails fast and exits. Documented in AGENTS.md. |
| `BEGIN/COMMIT` query blocks behave differently than SQLite tx wrappers | Conformance scenarios 6, 7, 16 test atomicity explicitly. |
| Schema drift between SurrealQL and SQLite DDL | The SurrealQL file is derived 1:1 from SQLite DDL during implementation; conformance suite catches drift. |
| Coverage tool (`cargo llvm-cov`) flaky on async code | Plan task includes a coverage-gate step; if tool drops below 90% spuriously, investigate before declaring done. |
| Pi/SSD path expansion edge cases (`$VAULT` placeholder) | Dedicated unit tests for path expansion; test on Linux + macOS + Windows path conventions. |

---

## 15. Acceptance Criteria

- [ ] `stores/zero-stores-surreal/` crate compiles, lints clean, ≥ 90% line coverage (`cargo llvm-cov` per-crate report).
- [ ] `stores/zero-stores-surreal/AGENTS.md` captures all locked decisions from this spec.
- [ ] `stores/zero-stores-surreal-recovery/` crate exists with placeholder impl + tests.
- [ ] All ~30 conformance scenarios pass on both SQLite and SurrealDB backends.
- [ ] `persistence_factory` selects backend from config; both paths covered by integration tests.
- [ ] HNSW idempotency on restart explicitly verified.
- [ ] Smoke harness flips backends end-to-end without UI/API regressions.
- [ ] `cargo fmt --all --check` and `cargo clippy --all-targets -- -D warnings` clean.
- [ ] Daemon refuses to start (with clear error) on corrupt RocksDB; CLI recovery subcommand wired.
- [ ] Settings UI exposes "Backend" dropdown + warning on switch.

---

## 16. Implementation-Plan Handoff

This spec feeds directly into `superpowers:writing-plans`. The plan will decompose §3–§12 into bite-sized, TDD-shaped tasks with exact file paths, code snippets, and commit boundaries. The plan is the next deliverable; this design is the contract it implements against.
