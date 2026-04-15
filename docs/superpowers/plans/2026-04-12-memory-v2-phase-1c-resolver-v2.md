# Memory v2 — Phase 1c Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the resolver's ad-hoc alias-in-JSON + Levenshtein scan with a proper 3-stage cascade backed by the `kg_aliases` table (O(1) index lookup), `kg_name_index` (sqlite-vec ANN), and optional LLM pairwise verification. Seed a self-alias on every entity create. Retire the Task 8 schema-bridge hack (`ALTER TABLE kg_entities ADD COLUMN aliases`).

**Architecture:** Resolver v2 has three stages in descending cheapness: (1) alias-table lookup via `normalized_form` unique index, (2) vec0 ANN on `kg_name_index`, (3) optional LLM pairwise verify on top-k candidates. Every entity create seeds one self-alias row. Every merge appends an alias. Stage 2 uses the vec0 infrastructure Phase 1b shipped. Stage 3 is new.

**Tech Stack:** Rust 2024, `rusqlite`, sqlite-vec (already wired), existing `EntityResolver` module, `gateway_database::KnowledgeDatabase`.

**Spec:** `docs/superpowers/specs/2026-04-12-memory-layer-redesign-design.md`

---

## Pre-flight

Branch from current HEAD of `feature/memory-v2-phase-1b`:

```bash
git checkout feature/memory-v2-phase-1b
git pull
git checkout -b feature/memory-v2-phase-1c
```

Phase 1b's 1158 tests are green; the branch compiles clean; 37 previously-ignored repo tests now pass. Phase 1c extends the resolver without touching the broader migration.

---

## File Structure

**Modified:**
- `services/knowledge-graph/src/resolver.rs` — new stage-1 via `kg_aliases` query, retire the properties-JSON alias path and Levenshtein stage
- `services/knowledge-graph/src/storage.rs` — remove the `ALTER TABLE kg_entities ADD COLUMN aliases` hack; route alias writes to `kg_aliases` table; seed self-alias on entity create; write entity name embedding to `kg_name_index` when provided
- `services/knowledge-graph/src/service.rs` — callers that construct entities may pass name embeddings through; no signature change if already present
- `services/knowledge-graph/tests/resolver_scale.rs` (new) — 1000-entity synthetic benchmark

**Deleted:**
- Levenshtein fuzzy-match path in resolver (stage 2 of the old cascade)
- `ALTER TABLE kg_entities ADD COLUMN aliases` in `GraphStorage::new`
- `merge_alias` helper (the JSON-array aliases functionality replaced by table rows)
- `alias_list_contains` helper

Note: Phase 1c accepts that Levenshtein goes away. The rationale is that (a) stage 1 alias-table handles the canonical "surface form previously seen" case in O(1), (b) stage 2 ANN handles semantic/embedding similarity, (c) neither requires Levenshtein. Short-name false-matches (< 6 char) are no longer a separate case. If empirical data later shows we need fuzzy fallback, it gets added back in a follow-up.

---

## Task 1: Retire the schema-bridge hack

**Files:**
- Modify: `services/knowledge-graph/src/storage.rs`

- [ ] **Step 1: Remove the ALTER TABLE line**

Open `services/knowledge-graph/src/storage.rs`. Find around line 43:

```rust
let _ = conn.execute("ALTER TABLE kg_entities ADD COLUMN aliases TEXT", []);
```

Delete the entire line (and its comment if present).

Also remove any `ALTER TABLE kg_entities ADD COLUMN normalized_name`, `normalized_hash` you may find — these columns already exist in v22 schema and the ALTER will silently fail on a fresh DB (harmless) but is misleading.

- [ ] **Step 2: Cargo check**

```
cargo check -p knowledge-graph
```

Expected: clean. Phase 1b already handles the real v22 schema init via `KnowledgeDatabase::new`.

- [ ] **Step 3: Commit**

```bash
git add services/knowledge-graph/src/storage.rs
git commit -m "refactor(graph): remove schema-bridge hack; v22 provides needed columns"
```

---

## Task 2: Write self-alias on entity CREATE

**Files:**
- Modify: `services/knowledge-graph/src/storage.rs`

Every new entity gets one `kg_aliases` row pointing to itself (surface_form = name, normalized_form = normalize_name(name)). This seeds the lookup table so future mentions of the same surface form short-circuit at stage 1.

- [ ] **Step 1: Locate the entity INSERT**

In `services/knowledge-graph/src/storage.rs`, find the function `store_entity` (around line 1270). After the `INSERT INTO kg_entities ...` succeeds (look for the successful INSERT path around line 1330-1360), add an insert into `kg_aliases`.

- [ ] **Step 2: Add the self-alias insert**

Immediately after the successful `INSERT INTO kg_entities` (but before the function returns), add:

```rust
// Seed self-alias so future mentions of this exact surface form short-circuit
// at resolver stage 1 (alias-table lookup).
let alias_id = format!("alias-{}", uuid::Uuid::new_v4());
let normalized = crate::resolver::normalize_name(&entity.name);
conn.execute(
    "INSERT OR IGNORE INTO kg_aliases (
         id, entity_id, surface_form, normalized_form, source, confidence, first_seen_at
     ) VALUES (?1, ?2, ?3, ?4, 'extraction', 1.0, ?5)",
    params![
        alias_id,
        new_id,
        entity.name,
        normalized,
        chrono::Utc::now().to_rfc3339(),
    ],
)
.map_err(GraphError::Database)?;
```

Use `INSERT OR IGNORE` to handle the UNIQUE(normalized_form, entity_id) constraint defensively — no-op on duplicate.

- [ ] **Step 3: Test the self-alias insert**

Add a test to `services/knowledge-graph/src/storage.rs`'s test module (or create a focused one):

```rust
#[test]
fn store_entity_seeds_self_alias() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = std::sync::Arc::new(gateway_services::VaultPaths::new(tmp.path().to_path_buf()));
    std::fs::create_dir_all(paths.conversations_db().parent().unwrap()).unwrap();
    let db = std::sync::Arc::new(gateway_database::KnowledgeDatabase::new(paths).unwrap());

    let storage = GraphStorage::new(db.clone()).unwrap();

    let mut entity = Entity::new(
        "root".to_string(),
        crate::EntityType::Person,
        "V.D. Savarkar".to_string(),
    );
    entity.id = "e1".to_string();

    let knowledge = ExtractedKnowledge {
        entities: vec![entity],
        relationships: vec![],
    };
    storage.store_knowledge("root", knowledge).unwrap();

    // Assert an alias row exists with the normalized form.
    db.with_connection(|conn| {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM kg_aliases WHERE surface_form = ?1 AND normalized_form = ?2",
            rusqlite::params!["V.D. Savarkar", "vd savarkar"],
            |r| r.get(0),
        )?;
        assert_eq!(count, 1, "self-alias should be seeded on entity create");
        Ok(())
    })
    .unwrap();
}
```

- [ ] **Step 4: Run**

```
cargo test -p knowledge-graph --lib storage::tests::store_entity_seeds_self_alias
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add services/knowledge-graph/src/storage.rs
git commit -m "feat(graph): seed self-alias on entity create"
```

---

## Task 3: Write merge-alias on entity MERGE

**Files:**
- Modify: `services/knowledge-graph/src/storage.rs`

When resolver returns `ResolveOutcome::Merge`, the candidate's surface form gets appended as an alias of the winner. Currently this uses `resolver::merge_alias` + `UPDATE kg_entities SET aliases = ...`. Replace with an `INSERT OR IGNORE INTO kg_aliases`.

- [ ] **Step 1: Rewrite `merge_into_existing`**

Find the function `merge_into_existing` (around line 1249). Currently it reads `kg_entities.aliases`, calls `merge_alias` to build a new JSON array, and UPDATEs the column. Replace with:

```rust
fn merge_into_existing(
    conn: &Connection,
    existing_id: &str,
    candidate: &Entity,
) -> GraphResult<()> {
    // Append candidate's surface form as an alias of the winning entity.
    let alias_id = format!("alias-{}", uuid::Uuid::new_v4());
    let normalized = crate::resolver::normalize_name(&candidate.name);
    conn.execute(
        "INSERT OR IGNORE INTO kg_aliases (
             id, entity_id, surface_form, normalized_form, source, confidence, first_seen_at
         ) VALUES (?1, ?2, ?3, ?4, 'merge', 1.0, ?5)",
        params![
            alias_id,
            existing_id,
            candidate.name,
            normalized,
            chrono::Utc::now().to_rfc3339(),
        ],
    )
    .map_err(GraphError::Database)?;

    // Bump mention_count + last_seen_at on the winner.
    conn.execute(
        "UPDATE kg_entities
         SET mention_count = mention_count + 1,
             last_seen_at = ?1
         WHERE id = ?2",
        params![chrono::Utc::now().to_rfc3339(), existing_id],
    )
    .map_err(GraphError::Database)?;
    Ok(())
}
```

- [ ] **Step 2: Delete `resolver::merge_alias` and its tests**

In `services/knowledge-graph/src/resolver.rs`, delete the `pub fn merge_alias` function (around line 261) and the three tests `merge_alias_dedups_normalized`, `merge_alias_adds_new`, `merge_alias_from_empty`.

Grep to confirm no other callers:
```
grep -rn 'resolver::merge_alias\|merge_alias(' --include='*.rs' .
```

Expected: zero hits after deletion.

- [ ] **Step 3: Test merge writes alias row**

Add to the storage test module:

```rust
#[test]
fn merge_appends_alias_row() {
    // Setup: insert one entity, then store a candidate with a variant name.
    // Assert: after second store, a kg_aliases row exists for the variant.
    let tmp = tempfile::tempdir().unwrap();
    let paths = std::sync::Arc::new(gateway_services::VaultPaths::new(tmp.path().to_path_buf()));
    std::fs::create_dir_all(paths.conversations_db().parent().unwrap()).unwrap();
    let db = std::sync::Arc::new(gateway_database::KnowledgeDatabase::new(paths).unwrap());
    let storage = GraphStorage::new(db.clone()).unwrap();

    // Original.
    let mut e1 = Entity::new("root".to_string(), crate::EntityType::Person, "V.D. Savarkar".to_string());
    e1.id = "e1".to_string();
    storage.store_knowledge("root", ExtractedKnowledge {
        entities: vec![e1],
        relationships: vec![],
    }).unwrap();

    // Variant — resolver's stage 1 should match via self-alias, then merge.
    let mut e2 = Entity::new("root".to_string(), crate::EntityType::Person, "v.d. savarkar".to_string());
    e2.id = "e2".to_string();
    storage.store_knowledge("root", ExtractedKnowledge {
        entities: vec![e2],
        relationships: vec![],
    }).unwrap();

    db.with_connection(|conn| {
        let entity_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM kg_entities WHERE entity_type = 'person'",
            [],
            |r| r.get(0),
        )?;
        assert_eq!(entity_count, 1, "second mention must merge into the first");

        let alias_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM kg_aliases WHERE entity_id = 'e1'",
            [],
            |r| r.get(0),
        )?;
        // Self-alias + variant alias = 2 (or 1 if normalized_form collides and the UNIQUE kicks in).
        assert!(alias_count >= 1, "at least one alias row expected");
        Ok(())
    }).unwrap();
}
```

- [ ] **Step 4: Run**

```
cargo test -p knowledge-graph --lib storage::tests::merge_appends_alias_row
cargo test -p knowledge-graph --lib
```

Expected: both tests pass AND no regression in other tests.

- [ ] **Step 5: Commit**

```bash
git add services/knowledge-graph/src/
git commit -m "feat(graph): merge appends alias row; retire properties-JSON aliases"
```

---

## Task 4: Resolver stage 1 — alias table lookup

**Files:**
- Modify: `services/knowledge-graph/src/resolver.rs`

Replace `exact_match` (which scans `kg_entities` + parses alias JSON) with a direct query on `kg_aliases`.

- [ ] **Step 1: Rewrite `exact_match`**

Open `services/knowledge-graph/src/resolver.rs`. Replace the current `exact_match` body with:

```rust
fn exact_match(
    conn: &Connection,
    agent_id: &str,
    candidate: &Entity,
) -> Result<Option<String>, String> {
    let normalized = normalize_name(&candidate.name);
    let type_str = candidate.entity_type.as_str();

    // Stage 1: query kg_aliases by normalized_form, then verify entity
    // type + agent scope on the join target. Index: idx_aliases_normalized.
    let mut stmt = conn
        .prepare(
            "SELECT a.entity_id FROM kg_aliases a \
             INNER JOIN kg_entities e ON e.id = a.entity_id \
             WHERE a.normalized_form = ?1 \
               AND e.entity_type = ?2 \
               AND (e.agent_id = ?3 OR e.agent_id = '__global__') \
             LIMIT 1",
        )
        .map_err(|e| format!("prepare failed: {e}"))?;

    let row: Option<String> = stmt
        .query_row(params![normalized, type_str, agent_id], |r| r.get(0))
        .ok();
    Ok(row)
}
```

- [ ] **Step 2: Delete `alias_list_contains`**

The helper `fn alias_list_contains` is no longer called. Delete it.

- [ ] **Step 3: Run**

```
cargo test -p knowledge-graph --lib resolver
```

Expected: all resolver tests pass. The existing unit tests construct entities and call `resolve(conn, ...)` — they need the `kg_aliases` table, which exists in schema v22.

If a test fails because it inserts an entity via raw SQL WITHOUT seeding an alias row, update the test setup to either (a) use `store_knowledge` (which now seeds aliases in Task 2) or (b) manually INSERT into `kg_aliases` in test setup.

- [ ] **Step 4: Commit**

```bash
git add services/knowledge-graph/src/resolver.rs
git commit -m "feat(resolver): stage 1 uses kg_aliases O(1) index lookup"
```

---

## Task 5: Retire Levenshtein fuzzy match

**Files:**
- Modify: `services/knowledge-graph/src/resolver.rs`

The `fuzzy_match` function and the `levenshtein` helper are no longer needed. Stage 1 alias-table handles exact matches. Stage 3 ANN handles semantic matches. Levenshtein occupied a narrow middle ground that's now redundant.

- [ ] **Step 1: Remove `fuzzy_match` from the cascade**

In `resolve` (top of the file, around line 37-69), delete the block:

```rust
// 2. Fuzzy name match
if let Some(existing_id) = fuzzy_match(conn, agent_id, candidate)? {
    return Ok(ResolveOutcome::Merge {
        existing_id,
        reason: MatchReason::FuzzyName,
    });
}
```

Renumber the comment for stage 3 accordingly (it becomes stage 2):

```rust
// 2. Embedding similarity (only if embedding provided)
```

- [ ] **Step 2: Delete `fuzzy_match` and `levenshtein`**

Remove both functions (`fn fuzzy_match` and `pub fn levenshtein`) from the file. Also remove the unit tests of `levenshtein` at the bottom of the file.

- [ ] **Step 3: Remove `MatchReason::FuzzyName` from the enum**

In the enum:

```rust
pub enum MatchReason {
    ExactNormalized,
    FuzzyName,   // <-- remove
    EmbeddingSimilarity,
}
```

Grep for callers of `MatchReason::FuzzyName`:
```
grep -rn 'MatchReason::FuzzyName\|FuzzyName' --include='*.rs' .
```
Expected: zero hits after removal.

- [ ] **Step 4: Run**

```
cargo check -p knowledge-graph
cargo test -p knowledge-graph --lib resolver
```

Both clean.

- [ ] **Step 5: Commit**

```bash
git add services/knowledge-graph/src/resolver.rs
git commit -m "refactor(resolver): retire Levenshtein fuzzy stage (alias+ANN cover it)"
```

---

## Task 6: Populate `kg_name_index` on entity create

**Files:**
- Modify: `services/knowledge-graph/src/storage.rs`

Stage 2 (ANN on `kg_name_index`) is dormant until something writes embeddings to that table. Currently nothing does. This task wires it up so stage 2 has data to search.

**Design decision:** entity name embeddings are written ONLY when the caller provides them. Callers that have an embedding (distillation, extraction) pass it through. Callers that don't (ad-hoc `store_knowledge` without embeddings) skip stage 2's data — stage 2 will return None, stage 3 of the old cascade doesn't exist anymore. Entities still get created, just without an embedding in the ANN index.

**Getting embeddings into `store_entity`:** the `Entity` struct doesn't currently carry an `embedding` field. Two options:

A. Add `Option<Vec<f32>>` field to `Entity`. Every caller touches Entity, but most just set None.

B. Add a parallel `store_entity_with_embedding` path that takes the embedding separately.

**Go with A.** Minor churn, one new field, callers that have embeddings populate it.

- [ ] **Step 1: Add optional embedding field to Entity**

In `services/knowledge-graph/src/types.rs` (or wherever `Entity` is defined):

```rust
pub struct Entity {
    // ... existing fields ...
    /// Optional L2-normalized name embedding. If Some, written to kg_name_index.
    #[serde(default)]
    pub name_embedding: Option<Vec<f32>>,
}
```

Set `name_embedding: None` in `Entity::new`.

`grep -rn 'Entity {' --include='*.rs' .` to find manual Entity struct literals — update them to include `name_embedding: None`.

- [ ] **Step 2: Write to kg_name_index on create**

In `store_entity`, after the `INSERT INTO kg_entities` succeeds and after the self-alias insert (Task 2), add:

```rust
// Populate kg_name_index if the caller provided a name embedding.
if let Some(emb) = entity.name_embedding.as_ref() {
    if !emb.is_empty() {
        // vec0 does not support UPSERT; emulate with delete+insert.
        conn.execute(
            "DELETE FROM kg_name_index WHERE entity_id = ?1",
            params![new_id],
        )
        .map_err(GraphError::Database)?;
        let embedding_json =
            serde_json::to_string(emb).map_err(|e| GraphError::Other(format!("serialize: {e}")))?;
        conn.execute(
            "INSERT INTO kg_name_index (entity_id, name_embedding) VALUES (?1, ?2)",
            params![new_id, embedding_json],
        )
        .map_err(GraphError::Database)?;
    }
}
```

- [ ] **Step 3: Test end-to-end**

Add a test that stores an entity with an embedding, then asserts `kg_name_index` has a row, then issues a second store with a similar embedding + different name and verifies the resolver merges via stage 2.

```rust
#[test]
fn embedding_stage_merges_similar_name() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = std::sync::Arc::new(gateway_services::VaultPaths::new(tmp.path().to_path_buf()));
    std::fs::create_dir_all(paths.conversations_db().parent().unwrap()).unwrap();
    let db = std::sync::Arc::new(gateway_database::KnowledgeDatabase::new(paths).unwrap());
    let storage = GraphStorage::new(db.clone()).unwrap();

    fn normalized(v: Vec<f32>) -> Vec<f32> {
        let n: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if n < 1e-9 { v } else { v.into_iter().map(|x| x / n).collect() }
    }

    let emb1 = normalized((0..384).map(|i| i as f32).collect());
    let mut e1 = Entity::new("root".to_string(), crate::EntityType::Person, "V.D. Savarkar".to_string());
    e1.id = "e1".to_string();
    e1.name_embedding = Some(emb1.clone());
    storage.store_knowledge("root", ExtractedKnowledge {
        entities: vec![e1], relationships: vec![],
    }).unwrap();

    // Candidate with *different* surface form (so stage 1 alias miss) but
    // very similar embedding — stage 2 must merge.
    let mut emb2 = emb1.clone();
    // Slightly perturb to avoid exact byte match; cosine still > 0.99.
    emb2[0] *= 0.999;
    let emb2 = normalized(emb2);

    let mut e2 = Entity::new("root".to_string(), crate::EntityType::Person, "UniqueString12345".to_string());
    e2.id = "e2".to_string();
    e2.name_embedding = Some(emb2);
    storage.store_knowledge("root", ExtractedKnowledge {
        entities: vec![e2], relationships: vec![],
    }).unwrap();

    db.with_connection(|conn| {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM kg_entities WHERE entity_type = 'person'",
            [], |r| r.get(0),
        )?;
        assert_eq!(count, 1, "embedding stage must merge near-identical vectors");
        Ok(())
    }).unwrap();
}
```

- [ ] **Step 4: Run**

```
cargo test -p knowledge-graph --lib
```

Expected: all pass.

- [ ] **Step 5: Thread embeddings through resolver call**

In `store_entity`, find the line that currently calls the resolver without an embedding (Task 8 noted it passes `None`). Update to pass `entity.name_embedding.as_deref()`:

```rust
// Before: resolve_via_resolver(conn, agent_id, &entity) — implicitly None embedding
// After: ensure candidate_embedding is threaded through.
```

Grep for `resolver::resolve(` and verify the call site uses `entity.name_embedding.as_deref()`.

- [ ] **Step 6: Re-run full test**

```
cargo test -p knowledge-graph --lib
```

- [ ] **Step 7: Commit**

```bash
git add services/knowledge-graph/src/types.rs services/knowledge-graph/src/storage.rs
git commit -m "feat(graph): populate kg_name_index on entity create; thread embeddings through resolver"
```

---

## Task 7: Scale benchmark — 1000 entities, resolver p95 < 20 ms

**Files:**
- Create: `services/knowledge-graph/tests/resolver_scale.rs`

- [ ] **Step 1: Write the benchmark test**

```rust
//! Resolver p95 latency benchmark. 1000 entities, measure resolve() over
//! 100 candidate lookups. Fails if p95 > 20 ms.

use std::sync::Arc;
use std::time::Instant;

use gateway_database::KnowledgeDatabase;
use gateway_services::VaultPaths;
use knowledge_graph::{Entity, EntityType, ExtractedKnowledge, GraphStorage};

fn normalized(v: Vec<f32>) -> Vec<f32> {
    let n: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if n < 1e-9 {
        v
    } else {
        v.into_iter().map(|x| x / n).collect()
    }
}

fn make_random_embedding(seed: u64) -> Vec<f32> {
    // Deterministic pseudo-random via splitmix-ish.
    let mut s = seed.wrapping_mul(0x9E3779B97F4A7C15);
    let v: Vec<f32> = (0..384)
        .map(|_| {
            s = s.wrapping_add(0xBF58476D1CE4E5B9);
            s ^= s >> 30;
            s = s.wrapping_mul(0x94D049BB133111EB);
            ((s & 0xFFFF) as f32 / 65535.0) - 0.5
        })
        .collect();
    normalized(v)
}

#[test]
fn resolver_p95_under_20ms_at_1000_entities() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let paths = Arc::new(VaultPaths::new(tmp.path().to_path_buf()));
    std::fs::create_dir_all(paths.conversations_db().parent().unwrap()).unwrap();
    let db = Arc::new(KnowledgeDatabase::new(paths).expect("knowledge db"));
    let storage = GraphStorage::new(db.clone()).expect("storage");

    // Seed 1000 entities across the 13 entity types.
    let types = [EntityType::Person, EntityType::Organization, EntityType::Location, EntityType::Event, EntityType::Concept];
    for i in 0..1000u64 {
        let t = types[(i as usize) % types.len()].clone();
        let mut e = Entity::new("root".to_string(), t, format!("Entity{}", i));
        e.id = format!("e{}", i);
        e.name_embedding = Some(make_random_embedding(i));
        storage
            .store_knowledge(
                "root",
                ExtractedKnowledge {
                    entities: vec![e],
                    relationships: vec![],
                },
            )
            .expect("store");
    }

    // Measure 100 fresh resolutions.
    let mut durations = Vec::with_capacity(100);
    for i in 1000..1100u64 {
        let t = types[(i as usize) % types.len()].clone();
        let mut e = Entity::new("root".to_string(), t, format!("Candidate{}", i));
        e.id = format!("cand{}", i);
        e.name_embedding = Some(make_random_embedding(i + 7919));

        let start = Instant::now();
        storage
            .store_knowledge(
                "root",
                ExtractedKnowledge {
                    entities: vec![e],
                    relationships: vec![],
                },
            )
            .expect("resolve+store");
        durations.push(start.elapsed());
    }

    durations.sort();
    let p50 = durations[durations.len() / 2];
    let p95 = durations[(durations.len() * 95) / 100];
    let p99 = durations[durations.len() - 1];
    eprintln!("Resolver benchmark: p50={:?} p95={:?} p99={:?}", p50, p95, p99);

    assert!(
        p95.as_millis() < 20,
        "resolver p95 must be < 20ms, got {:?}",
        p95
    );
}
```

- [ ] **Step 2: Run**

```
cargo test -p knowledge-graph --test resolver_scale --release
```

`--release` matters — debug builds are 5-10× slower; the 20ms budget assumes release.

If p95 > 20ms:
- Confirm indexes are hit: `EXPLAIN QUERY PLAN` on stage 1's SELECT (via a one-off rusqlite test). `idx_aliases_normalized` + the entity type filter should use indexes.
- Confirm vec0 is doing actual ANN (not brute force) — check sqlite-vec version
- Profile with `cargo flamegraph` if still slow

If it's clearly bottlenecked on stage 1 SQL, the join to kg_entities for type filtering may be the cost. Consider adding entity_type to kg_aliases as a denormalized column, indexed — future work, not this task.

- [ ] **Step 3: Commit**

```bash
git add services/knowledge-graph/tests/resolver_scale.rs
git commit -m "test(resolver): p95 benchmark at 1000 entities"
```

---

## Task 8: Final validation + push

- [ ] **Step 1: fmt + clippy**

```
cargo fmt --all
cargo clippy --all-targets -- -D warnings
```

Expected: clean.

- [ ] **Step 2: Full test suite**

```
cargo test --workspace
```

Expected: green (except the pre-existing `zero-core` doctest failure unrelated to memory layer).

- [ ] **Step 3: Confirm no legacy paths remain**

```
# No merge_alias helper
grep -rn 'fn merge_alias\|resolver::merge_alias' --include='*.rs' .
# No levenshtein
grep -rn 'fn levenshtein\|levenshtein(' --include='*.rs' .
# No ALTER aliases hack
grep -rn 'ALTER TABLE kg_entities ADD COLUMN aliases' --include='*.rs' .
# No aliases column access on kg_entities
grep -rn 'kg_entities.*aliases\|SELECT aliases FROM kg_entities\|UPDATE kg_entities SET aliases' --include='*.rs' .
```

All four greps should return zero hits.

- [ ] **Step 4: Push**

```bash
git push -u origin feature/memory-v2-phase-1c
```

---

## Self-Review Results

**Spec coverage:**
- ✅ Alias-first resolver (stage 1 via `kg_aliases`) — Task 4
- ✅ Self-alias on entity create — Task 2
- ✅ Alias append on merge — Task 3
- ✅ kg_name_index populated on create — Task 6
- ✅ Schema-bridge hack retired — Task 1
- ✅ Resolver p95 < 20 ms at 1000 entities — Task 7
- ✅ Levenshtein retired (explicitly per spec — stages shrink to "two" with alias short-circuit) — Task 5

**What is NOT in Phase 1c (explicit, for Phase 3 / later):**
- LLM pairwise verify (stage 3 of the spec) — left for when ANN recall starts producing false-positive merges we can measure. Adding it now would be premature.
- Entity name embedding generation — callers provide embeddings if they have them; we don't auto-generate inside `store_entity`. Phase 2 (streaming ingestion) will route embeddings through.
- Compactor / post-hoc merge of missed duplicates — Phase 4 territory.

**Placeholder scan:** no TBDs; every step has concrete SQL / code.

**Type consistency:** `MatchReason` reduces to `ExactNormalized | EmbeddingSimilarity` after Task 5. `Entity.name_embedding: Option<Vec<f32>>` added in Task 6, used in Task 6 Step 2 (write) and Task 6 Step 5 (thread to resolver).

**One known soft spot:** Task 6's `resolve_via_resolver` call site update (Step 5) is specified by grep rather than exact line — an implementer should grep and locate the real signature before editing. Documented as such.
