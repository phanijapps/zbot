# Persistence Readiness Design — Knowledge Layer Abstraction & SurrealDB Sidecar

**Status:** approved (brainstorm complete, ready for implementation planning)
**Branch:** `feature/persistence-readiness`
**Related:** [`memory-bank/tech-debt.md`](../tech-debt.md) — fix plan and item registry

---

## Goal

Make the knowledge-side persistence layer **plug-and-play between backends**, so that a future swap from SQLite + sqlite-vec + FTS5 to SurrealDB (or anything else) is one new crate plus a config switch — with no edits required in `gateway-execution/`, `services/*`, agent tools, or HTTP handlers.

The conversations side stays SQLite indefinitely. Only the *knowledge* side (memory facts + knowledge graph) is being abstracted and prepared for SurrealDB.

This document captures the agreed design. The implementation plan lives in `memory-bank/tech-debt.md` (phased) and will be expanded by a forthcoming writing-plans output.

---

## Architecture overview

### Process layout

```
apps/daemon (long-running parent)
  ├─ HTTP/WS API           ◄── apps/ui (web frontend)
  ├─ HTTP/WS API           ◄── apps/cli (and future apps/mcp)
  ├─ ConversationStore     ──► conversations.db (SQLite, embedded library)
  ├─ KnowledgeGraphStore   ──► UDS / loopback (Mode B)
  ├─ MemoryFactStore       ──►   │
  └─ supervises ─────────────────┤
                                 ▼
              ┌────────────────────────────────┐
              │ surreal start (child process)  │
              │  bound to UDS / loopback       │
              │  data: $VAULT/data/knowledge.surreal/
              └────────────────────────────────┘
```

### Storage split (final form)

| Subsystem | Backend | Reason |
|---|---|---|
| Sessions, agent_executions, messages, execution_logs, artifacts, bridge_outbox, distillation_runs, recall_log, archived sessions, embedding_cache | **SQLite** (`conversations.db`, embedded library) | No graph/vector needs; stable; single-writer-via-pool works fine |
| Memory facts (current, ctx, primitives), fact archive, fact FTS index, fact vector index | **SurrealDB sidecar** (knowledge.surreal) | Vector ANN, FTS, scale path, future-proofing |
| KG entities, relationships, aliases, name embeddings, causal edges, episodes, procedures, ward wiki articles, goals | **SurrealDB sidecar** | Native graph traversal, vector for entity resolution, FTS for wiki |

### Crate layout

```
zero-stores/                  ← interface crate. NO db drivers.
  src/
    error.rs                  StoreError + StoreResult
    knowledge_graph.rs        KnowledgeGraphStore trait + types
    memory_facts.rs           MemoryFactStore trait (relocated from zero-core)
    embedding.rs              EmbeddingClient trait (re-exported)
    types/                    Entity, Relationship, MemoryFact, RecallResult, …

zero-stores-sqlite/           ← current behavior, refactored into trait impls
  src/
    knowledge_graph.rs        impl KnowledgeGraphStore for SqliteKgStore
    memory_facts.rs           impl MemoryFactStore for SqliteMemoryStore
    bootstrap.rs              idempotent CREATE TABLE IF NOT EXISTS …

zero-stores-surreal/          ← new
  src/
    supervisor.rs             SurrealSupervisor (process lifecycle)
    knowledge_graph.rs        impl KnowledgeGraphStore for SurrealKgStore
    memory_facts.rs           impl MemoryFactStore for SurrealMemoryStore
    bootstrap.rs              idempotent DEFINE TABLE IF NOT EXISTS …
    schema/                   .surql files

zero-stores-conformance/      ← cross-impl test scenarios (library, not test target)
  src/lib.rs                  pub async fn upsert_then_recall<S: …>(s: &S) { … }

apps/daemon
  └─ AppState holds Arc<dyn KnowledgeGraphStore>, Arc<dyn MemoryFactStore>
     resolved by PersistenceFactory based on config
```

The daemon depends only on `zero-stores`. **No business code anywhere imports `rusqlite` or `surrealdb` directly.** The factory in `apps/daemon` constructs the chosen impl from config.

### Single-supervisor invariant

- **Only `apps/daemon` may launch the SurrealDB child.**
- `apps/cli` and future `apps/mcp` (or any other client) talk to the **daemon's HTTP/WS API** — they never connect to SurrealDB directly.
- Headless CLI invocation (script / cron / pipeline use) is solved at a different layer: CLI ensures the daemon is running (deferred to apps/cli design — does not affect the store layer).

This keeps the SurrealDB consumer to exactly one process, eliminating multi-supervisor races and file-lock contention.

### Storage location guidance

Knowledge data lives on **a local block device** (Pi-attached SSD, USB-NVMe, internal disk). File-engine databases (RocksDB, SurrealKV) require reliable `fsync` and reliable file locking — network filesystems generally provide neither, with silent corruption as the failure mode. If shared-across-machines knowledge access is ever needed, graduate to networked SurrealDB server mode on the storage host (no application-code change — just a different connection URL).

---

## Supervisor lifecycle

The `SurrealSupervisor` lives in `zero-stores-surreal` and is owned by the daemon's `AppState`.

### Startup sequence

```
daemon::main
  └─► AppState::new(config)
        ├─ open SQLite (conversations.db)
        ├─ if knowledge_backend == Surreal:
        │     ├─ ensure $VAULT/data/knowledge.surreal/ exists
        │     ├─ ensure $XDG_RUNTIME_DIR/agentzero/ exists with 0700
        │     ├─ generate per-boot random root password
        │     ├─ resolve `surreal` binary location (PATH, then $VAULT/bin/)
        │     ├─ tokio::process::Command::new("surreal")
        │     │     .args([
        │     │        "start", "--log", "info",
        │     │        "--bind", "<UDS path or loopback>",
        │     │        "--user", "agentzero",
        │     │        "--pass", "<random-per-boot>",
        │     │        "rocksdb://$VAULT/data/knowledge.surreal",
        │     │     ])
        │     │     .stdout(piped()).stderr(piped()).kill_on_drop(true)
        │     │     .spawn()?
        │     ├─ spawn log-forwarder tasks (stdout/stderr → tracing)
        │     ├─ wait_until_ready (poll connect, ≤5 s)
        │     └─ return SurrealSupervisor { child, endpoint, creds }
        ├─ open SurrealDB client (single long-lived connection over UDS/loopback)
        ├─ run schema bootstrap (idempotent DEFINE … IF NOT EXISTS)
        ├─ assemble Arc<dyn KnowledgeGraphStore>, Arc<dyn MemoryFactStore>
        └─ start HTTP server (only after stores are ready)
```

The HTTP API does not open until the supervisor is healthy and the schema is in place. First-launch case (no data dir yet) is handled transparently — daemon creates the directory and runs bootstrap.

### Endpoint convention

| OS | Bind target | Discovery |
|---|---|---|
| Linux/macOS | `unix:$XDG_RUNTIME_DIR/agentzero/surreal.sock` (fallback `$VAULT/.run/surreal.sock`) | Fixed path; daemon picks, daemon connects |
| Windows | `127.0.0.1:<port>` where port is daemon-picked via brief `TcpListener::bind("127.0.0.1:0")` then released | Pass to child as `--bind`; tiny race window, acceptable |

### Binary acquisition

- Look for `surreal` on `PATH`
- Fallback: `$VAULT/bin/surreal`
- Else: daemon refuses to start with a clear install message
- Auto-download via an `agentzero install-deps` subcommand is deferred (not part of this design)

For Pi: the linux-arm64 (or linux-armv7l for older Pi) build from official SurrealDB releases is fetched once and placed in `$VAULT/bin/`. Installation tooling is a deployment concern, not a runtime concern.

### Ready detection

Poll `Surreal::new::<Ws>(endpoint)` (or `Wss`) every 50 ms up to 5 s. First successful auth = ready. Each retry logged. Hard-fail with an explicit error if the child doesn't come up — never block boot indefinitely.

### Crash detection & restart

A `tokio::spawn`'d watcher does `child.wait().await`. On unexpected exit:

1. Log full stderr tail + exit code at `error!`
2. Mark `SurrealSupervisor` as `Restarting`; store calls return `StoreError::Unavailable { retry_after: Some(…) }` for the duration
3. Restart with exponential backoff: 100 ms → 500 ms → 2 s → 5 s → 30 s (capped)
4. After **5 consecutive crashes within 60 s**, stop restarting and transition to `Failed`. Surface to daemon's `/health` endpoint
5. On successful restart, re-run schema bootstrap (idempotent) and reopen the client

The daemon **does not crash when SurrealDB does**. Stores return errors, the API can degrade gracefully, restart proceeds in the background.

### Graceful shutdown

```
daemon receives SIGTERM
  ├─ stop accepting new HTTP requests, drain in-flight
  ├─ close SurrealDB client cleanly
  ├─ supervisor.shutdown():
  │     ├─ child.signal(SIGTERM) [Unix] / TerminateProcess [Windows]
  │     ├─ wait up to 10 s
  │     └─ child.kill() if still alive
  └─ exit
```

`Command::kill_on_drop(true)` is a backstop — even if `Drop` runs without explicit shutdown, the child dies with the daemon. No orphan SurrealDB processes.

### Logging

Child stdout/stderr captured line-by-line and forwarded via `tracing::info!(target = "surrealdb", "{line}")`. Same span context as daemon logs so correlation works in the existing logging pipeline.

### Supervisor state observability

```rust
enum SupervisorState {
    Starting,
    Healthy,
    Restarting { since: Instant, attempt: u32 },
    Failed { reason: String, since: Instant },
}
```

Published via `tokio::sync::watch`. Daemon's `/health` endpoint reads it directly. Stores subscribe and short-circuit to `Unavailable` during `Restarting`/`Failed` without round-tripping to a dead socket. The watch channel powers a status indicator in the UI ("Knowledge layer: 🟢 Healthy" / "🟡 Restarting" / "🔴 Down").

---

## Trait surface

The complete public API exposed by `zero-stores`. Async, domain-typed, no SQL, no `Connection`, no `&mut`, no closures.

### `KnowledgeGraphStore`

```rust
#[async_trait]
pub trait KnowledgeGraphStore: Send + Sync {
    // ---- Entities --------------------------------------------------------
    async fn upsert_entity(&self, agent_id: &str, entity: Entity) -> StoreResult<EntityId>;
    async fn get_entity(&self, id: &EntityId) -> StoreResult<Option<Entity>>;
    async fn delete_entity(&self, id: &EntityId) -> StoreResult<()>;        // cascades to edges
    async fn bump_entity_mention(&self, id: &EntityId) -> StoreResult<()>;

    // ---- Aliases & resolution -------------------------------------------
    async fn add_alias(&self, entity_id: &EntityId, surface: &str) -> StoreResult<()>;
    async fn resolve_entity(
        &self,
        agent_id: &str,
        entity_type: &EntityType,
        name: &str,
        embedding: Option<&[f32]>,        // None ⇒ exact-alias only
    ) -> StoreResult<ResolveOutcome>;     // Match(EntityId) | NoMatch

    // ---- Relationships --------------------------------------------------
    async fn upsert_relationship(
        &self,
        agent_id: &str,
        rel: Relationship,
    ) -> StoreResult<RelationshipId>;
    async fn delete_relationship(&self, id: &RelationshipId) -> StoreResult<()>;

    // ---- Bulk ingest ----------------------------------------------------
    /// Atomic: either all entities + relationships land or none do.
    async fn store_knowledge(
        &self,
        agent_id: &str,
        knowledge: ExtractedKnowledge,
    ) -> StoreResult<StoreOutcome>;

    // ---- Read paths -----------------------------------------------------
    async fn get_neighbors(
        &self,
        id: &EntityId,
        direction: Direction,
        limit: usize,
    ) -> StoreResult<Vec<Neighbor>>;

    async fn traverse(
        &self,
        seed: &EntityId,
        max_hops: usize,
        limit: usize,
    ) -> StoreResult<Vec<TraversalHit>>;

    async fn search_entities_by_name(
        &self,
        agent_id: &str,
        query: &str,
        limit: usize,
    ) -> StoreResult<Vec<Entity>>;

    // ---- Maintenance ----------------------------------------------------
    async fn reindex_embeddings(&self, new_dim: usize) -> StoreResult<ReindexReport>;
    async fn stats(&self) -> StoreResult<KgStats>;
}
```

### `MemoryFactStore`

```rust
#[async_trait]
pub trait MemoryFactStore: Send + Sync {
    async fn save_fact(&self, fact: MemoryFact) -> StoreResult<FactId>;
    async fn delete_facts_by_key(&self, agent_id: &str, key: &str) -> StoreResult<usize>;

    /// Hybrid recall (BM25 + vector + RRF) — single round-trip on either backend.
    async fn recall(
        &self,
        agent_id: &str,
        ward_id: &str,
        query_embedding: &[f32],
        query_text: Option<&str>,
        limit: usize,
        config: &RecallConfig,
    ) -> StoreResult<Vec<RecallHit>>;

    async fn recall_prioritized(
        &self,
        agent_id: &str,
        ward_id: &str,
        query_embedding: &[f32],
        config: &PrioritizedRecallConfig,
    ) -> StoreResult<Vec<RecallHit>>;

    async fn get_ctx_fact(
        &self,
        session_id: &str,
        ward_id: &str,
        key: &str,
    ) -> StoreResult<Option<MemoryFact>>;

    async fn save_ctx_fact(&self, fact: MemoryFact) -> StoreResult<()>;
    async fn upsert_primitive(&self, primitive: Primitive) -> StoreResult<()>;
    async fn stats(&self) -> StoreResult<MemoryStats>;
}
```

### Error type

```rust
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("not found")]
    NotFound,
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("backend unavailable (retry hint: {retry_after:?})")]
    Unavailable { retry_after: Option<Duration> },
    #[error("schema error: {0}")]
    Schema(String),
    #[error("backend error: {0}")]
    Backend(String),
    #[error("invalid input: {0}")]
    Invalid(String),
}
pub type StoreResult<T> = Result<T, StoreError>;
```

### Vector ops folded in (TD-013 resolved)

There is **no public `VectorIndex` trait**. Vector operations are part of `MemoryFactStore::recall` and `KnowledgeGraphStore::resolve_entity`. The SQLite impl keeps `SqliteVecIndex` internally as an implementation detail; the SurrealDB impl uses HNSW indexes inline on the record. Both backends honor the contract; neither leaks its mechanism.

### Transactions deliberately not exposed

There is no public `Tx` handle. Multi-op operations that require atomicity are single trait methods (`store_knowledge`, `archive_session`). The impl handles the transaction internally. Reasons:

- Cross-backend transaction shapes do not compose (SQLite begin/commit vs. SurrealDB BEGIN/CANCEL/COMMIT inside SurQL block)
- Exposing `Tx` leaks lifetime contracts neither backend wants to honor identically
- Audit of current code shows the few sites that need atomicity (e.g., the archiver bug TD-001) become single named operations cleanly

### Async story

All trait methods are `async`. SQLite impl uses `tokio::task::spawn_blocking` to avoid blocking the runtime. SurrealDB impl is natively async. No mixed sync/async surface.

### Out of scope for the trait

- Migration management (no `Migrator` trait — see "What we are explicitly not doing")
- Health probes (supervisor concern, exposed via daemon's `/health` endpoint)
- Connection pool config (internal to each impl)
- Backup / export

---

## Data model mapping

Both impls implement the same trait surface. The difference is how records sit on disk.

### Knowledge graph

| Logical entity | Today (SQLite + sqlite-vec + FTS5) | Future (SurrealDB) |
|---|---|---|
| Entity | row in `kg_entities` + row in `kg_name_index` (vec0) | record in `entity` SCHEMALESS table; `name_embedding` is a field; HNSW index on the field |
| Relationship | row in `kg_relationships` (FKs to entities) | `RELATE entity:a -> related -> entity:b SET …` (typed graph edge) |
| Alias | row in `kg_aliases` with FK to entity | record in `alias` table with a record-link to the entity; UNIQUE index on `(agent_id, entity_type, normalized_form)` |
| Causal edge | row in `kg_causal_edges` | `RELATE entity:cause -> causes -> entity:effect SET …` |
| Episode | row in `kg_episodes` + `kg_episode_payloads` | record in `episode` table; payload as nested object |
| Compaction log | row in `kg_compactions` | record in `kg_compaction` table |

**Aliases stay separate, not embedded** — reverse lookup (alias → entity) is the dominant query (entity resolution stage 1), and a UNIQUE constraint across all aliases is needed for dedup correctness.

### Memory facts

| Logical entity | Today | Future |
|---|---|---|
| Fact | row in `memory_facts` + row in `memory_facts_index` (vec0) | record in `fact` SCHEMALESS table; `embedding` is a field; HNSW index on the field |
| Fact FTS | `memory_facts_fts` FTS5 contentless table + sync triggers | SEARCH analyzer (`DEFINE ANALYZER`) + BM25 index on `fact.content` |
| Fact archive | `memory_facts_archive` (no embedding) | record in `fact_archive` table; same shape, no HNSW index |
| Ctx fact | row in `memory_facts` with `agent_id='__ctx__'`, `scope='session'` | record in `fact` with same convention; same table, same trait method |
| Primitive | row in `memory_facts` with `category='primitive'`, `agent_id='__ward__'` | same convention |

**Single `fact` table covers facts, ctx-facts, primitives** — same as today. The trait methods (`save_fact`, `save_ctx_fact`, `upsert_primitive`) discriminate via the records' `scope` / `agent_id` / `category` fields, not separate physical tables.

### Ancillary

| Logical entity | Today | Future |
|---|---|---|
| Skill index state | `skill_index_state` table | `skill_index` table |
| Procedure | `procedures` | `procedure` |
| Ward wiki article | `ward_wiki_articles` + `ward_wiki_articles_fts` FTS5 | `wiki` table with SEARCH analyzer for FTS |
| Goal | `kg_goals` | `goal` |
| Session episode | `session_episodes` + `session_episodes_index` (vec0) | `session_episode` with HNSW index on embedding field |

### Stays SQLite

| Item | Reason |
|---|---|
| `embedding_cache` (SHA256 → embedding BLOB) | Pure side cache, not "knowledge"; sits behind `EmbeddingClient`, not the store traits. No graph/vector features needed. |
| `recall_log` | Per-session bookkeeping for which facts were surfaced; tied to session lifecycle. |
| All conversations tables (sessions, messages, executions, logs, artifacts, outbox, distillation_runs) | Stable, single-writer-via-pool works fine, no graph/vector needs. |

### SurrealDB schema sketch (illustrative, not normative)

This is the *shape*, not the spec — final DDL is verified against SurrealDB 3.0 docs at impl time.

```surql
DEFINE TABLE entity SCHEMALESS;
DEFINE FIELD agent_id        ON entity TYPE string;
DEFINE FIELD entity_type     ON entity TYPE string;
DEFINE FIELD name            ON entity TYPE string;
DEFINE FIELD normalized_name ON entity TYPE string;
DEFINE FIELD properties      ON entity TYPE object;
DEFINE FIELD name_embedding  ON entity TYPE option<array<float>>;
DEFINE FIELD epistemic_class ON entity TYPE string DEFAULT 'current';
DEFINE FIELD created_at      ON entity TYPE datetime;
DEFINE FIELD last_seen_at    ON entity TYPE datetime;
DEFINE INDEX entity_alias_lookup ON entity FIELDS agent_id, entity_type, normalized_name;
DEFINE INDEX entity_name_hnsw    ON entity FIELDS name_embedding HNSW DIMENSION 384 DIST COSINE;

DEFINE TABLE related TYPE RELATION FROM entity TO entity SCHEMALESS;
DEFINE FIELD relationship_type ON related TYPE string;
DEFINE FIELD properties        ON related TYPE object;
DEFINE FIELD created_at        ON related TYPE datetime;
DEFINE FIELD mention_count     ON related TYPE int DEFAULT 1;

DEFINE TABLE fact SCHEMALESS;
DEFINE FIELD agent_id   ON fact TYPE string;
DEFINE FIELD scope      ON fact TYPE string;
DEFINE FIELD ward_id    ON fact TYPE string;
DEFINE FIELD key        ON fact TYPE string;
DEFINE FIELD content    ON fact TYPE string;
DEFINE FIELD category   ON fact TYPE string;
DEFINE FIELD embedding  ON fact TYPE option<array<float>>;
DEFINE FIELD valid_from ON fact TYPE option<datetime>;
DEFINE FIELD valid_until ON fact TYPE option<datetime>;
DEFINE INDEX fact_dedup ON fact FIELDS agent_id, scope, ward_id, key UNIQUE;
DEFINE INDEX fact_hnsw  ON fact FIELDS embedding HNSW DIMENSION 384 DIST COSINE;
DEFINE ANALYZER fact_text TOKENIZERS class FILTERS lowercase, snowball(english);
DEFINE INDEX fact_fts   ON fact FIELDS content SEARCH ANALYZER fact_text BM25;
```

---

## Error handling & graceful degradation

### Failure shape

| Failure | StoreError variant | Graceful response |
|---|---|---|
| SurrealDB child mid-restart (1-30 s) | `Unavailable { retry_after }` | Reads return empty; writes fail-fast with retry hint |
| SurrealDB child gave up (5 crashes / 60 s) | `Unavailable { retry_after: None }` | All store calls fail; daemon `/health` reports degraded; user sees clear error |
| First-launch bootstrap | (creates dir + schema transparently) | No error — initialization is part of normal startup |
| Schema bootstrap actually failed (post-init) | `Schema(msg)` | Daemon refuses to serve, exits with explicit message |
| Single-query timeout | `Backend("query timeout")` | Caller decides retry; agent treats as "skip this recall" |
| Unique-violation on upsert | `Conflict(msg)` | Caller chooses to merge or surface |
| Embedding dimension mismatch | `Schema("dim mismatch, run reindex")` | Daemon triggers `reindex_embeddings` automatically (already in current code path) |
| Disk full | warning logged + `Backend(msg)` | Surface in UI; can't recover automatically |
| Corrupted-but-rebuildable data | `Backend(msg)` initially; future `Recoverable` variant | Future "rebuild from sources" pipeline (memory facts from session distillation, KG entities from source artifacts). Recovery tooling deferred to a separate `agentzero-recover` crate |
| Corrupted unrecoverably | `Backend(msg)` | User runs future recovery tool; not in this design |
| `surreal` binary missing | daemon refuses to start with explicit message | User runs `agentzero install-deps` (deferred) or installs manually |

### Daemon-level translation (HTTP layer)

```
StoreError::NotFound     → 404
StoreError::Conflict     → 409
StoreError::Invalid      → 400
StoreError::Unavailable  → 503 + Retry-After header (from retry_after field)
StoreError::Schema       → 500 with sanitized message
StoreError::Backend      → 500 with sanitized message
```

Sanitization strips connection strings, file paths, and stack details from error messages before the wire — internal details stay in `tracing::error!` logs.

### Agent-runtime "graceful" rules

**Rule 1: Recall failures never block agent responses.**
- `MemoryFactStore::recall` returning `Unavailable` → recall hits = empty, agent continues without prior context.
- A single-line note inserted into the agent's working memory: `"⚠ memory layer unavailable — responding without recalled context"` so the LLM is aware.
- Logged at `warn!` once per outage window (de-duped via the supervisor's healthy-state transition).

**Rule 2: Memory writes that fail are surfaced, not silently dropped.**
- `save_fact` failure → returned to caller; caller logs at `warn!` with the fact content trimmed for log hygiene.
- **No retry buffer.** Buffering writes across an outage is silent-corruption territory — surfacing the failure is honest.

**Rule 3: Knowledge graph failures degrade per-call.**
- `traverse` / `resolve_entity` failure → tool call surfaces a tool-result error; the agent decides what to do (typically: continue without graph context).
- `store_knowledge` failure during ingest → entire batch rejected; ingest pipeline retries on next sleep cycle (existing pattern).

---

## Testing strategy

### Test pyramid

```
Unit            zero-stores/                  type semantics, error variants, RecallConfig math
Impl unit       zero-stores-sqlite/           SQLite-specific: FTS5 sync triggers, vec0 dim mismatch
                zero-stores-surreal/          SurrealDB-specific: HNSW DDL, supervisor lifecycle
Conformance     zero-stores-conformance/      Same scenarios run against EVERY impl
E2E             apps/daemon/tests/            Full daemon, HTTP API exercised
```

### Cross-impl conformance suite (the key pattern)

`zero-stores-conformance` exposes scenarios as **library functions**, not `#[test]` functions:

```rust
// zero-stores-conformance/src/lib.rs
pub async fn upsert_then_recall_returns_fact<S: MemoryFactStore>(store: &S) { … }
pub async fn delete_cascades_to_relationships<S: KnowledgeGraphStore>(store: &S) { … }
pub async fn resolve_entity_alias_then_embedding_match<S: KnowledgeGraphStore>(store: &S) { … }
// … ~30-50 scenarios total
```

Each impl crate calls them from its integration tests:

```rust
// zero-stores-sqlite/tests/conformance.rs
#[tokio::test] async fn upsert_then_recall() {
    let store = test_store_sqlite().await;
    zero_stores_conformance::upsert_then_recall_returns_fact(&store).await;
}

// zero-stores-surreal/tests/conformance.rs
#[tokio::test] async fn upsert_then_recall() {
    let _guard = test_surreal_sidecar().await;   // RAII drop = kill child
    let store = test_store_surreal(&_guard).await;
    zero_stores_conformance::upsert_then_recall_returns_fact(&store).await;
}
```

Same scenarios, both backends. Drift between impls produces failing assertions.

### Test fixtures

| Backend | Fixture | Notes |
|---|---|---|
| SQLite | `tempfile::tempdir()` + `SqliteKgStore::open(path)` | Same as today; fast; no external deps |
| SurrealDB | `TestSidecar::start()` — spawns `surreal start --bind unix:$tmp/test.sock memory://`, waits for ready, returns a guard whose `Drop` kills the child | `memory://` storage = ephemeral, isolated, fast (<1 s per test); RocksDB-backed reserved for targeted "real persistence" scenarios |

### Supervisor lifecycle tests

These can't use `memory://` — they're testing process management:

- `test_supervisor_starts_and_reports_healthy()`
- `test_supervisor_restarts_on_crash()` — supervisor monitors a child we kill manually
- `test_supervisor_gives_up_after_5_crashes_in_60s()` — uses a fast-fail mock binary
- `test_graceful_shutdown_kills_child()`
- `test_kill_on_drop_backstop()` — drop the supervisor without explicit shutdown

Live in `zero-stores-surreal/tests/supervisor.rs`. Kept in a separate test target (slower).

### CI considerations

- **GitHub Actions:** install `surreal` binary in setup step, cached by version. ~2 s per CI run after cache is warm.
- **Local dev:** developers need `surreal` on `PATH`. Documented in `CONTRIBUTING.md`. `cargo test` skips SurrealDB tests with a clear message if binary is missing.
- **Pi CI:** same flow, ARM64 binary.

---

## Consumer impact

A hard requirement of this design is that existing consumers continue to work without modification beyond field-type changes. The trait abstraction is the contract.

### Agent tools (continue working unchanged)

| Tool | File | Affected by | Breaks? |
|---|---|---|---|
| `memory` (recall + save) | `runtime/agent-tools/src/tools/memory.rs` | Trait method calls | No — same `MemoryFactStore` methods |
| `query_graph` | `runtime/agent-tools/src/tools/graph_query.rs` | Trait method calls | No — same `KnowledgeGraphStore` methods |
| `analyze_intent` (KG-backed) | `runtime/agent-tools/src/tools/intent.rs` | Trait method calls | No — same `KnowledgeGraphStore` methods |

The agent-tool layer holds `Arc<dyn MemoryFactStore>` / `Arc<dyn KnowledgeGraphStore>` (today: holds the concrete repos). Once the field types switch to trait objects, the tools work identically against either backend.

### HTTP/WS API exposed to UI (continue working unchanged)

| Endpoint group | File | Affected by | Breaks? |
|---|---|---|---|
| Memory APIs | `gateway/src/http/memory.rs` | Trait method calls | No — same logical operations |
| Graph APIs | `gateway/src/http/graph.rs` | Trait method calls | No |
| Wiki APIs | `gateway/src/http/wiki.rs` | Trait method calls | No |
| Recall APIs | (wherever exposed) | Trait method calls | No |

The HTTP handlers receive `Arc<dyn …>` from `AppState`. JSON request/response shapes are unchanged because the domain types (`MemoryFact`, `Entity`, `Relationship`) are unchanged.

### Sleep / batch jobs (require routing change)

The 6 files in `gateway-execution/sleep/*` currently bypass abstractions and use raw `rusqlite::Connection`. They must be refactored to route through the new traits — this is **TD-012** in the registry. Tracked separately as part of the implementation phasing.

### `services/execution-state` (partial refactor needed)

This crate's repository touches both conversations.db and knowledge.db. Knowledge-side ops migrate to use `KnowledgeGraphStore` and `MemoryFactStore`. Session/execution ops stay as-is. Tracked as **TD-014**.

---

## Fallback strategy

The worst-case operational story is: **switch back to SQLite.** The design supports this as a first-class fallback, not a hack.

**Code-level fallback is one config flag.** `knowledge_backend: sqlite` (instead of `surrealdb`) makes `PersistenceFactory` construct `SqliteKgStore` + `SqliteMemoryStore` instead of the SurrealDB impls. The trait surface is identical, so agent tools, HTTP handlers, and sleep jobs work unchanged. Same daemon binary; no rebuild required.

**Data does not auto-transfer between backends.** SQLite and SurrealDB store on disk in their own formats. Falling back from SurrealDB → SQLite starts with an empty `knowledge.db`; the SurrealDB data remains intact at `$VAULT/data/knowledge.surreal/` (recoverable by switching back later) but is not migrated into SQLite. The "rebuild from sources" pipeline closes the gap when you commit to one backend: memory facts re-derive from session distillation, KG entities re-derive from source artifacts (wiki, skills, ward indexer inputs).

**The SQLite impl stays maintained long-term, not deprecated.** The cross-impl conformance suite runs both backends in CI on every PR; bug fixes apply to both; new trait methods are implemented in both. This is an explicit ongoing commitment, captured here so it doesn't drift away. If at some future point the project chooses to drop SQLite support, that's a separate decision (with its own design doc) — it is not the trajectory of this work.

---

## What we are explicitly NOT doing

- **No migration tooling.** Schema bootstrap is per-impl, idempotent, runs every startup. Migration between schema versions, or between SQLite and SurrealDB data, is a future workstream and a future crate (`zero-stores-migrate`).
- **No write-ahead log / no retry buffer for failed writes.** Writes that fail during an outage are surfaced and logged. Buffering across outages is silent-corruption territory.
- **No exposed `Tx` handle.** Atomic multi-op operations are single trait methods.
- **No automatic recovery for corrupted databases.** Surface clearly with a pointer to a future `agentzero-recover` tool.
- **No conversations-side abstraction in this scope.** `ConversationStore`, `LogStore`, `OutboxStore` traits are deferred to Phase 6 in the tech-debt registry — strictly hygiene, not on the SurrealDB critical path.
- **No data-side compatibility layer between backends.** Per-impl schemas only. Dual-write / fall-back is explicitly out of scope.
- **No performance benchmarks in this design.** Useful but separate; benchmarks live in `criterion` benches added later.

---

## Open items deferred to implementation time

These are not gaps in the design — they are decisions that benefit from being made when actual code exists:

- **SurrealDB 3.0 DDL syntax verification.** The `surql` sketches in this doc are illustrative; the SurrealDB impl crate will verify exact syntax via `context7` against current 3.0 docs.
- **HNSW parameter tuning.** `M`, `ef_construction`, etc., default values chosen at impl time based on smoke-test corpus.
- **Pi-specific RocksDB tuning.** WAL size, compaction throttle — operational config, captured during deployment phasing.
- **Windows IPC story.** Windows gets `127.0.0.1:0` loopback fallback; named-pipe support deferred unless Windows becomes a tier-1 deployment target.
- **`agentzero install-deps` subcommand.** Auto-downloads the `surreal` binary for the host architecture. Quality-of-life addition, not part of the persistence design.
- **Headless CLI ergonomics.** CLI uses daemon's HTTP API; how CLI ensures the daemon is running (auto-start, systemd unit, error message) is an apps/cli design concern.

---

## Reading guide

- **Tech debt registry & phased fix plan:** [`memory-bank/tech-debt.md`](../tech-debt.md)
- **Architecture overview:** [`memory-bank/architecture.md`](../architecture.md)
- **Design decisions log:** [`memory-bank/decisions.md`](../decisions.md) — record any deviations from this design here
- **General code-health scan:** [`memory-bank/sonar_scan_report.md`](../sonar_scan_report.md)
