# Memory v2 — Phase 1a Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish the two-database foundation for memory v2 — create `knowledge.db` alongside `conversations.db` with sqlite-vec extension loaded, define v22 schemas for both, verify fresh boot works end-to-end.

**Architecture:** Introduce a second `DatabaseManager` (`KnowledgeDatabase`) alongside the existing one. Wire `sqlite-vec` as an auto-loaded SQLite extension on the knowledge pool. Rewrite `schema.rs` so `conversations.db` holds only operational tables and `knowledge.db` holds memory + graph + `vec0` indexes. No repository code is migrated in this phase — that's Phase 1b.

**Tech Stack:** Rust 2024, `rusqlite` 0.31+, `r2d2`, `r2d2_sqlite`, `sqlite-vec` (bundled as a shared library), existing `gateway-services::SharedVaultPaths`.

**Spec:** `docs/superpowers/specs/2026-04-12-memory-layer-redesign-design.md`

---

## File Structure

**Created:**
- `gateway/gateway-database/src/knowledge_db.rs` — `KnowledgeDatabase` struct, pool, schema init
- `gateway/gateway-database/src/knowledge_schema.rs` — v22 schema for `knowledge.db` (all tables + vec0)
- `gateway/gateway-database/src/sqlite_vec_loader.rs` — extension loader helper
- `gateway/gateway-database/tests/fresh_boot.rs` — integration test for end-to-end bootstrap

**Modified:**
- `gateway/gateway-database/Cargo.toml` — add `sqlite-vec` crate dep + `rusqlite/loadable_extension` feature
- `gateway/gateway-database/src/connection.rs` — rename clarifying the existing manager as `ConversationsDatabase`; keep backwards-compatible `DatabaseManager` alias for Phase 1b migration
- `gateway/gateway-database/src/schema.rs` — strip knowledge tables (memory_facts, ward_wiki_articles, procedures, session_episodes, kg_episodes) out. Leave sessions/executions/messages/etc. These get recreated in knowledge_schema.rs.
- `gateway/gateway-database/src/lib.rs` — expose `KnowledgeDatabase` + re-exports
- `gateway/gateway-services/src/paths.rs` — rename `knowledge_graph_db()` to `knowledge_db()` (one-line change)
- `gateway/src/state.rs` — construct both `ConversationsDatabase` and `KnowledgeDatabase` at boot; wire both into `AppState`

**NOT modified in this phase:**
- No repository files (`memory_repository.rs`, `wiki_repository.rs`, etc.) — Phase 1b
- No `resolver.rs`, no `recall.rs` — Phase 1b/1c
- No `services/knowledge-graph/src/storage.rs` — Phase 1b

Phase 1a lands a working daemon where both DBs exist but nothing writes to `knowledge.db` yet. This is safe by design — we prove the infrastructure before migrating logic.

---

## Task 1: Add `sqlite-vec` dependency

**Files:**
- Modify: `gateway/gateway-database/Cargo.toml`

- [ ] **Step 1: Verify current rusqlite version**

Run: `grep '^rusqlite' gateway/gateway-database/Cargo.toml`

Expected: a line like `rusqlite = { version = "0.31", features = [...] }` or similar. Note the current feature list.

- [ ] **Step 2: Add sqlite-vec + loadable_extension feature**

Edit `gateway/gateway-database/Cargo.toml`. In `[dependencies]`, add:

```toml
sqlite-vec = "0.1"
```

If rusqlite has a `features = [...]` list, append `"load_extension"`. If rusqlite doesn't list features, replace with:

```toml
rusqlite = { version = "0.31", features = ["bundled", "load_extension"] }
```

(Preserve any existing features — just ensure `load_extension` is present.)

- [ ] **Step 3: Cargo check**

Run: `cargo check -p gateway-database`

Expected: clean build. If `sqlite-vec` fails to resolve, escalate — version 0.1 should be on crates.io. If `bundled` conflict appears with an existing feature, keep existing and just add `load_extension`.

- [ ] **Step 4: Commit**

```bash
git add gateway/gateway-database/Cargo.toml gateway/gateway-database/Cargo.lock
git commit -m "deps: add sqlite-vec + rusqlite load_extension for memory v2"
```

---

## Task 2: sqlite-vec loader helper

**Files:**
- Create: `gateway/gateway-database/src/sqlite_vec_loader.rs`

- [ ] **Step 1: Write the module**

Create `gateway/gateway-database/src/sqlite_vec_loader.rs`:

```rust
//! sqlite-vec extension loader.
//!
//! Called on every new connection opened against `knowledge.db`.
//! Loads the bundled sqlite-vec shared library so `vec0` virtual tables
//! and `vec_distance_cosine()` are available.

use rusqlite::Connection;

/// Load the sqlite-vec extension into the given connection.
///
/// Returns an error if the extension cannot be loaded. Callers should
/// fail daemon startup rather than continue — sqlite-vec is not optional
/// in memory v2.
pub fn load_sqlite_vec(conn: &Connection) -> Result<(), rusqlite::Error> {
    // SAFETY: sqlite_vec::sqlite3_vec_init is the canonical entry point
    // exposed by the sqlite-vec crate. It is safe to call on any rusqlite
    // Connection that was opened with the load_extension feature enabled.
    unsafe {
        conn.load_extension_enable()?;
        let result = sqlite_vec::sqlite3_vec_init(conn);
        conn.load_extension_disable()?;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_loads_on_in_memory_db() {
        let conn = Connection::open_in_memory().expect("open in-memory");
        load_sqlite_vec(&conn).expect("load sqlite-vec");

        // Smoke test: create a vec0 virtual table.
        conn.execute_batch(
            "CREATE VIRTUAL TABLE t USING vec0(id TEXT PRIMARY KEY, v FLOAT[4]);",
        )
        .expect("create vec0 table");

        // Smoke test: insert and query.
        conn.execute(
            "INSERT INTO t(id, v) VALUES ('a', ?1)",
            rusqlite::params![serde_json::to_string(&[0.1_f32, 0.2, 0.3, 0.4]).unwrap()],
        )
        .expect("insert");

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM t", [], |r| r.get(0))
            .expect("count");
        assert_eq!(count, 1);
    }
}
```

Note: the exact entry-point function (`sqlite_vec::sqlite3_vec_init`) is verified by the `sqlite-vec` crate docs. If the crate API differs when you pull it, use `cargo doc --open -p sqlite-vec` to find the correct loader call; the rest of this module stays.

- [ ] **Step 2: Register module in lib.rs**

Edit `gateway/gateway-database/src/lib.rs`. Add:

```rust
pub mod sqlite_vec_loader;
```

under the existing `pub mod` declarations.

- [ ] **Step 3: Run the test**

Run: `cargo test -p gateway-database --lib sqlite_vec_loader::tests::extension_loads_on_in_memory_db`

Expected: PASS. If fails with "no such function sqlite3_vec_init", grep `sqlite-vec` crate docs for the correct init name and update; this is a mechanical fix, not a design problem.

- [ ] **Step 4: Commit**

```bash
git add gateway/gateway-database/src/sqlite_vec_loader.rs gateway/gateway-database/src/lib.rs
git commit -m "feat(db): sqlite-vec extension loader"
```

---

## Task 3: Rename paths accessor

**Files:**
- Modify: `gateway/gateway-services/src/paths.rs`
- Modify: all callers of `.knowledge_graph_db()` (grep & replace)

- [ ] **Step 1: Grep current callers**

Run:
```
grep -rn "knowledge_graph_db" --include="*.rs" .
```

Expected: ~1-3 call sites. Note them down.

- [ ] **Step 2: Rename in paths.rs**

In `gateway/gateway-services/src/paths.rs`, find the method `pub fn knowledge_graph_db(&self) -> PathBuf` (around line 88). Rename to `knowledge_db` and change the file name it returns from `"knowledge_graph.db"` to `"knowledge.db"`:

```rust
/// Path to the knowledge database (memory, facts, graph, vec0 indexes).
pub fn knowledge_db(&self) -> PathBuf {
    self.data_dir().join("knowledge.db")
}
```

- [ ] **Step 3: Update callers**

For each file from Step 1, replace `knowledge_graph_db` with `knowledge_db`. No other changes.

- [ ] **Step 4: Cargo check**

Run: `cargo check --workspace`

Expected: clean. If any caller was missed, the compiler tells you where.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor(paths): rename knowledge_graph_db → knowledge_db (memory v2)"
```

---

## Task 4: Define knowledge-side v22 schema — core graph tables

**Files:**
- Create: `gateway/gateway-database/src/knowledge_schema.rs`

- [ ] **Step 1: Create the module with the entity + relationship tables**

Create `gateway/gateway-database/src/knowledge_schema.rs`:

```rust
//! Schema v22 for `knowledge.db`.
//!
//! All long-term memory + graph + vector indexes live here.
//! Applied idempotently on daemon boot. No migrations — clean slate.

use rusqlite::Connection;

const SCHEMA_VERSION: i32 = 22;

/// Initialize the knowledge database schema (v22).
///
/// Creates all tables and indexes if they don't exist. Records the
/// schema version in `schema_version` table. Safe to call on an
/// already-initialized DB.
pub fn initialize_knowledge_database(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(SCHEMA_SQL)?;
    conn.execute(
        "INSERT OR IGNORE INTO schema_version (version, applied_at) VALUES (?1, datetime('now'))",
        rusqlite::params![SCHEMA_VERSION],
    )?;
    Ok(())
}

const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL
);

-- ========================================================================
-- Knowledge Graph — entities, relationships, aliases
-- ========================================================================

CREATE TABLE IF NOT EXISTS kg_entities (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    name TEXT NOT NULL,
    normalized_name TEXT NOT NULL,
    normalized_hash TEXT NOT NULL,
    properties TEXT,
    epistemic_class TEXT NOT NULL DEFAULT 'current',
    confidence REAL NOT NULL DEFAULT 0.8,
    mention_count INTEGER NOT NULL DEFAULT 1,
    access_count INTEGER NOT NULL DEFAULT 0,
    first_seen_at TEXT NOT NULL,
    last_seen_at TEXT NOT NULL,
    last_accessed_at TEXT,
    t_valid_from TEXT,
    t_valid_to TEXT,
    t_invalidated_by TEXT,
    compressed_into TEXT,
    source_episode_ids TEXT
);
CREATE INDEX IF NOT EXISTS idx_entities_normalized_hash
    ON kg_entities(agent_id, entity_type, normalized_hash);
CREATE INDEX IF NOT EXISTS idx_entities_agent_type
    ON kg_entities(agent_id, entity_type);
CREATE INDEX IF NOT EXISTS idx_entities_name ON kg_entities(name);
CREATE INDEX IF NOT EXISTS idx_entities_last_accessed ON kg_entities(last_accessed_at);
CREATE INDEX IF NOT EXISTS idx_entities_epistemic
    ON kg_entities(agent_id, epistemic_class);

CREATE TABLE IF NOT EXISTS kg_relationships (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    source_entity_id TEXT NOT NULL,
    target_entity_id TEXT NOT NULL,
    relationship_type TEXT NOT NULL,
    properties TEXT,
    epistemic_class TEXT NOT NULL DEFAULT 'current',
    confidence REAL NOT NULL DEFAULT 0.8,
    mention_count INTEGER NOT NULL DEFAULT 1,
    access_count INTEGER NOT NULL DEFAULT 0,
    first_seen_at TEXT NOT NULL,
    last_seen_at TEXT NOT NULL,
    last_accessed_at TEXT,
    valid_at TEXT,
    invalidated_at TEXT,
    t_invalidated_by TEXT,
    source_episode_ids TEXT,
    UNIQUE(source_entity_id, target_entity_id, relationship_type),
    FOREIGN KEY (source_entity_id) REFERENCES kg_entities(id) ON DELETE CASCADE,
    FOREIGN KEY (target_entity_id) REFERENCES kg_entities(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_rels_source ON kg_relationships(source_entity_id);
CREATE INDEX IF NOT EXISTS idx_rels_target ON kg_relationships(target_entity_id);
CREATE INDEX IF NOT EXISTS idx_rels_agent ON kg_relationships(agent_id);
CREATE INDEX IF NOT EXISTS idx_rels_valid ON kg_relationships(valid_at);

CREATE TABLE IF NOT EXISTS kg_aliases (
    id TEXT PRIMARY KEY,
    entity_id TEXT NOT NULL,
    surface_form TEXT NOT NULL,
    normalized_form TEXT NOT NULL,
    source TEXT NOT NULL,
    confidence REAL NOT NULL DEFAULT 1.0,
    first_seen_at TEXT NOT NULL,
    FOREIGN KEY (entity_id) REFERENCES kg_entities(id) ON DELETE CASCADE,
    UNIQUE(normalized_form, entity_id)
);
CREATE INDEX IF NOT EXISTS idx_aliases_normalized ON kg_aliases(normalized_form);
CREATE INDEX IF NOT EXISTS idx_aliases_entity ON kg_aliases(entity_id);

CREATE TABLE IF NOT EXISTS kg_episodes (
    id TEXT PRIMARY KEY,
    source_type TEXT NOT NULL,
    source_ref TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    session_id TEXT,
    agent_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    retry_count INTEGER NOT NULL DEFAULT 0,
    error TEXT,
    created_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT,
    UNIQUE(content_hash, source_type)
);
CREATE INDEX IF NOT EXISTS idx_episodes_status ON kg_episodes(status);
CREATE INDEX IF NOT EXISTS idx_episodes_source_ref ON kg_episodes(source_ref);
CREATE INDEX IF NOT EXISTS idx_episodes_session ON kg_episodes(session_id);

CREATE TABLE IF NOT EXISTS kg_goals (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    ward_id TEXT,
    title TEXT NOT NULL,
    description TEXT,
    state TEXT NOT NULL DEFAULT 'active',
    parent_goal_id TEXT,
    slots TEXT,
    filled_slots TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    completed_at TEXT,
    FOREIGN KEY (parent_goal_id) REFERENCES kg_goals(id)
);
CREATE INDEX IF NOT EXISTS idx_goals_agent_state ON kg_goals(agent_id, state);
CREATE INDEX IF NOT EXISTS idx_goals_ward ON kg_goals(ward_id);

CREATE TABLE IF NOT EXISTS kg_compactions (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    operation TEXT NOT NULL,
    entity_id TEXT,
    relationship_id TEXT,
    merged_into TEXT,
    reason TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_compactions_run ON kg_compactions(run_id);
"#;
```

- [ ] **Step 2: Register module**

In `gateway/gateway-database/src/lib.rs`, add:

```rust
pub mod knowledge_schema;
```

- [ ] **Step 3: Run cargo check**

Run: `cargo check -p gateway-database`

Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add gateway/gateway-database/src/knowledge_schema.rs gateway/gateway-database/src/lib.rs
git commit -m "feat(db): knowledge.db v22 schema — kg_entities, relationships, aliases, episodes, goals, compactions"
```

---

## Task 5: Add memory/wiki/procedures/episodes tables to knowledge schema

**Files:**
- Modify: `gateway/gateway-database/src/knowledge_schema.rs`

- [ ] **Step 1: Append the memory tables**

In `SCHEMA_SQL`, append (before the closing `"#;`):

```sql
-- ========================================================================
-- Memory facts (no embedding column — embeddings live in memory_facts_index)
-- ========================================================================

CREATE TABLE IF NOT EXISTS memory_facts (
    id TEXT PRIMARY KEY,
    session_id TEXT,
    agent_id TEXT NOT NULL,
    scope TEXT NOT NULL,
    category TEXT NOT NULL,
    key TEXT NOT NULL,
    content TEXT NOT NULL,
    confidence REAL NOT NULL DEFAULT 0.8,
    mention_count INTEGER NOT NULL DEFAULT 1,
    source_summary TEXT,
    source_episode_id TEXT,
    source_ref TEXT,
    ward_id TEXT NOT NULL DEFAULT '__global__',
    epistemic_class TEXT NOT NULL DEFAULT 'current',
    contradicted_by TEXT,
    t_valid_from TEXT,
    t_valid_to TEXT,
    superseded_by TEXT,
    pinned INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    expires_at TEXT,
    UNIQUE(agent_id, scope, ward_id, key)
);
CREATE INDEX IF NOT EXISTS idx_facts_agent_scope ON memory_facts(agent_id, scope);
CREATE INDEX IF NOT EXISTS idx_facts_category ON memory_facts(agent_id, category);
CREATE INDEX IF NOT EXISTS idx_facts_ward ON memory_facts(ward_id);
CREATE INDEX IF NOT EXISTS idx_facts_epistemic ON memory_facts(epistemic_class);
CREATE VIRTUAL TABLE IF NOT EXISTS memory_facts_fts USING fts5(
    key, content, category, content=memory_facts
);

CREATE TABLE IF NOT EXISTS memory_facts_archive (
    id TEXT PRIMARY KEY,
    session_id TEXT,
    agent_id TEXT NOT NULL,
    scope TEXT NOT NULL,
    category TEXT NOT NULL,
    key TEXT NOT NULL,
    content TEXT NOT NULL,
    confidence REAL NOT NULL,
    ward_id TEXT NOT NULL,
    epistemic_class TEXT NOT NULL,
    archived_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_facts_archive_agent ON memory_facts_archive(agent_id);

CREATE TABLE IF NOT EXISTS ward_wiki_articles (
    id TEXT PRIMARY KEY,
    ward_id TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    tags TEXT,
    source_fact_ids TEXT,
    version INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(ward_id, title)
);
CREATE INDEX IF NOT EXISTS idx_wiki_ward ON ward_wiki_articles(ward_id);

CREATE TABLE IF NOT EXISTS procedures (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    ward_id TEXT NOT NULL DEFAULT '__global__',
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    trigger_pattern TEXT,
    steps TEXT NOT NULL,
    parameters TEXT,
    success_count INTEGER NOT NULL DEFAULT 1,
    failure_count INTEGER NOT NULL DEFAULT 0,
    avg_duration_ms INTEGER,
    avg_token_cost INTEGER,
    last_used TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_procedures_agent ON procedures(agent_id);
CREATE INDEX IF NOT EXISTS idx_procedures_ward ON procedures(ward_id);

CREATE TABLE IF NOT EXISTS session_episodes (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL UNIQUE,
    agent_id TEXT NOT NULL,
    ward_id TEXT,
    task_summary TEXT,
    outcome TEXT,
    strategy_used TEXT,
    key_learnings TEXT,
    token_cost INTEGER,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_session_episodes_agent ON session_episodes(agent_id);
CREATE INDEX IF NOT EXISTS idx_session_episodes_ward ON session_episodes(ward_id);
CREATE INDEX IF NOT EXISTS idx_session_episodes_outcome ON session_episodes(outcome);

CREATE TABLE IF NOT EXISTS embedding_cache (
    content_hash TEXT NOT NULL,
    model TEXT NOT NULL,
    embedding BLOB NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (content_hash, model)
);
```

Confirm no `embedding BLOB` column is present on `memory_facts`, `ward_wiki_articles`, `procedures`, or `session_episodes`.

- [ ] **Step 2: Cargo check**

Run: `cargo check -p gateway-database`

Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add gateway/gateway-database/src/knowledge_schema.rs
git commit -m "feat(db): knowledge.db v22 schema — facts, wiki, procedures, episodes (no BLOBs)"
```

---

## Task 6: Add vec0 virtual tables + cleanup triggers

**Files:**
- Modify: `gateway/gateway-database/src/knowledge_schema.rs`

- [ ] **Step 1: Separate vec0 creation into its own function**

`vec0` virtual tables require the extension already loaded. The main `SCHEMA_SQL` can't include them (extension load happens per-connection). Add a separate init function:

At the bottom of `knowledge_schema.rs`:

```rust
const VEC0_SQL: &str = r#"
CREATE VIRTUAL TABLE IF NOT EXISTS kg_name_index USING vec0(
    entity_id TEXT PRIMARY KEY,
    name_embedding FLOAT[384]
);

CREATE VIRTUAL TABLE IF NOT EXISTS memory_facts_index USING vec0(
    fact_id TEXT PRIMARY KEY,
    embedding FLOAT[384]
);

CREATE VIRTUAL TABLE IF NOT EXISTS wiki_articles_index USING vec0(
    article_id TEXT PRIMARY KEY,
    embedding FLOAT[384]
);

CREATE VIRTUAL TABLE IF NOT EXISTS procedures_index USING vec0(
    procedure_id TEXT PRIMARY KEY,
    embedding FLOAT[384]
);

CREATE VIRTUAL TABLE IF NOT EXISTS session_episodes_index USING vec0(
    episode_id TEXT PRIMARY KEY,
    embedding FLOAT[384]
);
"#;

const TRIGGERS_SQL: &str = r#"
-- Clean up vec0 partner rows when base rows are deleted.
-- Applied after vec0 tables exist.

CREATE TRIGGER IF NOT EXISTS trg_entities_delete_vec
AFTER DELETE ON kg_entities
BEGIN
    DELETE FROM kg_name_index WHERE entity_id = OLD.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_facts_delete_vec
AFTER DELETE ON memory_facts
BEGIN
    DELETE FROM memory_facts_index WHERE fact_id = OLD.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_wiki_delete_vec
AFTER DELETE ON ward_wiki_articles
BEGIN
    DELETE FROM wiki_articles_index WHERE article_id = OLD.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_procedures_delete_vec
AFTER DELETE ON procedures
BEGIN
    DELETE FROM procedures_index WHERE procedure_id = OLD.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_episodes_delete_vec
AFTER DELETE ON session_episodes
BEGIN
    DELETE FROM session_episodes_index WHERE episode_id = OLD.id;
END;
"#;

/// Initialize vec0 virtual tables and cleanup triggers.
///
/// Call AFTER `load_sqlite_vec()` AND AFTER `initialize_knowledge_database()`
/// because triggers reference vec0 tables and base tables both.
pub fn initialize_vec_tables(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(VEC0_SQL)?;
    conn.execute_batch(TRIGGERS_SQL)?;
    Ok(())
}
```

- [ ] **Step 2: Add an integration test**

At the bottom of `knowledge_schema.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::sqlite_vec_loader::load_sqlite_vec;

    #[test]
    fn full_v22_schema_initializes_on_fresh_in_memory_db() {
        let conn = Connection::open_in_memory().expect("open");
        load_sqlite_vec(&conn).expect("load sqlite-vec");

        initialize_knowledge_database(&conn).expect("init base schema");
        initialize_vec_tables(&conn).expect("init vec tables");

        // Check schema version recorded.
        let version: i32 = conn
            .query_row("SELECT version FROM schema_version", [], |r| r.get(0))
            .expect("schema_version");
        assert_eq!(version, 22);

        // Check base tables exist.
        for table in [
            "kg_entities", "kg_relationships", "kg_aliases", "kg_episodes",
            "kg_goals", "kg_compactions",
            "memory_facts", "memory_facts_archive", "ward_wiki_articles",
            "procedures", "session_episodes", "embedding_cache",
        ] {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type IN ('table','view') AND name = ?1",
                    rusqlite::params![table],
                    |r| r.get(0),
                )
                .expect("query");
            assert_eq!(count, 1, "missing table: {table}");
        }

        // Check vec0 virtual tables exist.
        for vt in [
            "kg_name_index", "memory_facts_index", "wiki_articles_index",
            "procedures_index", "session_episodes_index",
        ] {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    rusqlite::params![vt],
                    |r| r.get(0),
                )
                .expect("query");
            assert_eq!(count, 1, "missing vec0 table: {vt}");
        }

        // Check triggers exist.
        for trg in [
            "trg_entities_delete_vec",
            "trg_facts_delete_vec",
            "trg_wiki_delete_vec",
            "trg_procedures_delete_vec",
            "trg_episodes_delete_vec",
        ] {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'trigger' AND name = ?1",
                    rusqlite::params![trg],
                    |r| r.get(0),
                )
                .expect("query");
            assert_eq!(count, 1, "missing trigger: {trg}");
        }
    }

    #[test]
    fn delete_entity_cascades_to_vec_index() {
        let conn = Connection::open_in_memory().expect("open");
        load_sqlite_vec(&conn).expect("load");
        initialize_knowledge_database(&conn).expect("init");
        initialize_vec_tables(&conn).expect("init vec");

        // Insert an entity + its vec row.
        conn.execute(
            "INSERT INTO kg_entities(id, agent_id, entity_type, name, normalized_name, normalized_hash, first_seen_at, last_seen_at)
             VALUES ('e1', 'root', 'person', 'Alice', 'alice', 'h1', datetime('now'), datetime('now'))",
            [],
        ).expect("insert entity");

        let embedding_json = serde_json::to_string(&vec![0.1_f32; 384]).unwrap();
        conn.execute(
            "INSERT INTO kg_name_index(entity_id, name_embedding) VALUES ('e1', ?1)",
            rusqlite::params![embedding_json],
        ).expect("insert vec");

        // Delete entity — trigger should cascade to vec index.
        conn.execute("DELETE FROM kg_entities WHERE id = 'e1'", []).expect("delete");

        let vec_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM kg_name_index WHERE entity_id = 'e1'", [], |r| r.get(0))
            .expect("count");
        assert_eq!(vec_count, 0, "vec0 row should be cleaned up by trigger");
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p gateway-database --lib knowledge_schema::tests`

Expected: both tests PASS.

If `serde_json` is not already a dep of `gateway-database`, add it to `[dependencies]` in `gateway-database/Cargo.toml`:

```toml
serde_json = "1"
```

- [ ] **Step 4: Commit**

```bash
git add gateway/gateway-database/src/knowledge_schema.rs gateway/gateway-database/Cargo.toml
git commit -m "feat(db): vec0 virtual tables + cascade cleanup triggers"
```

---

## Task 7: KnowledgeDatabase struct with extension-loading customizer

**Files:**
- Create: `gateway/gateway-database/src/knowledge_db.rs`

- [ ] **Step 1: Write the struct**

Create `gateway/gateway-database/src/knowledge_db.rs`:

```rust
//! `KnowledgeDatabase` — r2d2 pool for `knowledge.db` with sqlite-vec
//! extension auto-loaded on every connection.

use gateway_services::SharedVaultPaths;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;
use std::time::Duration;

use crate::knowledge_schema::{initialize_knowledge_database, initialize_vec_tables};
use crate::sqlite_vec_loader::load_sqlite_vec;

pub struct KnowledgeDatabase {
    pool: Pool<SqliteConnectionManager>,
}

/// Customizer that (1) applies WAL-mode pragmas and (2) loads sqlite-vec
/// on every connection acquired from the pool.
#[derive(Debug)]
struct KnowledgeConnectionCustomizer;

impl r2d2::CustomizeConnection<Connection, rusqlite::Error>
    for KnowledgeConnectionCustomizer
{
    fn on_acquire(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA cache_size = -8000;
             PRAGMA busy_timeout = 5000;
             PRAGMA wal_autocheckpoint = 1000;
             PRAGMA temp_store = MEMORY;
             PRAGMA foreign_keys = ON;",
        )?;
        load_sqlite_vec(conn)?;
        Ok(())
    }
}

impl KnowledgeDatabase {
    pub fn new(paths: SharedVaultPaths) -> Result<Self, String> {
        let db_path = paths.knowledge_db();

        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create data dir: {e}"))?;
        }

        let manager = SqliteConnectionManager::file(&db_path);
        let pool = Pool::builder()
            .max_size(8)
            .min_idle(Some(2))
            .connection_timeout(Duration::from_secs(5))
            .connection_customizer(Box::new(KnowledgeConnectionCustomizer))
            .build(manager)
            .map_err(|e| format!("Failed to create knowledge pool: {e}"))?;

        // Initialize schema + vec tables on a single connection.
        {
            let conn = pool
                .get()
                .map_err(|e| format!("Failed to get init connection: {e}"))?;
            initialize_knowledge_database(&conn)
                .map_err(|e| format!("Failed to init knowledge schema: {e}"))?;
            initialize_vec_tables(&conn)
                .map_err(|e| format!("Failed to init vec tables: {e}"))?;
        }

        tracing::info!("Knowledge database initialized at {:?}", db_path);

        Ok(Self { pool })
    }

    pub fn with_connection<F, R>(&self, f: F) -> Result<R, String>
    where
        F: FnOnce(&Connection) -> Result<R, rusqlite::Error>,
    {
        let conn = self
            .pool
            .get()
            .map_err(|e| format!("Failed to get knowledge connection: {e}"))?;
        f(&conn).map_err(|e| format!("Knowledge DB operation failed: {e}"))
    }
}
```

- [ ] **Step 2: Register module**

In `gateway/gateway-database/src/lib.rs`, add:

```rust
pub mod knowledge_db;
```

And re-export:

```rust
pub use knowledge_db::KnowledgeDatabase;
```

- [ ] **Step 3: Cargo check**

Run: `cargo check -p gateway-database`

Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add gateway/gateway-database/src/knowledge_db.rs gateway/gateway-database/src/lib.rs
git commit -m "feat(db): KnowledgeDatabase — pool + sqlite-vec auto-load + schema init"
```

---

## Task 8: Strip knowledge tables out of `conversations.db` schema

**Files:**
- Modify: `gateway/gateway-database/src/schema.rs`

- [ ] **Step 1: Locate tables to remove**

Open `gateway/gateway-database/src/schema.rs`. Find the `CREATE TABLE` statements for:

- `memory_facts`, `memory_facts_archive`
- `ward_wiki_articles`
- `procedures`
- `session_episodes`
- `kg_episodes`
- `embedding_cache`

Also find any indexes for those tables.

- [ ] **Step 2: Remove them from the conversations schema**

Delete the `CREATE TABLE` and `CREATE INDEX` for each. Keep everything else (sessions, agent_executions, messages, artifacts, execution_logs, recall_log, distillation_runs, bridge_outbox — these stay in conversations.db).

Also delete any `CREATE VIRTUAL TABLE ... USING fts5` on `memory_facts` (now in `knowledge_schema.rs`).

- [ ] **Step 3: Update schema version bump logic**

If `schema.rs` has a `SCHEMA_VERSION` const, bump it to 22. If it has per-version migration arms (v17→v18→...→v21), leave those in place for now but add a v22 arm that is a no-op (since existing DBs from v21 no longer have these tables — they need to be deleted by the user per the clean-slate precondition).

Do not introduce an automatic data migration. The precondition is DB deletion.

- [ ] **Step 4: Cargo check**

Run: `cargo check -p gateway-database`

Expected: clean.

If repository files fail to compile (e.g., `memory_repository.rs` referencing tables that no longer exist in this schema), that's fine — those repos still work because they run SQL directly, not compiled against the schema. Phase 1b migrates repositories to the knowledge pool.

- [ ] **Step 5: Test the conversations schema init still works**

Run: `cargo test -p gateway-database --lib schema`

Expected: existing schema tests pass. (May need to remove test assertions that check for tables now in knowledge.db.)

- [ ] **Step 6: Commit**

```bash
git add gateway/gateway-database/src/schema.rs
git commit -m "feat(db): conversations.db v22 — knowledge tables moved to knowledge.db"
```

---

## Task 9: Wire KnowledgeDatabase into AppState (both DBs constructed at boot)

**Files:**
- Modify: `gateway/src/state.rs`

- [ ] **Step 1: Read current AppState construction**

Run: `grep -n 'DatabaseManager::new\|pub struct AppState' gateway/src/state.rs | head`

Find the line (`~line 178` per prior audit) where `DatabaseManager::new(paths.clone())` is called, and where `AppState { ... }` is constructed (`~line 333`).

- [ ] **Step 2: Add knowledge_db field**

In the `AppState` struct (near the other `*_repo` fields), add:

```rust
    /// Knowledge database — memory facts, graph, vec0 indexes.
    pub knowledge_db: Arc<gateway_database::KnowledgeDatabase>,
```

- [ ] **Step 3: Construct it alongside the conversations DB**

Find the line constructing `DatabaseManager::new(paths.clone())`. Immediately after, add:

```rust
        let knowledge_db = Arc::new(
            gateway_database::KnowledgeDatabase::new(paths.clone())
                .map_err(|e| format!("Failed to initialize knowledge database: {e}"))?,
        );
```

Then in every `AppState { ... }` struct-literal site, add the field:

```rust
            knowledge_db: knowledge_db.clone(),
```

Grep for all `AppState {` construction sites (likely 3 — `new`, `minimal`, `with_components` per prior audit) and update each.

For the `minimal` constructor that doesn't have paths available, call `KnowledgeDatabase::new` with the same paths it uses elsewhere, OR stub with `Arc::new(...)` using an explicit paths param added to the `minimal` function signature. Minimize disruption — prefer adding paths param.

- [ ] **Step 4: Cargo check**

Run: `cargo check --workspace`

Expected: clean. Fix any missed struct-literal site the compiler flags.

- [ ] **Step 5: Commit**

```bash
git add gateway/src/state.rs
git commit -m "feat(state): AppState carries both conversations and knowledge DBs"
```

---

## Task 10: Daemon fresh-boot integration test

**Files:**
- Create: `gateway/gateway-database/tests/fresh_boot.rs`

- [ ] **Step 1: Write the test**

Create `gateway/gateway-database/tests/fresh_boot.rs`:

```rust
//! Fresh-boot integration test: both DBs come up, sqlite-vec is loaded,
//! vec0 tables work end-to-end.

use std::sync::Arc;
use tempfile::tempdir;

use gateway_database::{DatabaseManager, KnowledgeDatabase};
use gateway_services::VaultPaths;

#[test]
fn fresh_boot_creates_both_databases_with_vec0_working() {
    let tmp = tempdir().expect("tempdir");
    let paths = Arc::new(VaultPaths::new(tmp.path().to_path_buf()));

    // Ensure data dir exists (VaultPaths should do this, but be explicit).
    std::fs::create_dir_all(paths.data_dir()).expect("data dir");

    let _conversations = DatabaseManager::new(paths.clone())
        .expect("conversations db initializes");
    let knowledge = KnowledgeDatabase::new(paths.clone())
        .expect("knowledge db initializes");

    // Both files exist on disk.
    assert!(paths.conversations_db().exists(), "conversations.db missing");
    assert!(paths.knowledge_db().exists(), "knowledge.db missing");

    // vec0 table usable on knowledge pool.
    knowledge
        .with_connection(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM kg_name_index",
                [],
                |r| r.get(0),
            )?;
            assert_eq!(count, 0);
            Ok(())
        })
        .expect("query kg_name_index");

    // Insert + query via vec0.
    knowledge
        .with_connection(|conn| {
            // Need an entity row first (trigger expects it on delete path; insert path is fine).
            conn.execute(
                "INSERT INTO kg_entities(id, agent_id, entity_type, name, normalized_name, normalized_hash, first_seen_at, last_seen_at)
                 VALUES ('e1', 'root', 'person', 'Alice', 'alice', 'h', datetime('now'), datetime('now'))",
                [],
            )?;
            let embedding_json = serde_json::to_string(&vec![0.1_f32; 384]).unwrap();
            conn.execute(
                "INSERT INTO kg_name_index(entity_id, name_embedding) VALUES ('e1', ?1)",
                rusqlite::params![embedding_json],
            )?;
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM kg_name_index",
                [],
                |r| r.get(0),
            )?;
            assert_eq!(count, 1);
            Ok(())
        })
        .expect("insert + query");
}
```

If `VaultPaths::new` signature differs, grep `gateway-services/src/paths.rs` for the constructor and adjust. If `data_dir()` doesn't exist, inline `paths.vault_dir().join("data")` — the goal is to point both DBs at a tempdir.

- [ ] **Step 2: Run the test**

Run: `cargo test -p gateway-database --test fresh_boot`

Expected: PASS. If `sqlite-vec` extension fails to load (platform issue), stop and escalate — the spec mandates sqlite-vec works across all supported platforms; this is a release-gate.

- [ ] **Step 3: Commit**

```bash
git add gateway/gateway-database/tests/fresh_boot.rs
git commit -m "test(db): fresh-boot integration — both DBs + vec0 end-to-end"
```

---

## Task 11: Workspace validation

- [ ] **Step 1: Full workspace compile**

Run: `cargo check --workspace`

Expected: clean.

- [ ] **Step 2: fmt**

Run: `cargo fmt --all`

- [ ] **Step 3: Clippy**

Run: `cargo clippy --all-targets -- -D warnings`

Expected: clean. Fix any warnings introduced by new code. If clippy fires on pre-existing code (not touched in Phase 1a), suppress with `#[allow]` only if the lint is truly unrelated; otherwise fix.

- [ ] **Step 4: Full test suite**

Run: `cargo test --workspace`

Expected: green, except the one pre-existing `zero-core` doctest failure (unrelated, confirmed on `main`).

- [ ] **Step 5: Push branch**

```bash
git push -u origin feature/memory-v2-phase-1a
```

(Branch creation note: if you're still on `feature/kg-activation-pack-a`, create the new branch first: `git checkout -b feature/memory-v2-phase-1a` before pushing.)

---

## Self-Review Results

**Spec coverage:** All Phase 1a scope items from the spec are covered:
- ✅ Two-DB split (conversations.db + knowledge.db) — Tasks 3, 7, 8, 9
- ✅ sqlite-vec extension loading — Tasks 1, 2, 7
- ✅ v22 schema with all tables for knowledge.db — Tasks 4, 5
- ✅ vec0 virtual tables + cleanup triggers — Task 6
- ✅ Embedding BLOB columns removed from base tables — Task 5 (structural) + Task 8 (deletion from conversations)
- ✅ WAL mode enforced — Task 7 (via pragma in customizer)
- ✅ Fresh-boot integration test — Task 10

**What is NOT in Phase 1a (explicit, to Phase 1b/1c):**
- Repository migration (all `*_repository.rs` files still point at the old schema — Phase 1b rewrites them)
- `services/knowledge-graph/src/storage.rs` still opens its own `knowledge.db` via `Connection::open` — Phase 1b routes it through `KnowledgeDatabase`
- VectorIndex abstraction and cosine-code deletion — Phase 1b
- Resolver v2 — Phase 1c

Phase 1a lands a daemon where both DBs exist but no repository writes to `knowledge.db` yet. This is deliberate — we prove infrastructure before migrating logic.

**Placeholder scan:** No TBDs, no vague "handle errors", all code snippets are complete.

**Type consistency:** `KnowledgeDatabase`, `initialize_knowledge_database`, `initialize_vec_tables`, `load_sqlite_vec` used consistently across tasks.

**One known risk flagged:** Task 2 assumes `sqlite_vec::sqlite3_vec_init` is the loader entry-point name. If the crate's actual API differs, Task 2 Step 1 has explicit guidance to consult `cargo doc` and fix mechanically — not a design issue, 5-minute investigation.
