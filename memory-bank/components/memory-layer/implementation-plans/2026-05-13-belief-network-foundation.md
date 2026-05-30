# Belief Network Foundation (Phase 4) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Apply temporal decay to knowledge-graph entity and relationship `confidence` (today these columns exist but never decay), and add an `evidence` JSON column on both tables as the schema foundation for future contradiction-propagation work.

**Architecture:** The DB columns are already in place (`confidence REAL DEFAULT 0.8` on both `kg_entities` and `kg_relationships`). What's missing is (1) decay logic — a sleep-time pass that reduces confidence based on `last_seen_at` age, (2) an `evidence` JSON column to hold the source-fact provenance later phases will populate, and (3) configurable knobs in `RecallConfig`. Recall integration (using confidence as a traversal weight) is intentionally out of scope — the user chose "just decay" so this phase only updates the numbers, doesn't change how they're read.

**Tech Stack:** Rust, SQLite (via `stores/zero-stores-sqlite`), `gateway-services::RecallConfig`, `gateway-execution::sleep::DecayEngine` + worker.

---

## Files Changed

| File | What changes |
|------|-------------|
| `stores/zero-stores-sqlite/src/knowledge_schema.rs` | Add `evidence TEXT` column to `kg_entities` and `kg_relationships` table definitions (idempotent — `CREATE TABLE IF NOT EXISTS` plus explicit `ALTER TABLE ADD COLUMN` migration guards for existing DBs) |
| `gateway/gateway-services/src/recall_config.rs` | Add `KgDecayConfig` substruct to `RecallConfig` with `enabled`, `entity_half_life_days`, `relationship_half_life_days`, `min_confidence`, `skip_recent_hours` |
| `stores/zero-stores/src/knowledge_graph.rs` | Add two trait methods: `decay_entity_confidence` and `decay_relationship_confidence` (returns count of rows updated) |
| `stores/zero-stores-sqlite/src/kg/storage.rs` | Implement the two trait methods — fetch stale rows, compute new confidence in Rust, batch UPDATE |
| `gateway/gateway-execution/src/sleep/decay.rs` | Add `decay_kg_confidence(agent_id, kg_decay_config)` method to `DecayEngine`; new stats struct `KgDecayStats` |
| `gateway/gateway-execution/src/sleep/worker.rs` | Add `decay_kg` block to `run_cycle` between synthesis and existing `list_prune_candidates`; add `kg_entities_decayed`/`kg_relationships_decayed` to `CycleStats` |
| `gateway/src/state/mod.rs` | Pass `RecallConfig::default()` (or loaded) into `DecayEngine::new` so the engine has access to the new config |
| `memory-bank/future-state/2026-05-13-memory-crate-extraction-tracking.md` | Append Phase 4 foundation entries (schema migration, KG decay, settings additions) to the tracking doc |

---

## Task 1: Add `evidence` column to KG tables

**Files:**
- Modify: `stores/zero-stores-sqlite/src/knowledge_schema.rs`

### Context

`kg_entities` and `kg_relationships` are defined in `knowledge_schema.rs` around lines 66 and 96. Adding a new column requires both updating the `CREATE TABLE` statement (for fresh DBs) AND running a separate `ALTER TABLE` for existing DBs that already have these tables.

SQLite supports `ALTER TABLE ... ADD COLUMN`. To make it idempotent across re-runs, query `pragma_table_info` first and only add when missing.

- [ ] **Step 1.1: Find existing migration / schema-init pattern**

```bash
cd /home/videogamer/projects/agentzero
grep -n "ALTER TABLE\|pragma_table_info\|add_column" stores/zero-stores-sqlite/src/knowledge_schema.rs | head -10
grep -n "fn init_schema\|fn migrate\|fn apply" stores/zero-stores-sqlite/src/*.rs | head -10
```

Read whatever pattern exists. If migrations use `ALTER TABLE ... ADD COLUMN` guarded by a `pragma_table_info` check, follow that. If they use sequential migration files, add a new one.

- [ ] **Step 1.2: Add `evidence TEXT` to the `CREATE TABLE` statements**

In `stores/zero-stores-sqlite/src/knowledge_schema.rs`, find:

```sql
CREATE TABLE IF NOT EXISTS kg_entities (
    ...
    source_episode_ids TEXT
);
```

Change the column list to add `evidence TEXT` after `source_episode_ids`:

```sql
CREATE TABLE IF NOT EXISTS kg_entities (
    ...
    source_episode_ids TEXT,
    evidence TEXT
);
```

Same for `kg_relationships`:

```sql
CREATE TABLE IF NOT EXISTS kg_relationships (
    ...
    source_episode_ids TEXT,
    evidence TEXT,
    UNIQUE(source_entity_id, target_entity_id, relationship_type),
    ...
);
```

(Note: the `UNIQUE` and `FOREIGN KEY` clauses come after columns — be careful with the comma placement so `evidence TEXT,` sits before them.)

- [ ] **Step 1.3: Add migration for existing DBs**

In the same file (or wherever the schema-init code runs after `CREATE TABLE`), append a migration block that uses `pragma_table_info` to add the column only when absent. This Rust snippet goes in the function that executes the schema SQL (find the function that calls `conn.execute_batch(...)` on the schema string):

```rust
/// Ensure the `evidence TEXT` column exists on the given KG table.
/// Idempotent — does nothing if the column already exists.
fn ensure_evidence_column(conn: &rusqlite::Connection, table: &str) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let has_evidence = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(|r| r.ok())
        .any(|name| name == "evidence");
    if !has_evidence {
        conn.execute(&format!("ALTER TABLE {table} ADD COLUMN evidence TEXT"), [])?;
    }
    Ok(())
}
```

Then call it after schema init:

```rust
ensure_evidence_column(conn, "kg_entities")?;
ensure_evidence_column(conn, "kg_relationships")?;
```

If the existing code structure doesn't have a clear spot for this, ask before guessing.

- [ ] **Step 1.4: Write a test that creates a DB, runs schema init, and verifies the column exists**

Add to the test module (search for `#[cfg(test)]` near the bottom of `knowledge_schema.rs`; if none exists in that file, create the test in `stores/zero-stores-sqlite/src/kg/storage.rs` test module instead):

```rust
#[test]
fn kg_entities_and_relationships_have_evidence_column() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = std::sync::Arc::new(
        gateway_services::VaultPaths::new(tmp.path().to_path_buf())
    );
    std::fs::create_dir_all(paths.conversations_db().parent().unwrap()).unwrap();
    let db = crate::KnowledgeDatabase::new(paths).expect("db");

    db.with_connection(|conn| {
        for table in ["kg_entities", "kg_relationships"] {
            let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
            let cols: Vec<String> = stmt
                .query_map([], |row| row.get::<_, String>(1))?
                .filter_map(|r| r.ok())
                .collect();
            assert!(
                cols.contains(&"evidence".to_string()),
                "{table} must have evidence column; got: {cols:?}"
            );
        }
        Ok(())
    })
    .unwrap();
}
```

- [ ] **Step 1.5: Run tests**

```bash
cargo test -p zero-stores-sqlite kg_entities_and_relationships_have_evidence_column -- --nocapture 2>&1 | tail -10
```

Expected: PASS.

- [ ] **Step 1.6: Cargo check**

```bash
cargo check --workspace 2>&1 | grep "^error" | head -5
```

Expected: no errors.

- [ ] **Step 1.7: Commit**

```bash
git add stores/zero-stores-sqlite/src/knowledge_schema.rs stores/zero-stores-sqlite/src/kg/storage.rs
git commit -m "feat(kg): add evidence column to kg_entities and kg_relationships"
```

(Adjust the `git add` paths if the test ended up in a different file.)

---

## Task 2: Add `KgDecayConfig` to `RecallConfig`

**Files:**
- Modify: `gateway/gateway-services/src/recall_config.rs`

### Context

`RecallConfig` already has `temporal_decay: TemporalDecayConfig` (per-category half-lives for memory_facts). KG decay needs its own config because the model is simpler — one half-life per entity-vs-relationship, plus a floor and a "skip recent" guard.

- [ ] **Step 2.1: Write failing tests**

In `gateway/gateway-services/src/recall_config.rs`, find the existing test module and add:

```rust
#[test]
fn kg_decay_config_defaults() {
    let c = RecallConfig::default();
    assert!(c.kg_decay.enabled);
    assert_eq!(c.kg_decay.entity_half_life_days, 90.0);
    assert_eq!(c.kg_decay.relationship_half_life_days, 90.0);
    assert_eq!(c.kg_decay.min_confidence, 0.01);
    assert_eq!(c.kg_decay.skip_recent_hours, 24);
}

#[test]
fn kg_decay_partial_override() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config");
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(
        path.join("recall_config.json"),
        r#"{"kg_decay": {"entity_half_life_days": 30.0}}"#,
    )
    .unwrap();
    let c = RecallConfig::load_from_path(dir.path());
    assert_eq!(c.kg_decay.entity_half_life_days, 30.0);
    // others remain default
    assert_eq!(c.kg_decay.relationship_half_life_days, 90.0);
    assert!(c.kg_decay.enabled);
}
```

- [ ] **Step 2.2: Run to confirm failure**

```bash
cargo test -p gateway-services kg_decay -- --nocapture 2>&1 | tail -10
```

Expected: compile error (`kg_decay` field doesn't exist).

- [ ] **Step 2.3: Add the struct and field**

Add the struct definition near the other config substructs (e.g. after `TemporalDecayConfig`):

```rust
/// Knowledge-graph decay configuration — controls how entity and
/// relationship `confidence` is reduced over time based on `last_seen_at`.
/// Applied during the sleep-time cycle. Unlike `temporal_decay` (which is
/// per-category for `memory_facts`), KG decay uses a single half-life
/// for entities and another for relationships.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KgDecayConfig {
    pub enabled: bool,
    pub entity_half_life_days: f64,
    pub relationship_half_life_days: f64,
    /// Floor — confidence never drops below this value.
    pub min_confidence: f64,
    /// Skip rows whose `last_seen_at` is within this many hours.
    pub skip_recent_hours: i64,
}

impl Default for KgDecayConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            entity_half_life_days: 90.0,
            relationship_half_life_days: 90.0,
            min_confidence: 0.01,
            skip_recent_hours: 24,
        }
    }
}
```

In `RecallConfig`, add the field after `session_offload`:

```rust
pub session_offload: SessionOffloadConfig,
pub kg_decay: KgDecayConfig,
```

In `RecallConfig::default()`, populate it:

```rust
session_offload: SessionOffloadConfig::default(),
kg_decay: KgDecayConfig::default(),
```

- [ ] **Step 2.4: Run tests**

```bash
cargo test -p gateway-services -- --nocapture 2>&1 | grep -E "FAILED|ok\." | head -10
```

Expected: all pass, including the two new tests.

- [ ] **Step 2.5: Commit**

```bash
git add gateway/gateway-services/src/recall_config.rs
git commit -m "feat(recall): add KgDecayConfig to RecallConfig"
```

---

## Task 3: Add KG decay trait methods + SQLite impl

**Files:**
- Modify: `stores/zero-stores/src/knowledge_graph.rs` (trait)
- Modify: `stores/zero-stores-sqlite/src/kg/storage.rs` (impl)

### Context

`KnowledgeGraphStore` trait lives in `stores/zero-stores/src/knowledge_graph.rs`. We add two methods that perform the decay update atomically. Each returns `Result<u64>` (count of rows updated).

The decay formula: `new = max(min_confidence, old * 0.5^(days_since_last_seen / half_life))`. We compute the multiplier in Rust and apply via a batch UPDATE statement using a CASE expression over fetched IDs — keeps the math portable (doesn't depend on SQLite's optional `exp()` extension).

- [ ] **Step 3.1: Add trait methods**

In `stores/zero-stores/src/knowledge_graph.rs`, add to the `KnowledgeGraphStore` trait (near `mark_entity_archival` around line 133):

```rust
/// Apply temporal confidence decay to non-archival entities for an agent.
///
/// For each entity whose `last_seen_at` is older than `skip_recent_hours`,
/// compute `new_confidence = max(min_confidence, old * 0.5^(days/half_life))`
/// where `days = now - last_seen_at`. Returns the number of rows updated.
async fn decay_entity_confidence(
    &self,
    _agent_id: &str,
    _half_life_days: f64,
    _min_confidence: f64,
    _skip_recent_hours: i64,
) -> StoreResult<u64> {
    Ok(0)
}

/// Same as [`decay_entity_confidence`] but for `kg_relationships`.
async fn decay_relationship_confidence(
    &self,
    _agent_id: &str,
    _half_life_days: f64,
    _min_confidence: f64,
    _skip_recent_hours: i64,
) -> StoreResult<u64> {
    Ok(0)
}
```

Default implementations return `Ok(0)` so non-SQLite stores degrade to no-op.

- [ ] **Step 3.2: Cargo check (trait additions compile)**

```bash
cargo check --workspace 2>&1 | grep "^error" | head -5
```

Expected: no errors.

- [ ] **Step 3.3: Implement for `SqliteKgStore`**

In `stores/zero-stores-sqlite/src/kg/storage.rs` (or wherever `impl KnowledgeGraphStore for SqliteKgStore` lives — `grep -n "impl KnowledgeGraphStore for SqliteKgStore" stores/zero-stores-sqlite/src/`), add inside that impl block:

```rust
async fn decay_entity_confidence(
    &self,
    agent_id: &str,
    half_life_days: f64,
    min_confidence: f64,
    skip_recent_hours: i64,
) -> StoreResult<u64> {
    decay_kg_table(&self.graph, "kg_entities", agent_id, half_life_days, min_confidence, skip_recent_hours)
}

async fn decay_relationship_confidence(
    &self,
    agent_id: &str,
    half_life_days: f64,
    min_confidence: f64,
    skip_recent_hours: i64,
) -> StoreResult<u64> {
    decay_kg_table(&self.graph, "kg_relationships", agent_id, half_life_days, min_confidence, skip_recent_hours)
}
```

Then add a free function in the same file:

```rust
/// Batch-apply temporal confidence decay to a KG table.
/// Both `kg_entities` and `kg_relationships` share the same schema for
/// `id`, `agent_id`, `confidence`, `last_seen_at`, and `epistemic_class`,
/// so one helper covers both.
fn decay_kg_table(
    graph: &Arc<GraphStorage>,
    table: &str,
    agent_id: &str,
    half_life_days: f64,
    min_confidence: f64,
    skip_recent_hours: i64,
) -> StoreResult<u64> {
    if half_life_days <= 0.0 {
        return Err(StoreError::Other("half_life_days must be > 0".into()));
    }
    let cutoff = chrono::Utc::now() - chrono::Duration::hours(skip_recent_hours);
    let cutoff_rfc = cutoff.to_rfc3339();
    let decay_constant = std::f64::consts::LN_2 / half_life_days;
    let now = chrono::Utc::now();

    let mut total_updated: u64 = 0;
    graph.with_connection(|conn| {
        let select_sql = format!(
            "SELECT id, confidence, last_seen_at FROM {table}
             WHERE agent_id = ?1
               AND epistemic_class != 'archival'
               AND last_seen_at < ?2"
        );
        let mut stmt = conn.prepare(&select_sql)?;
        let rows: Vec<(String, f64, String)> = stmt
            .query_map(rusqlite::params![agent_id, cutoff_rfc], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?, row.get::<_, String>(2)?))
            })?
            .filter_map(|r| r.ok())
            .collect();

        let update_sql = format!("UPDATE {table} SET confidence = ?1 WHERE id = ?2");
        let mut update = conn.prepare(&update_sql)?;
        for (id, old_conf, last_seen) in rows {
            let last_seen_dt = match chrono::DateTime::parse_from_rfc3339(&last_seen) {
                Ok(dt) => dt.with_timezone(&chrono::Utc),
                Err(_) => continue,
            };
            let days = (now - last_seen_dt).num_seconds() as f64 / 86_400.0;
            if days <= 0.0 {
                continue;
            }
            let new_conf = (old_conf * (-decay_constant * days).exp()).max(min_confidence);
            if (new_conf - old_conf).abs() < 1e-6 {
                continue;
            }
            update.execute(rusqlite::params![new_conf, id])?;
            total_updated += 1;
        }
        Ok(())
    })?;
    Ok(total_updated)
}
```

(Imports for `chrono`, `rusqlite::params`, and `StoreError` should already be in scope or close. Add what's missing.)

- [ ] **Step 3.4: Write integration test**

In the same `storage.rs` test module, add:

```rust
#[tokio::test]
async fn decay_entity_confidence_reduces_old_entities() {
    use chrono::Duration as ChronoDuration;
    let tmp = tempfile::tempdir().unwrap();
    let paths = std::sync::Arc::new(
        gateway_services::VaultPaths::new(tmp.path().to_path_buf())
    );
    std::fs::create_dir_all(paths.conversations_db().parent().unwrap()).unwrap();
    let db = std::sync::Arc::new(KnowledgeDatabase::new(paths).expect("db"));
    let graph = std::sync::Arc::new(GraphStorage::new(db).expect("graph"));
    let store = SqliteKgStore::new(graph.clone());

    let agent_id = "agent-decay-test";
    let stale_time = chrono::Utc::now() - ChronoDuration::days(180); // 2 half-lives
    let fresh_time = chrono::Utc::now();

    // Insert one stale + one fresh entity directly via SQL for full control.
    graph.with_connection(|conn| {
        conn.execute(
            "INSERT INTO kg_entities
                (id, agent_id, entity_type, name, normalized_name, normalized_hash,
                 epistemic_class, confidence, mention_count, access_count,
                 first_seen_at, last_seen_at)
             VALUES ('stale', ?1, 'Concept', 'Stale', 'stale', 'h1',
                     'current', 0.8, 1, 0, ?2, ?2)",
            rusqlite::params![agent_id, stale_time.to_rfc3339()],
        )?;
        conn.execute(
            "INSERT INTO kg_entities
                (id, agent_id, entity_type, name, normalized_name, normalized_hash,
                 epistemic_class, confidence, mention_count, access_count,
                 first_seen_at, last_seen_at)
             VALUES ('fresh', ?1, 'Concept', 'Fresh', 'fresh', 'h2',
                     'current', 0.8, 1, 0, ?2, ?2)",
            rusqlite::params![agent_id, fresh_time.to_rfc3339()],
        )?;
        Ok(())
    }).unwrap();

    let updated = store
        .decay_entity_confidence(agent_id, 90.0, 0.01, 24)
        .await
        .expect("decay");
    assert_eq!(updated, 1, "exactly the stale entity should be decayed");

    // Verify stale confidence approximately halved twice (2 half-lives → 0.25× of 0.8 = 0.2).
    let new_stale_conf: f64 = graph.with_connection(|conn| {
        conn.query_row(
            "SELECT confidence FROM kg_entities WHERE id = 'stale'",
            [],
            |row| row.get(0),
        )
    }).unwrap();
    assert!((new_stale_conf - 0.2).abs() < 0.02, "stale conf {new_stale_conf} should be ~0.2");

    // Fresh entity unchanged.
    let fresh_conf: f64 = graph.with_connection(|conn| {
        conn.query_row(
            "SELECT confidence FROM kg_entities WHERE id = 'fresh'",
            [],
            |row| row.get(0),
        )
    }).unwrap();
    assert!((fresh_conf - 0.8).abs() < 1e-6, "fresh conf should be unchanged");
}
```

- [ ] **Step 3.5: Run the integration test**

```bash
cargo test -p zero-stores-sqlite decay_entity_confidence_reduces_old_entities -- --nocapture 2>&1 | tail -10
```

Expected: PASS.

- [ ] **Step 3.6: Run full workspace tests to catch regressions**

```bash
cargo test --workspace 2>&1 | grep -E "FAILED" | head -10
```

Expected: no failures.

- [ ] **Step 3.7: Commit**

```bash
git add stores/zero-stores/src/knowledge_graph.rs stores/zero-stores-sqlite/src/kg/storage.rs
git commit -m "feat(kg): add decay_entity_confidence + decay_relationship_confidence store methods"
```

---

## Task 4: Add `decay_kg_confidence` to `DecayEngine`

**Files:**
- Modify: `gateway/gateway-execution/src/sleep/decay.rs`

### Context

`DecayEngine` lives in `sleep/decay.rs`. Today its only public method is `list_prune_candidates`. We add `decay_kg_confidence` that calls both new store methods and returns a stats struct.

`DecayEngine::new` currently takes `(kg_store, DecayConfig)`. We need to also pass the `KgDecayConfig`. Two options: extend `DecayConfig` to include it (touches more callers), or take it as a separate parameter on the new method. The cleaner option is to pass `KgDecayConfig` to the method directly — the engine doesn't need to own it.

- [ ] **Step 4.1: Add stats struct and method**

In `gateway/gateway-execution/src/sleep/decay.rs`, near the existing `PruneCandidate` struct, add:

```rust
/// Counts returned by [`DecayEngine::decay_kg_confidence`].
#[derive(Debug, Default, Clone)]
pub struct KgDecayStats {
    pub entities_decayed: u64,
    pub relationships_decayed: u64,
}
```

Inside `impl DecayEngine`, add:

```rust
/// Apply temporal confidence decay to KG entities and relationships.
/// Conservative: errors are logged and the cycle returns whatever stats
/// were collected before the failure.
pub async fn decay_kg_confidence(
    &self,
    agent_id: &str,
    config: &gateway_services::KgDecayConfig,
) -> KgDecayStats {
    let mut stats = KgDecayStats::default();
    if !config.enabled {
        return stats;
    }
    match self
        .kg_store
        .decay_entity_confidence(
            agent_id,
            config.entity_half_life_days,
            config.min_confidence,
            config.skip_recent_hours,
        )
        .await
    {
        Ok(n) => stats.entities_decayed = n,
        Err(e) => tracing::warn!(error = %e, "decay_entity_confidence failed"),
    }
    match self
        .kg_store
        .decay_relationship_confidence(
            agent_id,
            config.relationship_half_life_days,
            config.min_confidence,
            config.skip_recent_hours,
        )
        .await
    {
        Ok(n) => stats.relationships_decayed = n,
        Err(e) => tracing::warn!(error = %e, "decay_relationship_confidence failed"),
    }
    stats
}
```

You'll need to ensure `gateway_services::KgDecayConfig` is re-exported. Check by running `cargo check -p gateway-execution` after the edit; if it errors, add `pub use recall_config::KgDecayConfig;` to `gateway/gateway-services/src/lib.rs` (next to where `RecallConfig` is re-exported).

- [ ] **Step 4.2: Add a unit test using a mock store**

Add a test in the `tests` module at the bottom of `decay.rs`:

```rust
#[tokio::test]
async fn decay_kg_confidence_returns_stats_when_enabled() {
    let (_tmp, graph) = setup();
    let agent_id = "agent-kg-decay";

    // Seed one old entity.
    graph.with_connection(|conn| {
        conn.execute(
            "INSERT INTO kg_entities
                (id, agent_id, entity_type, name, normalized_name, normalized_hash,
                 epistemic_class, confidence, mention_count, access_count,
                 first_seen_at, last_seen_at)
             VALUES ('old-1', ?1, 'Concept', 'Old', 'old', 'h1', 'current',
                     0.8, 1, 0, ?2, ?2)",
            rusqlite::params![agent_id, (chrono::Utc::now() - chrono::Duration::days(180)).to_rfc3339()],
        )?;
        Ok(())
    }).unwrap();

    let kg_store: Arc<dyn KnowledgeGraphStore> =
        Arc::new(zero_stores_sqlite::SqliteKgStore::new(graph.clone()));
    let engine = DecayEngine::new(kg_store, DecayConfig::default());
    let config = gateway_services::KgDecayConfig::default();
    let stats = engine.decay_kg_confidence(agent_id, &config).await;
    assert_eq!(stats.entities_decayed, 1);
    assert_eq!(stats.relationships_decayed, 0);
}

#[tokio::test]
async fn decay_kg_confidence_no_op_when_disabled() {
    let (_tmp, graph) = setup();
    let kg_store: Arc<dyn KnowledgeGraphStore> =
        Arc::new(zero_stores_sqlite::SqliteKgStore::new(graph));
    let engine = DecayEngine::new(kg_store, DecayConfig::default());
    let mut config = gateway_services::KgDecayConfig::default();
    config.enabled = false;
    let stats = engine.decay_kg_confidence("any", &config).await;
    assert_eq!(stats.entities_decayed, 0);
    assert_eq!(stats.relationships_decayed, 0);
}
```

You may need to add `use rusqlite;` at the top of the test module if it's not already imported.

- [ ] **Step 4.3: Run the new tests**

```bash
cargo test -p gateway-execution decay_kg -- --nocapture 2>&1 | tail -10
```

Expected: PASS.

- [ ] **Step 4.4: Run full gateway-execution tests**

```bash
cargo test -p gateway-execution 2>&1 | grep -E "FAILED|^test result" | head -10
```

Expected: all pass.

- [ ] **Step 4.5: Commit**

```bash
git add gateway/gateway-execution/src/sleep/decay.rs gateway/gateway-services/src/lib.rs
git commit -m "feat(sleep): add decay_kg_confidence to DecayEngine"
```

(Only `lib.rs` is staged if you added the `pub use` re-export there.)

---

## Task 5: Wire `decay_kg_confidence` into the sleep cycle

**Files:**
- Modify: `gateway/gateway-execution/src/sleep/worker.rs`
- Modify: `gateway/src/state/mod.rs`

### Context

The sleep `run_cycle` calls existing decay/prune ops via `decay_engine.list_prune_candidates`. We add a new call to `decay_engine.decay_kg_confidence` right before that — newly-decayed entities should be eligible for the existing orphan-age prune sweep in the same cycle.

The `KgDecayConfig` value has to reach `run_cycle`. The cleanest path: store the loaded `RecallConfig` on the gateway state already (it's used by recall), and pass `Arc<RecallConfig>` into `SleepTimeWorker::start_with_ops` as a new parameter. Inside `run_cycle`, read `recall_config.kg_decay`.

- [ ] **Step 5.1: Confirm where `RecallConfig` is loaded**

```bash
grep -n "RecallConfig::" gateway/src/state/mod.rs | head -5
```

Read the line(s) to see how it's constructed and stored. If it's already on `AppState`, we can clone it into the worker call. If not, load it the same way other configs are loaded.

- [ ] **Step 5.2: Extend `SleepTimeWorker::start_with_ops` to accept a `KgDecayConfig`**

In `gateway/gateway-execution/src/sleep/worker.rs`, find the signature:

```rust
pub fn start_with_ops(
    compactor: Arc<Compactor>,
    decay_engine: Arc<DecayEngine>,
    pruner: Arc<Pruner>,
    ops: SleepOps,
    interval: Duration,
    agent_id: String,
) -> Self {
```

Change to:

```rust
pub fn start_with_ops(
    compactor: Arc<Compactor>,
    decay_engine: Arc<DecayEngine>,
    pruner: Arc<Pruner>,
    ops: SleepOps,
    kg_decay_config: gateway_services::KgDecayConfig,
    interval: Duration,
    agent_id: String,
) -> Self {
```

Capture `kg_decay_config` in the spawned task closure (it's `Clone` from the `derive` we added on `KgDecayConfig` — verify the `derive(Clone)` is there; if not, add it back in Task 2):

```rust
tokio::spawn(async move {
    let kg_decay_config = kg_decay_config;
    let mut ticker = tokio::time::interval(interval);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    ticker.tick().await;

    tracing::info!(
        interval_secs = interval.as_secs(),
        agent_id = %agent_id,
        "sleep-time worker started",
    );

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                run_cycle("scheduled", &compactor, &decay_engine, &pruner, &ops, &kg_decay_config, &agent_id).await;
            }
            maybe = rx.recv() => {
                if maybe.is_none() {
                    tracing::info!("sleep-time worker trigger channel closed; exiting");
                    break;
                }
                run_cycle("on-demand", &compactor, &decay_engine, &pruner, &ops, &kg_decay_config, &agent_id).await;
            }
        }
    }
});
```

Update the legacy non-ops `start` method to pass `KgDecayConfig::default()`:

```rust
pub fn start(
    compactor: Arc<Compactor>,
    decay_engine: Arc<DecayEngine>,
    pruner: Arc<Pruner>,
    interval: Duration,
    agent_id: String,
) -> Self {
    Self::start_with_ops(
        compactor,
        decay_engine,
        pruner,
        SleepOps::default(),
        gateway_services::KgDecayConfig::default(),
        interval,
        agent_id,
    )
}
```

- [ ] **Step 5.3: Update `run_cycle` signature and add the decay call**

Change the signature:

```rust
async fn run_cycle(
    kind: &str,
    compactor: &Arc<Compactor>,
    decay_engine: &Arc<DecayEngine>,
    pruner: &Arc<Pruner>,
    ops: &SleepOps,
    kg_decay_config: &gateway_services::KgDecayConfig,
    agent_id: &str,
) -> CycleStats {
```

Add `kg_entities_decayed` and `kg_relationships_decayed` to `CycleStats`:

```rust
pub kg_entities_decayed: u64,
pub kg_relationships_decayed: u64,
```

Inside `run_cycle`, just before the existing `let candidates = decay_engine.list_prune_candidates(...)`, add:

```rust
// KG confidence decay — runs before prune candidate list so newly-decayed
// entities are still considered by the existing orphan-age heuristic.
let kg_decay_stats = decay_engine.decay_kg_confidence(agent_id, kg_decay_config).await;
stats.kg_entities_decayed = kg_decay_stats.entities_decayed;
stats.kg_relationships_decayed = kg_decay_stats.relationships_decayed;
```

Add the two new fields to the existing cycle-done tracing log block:

```rust
kg_entities_decayed = stats.kg_entities_decayed,
kg_relationships_decayed = stats.kg_relationships_decayed,
```

- [ ] **Step 5.4: Fix test fixtures that call `run_cycle` or `start_with_ops` directly**

The worker test module has direct calls to `run_cycle(...)` and `SleepOps { ... }` literals. Search:

```bash
grep -n "run_cycle\|SleepOps {" /home/videogamer/projects/agentzero/gateway/gateway-execution/src/sleep/worker.rs
```

Each `run_cycle("test", &c, &d, &p, &ops, "agent-...")` call must become `run_cycle("test", &c, &d, &p, &ops, &gateway_services::KgDecayConfig::default(), "agent-...")`. The `SleepOps { ... }` literals already have all five fields from Phase 3 — no change needed there.

- [ ] **Step 5.5: Update gateway state wiring**

In `gateway/src/state/mod.rs`, find the existing call site:

```bash
grep -n "SleepTimeWorker::start_with_ops" /home/videogamer/projects/agentzero/gateway/src/state/mod.rs
```

The call should currently look like:

```rust
gateway_execution::sleep::SleepTimeWorker::start_with_ops(
    compactor,
    decay,
    pruner,
    ops,
    std::time::Duration::from_secs(60 * 60),
    "root".to_string(),
)
```

Add the `kg_decay_config` argument. Where does the value come from? The `RecallConfig` is loaded somewhere on `AppState::new`. Search:

```bash
grep -n "RecallConfig::load_from_path\|RecallConfig::default" /home/videogamer/projects/agentzero/gateway/src/
```

If `RecallConfig` is loaded, clone `.kg_decay`. If it's not loaded yet for the sleep worker scope, load it via `RecallConfig::load_from_path(paths.data_dir())` right before the worker construction and pass `.kg_decay`. Example:

```rust
let recall_config_for_decay = gateway_services::RecallConfig::load_from_path(paths.vault_dir());
let ops = ...;
Some(Arc::new(
    gateway_execution::sleep::SleepTimeWorker::start_with_ops(
        compactor,
        decay,
        pruner,
        ops,
        recall_config_for_decay.kg_decay.clone(),
        std::time::Duration::from_secs(60 * 60),
        "root".to_string(),
    ),
))
```

If `paths.vault_dir()` is unclear, use the same path helper that `RecallConfig` is loaded with elsewhere (search the recall path init).

- [ ] **Step 5.6: Cargo check**

```bash
cargo check --workspace 2>&1 | grep "^error" | head -10
```

Expected: no errors.

- [ ] **Step 5.7: Run all tests**

```bash
cargo test --workspace 2>&1 | grep -E "FAILED" | head -10
```

Expected: no failures.

- [ ] **Step 5.8: cargo fmt + clippy**

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings 2>&1 | grep "^error" | head -10
```

Expected: clean.

- [ ] **Step 5.9: Commit**

```bash
git add gateway/gateway-execution/src/sleep/worker.rs gateway/src/state/mod.rs
git commit -m "feat(sleep): wire KG confidence decay into sleep worker"
```

---

## Task 6: Update tracking doc

**Files:**
- Modify: `memory-bank/future-state/2026-05-13-memory-crate-extraction-tracking.md`

- [ ] **Step 6.1: Append Phase 4 foundation entries**

In `memory-bank/future-state/2026-05-13-memory-crate-extraction-tracking.md`, add a new section after the Phase 1–3 commit inventory:

```markdown
### Phase 4 Foundation — KG Confidence Decay (2026-05-13)

**What's new:**
- Schema: `evidence TEXT` column added to `kg_entities` and `kg_relationships`. No code populates it yet — preparatory for future contradiction-propagation work.
- KG store: two new trait methods on `KnowledgeGraphStore` — `decay_entity_confidence`, `decay_relationship_confidence`. Default `Ok(0)`.
- Sleep cycle: `DecayEngine::decay_kg_confidence` runs at the start of each cycle, before prune-candidate listing.
- Settings: `RecallConfig.kg_decay` (`KgDecayConfig` struct) — `enabled`, `entity_half_life_days`, `relationship_half_life_days`, `min_confidence`, `skip_recent_hours`.

**Files for future extraction inventory:**
- `stores/zero-stores-sqlite/src/knowledge_schema.rs` (schema migration)
- `stores/zero-stores/src/knowledge_graph.rs` (trait methods)
- `stores/zero-stores-sqlite/src/kg/storage.rs` (impl)
- `gateway/gateway-services/src/recall_config.rs` (`KgDecayConfig`)
- `gateway/gateway-execution/src/sleep/decay.rs` (`decay_kg_confidence`)
- `gateway/gateway-execution/src/sleep/worker.rs` (cycle wiring + `kg_entities_decayed`/`kg_relationships_decayed` on `CycleStats`)
- `gateway/src/state/mod.rs` (construction site)

**Deferred** (intentionally out of scope for the foundation): contradiction propagation from facts to KG nodes; graph-traversal weighting by entity/relationship confidence.
```

- [ ] **Step 6.2: Commit**

```bash
git add memory-bank/future-state/2026-05-13-memory-crate-extraction-tracking.md
git commit -m "docs(memory): track Phase 4 foundation in extraction inventory"
```

---

## Final Validation

- [ ] Full workspace test:

```bash
cargo test --workspace 2>&1 | grep -E "FAILED" | head -10
```

Expected: no failures.

- [ ] Push:

```bash
git push origin feat/parallel-delegation-aggregation
```

- [ ] Update roadmap memory:

Edit `/home/videogamer/.claude/projects/-home-videogamer-projects-agentzero/memory/project_reflective_memory_roadmap.md` and change item 12 row to:

```
| 12 | Belief network — confidence + evidence on KG nodes | 4 | ✅ Foundation done (decay + evidence column) |
```

(Mark it "foundation done" not "fully done" — propagation and recall integration remain for a future Phase 4b.)

---

## Self-Review Against Spec

| User-chosen scope | Task |
|-------------------|------|
| Just decay (the foundation) | Tasks 3, 4, 5 |
| Add evidence column | Task 1 |
| Configurable knobs | Task 2 |
| Wired into sleep cycle | Task 5 |
| Tracked for future extraction | Task 6 |

**Deferred (per user scope choice):**
- Contradiction propagation from `memory_facts.contradicted_by` to KG entities — not in this plan
- Graph-traversal weighting by confidence in `recall_unified` — not in this plan
- `evidence` field population logic — column added but no writer

These are natural follow-ups in a future Phase 4b.
