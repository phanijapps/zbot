# Memory v2 — Phase 3 Implementation Plan: Unified Scored Recall + Goals

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Graph neighborhoods become first-class scored items competing alongside facts, wiki articles, and procedures in a single Reciprocal Rank Fusion pool. Goals are a first-class primitive: agents can create/update/complete goals, and active-goal slots boost semantically-aligned items in recall. The legacy free-text graph tail is retired.

**Architecture:** A new `ScoredItem` enum unifies every retrievable item with a provenance + score. Each source (facts, wiki, procedures, graph, goals) has an adapter that produces `Vec<ScoredItem>` using the same scoring primitives (base cosine × category weight × ward affinity × access decay × mention boost). A central `rrf_merge` combines all lists with k=60. The existing `MemoryRecall::recall` methods keep working (backward-compat); a new `recall_unified` method is the Phase 3 entry point; Phase 4 migrates remaining callers.

**Tech Stack:** Rust 2024, existing Phase 1-2 infrastructure (sqlite-vec, KnowledgeDatabase, resolver, VectorIndex).

**Spec:** `docs/superpowers/specs/2026-04-12-memory-layer-redesign-design.md`

---

## Pre-flight

```bash
git checkout feature/memory-v2-phase-2
git pull
git checkout -b feature/memory-v2-phase-3
```

Phase 2 ended green: 1177 tests, concurrency p95 = 106µs, streaming ingestion end-to-end. Phase 3 builds on that.

---

## File Structure

**Created:**
- `gateway/gateway-execution/src/recall/scored_item.rs` — the unified `ScoredItem` enum + `rrf_merge`
- `gateway/gateway-execution/src/recall/adapters.rs` — source adapters (fact/wiki/procedure/graph/goal → ScoredItem)
- `gateway/gateway-database/src/goal_repository.rs` — `kg_goals` CRUD
- `runtime/agent-tools/src/tools/goal.rs` — agent-facing `goal` tool
- `gateway/gateway-execution/tests/recall_unified.rs` — end-to-end integration test

**Modified:**
- `gateway/gateway-execution/src/recall.rs` → becomes `gateway/gateway-execution/src/recall/mod.rs` (split for the new submodules); add `recall_unified` method to `MemoryRecall`
- `gateway/gateway-database/src/lib.rs` — re-export `GoalRepository`
- `runtime/agent-tools/src/tools/mod.rs` — register `goal` tool
- `gateway/templates/shards/tooling_skills.md` — document `goal` tool
- `gateway/src/state.rs` — construct `GoalRepository`

**NOT modified in this phase:**
- Existing `recall()`, `recall_with_graph()`, `recall_for_intent()` — keep for backward compat; deprecated comments added
- Resolver (no changes)
- Distillation / ingestion (no changes)

---

## Task 1: ScoredItem + RRF merge

**Files:**
- Create: `gateway/gateway-execution/src/recall/mod.rs` (initially just re-exports the existing `recall.rs` content — see Step 1)
- Create: `gateway/gateway-execution/src/recall/scored_item.rs`

- [ ] **Step 1: Split recall.rs into a module directory**

Current layout: `gateway/gateway-execution/src/recall.rs` (1958 lines). To add submodules cleanly:

```bash
mkdir gateway/gateway-execution/src/recall
git mv gateway/gateway-execution/src/recall.rs gateway/gateway-execution/src/recall/mod.rs
```

Verify the crate still compiles:
```
cargo check -p gateway-execution
```

- [ ] **Step 2: Add `scored_item.rs`**

Create `gateway/gateway-execution/src/recall/scored_item.rs`:

```rust
//! Unified retrievable item — every source (facts, wiki, procedures, graph,
//! goals) projects into `ScoredItem` so they compete in one scored pool.
//!
//! Scoring primitives (applied by adapters before merge):
//!   base_score × category_weight × ward_affinity × access_decay × mention_boost
//!
//! After per-source scoring, `rrf_merge(lists, k=60)` combines N ranked lists
//! via Reciprocal Rank Fusion, cap by `budget`.

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum ItemKind {
    Fact,
    Wiki,
    Procedure,
    GraphNode,
    Goal,
}

#[derive(Debug, Clone)]
pub struct Provenance {
    pub source: String,            // "memory_facts" | "kg_name_index" | "ward_wiki" | etc.
    pub source_id: String,         // row id of the origin
    pub session_id: Option<String>,
    pub ward_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ScoredItem {
    pub kind: ItemKind,
    pub id: String,
    pub content: String,
    pub score: f64,
    pub provenance: Provenance,
}

/// Reciprocal Rank Fusion across ranked lists.
///
/// For each list, item rank r (1-indexed) contributes score = 1 / (k + r).
/// Same item appearing in multiple lists sums contributions.
/// Caller pre-sorts each list by descending `score` (RRF uses rank, not score).
///
/// Returns items sorted by combined RRF score, descending, truncated to `budget`.
pub fn rrf_merge(lists: Vec<Vec<ScoredItem>>, k: f64, budget: usize) -> Vec<ScoredItem> {
    let mut rrf: HashMap<String, (f64, ScoredItem)> = HashMap::new();
    for list in lists {
        for (rank, item) in list.into_iter().enumerate() {
            let contribution = 1.0 / (k + (rank as f64) + 1.0);
            rrf.entry(item.id.clone())
                .and_modify(|(s, _)| *s += contribution)
                .or_insert_with(|| (contribution, item));
        }
    }
    let mut out: Vec<(f64, ScoredItem)> = rrf.into_values().collect();
    out.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    out.truncate(budget);
    out.into_iter()
        .map(|(rrf_score, mut item)| {
            item.score = rrf_score;
            item
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk(id: &str, kind: ItemKind, score: f64) -> ScoredItem {
        ScoredItem {
            kind,
            id: id.to_string(),
            content: id.to_string(),
            score,
            provenance: Provenance {
                source: "test".into(),
                source_id: id.into(),
                session_id: None,
                ward_id: None,
            },
        }
    }

    #[test]
    fn empty_lists_produce_empty_merge() {
        assert!(rrf_merge(vec![], 60.0, 10).is_empty());
    }

    #[test]
    fn same_item_in_multiple_lists_sums_contributions() {
        let list_a = vec![mk("x", ItemKind::Fact, 1.0), mk("y", ItemKind::Fact, 0.9)];
        let list_b = vec![mk("x", ItemKind::Wiki, 1.0)];
        let merged = rrf_merge(vec![list_a, list_b], 60.0, 10);
        // x appears in both lists at rank 1, y only in A at rank 2.
        assert_eq!(merged[0].id, "x");
        assert!(merged[0].score > merged[1].score);
    }

    #[test]
    fn budget_caps_result_size() {
        let many: Vec<ScoredItem> = (0..100)
            .map(|i| mk(&format!("i{i}"), ItemKind::Fact, 1.0))
            .collect();
        let merged = rrf_merge(vec![many], 60.0, 5);
        assert_eq!(merged.len(), 5);
    }

    #[test]
    fn rank_one_beats_rank_ten() {
        // Single list; rank ordering preserved.
        let items: Vec<ScoredItem> = (0..10)
            .map(|i| mk(&format!("i{i}"), ItemKind::Fact, 1.0))
            .collect();
        let merged = rrf_merge(vec![items], 60.0, 10);
        assert_eq!(merged[0].id, "i0");
        assert_eq!(merged[9].id, "i9");
    }
}
```

- [ ] **Step 3: Register submodule in `recall/mod.rs`**

At the TOP of `gateway/gateway-execution/src/recall/mod.rs` (above all the existing `use` statements and code), add:

```rust
pub mod scored_item;
pub use scored_item::{rrf_merge, ItemKind, Provenance, ScoredItem};
```

- [ ] **Step 4: Run tests**

```
cargo test -p gateway-execution --lib recall::scored_item
```

Expected: 4 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add gateway/gateway-execution/src/recall/
git commit -m "feat(recall): ScoredItem + rrf_merge foundation"
```

---

## Task 2: GoalRepository — kg_goals CRUD

**Files:**
- Create: `gateway/gateway-database/src/goal_repository.rs`
- Modify: `gateway/gateway-database/src/lib.rs` — declare + export

v22 already has `kg_goals` table (Phase 1a). This task adds the repository.

- [ ] **Step 1: Create the repository**

```rust
//! CRUD over the `kg_goals` table. Goals are agent intents with lifecycle
//! state (active/blocked/satisfied/abandoned) and decomposition edges.

use std::sync::Arc;

use crate::KnowledgeDatabase;

#[derive(Debug, Clone)]
pub struct Goal {
    pub id: String,
    pub agent_id: String,
    pub ward_id: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub state: String,
    pub parent_goal_id: Option<String>,
    pub slots: Option<String>,         // JSON
    pub filled_slots: Option<String>,  // JSON
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
}

pub struct GoalRepository {
    db: Arc<KnowledgeDatabase>,
}

impl GoalRepository {
    pub fn new(db: Arc<KnowledgeDatabase>) -> Self {
        Self { db }
    }

    pub fn create(&self, goal: &Goal) -> Result<String, String> {
        self.db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO kg_goals (
                    id, agent_id, ward_id, title, description, state,
                    parent_goal_id, slots, filled_slots, created_at, updated_at, completed_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                rusqlite::params![
                    goal.id, goal.agent_id, goal.ward_id, goal.title, goal.description,
                    goal.state, goal.parent_goal_id, goal.slots, goal.filled_slots,
                    goal.created_at, goal.updated_at, goal.completed_at,
                ],
            )?;
            Ok(())
        })?;
        Ok(goal.id.clone())
    }

    pub fn update_state(&self, goal_id: &str, new_state: &str) -> Result<(), String> {
        let now = chrono::Utc::now().to_rfc3339();
        let completed = if new_state == "satisfied" || new_state == "abandoned" {
            Some(now.clone())
        } else {
            None
        };
        self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE kg_goals
                 SET state = ?1, updated_at = ?2, completed_at = COALESCE(?3, completed_at)
                 WHERE id = ?4",
                rusqlite::params![new_state, now, completed, goal_id],
            )?;
            Ok(())
        })
    }

    pub fn update_filled_slots(&self, goal_id: &str, filled_slots_json: &str) -> Result<(), String> {
        let now = chrono::Utc::now().to_rfc3339();
        self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE kg_goals SET filled_slots = ?1, updated_at = ?2 WHERE id = ?3",
                rusqlite::params![filled_slots_json, now, goal_id],
            )?;
            Ok(())
        })
    }

    pub fn list_active(&self, agent_id: &str) -> Result<Vec<Goal>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, agent_id, ward_id, title, description, state,
                        parent_goal_id, slots, filled_slots, created_at, updated_at, completed_at
                 FROM kg_goals
                 WHERE agent_id = ?1 AND state = 'active'
                 ORDER BY created_at DESC",
            )?;
            let rows = stmt.query_map(rusqlite::params![agent_id], row_to_goal)?;
            let mut out = Vec::new();
            for r in rows {
                out.push(r?);
            }
            Ok(out)
        })
    }

    pub fn get(&self, goal_id: &str) -> Result<Option<Goal>, String> {
        self.db.with_connection(|conn| {
            match conn.query_row(
                "SELECT id, agent_id, ward_id, title, description, state,
                        parent_goal_id, slots, filled_slots, created_at, updated_at, completed_at
                 FROM kg_goals WHERE id = ?1",
                rusqlite::params![goal_id],
                row_to_goal,
            ) {
                Ok(g) => Ok(Some(g)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
    }
}

fn row_to_goal(row: &rusqlite::Row) -> rusqlite::Result<Goal> {
    Ok(Goal {
        id: row.get(0)?,
        agent_id: row.get(1)?,
        ward_id: row.get(2)?,
        title: row.get(3)?,
        description: row.get(4)?,
        state: row.get(5)?,
        parent_goal_id: row.get(6)?,
        slots: row.get(7)?,
        filled_slots: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
        completed_at: row.get(11)?,
    })
}

#[cfg(test)]
mod tests {
    // Standard pattern: tempdir + KnowledgeDatabase setup.
    // Tests: create + get, update_state, list_active filters by state.
}
```

- [ ] **Step 2: Write the tests**

Follow the setup pattern from other `*_repository.rs` test modules (tempdir → VaultPaths → KnowledgeDatabase → GoalRepository). Write at least 3 tests:
- `create_and_get_roundtrip`
- `update_state_marks_completed_at_on_terminal_state`
- `list_active_filters_out_satisfied`

- [ ] **Step 3: Register + export in lib.rs**

Edit `gateway/gateway-database/src/lib.rs`:

```rust
pub mod goal_repository;
pub use goal_repository::{Goal, GoalRepository};
```

- [ ] **Step 4: Run + commit**

```
cargo test -p gateway-database --lib goal_repository
cargo fmt --all
cargo clippy -p gateway-database --all-targets -- -D warnings
```

```bash
git add gateway/gateway-database/src/goal_repository.rs gateway/gateway-database/src/lib.rs
git commit -m "feat(db): GoalRepository — kg_goals CRUD"
```

---

## Task 3: goal agent tool + shard

**Files:**
- Create: `runtime/agent-tools/src/tools/goal.rs`
- Modify: `runtime/agent-tools/src/tools/mod.rs`
- Modify: `gateway/templates/shards/tooling_skills.md`

Follow the `ingest.rs` pattern from Phase 2 Task 11: define a `GoalAccess` trait in agent-tools, let gateway wire a concrete impl.

Actions:
- `goal(action="create", title, description?, slots?)` — returns `{goal_id, state}`
- `goal(action="update_state", id, state)` — state ∈ {active, blocked, satisfied, abandoned}
- `goal(action="update_slots", id, filled_slots)` — JSON object of filled slot values
- `goal(action="list_active")` — returns `[{id, title, description, ...}]`
- `goal(action="get", id)` — returns full goal

Write the tool with the same shape as `ingest.rs`. Shard section at the bottom of `tooling_skills.md`:

```markdown
### goal
Create, update, and query agent goals. Active goals steer recall — the memory layer boosts items aligned with open goal slots.

- `goal(action="create", title="<short>", description?="<detail>", slots?=[{"name":"tickers","type":"list"}])` — returns {goal_id, state="active"}
- `goal(action="update_state", id=<goal_id>, state="satisfied"|"abandoned"|"blocked"|"active")`
- `goal(action="update_slots", id=<goal_id>, filled_slots={"tickers": ["AAPL","MSFT"]})`
- `goal(action="list_active")` — inventory of open goals
- `goal(action="get", id=<goal_id>)`

Use when: starting a multi-session workflow; tracking slot fills; reporting completion. Active goals automatically boost semantically-aligned recall.
```

Commit:
```bash
git add runtime/agent-tools/src/tools/goal.rs runtime/agent-tools/src/tools/mod.rs gateway/templates/shards/tooling_skills.md
git commit -m "feat(goal): agent-facing goal tool + shard"
```

---

## Task 4: Adapters for facts, wiki, procedures → ScoredItem

**Files:**
- Create: `gateway/gateway-execution/src/recall/adapters.rs`
- Modify: `gateway/gateway-execution/src/recall/mod.rs` — register submodule

- [ ] **Step 1: Build adapter module**

Write `adapters.rs` with pure functions:

```rust
use gateway_database::{MemoryFact, WikiArticle, Procedure};
use crate::recall::scored_item::{ItemKind, Provenance, ScoredItem};

pub fn fact_to_item(fact: &MemoryFact, score: f64) -> ScoredItem {
    ScoredItem {
        kind: ItemKind::Fact,
        id: fact.id.clone(),
        content: format!("[{}] {}: {}", fact.category, fact.key, fact.content),
        score,
        provenance: Provenance {
            source: "memory_facts".to_string(),
            source_id: fact.id.clone(),
            session_id: fact.session_id.clone(),
            ward_id: Some(fact.ward_id.clone()),
        },
    }
}

pub fn wiki_to_item(article: &WikiArticle, score: f64) -> ScoredItem {
    ScoredItem {
        kind: ItemKind::Wiki,
        id: article.id.clone(),
        content: format!("# {}\n{}", article.title, article.content),
        score,
        provenance: Provenance {
            source: "ward_wiki".to_string(),
            source_id: article.id.clone(),
            session_id: None,
            ward_id: Some(article.ward_id.clone()),
        },
    }
}

pub fn procedure_to_item(proc: &Procedure, score: f64) -> ScoredItem {
    ScoredItem {
        kind: ItemKind::Procedure,
        id: proc.id.clone(),
        content: format!("Procedure: {}\n{}\nSteps: {}", proc.name, proc.description, proc.steps),
        score,
        provenance: Provenance {
            source: "procedures".to_string(),
            source_id: proc.id.clone(),
            session_id: None,
            ward_id: Some(proc.ward_id.clone()),
        },
    }
}
```

Exact field names come from current repo structs — check with `grep 'pub struct MemoryFact' gateway/gateway-database/src/memory_repository.rs` etc. If a field name differs, adjust.

- [ ] **Step 2: Test**

Unit tests that construct fake structs and assert the adapter output shape.

- [ ] **Step 3: Commit**

```bash
git add gateway/gateway-execution/src/recall/adapters.rs gateway/gateway-execution/src/recall/mod.rs
git commit -m "feat(recall): adapters — fact/wiki/procedure → ScoredItem"
```

---

## Task 5: Graph neighborhood adapter

**Files:**
- Modify: `gateway/gateway-execution/src/recall/adapters.rs`

Graph items enter recall differently — we don't have a pre-existing struct; instead, we do an ANN query on `kg_name_index`, fetch the entities + their 1-hop neighbors, and project into `ScoredItem::GraphNode`.

- [ ] **Step 1: Add `graph_ann_to_items`**

```rust
use knowledge_graph::GraphStorage;
use std::sync::Arc;

pub async fn graph_ann_to_items(
    graph: &Arc<GraphStorage>,
    query_embedding: &[f32],
    top_k: usize,
    agent_id: &str,
) -> Result<Vec<ScoredItem>, String> {
    // TODO: Phase 3 wires this. Signature: query kg_name_index via the
    // existing SqliteVecIndex + fetch entities via storage + 1-hop neighbors.
    // For the initial task, return Vec::new() if graph storage doesn't
    // expose a dedicated API; Phase 3 Task 7 fills in the implementation.
    let _ = (graph, query_embedding, top_k, agent_id);
    Ok(Vec::new())
}
```

This is deliberately a stub for Task 5 — the real implementation lands in Task 7 where we wire everything into `recall_unified`. Task 5 just gets the shape right.

- [ ] **Step 2: Commit**

```bash
git add gateway/gateway-execution/src/recall/adapters.rs
git commit -m "feat(recall): graph-ann adapter signature (stub); impl lands in Task 7"
```

---

## Task 6: Intent boost — active-goal slot match

**Files:**
- Modify: `gateway/gateway-execution/src/recall/scored_item.rs` (add a boost helper)

- [ ] **Step 1: Add intent_boost function**

```rust
/// Boost items whose content mentions any unfilled goal slot name.
/// MemGuide-style: items aligned with active intents get a 1.3× multiplier.
pub fn intent_boost(items: &mut [ScoredItem], active_goals: &[GoalLite]) {
    if active_goals.is_empty() {
        return;
    }
    // Collect every unfilled slot name across active goals.
    let mut slot_tokens: Vec<String> = Vec::new();
    for g in active_goals {
        for s in &g.unfilled_slot_names {
            slot_tokens.push(s.to_lowercase());
        }
    }
    if slot_tokens.is_empty() {
        return;
    }
    for item in items.iter_mut() {
        let content_lower = item.content.to_lowercase();
        if slot_tokens.iter().any(|t| content_lower.contains(t)) {
            item.score *= 1.3;
        }
    }
}

/// Lightweight snapshot of a goal for boost computation.
#[derive(Debug, Clone)]
pub struct GoalLite {
    pub id: String,
    pub title: String,
    pub unfilled_slot_names: Vec<String>,
}
```

- [ ] **Step 2: Test**

```rust
#[test]
fn intent_boost_promotes_matching_items() {
    let mut items = vec![
        mk("a", ItemKind::Fact, 1.0),  // content = "a"
        mk("tickers", ItemKind::Fact, 1.0),  // content = "tickers"
    ];
    let goals = vec![GoalLite {
        id: "g1".into(),
        title: "portfolio".into(),
        unfilled_slot_names: vec!["tickers".into()],
    }];
    intent_boost(&mut items, &goals);
    assert!((items[1].score - 1.3).abs() < 1e-6);
    assert!((items[0].score - 1.0).abs() < 1e-6);
}
```

- [ ] **Step 3: Commit**

```bash
git add gateway/gateway-execution/src/recall/scored_item.rs
git commit -m "feat(recall): intent_boost — 1.3× items aligned with active goal slots"
```

---

## Task 7: `recall_unified` method

**Files:**
- Modify: `gateway/gateway-execution/src/recall/mod.rs` — add new method on `MemoryRecall`
- Modify: `gateway/gateway-execution/src/recall/adapters.rs` — fill in `graph_ann_to_items`

- [ ] **Step 1: Implement `graph_ann_to_items`**

Using existing `SqliteVecIndex` wired around `kg_name_index` (Phase 1b):

```rust
// Take Arc<dyn VectorIndex> + Arc<GraphStorage> as inputs.
// 1. vec_index.query_nearest(query_embedding, top_k) → [(entity_id, distance)]
// 2. For each entity_id, fetch via graph.get_entity_by_id (add this method if absent)
// 3. 1-hop neighbors via graph.get_relationships / traversal
// 4. Format as "Entity: X [type] — connected to A (rel), B (rel)"
// 5. Score: 1.0 - dist/2.0 (cosine from L2_sq)
```

If the current `GraphStorage` lacks `get_entity_by_id` as a public method, use whatever fetcher exists. Keep the implementation defensive — any failure returns empty list, not an error that aborts recall.

- [ ] **Step 2: Add `recall_unified` method on MemoryRecall**

```rust
impl MemoryRecall {
    pub async fn recall_unified(
        &self,
        agent_id: &str,
        query: &str,
        ward_id: Option<&str>,
        active_goals: &[scored_item::GoalLite],
        budget: usize,
    ) -> Result<Vec<ScoredItem>, String> {
        // Embed the query via existing embedding_client; None tolerated.
        let query_emb = self.embed_query(query).await.ok().flatten();

        // 1. Facts (existing search path; adapt results).
        let facts = self
            .memory_repo
            .search_facts(...)  // use whichever search method exists
            .unwrap_or_default();
        let fact_items: Vec<ScoredItem> = facts
            .into_iter()
            .map(|(fact, score)| adapters::fact_to_item(&fact, score))
            .collect();

        // 2. Wiki articles (vector search via WardWikiRepository).
        let wiki_items = /* ... */;

        // 3. Procedures (embedding search).
        let procedure_items = /* ... */;

        // 4. Graph ANN.
        let graph_items = if let (Some(graph), Some(emb)) = (self.graph_storage.as_ref(), query_emb.as_ref()) {
            adapters::graph_ann_to_items(graph, emb, 10, agent_id).await.unwrap_or_default()
        } else {
            Vec::new()
        };

        // 5. Goals (inject active goals as retrievable context).
        let goal_items: Vec<ScoredItem> = active_goals
            .iter()
            .map(|g| ScoredItem {
                kind: ItemKind::Goal,
                id: g.id.clone(),
                content: format!("Active goal: {}", g.title),
                score: 1.0,
                provenance: Provenance {
                    source: "kg_goals".into(),
                    source_id: g.id.clone(),
                    session_id: None,
                    ward_id: ward_id.map(String::from),
                },
            })
            .collect();

        // Intent boost on all non-goal lists.
        let mut all_lists = vec![fact_items, wiki_items, procedure_items, graph_items];
        for list in &mut all_lists {
            scored_item::intent_boost(list, active_goals);
        }
        // Goals themselves don't get boosted by themselves.
        all_lists.push(goal_items);

        // RRF merge with k=60.
        Ok(rrf_merge(all_lists, 60.0, budget))
    }
}
```

Placeholders above (`...`) are for Phase 3 — adapt to the real repository method signatures using existing code paths. Check `MemoryRecall::recall` for how facts are currently retrieved; mirror that pattern and adapt outputs.

- [ ] **Step 3: Integration test**

`gateway/gateway-execution/tests/recall_unified.rs`:

```rust
// Setup: KnowledgeDatabase with 3 facts, 1 wiki article, 1 procedure, 2 entities.
// Call recall_unified with a query that matches one of each.
// Assert: returned Vec<ScoredItem> contains items of kinds Fact, Wiki, Procedure, GraphNode.
// Assert: intent_boost applies when an active goal has matching slots.
```

Keep this test focused — synthetic data, no live LLM.

- [ ] **Step 4: Commit**

```bash
git add gateway/gateway-execution/src/recall/
git commit -m "feat(recall): recall_unified — scored pool + RRF + intent boost"
```

---

## Task 8: Wire one caller to recall_unified

**Files:**
- Modify: whichever call site wants to try the unified path first — suggest `runner.rs` session start

The other `recall` methods stay for backward compat. Pick one call site (session start is a safe choice), replace the existing call with `recall_unified`, format the ScoredItems into a system-message string.

- [ ] **Step 1: Identify the call site**

```
grep -n 'memory_recall\.\|recall_with_graph\|\.recall(' gateway/gateway-execution/src/runner.rs | head
```

- [ ] **Step 2: Replace at session-start**

Find the session-start recall call (usually inside `start_session` or similar). Swap to `recall_unified` with a formatter that produces the same system-message shape.

- [ ] **Step 3: Commit**

```bash
git add gateway/gateway-execution/src/runner.rs
git commit -m "feat(recall): session start uses recall_unified"
```

---

## Task 9: Gateway wiring — GoalRepository + tool access

**Files:**
- Modify: `gateway/src/state.rs` — construct `GoalRepository`
- Modify: `gateway/src/http/` — optional HTTP surface for goals (can defer)
- Modify: wiring that registers the `goal` tool with a concrete `GoalAccess` impl

Mirror how the `ingest` tool might be wired (Phase 2 Task 11 defined the trait but gateway-side impl was deferred — Task 9 can also defer the runtime registration if scope is tight; prioritize `goal_repo` being available in AppState).

Commit:
```bash
git add gateway/src/state.rs
git commit -m "feat(goal): construct GoalRepository in AppState"
```

---

## Task 10: Final validation + push

- [ ] **Step 1: fmt + clippy**
  ```
  cargo fmt --all
  cargo clippy --all-targets -- -D warnings
  ```

- [ ] **Step 2: Full test suite**
  ```
  cargo test --workspace --lib
  ```

- [ ] **Step 3: Unified recall test**
  ```
  cargo test -p gateway-execution --test recall_unified
  ```

- [ ] **Step 4: Push**
  ```
  git push -u origin feature/memory-v2-phase-3
  ```

---

## Self-Review

**Spec coverage:**
- ✅ ScoredItem unified type (Task 1)
- ✅ rrf_merge (Task 1)
- ✅ Adapters — fact/wiki/procedure (Task 4), graph (Task 5+7)
- ✅ kg_goals CRUD (Task 2)
- ✅ Goal agent tool + shard (Task 3)
- ✅ Intent boost via active-goal slots (Task 6)
- ✅ recall_unified entry point (Task 7)
- ✅ One caller migrated (Task 8)

**What is NOT in Phase 3:**
- Migrating ALL existing recall entry points — backward-compat path kept.
- Legacy free-text graph section removal from ALL call sites — Task 8 only removes it from the session-start path.
- Goal-tool runtime registration with a concrete `GoalAccess` impl — if time-boxed, add to Task 9; otherwise defer.
- Goals as standalone HTTP endpoints — not in Phase 3 scope unless needed for UI.

**Placeholder scan:** Task 5 and 7 have `...` placeholders where real-repo method signatures must be substituted at implementation time. Each is explicitly flagged in the plan text as "adapt to the real method signatures."

**Type consistency:** `ScoredItem`, `ItemKind`, `Provenance`, `GoalLite`, `rrf_merge(lists, k, budget)` used consistently.

**Known risks:**
- Task 7's `recall_unified` may discover that the existing `MemoryRecall::recall` flow has assumptions (e.g., required fields on RecallResult) that the unified path doesn't meet. Mitigation: keep recall_unified parallel to existing methods, migrate callers incrementally.
- Graph ANN → subgraph formatting is prose; quality depends on how GraphStorage exposes neighbor fetch. Phase 3 accepts "best-effort with whatever API exists"; Phase 4 can tighten.
