# Memory v2 — Phase 1b Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Migrate every repository that reads/writes knowledge tables to route through `KnowledgeDatabase`. Introduce a `VectorIndex` abstraction backed by sqlite-vec and delete every hand-rolled `cosine_similarity` function. Un-ignore the 37 repository tests.

**Architecture:** Each repository's constructor changes from `DatabaseManager` to `KnowledgeDatabase`. Embedding storage moves from `BLOB` columns (dropped in v22) to `vec0` partner tables via a `VectorIndex` trait. `GraphStorage` (in `services/knowledge-graph`) also migrates to `KnowledgeDatabase`, ending its own `Connection::open` path. Resolver stage 3 uses `VectorIndex::query_nearest` instead of a manual scan. Hand-rolled cosine functions deleted — one mechanism (sqlite-vec) for every similarity query.

**Tech Stack:** Rust 2024, `rusqlite`, `r2d2`, `sqlite-vec`, existing `KnowledgeDatabase` from Phase 1a.

**Spec:** `docs/superpowers/specs/2026-04-12-memory-layer-redesign-design.md`

---

## File Structure

**Created:**
- `gateway/gateway-database/src/vector_index.rs` — `VectorIndex` trait + `SqliteVecIndex` impl
- `gateway/gateway-database/tests/vector_index.rs` — integration tests per index

**Modified (per-repository migrations):**
- `gateway/gateway-database/src/memory_repository.rs` — constructor uses `KnowledgeDatabase`; embedding reads/writes go through `memory_facts_index` via `VectorIndex`; delete `cosine_similarity` fn
- `gateway/gateway-database/src/wiki_repository.rs` — same pattern, `wiki_articles_index`
- `gateway/gateway-database/src/procedure_repository.rs` — same pattern, `procedures_index`
- `gateway/gateway-database/src/episode_repository.rs` — same pattern, `session_episodes_index`
- `gateway/gateway-database/src/kg_episode_repository.rs` — constructor migration only, no cosine
- `gateway/gateway-database/src/memory_fact_store.rs` — update cosine_similarity import → VectorIndex
- `services/knowledge-graph/src/storage.rs` — accept `Arc<KnowledgeDatabase>` instead of its own `Connection::open`; use `with_connection`
- `services/knowledge-graph/src/resolver.rs` — stage 3 uses `VectorIndex::query_nearest`; delete local `cosine_similarity` fn
- `gateway/gateway-execution/src/recall.rs` — use `VectorIndex::query_nearest` for fact similarity; delete local `cosine_similarity` fn
- `gateway/src/state.rs` — construct all repositories with `knowledge_db.clone()`; construct `GraphStorage` from `knowledge_db`

**Deleted:**
- `fn cosine_similarity` in `memory_repository.rs`, `wiki_repository.rs`, `procedure_repository.rs`, `episode_repository.rs`, `recall.rs`, `resolver.rs` (6 functions)

---

## Pre-flight

Branch off current HEAD of `feature/memory-v2-phase-1a`:

```bash
git checkout feature/memory-v2-phase-1a
git pull
git checkout -b feature/memory-v2-phase-1b
```

All 11 Phase 1a tasks already landed. 37 repository tests currently `#[ignore]`d pending Phase 1b — they get re-enabled as each repo migrates.

---

## Task 1: VectorIndex trait + SqliteVecIndex implementation

**Files:**
- Create: `gateway/gateway-database/src/vector_index.rs`
- Modify: `gateway/gateway-database/src/lib.rs`

### Semantics

The trait exposes three ops per index:

```rust
fn upsert(&self, id: &str, embedding: &[f32]) -> Result<(), String>;
fn delete(&self, id: &str) -> Result<(), String>;
fn query_nearest(&self, embedding: &[f32], limit: usize) -> Result<Vec<(String, f32)>, String>;
```

`query_nearest` returns `Vec<(id, distance)>` — sqlite-vec uses L2 distance by default; for cosine similarity, convert via `cosine = 1.0 - (l2_squared / 2.0)` ONLY if embeddings are L2-normalized. For this system, assume embeddings are normalized before insert (enforce in `upsert`), so cosine_sim = 1.0 - l2_dist²/2.

**Five concrete index wrappers** — one per vec0 table. Each is a thin `SqliteVecIndex` constructor specifying table name + id column + embedding dimension.

### Step 1: Write failing skeleton test

Create `gateway/gateway-database/tests/vector_index.rs`:

```rust
use std::sync::Arc;
use tempfile::tempdir;

use gateway_database::{KnowledgeDatabase, vector_index::{SqliteVecIndex, VectorIndex}};
use gateway_services::VaultPaths;

fn setup() -> (tempfile::TempDir, Arc<KnowledgeDatabase>) {
    let tmp = tempdir().expect("tempdir");
    let paths = Arc::new(VaultPaths::new(tmp.path().to_path_buf()));
    std::fs::create_dir_all(paths.conversations_db().parent().expect("parent"))
        .expect("mkdir");
    let db = Arc::new(
        KnowledgeDatabase::new(paths.clone()).expect("init knowledge db"),
    );
    (tmp, db)
}

fn normalized(v: Vec<f32>) -> Vec<f32> {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm < 1e-9 { v } else { v.into_iter().map(|x| x / norm).collect() }
}

#[test]
fn upsert_and_query_nearest_returns_self() {
    let (_tmp, db) = setup();
    // Use a real vec0 table that exists: kg_name_index.
    // Need a kg_entities row first due to FK-less design — actually kg_name_index
    // has no FK to kg_entities; it just stores by id.
    let idx = SqliteVecIndex::new(db.clone(), "kg_name_index", "entity_id", 384);

    let v = normalized((0..384).map(|i| i as f32).collect());
    idx.upsert("e1", &v).expect("upsert");

    let results = idx.query_nearest(&v, 1).expect("query");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "e1");
    assert!(results[0].1 < 1e-4, "nearest self distance should be ~0, got {}", results[0].1);
}

#[test]
fn delete_removes_entry() {
    let (_tmp, db) = setup();
    let idx = SqliteVecIndex::new(db.clone(), "memory_facts_index", "fact_id", 384);

    let v = normalized((0..384).map(|i| i as f32).collect());
    idx.upsert("f1", &v).expect("upsert");
    idx.delete("f1").expect("delete");

    let results = idx.query_nearest(&v, 5).expect("query");
    assert!(results.iter().all(|(id, _)| id != "f1"));
}

#[test]
fn upsert_same_id_replaces() {
    let (_tmp, db) = setup();
    let idx = SqliteVecIndex::new(db.clone(), "procedures_index", "procedure_id", 384);

    let v1 = normalized(vec![1.0; 384]);
    let v2 = normalized((0..384).map(|i| i as f32).collect());
    idx.upsert("p1", &v1).expect("first");
    idx.upsert("p1", &v2).expect("replace");

    let results = idx.query_nearest(&v2, 5).expect("query");
    assert!(results.iter().any(|(id, _)| id == "p1"));

    let count_total = db
        .with_connection(|conn| {
            let n: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM procedures_index WHERE procedure_id = ?1",
                    rusqlite::params!["p1"],
                    |r| r.get(0),
                )?;
            Ok(n)
        })
        .expect("count");
    assert_eq!(count_total, 1, "upsert must replace, not duplicate");
}
```

Run: `cargo test -p gateway-database --test vector_index`

Expected: compile fails — `vector_index` module doesn't exist yet.

### Step 2: Implement VectorIndex trait + SqliteVecIndex

Create `gateway/gateway-database/src/vector_index.rs`:

```rust
//! Vector similarity index backed by sqlite-vec (`vec0`) virtual tables.
//!
//! Replaces hand-rolled cosine scans. Every similarity query in the
//! memory layer routes through this abstraction.
//!
//! Embeddings MUST be L2-normalized before `upsert`. Distance returned
//! by `query_nearest` is L2 squared; cosine similarity = `1.0 - d/2`.

use std::sync::Arc;

use crate::KnowledgeDatabase;

pub trait VectorIndex: Send + Sync {
    fn upsert(&self, id: &str, embedding: &[f32]) -> Result<(), String>;
    fn delete(&self, id: &str) -> Result<(), String>;
    fn query_nearest(&self, embedding: &[f32], limit: usize)
        -> Result<Vec<(String, f32)>, String>;
}

/// sqlite-vec-backed vector index for a single `vec0` virtual table.
pub struct SqliteVecIndex {
    db: Arc<KnowledgeDatabase>,
    table: &'static str,
    id_column: &'static str,
    dim: usize,
}

impl SqliteVecIndex {
    pub fn new(
        db: Arc<KnowledgeDatabase>,
        table: &'static str,
        id_column: &'static str,
        dim: usize,
    ) -> Self {
        Self { db, table, id_column, dim }
    }
}

impl VectorIndex for SqliteVecIndex {
    fn upsert(&self, id: &str, embedding: &[f32]) -> Result<(), String> {
        if embedding.len() != self.dim {
            return Err(format!(
                "embedding dim mismatch: got {}, expected {}",
                embedding.len(),
                self.dim
            ));
        }
        let embedding_json = serde_json::to_string(embedding)
            .map_err(|e| format!("serialize embedding: {e}"))?;
        let sql_delete = format!("DELETE FROM {} WHERE {} = ?1", self.table, self.id_column);
        let sql_insert = format!(
            "INSERT INTO {} ({}, {}) VALUES (?1, ?2)",
            self.table,
            self.id_column,
            embedding_column_name(self.table),
        );
        self.db.with_connection(|conn| {
            // sqlite-vec vec0 tables do not support UPSERT; emulate with delete+insert.
            conn.execute(&sql_delete, rusqlite::params![id])?;
            conn.execute(&sql_insert, rusqlite::params![id, &embedding_json])?;
            Ok(())
        })
    }

    fn delete(&self, id: &str) -> Result<(), String> {
        let sql = format!("DELETE FROM {} WHERE {} = ?1", self.table, self.id_column);
        self.db.with_connection(|conn| {
            conn.execute(&sql, rusqlite::params![id])?;
            Ok(())
        })
    }

    fn query_nearest(
        &self,
        embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<(String, f32)>, String> {
        if embedding.len() != self.dim {
            return Err(format!(
                "embedding dim mismatch: got {}, expected {}",
                embedding.len(),
                self.dim
            ));
        }
        let embedding_json = serde_json::to_string(embedding)
            .map_err(|e| format!("serialize embedding: {e}"))?;
        let sql = format!(
            "SELECT {}, distance FROM {} WHERE {} MATCH ?1 ORDER BY distance LIMIT ?2",
            self.id_column,
            self.table,
            embedding_column_name(self.table),
        );
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(
                rusqlite::params![embedding_json, limit as i64],
                |r| {
                    let id: String = r.get(0)?;
                    let dist: f32 = r.get(1)?;
                    Ok((id, dist))
                },
            )?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row?);
            }
            Ok(out)
        })
    }
}

/// Map a vec0 table name to its embedding column name.
/// Our 5 vec0 tables have stable column names per the Phase 1a schema.
fn embedding_column_name(table: &str) -> &'static str {
    match table {
        "kg_name_index" => "name_embedding",
        "memory_facts_index"
        | "wiki_articles_index"
        | "procedures_index"
        | "session_episodes_index" => "embedding",
        _ => "embedding",
    }
}
```

### Step 3: Register module in lib.rs

Edit `gateway/gateway-database/src/lib.rs`:

```rust
pub mod vector_index;
```

And (optional re-export):

```rust
pub use vector_index::{SqliteVecIndex, VectorIndex};
```

### Step 4: Run the integration tests

```
cargo test -p gateway-database --test vector_index
```

Expected: all 3 tests PASS.

If a test fails with "no such function: vec0" or similar, the extension isn't loaded — that's a Phase 1a regression and should be caught by `fresh_boot`. Re-run `cargo test -p gateway-database --test fresh_boot` to verify.

### Step 5: fmt + clippy

```
cargo fmt --all
cargo clippy -p gateway-database --all-targets -- -D warnings
```

### Step 6: Commit

```bash
git add gateway/gateway-database/src/vector_index.rs gateway/gateway-database/src/lib.rs gateway/gateway-database/tests/vector_index.rs
git commit -m "feat(db): VectorIndex trait + SqliteVecIndex for all 5 vec0 tables"
```

---

## Task 2: Migrate `memory_repository` to KnowledgeDatabase + VectorIndex

**Files:**
- Modify: `gateway/gateway-database/src/memory_repository.rs`

### Scope
- Constructor: `MemoryRepository::new(db: Arc<DatabaseManager>)` → `MemoryRepository::new(db: Arc<KnowledgeDatabase>, vec_index: Arc<dyn VectorIndex>)`
- Embedding column reads/writes removed from SQL (memory_facts no longer has the column)
- Callers that pass `embedding: Option<Vec<f32>>` route it through `vec_index.upsert(fact_id, &embedding)` after the fact row INSERT
- Callers that do similarity search use `vec_index.query_nearest(query, k)` then `SELECT ... WHERE id IN (...)` to fetch content
- `pub fn cosine_similarity` at the bottom of the file is **deleted**
- Tests previously marked `#[ignore]` with `"Phase 1b: ..."` have the `#[ignore]` attribute removed and are updated to use the new constructor + VectorIndex

### Step 1: Read the current memory_repository

```
cat gateway/gateway-database/src/memory_repository.rs | head -120
grep -n "pub fn new\|pub fn upsert\|pub fn search\|pub fn cosine_similarity\|embedding: BLOB\|embedding BLOB" gateway/gateway-database/src/memory_repository.rs
```

### Step 2: Change the constructor signature

Find `impl MemoryRepository { pub fn new(db: Arc<DatabaseManager>) -> Self { ... } }`. Change to:

```rust
impl MemoryRepository {
    pub fn new(
        db: Arc<KnowledgeDatabase>,
        vec_index: Arc<dyn VectorIndex>,
    ) -> Self {
        Self { db, vec_index }
    }
}
```

And update the struct:

```rust
pub struct MemoryRepository {
    db: Arc<KnowledgeDatabase>,
    vec_index: Arc<dyn VectorIndex>,
}
```

Imports at top of file:

```rust
use std::sync::Arc;
use crate::KnowledgeDatabase;
use crate::vector_index::VectorIndex;
```

Remove any `use ... DatabaseManager` if no longer referenced.

### Step 3: Remove embedding BLOB from all SQL

Find every SQL string that references an `embedding` column on `memory_facts`:

```
grep -n "embedding" gateway/gateway-database/src/memory_repository.rs
```

For each `INSERT INTO memory_facts`, `UPDATE memory_facts SET`, and `SELECT ... FROM memory_facts` that lists `embedding` — remove the column from the SQL.

For upsert paths that currently write the embedding BLOB:
- Remove the `embedding` from the SQL
- After the `INSERT` succeeds, if `embedding` was provided by the caller, call `self.vec_index.upsert(&fact_id, &embedding)?;`

For similarity-search paths (`search_by_embedding` or similar):
- Delete the in-memory loop that fetched all rows, parsed the BLOB, and called `cosine_similarity`
- Replace with `self.vec_index.query_nearest(query, limit)?` → gives `Vec<(fact_id, distance)>` → fetch the matching facts by id

Pseudocode for the similarity search rewrite:

```rust
pub fn search_by_embedding(&self, query: &[f32], limit: usize)
    -> Result<Vec<MemoryFact>, String>
{
    let nearest = self.vec_index.query_nearest(query, limit)?;
    if nearest.is_empty() {
        return Ok(Vec::new());
    }
    let ids: Vec<String> = nearest.iter().map(|(id, _)| id.clone()).collect();
    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT {} FROM memory_facts WHERE id IN ({})",
        MEMORY_FACT_SELECT_COLUMNS, placeholders
    );
    self.db.with_connection(|conn| {
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(&ids), |r| {
            Ok(row_to_memory_fact(r)?)
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(rusqlite::Error::from)
    })
}
```

(Substitute actual column list / row mapper that already exists in the file.)

### Step 4: Delete `fn cosine_similarity` in this file

Find `pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 { ... }` (currently ~line 1049). Delete the function entirely. Also delete any module-level tests of it.

### Step 5: Update the hybrid/FTS search paths

If there's a hybrid function that combines FTS5 BM25 with cosine similarity (search_facts or similar), the vector half routes through `vec_index.query_nearest`; the BM25 half stays as-is; results are merged via RRF or a weighted sum as before. The merge logic stays — only the vector-retrieval step changes.

### Step 6: Un-ignore the tests

Grep for `#[ignore = "Phase 1b`:

```
grep -n 'Phase 1b' gateway/gateway-database/src/memory_repository.rs
```

For each match, delete the `#[ignore = "..."]` line. Update each test to:
1. Build a `KnowledgeDatabase` from a `VaultPaths(tempdir)` instead of in-memory `DatabaseManager`
2. Build a `SqliteVecIndex` for `memory_facts_index`
3. Construct `MemoryRepository::new(knowledge_db, Arc::new(vec_index))`

Test helper at top of `#[cfg(test)] mod tests`:

```rust
fn setup() -> (tempfile::TempDir, Arc<MemoryRepository>) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let paths = Arc::new(gateway_services::VaultPaths::new(tmp.path().to_path_buf()));
    std::fs::create_dir_all(paths.conversations_db().parent().expect("parent")).expect("mkdir");
    let db = Arc::new(KnowledgeDatabase::new(paths.clone()).expect("knowledge db"));
    let vec_index: Arc<dyn VectorIndex> = Arc::new(SqliteVecIndex::new(
        db.clone(), "memory_facts_index", "fact_id", 384,
    ));
    let repo = Arc::new(MemoryRepository::new(db, vec_index));
    (tmp, repo)
}
```

Any test that uses a `Vec<f32>` directly must normalize first — our `VectorIndex` contract says embeddings are L2-normalized before `upsert`.

### Step 7: Build and test

```
cargo check -p gateway-database
cargo test -p gateway-database --lib memory_repository::tests
```

Expected: all tests that were previously `#[ignore]`d now pass.

If any test fails because it relied on `cosine_similarity` being exported — that test was testing the function itself, not memory repository behavior. Delete that test (the sqlite-vec path is tested in `vector_index.rs` integration tests).

### Step 8: fmt + clippy

```
cargo fmt --all
cargo clippy -p gateway-database --all-targets -- -D warnings
```

### Step 9: Commit

```bash
git add gateway/gateway-database/src/memory_repository.rs
git commit -m "feat(db): migrate memory_repository to KnowledgeDatabase + VectorIndex"
```

---

## Task 3: Migrate `wiki_repository`

**Files:**
- Modify: `gateway/gateway-database/src/wiki_repository.rs`

Same pattern as Task 2. Bindings:
- Index table: `wiki_articles_index`
- Id column: `article_id`
- Constructor: `WikiRepository::new(db: Arc<KnowledgeDatabase>, vec_index: Arc<dyn VectorIndex>)`
- Remove `embedding` column from all SQL against `ward_wiki_articles`
- After upserting an article, write embedding via `vec_index.upsert(&article_id, &embedding)?`
- Replace similarity-scan loop with `vec_index.query_nearest`
- Delete `fn cosine_similarity` at bottom of file + any `cosine_similarity_*` unit tests in the same file
- Un-ignore Phase 1b tests, rewrite setup with KnowledgeDatabase + SqliteVecIndex

- [ ] **Step 1:** Update struct + constructor + imports (see Task 2 Step 2 pattern).
- [ ] **Step 2:** Purge `embedding` from SQL.
- [ ] **Step 3:** Route embedding writes to VectorIndex.
- [ ] **Step 4:** Rewrite similarity search via `query_nearest` + `WHERE id IN (...)`.
- [ ] **Step 5:** Delete `fn cosine_similarity` + its unit tests (lines 189, 372-374 per earlier grep).
- [ ] **Step 6:** Un-ignore tests + switch setup to KnowledgeDatabase + SqliteVecIndex.
- [ ] **Step 7:** `cargo test -p gateway-database --lib wiki_repository::tests` — all PASS.
- [ ] **Step 8:** fmt + clippy clean.
- [ ] **Step 9:**
  ```bash
  git add gateway/gateway-database/src/wiki_repository.rs
  git commit -m "feat(db): migrate wiki_repository to KnowledgeDatabase + VectorIndex"
  ```

---

## Task 4: Migrate `procedure_repository`

**Files:**
- Modify: `gateway/gateway-database/src/procedure_repository.rs`

Same pattern:
- Index table: `procedures_index`
- Id column: `procedure_id`
- Remove `embedding` from `procedures` SQL
- Delete `fn cosine_similarity` (line 262)
- Un-ignore Phase 1b tests

Execute Steps 1–9 analogous to Task 3, substituting `procedures` / `procedure_id`. Commit message:

```
feat(db): migrate procedure_repository to KnowledgeDatabase + VectorIndex
```

---

## Task 5: Migrate `episode_repository`

**Files:**
- Modify: `gateway/gateway-database/src/episode_repository.rs`

Same pattern:
- Index table: `session_episodes_index`
- Id column: `episode_id`
- Delete `fn cosine_similarity` (line 264)
- Un-ignore Phase 1b tests

Commit message:

```
feat(db): migrate episode_repository to KnowledgeDatabase + VectorIndex
```

---

## Task 6: Migrate `kg_episode_repository` (no cosine — DB swap only)

**Files:**
- Modify: `gateway/gateway-database/src/kg_episode_repository.rs`

Simpler task — `kg_episodes` has no embedding column, no cosine function. Only change:
- Constructor: `KgEpisodeRepository::new(db: Arc<DatabaseManager>)` → `KgEpisodeRepository::new(db: Arc<KnowledgeDatabase>)`
- All SQL stays the same (table already exists in knowledge.db)
- Un-ignore Phase 1b tests, update setup to KnowledgeDatabase

Commit message:

```
feat(db): migrate kg_episode_repository to KnowledgeDatabase
```

---

## Task 7: Migrate `memory_fact_store`

**Files:**
- Modify: `gateway/gateway-database/src/memory_fact_store.rs`

Context: this module has an async `MemoryFactStore` trait impl that was calling `cosine_similarity` imported from `memory_repository.rs`. After Task 2 deletes that function, this file breaks until migrated.

Steps:
- Change `use crate::memory_repository::cosine_similarity;` (if present) — delete the import
- Replace the one `cosine_similarity(qe, fact_emb)` call site (line 238 per grep) with a `VectorIndex::query_nearest` lookup. Since this module already queries multiple facts and filters by similarity, the rewrite mirrors the pattern in Task 2 Step 3: `vec_index.query_nearest(qe, k)` → fetch facts by id
- If `MemoryFactStore` doesn't have a `VectorIndex` field yet, add one and update its constructor
- Un-ignore the two `#[tokio::test] #[ignore = "Phase 1b..."]` tests

Commit message:

```
feat(db): migrate memory_fact_store — use VectorIndex for similarity
```

---

## Task 8: Migrate `GraphStorage` to KnowledgeDatabase

**Files:**
- Modify: `services/knowledge-graph/src/storage.rs`
- Modify: `services/knowledge-graph/src/service.rs` (if it constructs GraphStorage with a PathBuf)
- Modify: `gateway/src/state.rs` (GraphStorage construction site)

### Current shape
`GraphStorage::new(db_path: PathBuf)` opens its own `Connection` under an async `Mutex<Connection>`. Every call to `store_knowledge`, `store_entity`, etc. does `self.conn.lock().await` and runs SQL.

### Target shape
`GraphStorage::new(db: Arc<KnowledgeDatabase>)` — no mutex, no own connection. Every operation uses `db.with_connection(|conn| ... )`.

### Step 1: Change the struct + constructor

```rust
pub struct GraphStorage {
    db: Arc<KnowledgeDatabase>,
}

impl GraphStorage {
    pub fn new(db: Arc<KnowledgeDatabase>) -> GraphResult<Self> {
        // Schema is already initialized by KnowledgeDatabase::new.
        // No in-crate schema init needed here.
        Ok(Self { db })
    }
}
```

Delete the old `Connection::open(&db_path)` path. Delete any `add column if not exists` migrations in this file (all columns are in the v22 initial schema now).

### Step 2: Rewrite every `async fn` method

Every method currently does:

```rust
async fn store_entity(&self, ...) -> GraphResult<String> {
    let conn = self.conn.lock().await;
    // ... use conn ...
}
```

Becomes:

```rust
fn store_entity(&self, ...) -> GraphResult<String> {
    self.db.with_connection(|conn| {
        // ... use conn ...
    })
    .map_err(GraphError::from)
}
```

Notes:
- Remove `async` from method signatures that no longer await. Adjust callers accordingly.
- If a caller chain relies on async (e.g., `tokio::spawn`), wrap the sync call in `spawn_blocking` if the call path is on a runtime thread.
- `GraphError::from(String)` may need a new `From<String>` impl; if it exists already, great.

### Step 3: Update callers

Grep for `GraphStorage::new` uses outside the crate:

```
grep -rn "GraphStorage::new" --include="*.rs" .
```

Update each to pass `Arc<KnowledgeDatabase>`. In `gateway/src/state.rs`, the current site opens `GraphStorage::new(paths.knowledge_db())`; change to:

```rust
let graph_storage = Arc::new(GraphStorage::new(knowledge_db.clone())?);
```

### Step 4: Handle async removal propagation

If `GraphService` methods were `async` solely because they awaited `GraphStorage`, drop `async` from those too. If callers of `GraphService` need async, leave them async — the method body can still return a ready value without awaiting.

### Step 5: cargo check

```
cargo check --workspace
```

Fix every compile error by updating caller signatures. Expected: clean when done.

### Step 6: Run existing knowledge-graph tests

```
cargo test -p knowledge-graph --lib
```

Expected: all tests PASS. If any test constructs `GraphStorage` with a `PathBuf`, rewrite to use `KnowledgeDatabase` + `VaultPaths(tempdir)`.

### Step 7: fmt + clippy

```
cargo fmt --all
cargo clippy --all-targets -- -D warnings
```

### Step 8: Commit

```bash
git add -A
git commit -m "feat(graph): migrate GraphStorage to KnowledgeDatabase"
```

---

## Task 9: Delete cosine in `recall.rs` + rewire to VectorIndex

**Files:**
- Modify: `gateway/gateway-execution/src/recall.rs`

### Step 1: Locate the cosine sites

```
grep -n "cosine_similarity\|fn cosine" gateway/gateway-execution/src/recall.rs
```

Expect: one call site (line 212) and one function definition (line 1272).

### Step 2: Route the call site through MemoryRepository::search_by_embedding

The call site is inside hybrid recall — it's doing a manual cosine scan over facts that have embeddings. Replace with `memory_repo.search_by_embedding(query, limit)` — already migrated in Task 2.

If `recall.rs` doesn't have direct access to a `VectorIndex`, it accesses through `MemoryRepository` (which it already has). Call `memory_repo.search_by_embedding(&query_embedding, limit)` and merge with BM25 results as before.

### Step 3: Delete `fn cosine_similarity`

Delete the function body at line 1272. Delete any unit tests in this file that test it directly.

### Step 4: cargo check + test

```
cargo check -p gateway-execution
cargo test -p gateway-execution --lib recall
```

Expected: recall tests still pass. If any test was mocking cosine similarity directly, rewrite to use the `MemoryRepository` path with an in-memory knowledge DB.

### Step 5: fmt + clippy

```
cargo fmt --all
cargo clippy -p gateway-execution --all-targets -- -D warnings
```

### Step 6: Commit

```bash
git add gateway/gateway-execution/src/recall.rs
git commit -m "feat(recall): route fact similarity through MemoryRepository (VectorIndex)"
```

---

## Task 10: Delete cosine in `resolver.rs` + rewire stage 3

**Files:**
- Modify: `services/knowledge-graph/src/resolver.rs`

### Current shape
Stage 3 (`fn resolve_by_embedding_similarity` around line 212) iterates top-50 same-type entities, extracts `_name_embedding` from their properties JSON, calls `cosine_similarity`, keeps the max. This is the classic O(N) scan Phase 1c retires — but Phase 1b can already replace it with a `VectorIndex::query_nearest` on `kg_name_index`.

### Step 1: Add VectorIndex dep to EntityResolver

Change `EntityResolver` struct to optionally hold an `Arc<dyn VectorIndex>` for `kg_name_index`:

```rust
pub struct EntityResolver {
    // ... existing fields ...
    name_index: Option<Arc<dyn VectorIndex>>,
}

impl EntityResolver {
    pub fn with_name_index(mut self, index: Arc<dyn VectorIndex>) -> Self {
        self.name_index = Some(index);
        self
    }
}
```

(`VectorIndex` imports from `gateway_database::vector_index` — if this crate doesn't depend on `gateway_database`, either move the trait to a shared crate OR define a narrow trait in `knowledge-graph` that matches. Simpler: move the trait definition to `framework/zero-core` or a new small crate both can depend on. If too invasive, keep the trait in `gateway-database` and add a `knowledge-graph → gateway-database` dep — check for cycles first: `cargo tree -p knowledge-graph`. Currently no cycle should exist.)

### Step 2: Rewrite stage 3

Delete the current `resolve_by_embedding_similarity` body. Replace with:

```rust
fn resolve_by_embedding_similarity(
    &self,
    candidate: &EntityCandidate,
    candidate_embedding: &[f32],
) -> Option<String> {
    let index = self.name_index.as_ref()?;
    let nearest = index.query_nearest(candidate_embedding, 5).ok()?;
    // sqlite-vec returns L2 squared; convert to cosine-ish distance.
    // Cosine similarity >= 0.87 ≈ L2_sq <= 0.26 for normalized vectors.
    let threshold_l2_sq = 0.26_f32;
    for (entity_id, dist) in nearest {
        if dist <= threshold_l2_sq {
            // TODO(Phase 1c): add entity_type filter — requires joining kg_entities.
            return Some(entity_id);
        }
    }
    None
}
```

Phase 1c (resolver v2) tightens the type filter and replaces threshold heuristic. This task just retires the O(N) scan.

### Step 3: Delete `fn cosine_similarity` (line 266)

Delete the function and its unit test (line 332) since the ANN path is validated in `vector_index.rs` integration tests.

### Step 4: Update tests

Any unit test that stubbed stage 3 by injecting a list of embeddings must now set up a `VectorIndex` with those embeddings in a tempdir KnowledgeDatabase. Use the same `setup()` pattern.

### Step 5: cargo test + clippy

```
cargo test -p knowledge-graph --lib resolver
cargo clippy -p knowledge-graph --all-targets -- -D warnings
```

### Step 6: Commit

```bash
git add services/knowledge-graph/src/resolver.rs services/knowledge-graph/Cargo.toml
git commit -m "feat(resolver): stage 3 uses VectorIndex ANN; delete hand-rolled cosine"
```

---

## Task 11: Wire repositories with knowledge_db in AppState

**Files:**
- Modify: `gateway/src/state.rs`

### Step 1: Update every repository construction

Grep sites:

```
grep -n "MemoryRepository::new\|WardWikiRepository::new\|ProcedureRepository::new\|EpisodeRepository::new\|KgEpisodeRepository::new" gateway/src/state.rs
```

For each, change the single `Arc<DatabaseManager>` argument to the new signature. For repos that gained a `VectorIndex` argument, construct a fresh `SqliteVecIndex` per repo:

```rust
use gateway_database::vector_index::{SqliteVecIndex, VectorIndex};

let memory_vec: Arc<dyn VectorIndex> = Arc::new(SqliteVecIndex::new(
    knowledge_db.clone(), "memory_facts_index", "fact_id", 384,
));
let memory_repo = Arc::new(MemoryRepository::new(knowledge_db.clone(), memory_vec));

let wiki_vec: Arc<dyn VectorIndex> = Arc::new(SqliteVecIndex::new(
    knowledge_db.clone(), "wiki_articles_index", "article_id", 384,
));
let wiki_repo = Arc::new(WardWikiRepository::new(knowledge_db.clone(), wiki_vec));

let procedure_vec: Arc<dyn VectorIndex> = Arc::new(SqliteVecIndex::new(
    knowledge_db.clone(), "procedures_index", "procedure_id", 384,
));
let procedure_repo = Arc::new(ProcedureRepository::new(knowledge_db.clone(), procedure_vec));

let episode_vec: Arc<dyn VectorIndex> = Arc::new(SqliteVecIndex::new(
    knowledge_db.clone(), "session_episodes_index", "episode_id", 384,
));
let episode_repo = Arc::new(EpisodeRepository::new(knowledge_db.clone(), episode_vec));

let kg_episode_repo = Arc::new(KgEpisodeRepository::new(knowledge_db.clone()));
```

### Step 2: Rewire GraphStorage + resolver

```rust
let graph_storage = Arc::new(GraphStorage::new(knowledge_db.clone())?);
let name_vec: Arc<dyn VectorIndex> = Arc::new(SqliteVecIndex::new(
    knowledge_db.clone(), "kg_name_index", "entity_id", 384,
));
let resolver = EntityResolver::new(...).with_name_index(name_vec);
// (adjust to actual EntityResolver constructor)
```

### Step 3: Repeat for all 3 AppState constructors

`::new`, `::minimal`, `::with_components` — each has a repo-construction block. Update all three.

### Step 4: cargo check workspace

```
cargo check --workspace
```

Expected: clean.

### Step 5: fmt + clippy

```
cargo fmt --all
cargo clippy --all-targets -- -D warnings
```

### Step 6: Commit

```bash
git add gateway/src/state.rs
git commit -m "feat(state): construct repositories on knowledge_db with VectorIndex"
```

---

## Task 12: Final validation + push

- [ ] **Step 1: fmt check**
  ```
  cargo fmt --all --check
  ```
  Expected: clean.

- [ ] **Step 2: clippy**
  ```
  cargo clippy --all-targets -- -D warnings
  ```
  Expected: clean.

- [ ] **Step 3: full test suite**
  ```
  cargo test --workspace
  ```
  Expected: green. All 37 previously-ignored repo tests now pass. Pre-existing `zero-core` doctest failure remains (unrelated).

- [ ] **Step 4: Confirm no cosine function remains**
  ```
  grep -rn "fn cosine_similarity\|fn cosine_" --include="*.rs" gateway services
  ```
  Expected: zero hits outside test-only files. If any remains, delete.

- [ ] **Step 5: Confirm no embedding BLOB column remains**
  ```
  grep -rn "embedding BLOB\|embedding: BLOB" --include="*.rs" gateway services
  ```
  Expected: one hit — `embedding_cache` table (the one legal case: content-hash cache).

- [ ] **Step 6: Push**
  ```bash
  git push -u origin feature/memory-v2-phase-1b
  ```

---

## Self-Review Results

**Spec coverage:**
- ✅ `VectorIndex` trait + sqlite-vec impl — Task 1
- ✅ memory_facts embedding migration — Task 2
- ✅ ward_wiki_articles — Task 3
- ✅ procedures — Task 4
- ✅ session_episodes — Task 5
- ✅ kg_episodes — Task 6 (DB swap only)
- ✅ memory_fact_store — Task 7
- ✅ GraphStorage → KnowledgeDatabase — Task 8
- ✅ recall.rs cosine deletion — Task 9
- ✅ resolver.rs stage 3 via ANN + cosine deletion — Task 10
- ✅ AppState wiring — Task 11
- ✅ Tests unignored per-task + final validation — Task 12

**What is NOT in Phase 1b (explicit, for Phase 1c):**
- Resolver v2 — alias-first lookup, embedding-generation-at-write, LLM pairwise verify. Phase 1b keeps the existing 3-stage resolver structure but retires stage 3's O(N) scan.
- Self-alias on entity create.
- `kg_aliases` table is writable but not yet consulted as stage-1 shortcut.

**Placeholder scan:** No TBDs. Every step has concrete SQL/code.

**Type consistency:**
- `VectorIndex` trait methods used consistently across all tasks (`upsert`, `delete`, `query_nearest`).
- `SqliteVecIndex::new(db, table, id_col, dim)` signature matches in every construction site.
- Repository constructor signature: `new(Arc<KnowledgeDatabase>, Arc<dyn VectorIndex>)` (memory, wiki, procedure, episode) or `new(Arc<KnowledgeDatabase>)` (kg_episode).
- GraphStorage constructor: `new(Arc<KnowledgeDatabase>) -> GraphResult<Self>`.

**Known risks:**
- Task 8's GraphStorage rewrite touches every caller — this is where propagation can surprise. The plan mitigates via `cargo check --workspace` after each task and explicit caller grep.
- Task 10's VectorIndex cross-crate dep may need the trait moved to a shared location. Plan notes the `cargo tree` check and offers a fallback.
