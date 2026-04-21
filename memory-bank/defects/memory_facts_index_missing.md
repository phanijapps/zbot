# Defect — `memory.recall` tool errors with missing `memory_facts_index`

## Symptom

Every `memory` tool call with `action: "recall"` fails on the first run of
a session, emitting this tool error into the WebSocket feed:

```
Error: Tool execution failed: Tool("Knowledge DB operation failed: no such table: memory_facts_index")
```

Visible in the `/research-v2` StatusPill as a sticky red "Tool error"
(see R14e), and persisted in the session's message log as a `role: "tool"`
row immediately after the recall tool_call.

## Reproduction

Load any recent research session and query its messages — the error is
captured in the DB. Two examples from live sessions:

- `sess-d22f0b91-150b-4ae1-91f7-1acc04e94c11` (TSLA vs Ford) — fires
  twice, once at the start of root and once later.
- `sess-c962f36a-f4d8-41c6-8892-f037bd060f39` (NVDA vs AMD) — fires at
  least twice in root's execution.

Direct fetch to confirm:

```bash
curl -s "http://localhost:18791/api/sessions/sess-d22f0b91-150b-4ae1-91f7-1acc04e94c11/messages?scope=all" \
  | jq '.data[] | select(.content | test("Knowledge DB operation failed")) | .content'
```

## Scope

`memory_facts_index` is the `vec0` virtual table that backs embedding
lookups for `memory_facts` (see
`memory-bank/components/memory-layer/data-model.md:569` —
`CREATE VIRTUAL TABLE memory_facts_index USING vec0(fact_id TEXT
PRIMARY KEY, embedding FLOAT[384])`).

When the vec0 table is absent, every recall attempt fails with
`no such table: memory_facts_index`.

## Likely root cause — traced through the code

The embedding dim of the vec0 tables is **governed by a disk marker file**
(`data/.embedding-state`), NOT by the active embedding backend in
`settings.json`. That's the source of the settings-1024 vs table-384
mismatch the user observed.

### The chain at daemon boot

1. `gateway/src/state.rs:186` — `KnowledgeDatabase::new(paths)`
2. `knowledge_db.rs:50-56` — constructor reads `data/.embedding-state` via
   `read_indexed_dim_or_default(paths, 384)`. **Missing or malformed marker
   → falls back to 384.**
3. `knowledge_db.rs:85` — `initialize_vec_tables_with_dim(conn, dim)` creates
   the vec0 tables using whatever dim was read (or 384).
4. `knowledge_schema.rs:393-395` — the convenience `initialize_vec_tables()`
   hardcodes 384:
   ```rust
   pub fn initialize_vec_tables(conn: &Connection) -> Result<(), rusqlite::Error> {
       initialize_vec_tables_with_dim(conn, 384)
   }
   ```
5. `state.rs:778` — later `reconcile_dim()` compares the live
   `EmbeddingService.dimensions()` (1024 from user's Ollama config) against
   the indexed dim. On mismatch it triggers a reindex and calls
   `mark_indexed(current_dim)` to update the marker.

### Three scenarios that match the observed symptoms

| Scenario | Marker state | What the recall tool sees |
|---|---|---|
| A. Fresh install, reconcile not yet run | missing | Tables at 384; inserting a 1024-dim vector fails with `embedding dim mismatch: got 1024, expected 384` (vec0-level error) |
| B. Reindex crashed mid-run | stale + `*__new` orphan tables lingering | Next boot runs `cleanup_orphan_reindex_tables`; a second reconcile completes eventually |
| C. sqlite-vec extension failed to load | marker may or may not exist | `CREATE VIRTUAL TABLE USING vec0(…)` silently errors → table simply doesn't exist → `"no such table: memory_facts_index"` |

The **actual error text in the user's sessions is `no such table: memory_facts_index`** — that's scenario C, not a dim mismatch. The virtual table is entirely absent. Most likely root cause: `load_sqlite_vec(conn)` at
`knowledge_db.rs:38` (customizer's `on_acquire`) failed during DB init.

### Verification steps

```bash
# 1. Check if the marker file exists and what dim it pins
cat ~/Documents/zbot/data/.embedding-state
# Expected (if Ollama-1024 is active): "dim=1024"

# 2. Inspect the DB — does the table actually exist?
sqlite3 ~/Documents/zbot/data/knowledge.db \
  ".schema memory_facts_index"
# Expected if healthy: "CREATE VIRTUAL TABLE memory_facts_index USING vec0(fact_id TEXT PRIMARY KEY, embedding FLOAT[1024])"
# If empty: sqlite-vec extension failed to load; table was never created.

# 3. Daemon startup logs — grep for the exact failure
journalctl --user -u zerod --since "1 hour ago" | grep -iE "vec|sqlite_vec|embedding|Failed to init"

# 4. Orphan reindex tables left by a prior crash
sqlite3 ~/Documents/zbot/data/knowledge.db \
  "SELECT name FROM sqlite_master WHERE name LIKE '%__new'"
```

The test fixture at
`gateway/gateway-database/tests/vector_index.rs:110` confirms the v22
schema uses 1024-dim as the target (test passes a 1024-dim embedding);
`knowledge_schema.rs:392` still documents the hardcoded 384 default for
backward compatibility.

## Impact on the user

- First recall always misses — the agent loses pre-existing `memory_facts`
  context and starts cold.
- Tool error clutters the research-v2 news ticker even when the session
  is otherwise succeeding.
- Agent may still complete the task (it did in both NVDA and TSLA
  sessions), but quality is silently degraded — the "memory layer" isn't
  actually being read.

## Fix approach (proposed, not yet applied)

1. **Fail loud on sqlite-vec load failure.** `load_sqlite_vec(conn)` at
   `knowledge_db.rs:38` runs inside the r2d2 customizer `on_acquire` —
   if it errors on every connection the pool itself fails to build and
   `KnowledgeDatabase::new(…).expect("Failed to initialize knowledge database")`
   panics the daemon at state.rs:186. But if it errors on some paths and
   not others (e.g. platform-specific dylib path fallback), the DB can
   come up without vec0 and silently miss the `CREATE VIRTUAL TABLE`
   statements. Add a post-init presence check (`SELECT name FROM
   sqlite_master WHERE type='table' AND name='memory_facts_index'`) and
   refuse to boot if missing.

2. **Align marker with settings at boot.** If `settings.json`'s
   configured embedding backend reports `dimensions() != read_indexed_dim`,
   force a reindex **before** any recall tool call can hit the DB.
   Currently `reconcile_dim()` runs async after state is built and can
   race a user's first prompt.

3. **Guard the recall tool** (defensive): detect `no such table` /
   `embedding dim mismatch` errors in `memory.recall` and return an
   empty recall result with a `degraded_mode: true` flag instead of
   propagating a fatal tool error. Log once per session so the ticker
   doesn't spam.

4. **Surface in `/api/embeddings/health`**: the endpoint at
   `gateway/src/http/embeddings.rs` already returns `dim` + status.
   Add a `tables_present` field that confirms all 5 vec0 tables exist.
   UI can warn when missing.

## Files to audit

- `gateway/gateway-database/src/**` — migration scripts, `SqliteVecIndex`
  setup.
- `gateway/agent-tools/src/tools/memory.rs` (or similar) — the recall
  tool handler that emits the error.
- `runtime/agent-runtime/src/tools/memory.rs` — possibly the source of
  the error wrapping.

## Related

- R14e (LLM + tool errors in StatusPill) — the ticker surfacing this is
  already working; fixing the underlying tool is independent.
- `memory-bank/components/memory-layer/data-model.md:565-580` — vec0
  table definition.

## Resolution

Landed on branch `feature/ui-chat-research-redesign`. Four defences, one
per scenario from the "three scenarios" table above plus an observability
addition.

### Fix 1 — fail loud on missing vec0 tables at boot

`initialize_vec_tables_with_dim` in
`gateway/gateway-database/src/knowledge_schema.rs` now runs
`verify_vec_tables_present` after the `CREATE VIRTUAL TABLE` batch.
It counts rows in `sqlite_master` for the five expected vec0 names and
returns an `rusqlite::Error` with a descriptive message when the count
is short. `KnowledgeDatabase::new(...).expect("Failed to initialize
knowledge database")` at `gateway/src/state.rs:186` then panics the
daemon at boot with the full context instead of silently leaving the
tables absent.

**Addresses Scenario C** (sqlite-vec fails to load on a given connection
→ `CREATE VIRTUAL TABLE ... USING vec0(...)` silent no-op). Previously
the daemon booted successfully and then blew up on the first
`memory.recall` with the opaque `no such table: memory_facts_index`
error. Now it fails at boot with the real root cause ("sqlite-vec
extension likely failed to load").

### Fix 2 — align indexed dim with settings BEFORE any recall can fire

`gateway/src/state.rs` now synchronously reconciles the vec0 table
dim against the live `EmbeddingService.dimensions()` in `AppState::new`,
before the server starts accepting WebSocket invokes. When
`embedding_service.needs_reindex()` is true and the backend is configured
(dim != 0), we drop-and-recreate all five vec0 tables at the current dim
via `KnowledgeDatabase::reconcile_vec_tables_dim(current_dim)`, then call
`mark_indexed(current_dim)` so the marker matches the tables. Content
is repopulated by the reindex pipeline at the next sleep cycle; recall
returns empty in the interim instead of erroring on a dim mismatch.

The unconfigured-backend branch (`dim == 0`) is a no-op so fresh installs
that haven't picked an embedding backend yet keep the marker-derived
384-dim tables (still usable for FTS-only fallback).

**Addresses Scenario A** (fresh install, reconcile not yet run — tables
at 384, client embeds at 1024). The race window between
`AppState::new()` and `reconcile_embeddings_at_boot` is eliminated for
the dim-mismatch case.

### Fix 3 — defensive guard in `memory.recall`

`runtime/agent-tools/src/tools/memory.rs::action_recall` now catches
fact-store errors matching:
- `no such table: memory_facts_index` (and the other four vec0 table
  names)
- `embedding dim mismatch`

On match it returns a structured `{ recalled: [], results: [], count: 0,
degraded: true, reason: <brief> }` JSON instead of propagating
`ZeroError::Tool`. A `tracing::warn!` fires once per session (dedup via
`OnceLock<Mutex<HashSet<session_id>>>`) so the news ticker isn't spammed.

The classification is handled by a pure function
(`classify_recall_degradation`) that's unit-tested in isolation —
genuine tool errors (arg validation, provider outages) still propagate
normally.

**Addresses Scenarios A/B/C at the outer edge** — no matter which root
cause left the index in a bad state, the agent keeps running and the
session transcript stays clean.

### Fix 4 — `tables_present` / `tables_missing` in `/api/embeddings/health`

`gateway/src/http/embeddings.rs::HealthResponse` gained
`tables_present: Vec<String>` and `tables_missing: Vec<String>` lists
populated from the same `sqlite_master` query as Fix 1 (via a shared
`list_vec_table_presence` helper re-exported from `gateway-database`).
The UI can warn proactively when any of the five vec0 tables is
absent, even when the dim marker claims the install is healthy.

### Tests added

- `gateway-database/src/knowledge_schema.rs::tests` — three new unit
  tests: `initialize_vec_tables_errors_when_sqlite_vec_not_loaded`,
  `list_vec_table_presence_reports_all_five_when_initialized`,
  `list_vec_table_presence_reports_missing_when_dropped`.
- `gateway-database/src/knowledge_db.rs::tests` — one new unit test:
  `reconcile_vec_tables_dim_drops_and_recreates_at_new_dim`
  (seeds marker=384, constructs DB, calls reconcile(1024), asserts all
  five vec0 tables now carry `FLOAT[1024]`).
- `agent-tools/src/tools/memory.rs::tests` — seven new tests: four for
  the classifier (matches each known pattern + rejects unrelated
  errors), two for the once-per-session dedup, and one integration
  test driving `action_recall` against a `BrokenStore` that returns a
  `no such table` error to confirm the degraded JSON is returned
  instead of a fatal tool error.

### Files changed

- `gateway/gateway-database/src/knowledge_schema.rs` — `REQUIRED_VEC_TABLES`
  const, `verify_vec_tables_present`, `list_vec_table_presence`,
  `drop_and_recreate_vec_tables_at_dim` + 3 tests.
- `gateway/gateway-database/src/knowledge_db.rs` — `reconcile_vec_tables_dim`
  + 1 test.
- `gateway/gateway-database/src/lib.rs` — re-exports.
- `gateway/src/state.rs` — Fix 2 boot-time reconcile block.
- `gateway/src/http/embeddings.rs` — Fix 4 `tables_present` /
  `tables_missing` fields.
- `runtime/agent-tools/src/tools/memory.rs` — Fix 3 `action_recall`
  guard + `classify_recall_degradation` + `should_log_degradation` + 7
  tests.
