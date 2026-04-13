# Memory v2 — Phase 4 Implementation Plan: Sleep-Time Worker + Observability

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A hourly background worker keeps the graph lean by merging duplicate entities, decaying stale non-archival data, and archiving pruning candidates. `GET /api/memory/stats` + `/health` expose live counts and ingestion-queue state. `POST /api/memory/consolidate` triggers the worker on demand.

**Architecture:** `SleepTimeWorker` is a tokio task started at daemon boot, runs every 60 min (configurable) OR on explicit trigger via mpsc. Four ops per cycle: (1) compaction — find entity pairs with cosine ≥ 0.92 same-type, merge via LLM pairwise verify, write audit to `kg_compactions`; (2) decay scoring — compute access-based decay for non-archival entities, identify prune candidates; (3) pruning — mark orphan prune candidates as `compressed_into` with reason='pruned' (never hard-delete); (4) cross-session synthesis — deferred to Phase 5 (requires LLM subgraph summarization).

Spec: `docs/superpowers/specs/2026-04-12-memory-layer-redesign-design.md`. Archival rule preserved — entities with `epistemic_class='archival'` are exempt from every op.

**Tech Stack:** tokio periodic tasks + mpsc, existing `KnowledgeDatabase` + `GraphStorage`, `LlmClient` (reused from distillation pattern), `CompactionRepository` (new).

---

## Pre-flight

```bash
git checkout feature/memory-v2-phase-3
git pull
git checkout -b feature/memory-v2-phase-4
```

Phase 3 ended green: 1192 tests passing, unified recall shipped. Phase 4 builds on it.

---

## File Structure

**Created:**
- `gateway/gateway-database/src/compaction_repository.rs` — `kg_compactions` CRUD
- `gateway/gateway-execution/src/sleep/mod.rs` — module root
- `gateway/gateway-execution/src/sleep/compactor.rs` — duplicate detection + merge
- `gateway/gateway-execution/src/sleep/decay.rs` — decay scoring + prune candidates
- `gateway/gateway-execution/src/sleep/worker.rs` — periodic tokio task orchestrating ops
- `gateway/src/http/memory.rs` — stats/health/consolidate endpoints
- `gateway/gateway-execution/tests/compactor_e2e.rs` — integration test

**Modified:**
- `gateway/gateway-database/src/lib.rs` — re-export `CompactionRepository`
- `services/knowledge-graph/src/storage.rs` — add `find_duplicate_candidates`, `merge_entity_into`, `mark_entity_compressed`
- `gateway/src/state.rs` — start `SleepTimeWorker` at boot; wire `CompactionRepository`
- `gateway/src/http/mod.rs` — register 3 new routes

**Deferred to Phase 5:**
- Cross-session synthesis (strong-subgraph detection + LLM summarization → memory_facts)
- Observatory UI tab (backend ready; UI is separate work)

---

## Task 1: CompactionRepository

**Files:**
- Create: `gateway/gateway-database/src/compaction_repository.rs`
- Modify: `gateway/gateway-database/src/lib.rs`

`kg_compactions` table already exists in v22 with columns: `id, run_id, operation, entity_id, relationship_id, merged_into, reason, created_at`. Just need CRUD.

### Steps

1. Inspect schema: `grep -A 10 "CREATE TABLE IF NOT EXISTS kg_compactions" gateway/gateway-database/src/knowledge_schema.rs`
2. Write `CompactionRepository` mirroring `GoalRepository` structure:
   - `record_merge(run_id, loser_entity_id, winner_entity_id, reason)`
   - `record_prune(run_id, entity_id, reason)`
   - `list_run(run_id)` — returns `Vec<Compaction>`
   - `latest_run_summary()` — returns `{run_id, timestamp, merges, prunes}`
3. Write 3 unit tests (create/list/summary).
4. Register + re-export.
5. fmt + clippy + commit: `feat(db): CompactionRepository — kg_compactions CRUD`

---

## Task 2: GraphStorage duplicate detection

**Files:**
- Modify: `services/knowledge-graph/src/storage.rs`

Add `find_duplicate_candidates(agent_id, entity_type, cosine_threshold, limit) -> Vec<(String, String, f32)>` — returns pairs `(entity_id_a, entity_id_b, cosine_similarity)` of same-type entities with cosine ≥ threshold. Uses `kg_name_index` pairwise.

### Implementation sketch

```rust
pub async fn find_duplicate_candidates(
    &self,
    agent_id: &str,
    entity_type: &str,
    cosine_threshold: f32,
    limit: usize,
) -> GraphResult<Vec<(String, String, f32)>> {
    // For each entity with an embedding (kg_name_index row), ANN-query for its
    // 5 nearest neighbors. Emit pairs where cosine ≥ threshold AND both are
    // same type AND not already marked compressed_into AND not archival.
    // Return sorted by descending cosine, capped to `limit`.
    //
    // L2_sq = 2*(1-cosine); cosine 0.92 ⇒ L2_sq ≤ 0.16.
}
```

### Steps

1. Read current `storage.rs` structure; identify where `search_entities_by_name_embedding` lives (added in Phase 3).
2. Add `find_duplicate_candidates` — implementation reuses the kg_name_index ANN query.
3. Unit test: seed 3 same-type entities with two near-identical embeddings; assert one pair returned.
4. fmt + clippy + commit: `feat(graph): find_duplicate_candidates via kg_name_index ANN`

---

## Task 3: Merge-entities operation in GraphStorage

**Files:**
- Modify: `services/knowledge-graph/src/storage.rs`

Add `merge_entity_into(loser_id, winner_id) -> MergeResult` that atomically:
1. Re-point every `kg_relationships.source_entity_id = loser` → `winner`
2. Re-point every `kg_relationships.target_entity_id = loser` → `winner`
3. Append loser's aliases into winner's `kg_aliases` rows (re-point alias.entity_id)
4. Mark loser `compressed_into = winner`
5. Remove loser from `kg_name_index`
6. Return `MergeResult { relationships_repointed, aliases_transferred }`

Wrap in a single transaction. Handle UNIQUE constraint violations on kg_relationships (when re-pointing creates a duplicate triple) by deleting the duplicate.

### Steps

1. Write the method with careful transaction handling.
2. Unit test: create 2 entities + 1 relationship pointing at loser; merge; assert relationship now points at winner + loser marked `compressed_into`.
3. fmt + clippy + commit: `feat(graph): merge_entity_into — transactional entity merge`

---

## Task 4: Compactor orchestrator

**Files:**
- Create: `gateway/gateway-execution/src/sleep/mod.rs`
- Create: `gateway/gateway-execution/src/sleep/compactor.rs`
- Modify: `gateway/gateway-execution/src/lib.rs` — register `pub mod sleep;`

Compactor ties together: find_duplicate_candidates → optional LLM pairwise verify → merge_entity_into → record in CompactionRepository.

### Design

```rust
pub struct Compactor {
    graph: Arc<GraphStorage>,
    compaction_repo: Arc<CompactionRepository>,
    llm_verifier: Option<Arc<dyn PairwiseVerifier>>, // Phase 4: None acceptable; threshold-only
}

pub trait PairwiseVerifier: Send + Sync {
    async fn should_merge(&self, a: &Entity, b: &Entity) -> bool;
}

impl Compactor {
    pub async fn run(&self, run_id: &str) -> Result<CompactionStats, String> {
        // For each entity type, find_duplicate_candidates(threshold=0.92, limit=50).
        // For each pair (a, b):
        //   if a.epistemic_class == "archival" || b.epistemic_class == "archival" { continue }
        //   if let Some(v) = &self.llm_verifier {
        //       if !v.should_merge(&a, &b).await { continue }
        //   }
        //   merge_entity_into(loser=lower mention_count, winner=higher).
        //   compaction_repo.record_merge(run_id, loser.id, winner.id, "cosine≥0.92")?
        // Return counts.
    }
}
```

### Steps

1. Define trait + struct.
2. Implement `run()` — iterate entity types, threshold-only (no LLM verifier in Phase 4).
3. Unit test: seed 2 near-duplicate entities via GraphStorage, run Compactor, assert merge happened and kg_compactions has a row.
4. fmt + clippy + commit: `feat(sleep): Compactor — threshold-based entity merge with audit`

---

## Task 5: Decay scoring + prune candidates

**Files:**
- Create: `gateway/gateway-execution/src/sleep/decay.rs`

Compute decay_score for every non-archival entity: `decay = freshness × log(access_count+1)`, where `freshness = exp(-age_days / half_life)`. List entities where `decay_score < threshold AND no outgoing or incoming relationships AND age > 30 days AND epistemic_class != 'archival'`.

### Design

```rust
pub struct DecayConfig {
    pub half_life_days: f64,       // default 60
    pub prune_threshold: f64,      // default 0.05
    pub min_age_days: i64,         // default 30
}

pub struct DecayEngine {
    graph: Arc<GraphStorage>,
    config: DecayConfig,
}

pub struct PruneCandidate {
    pub entity_id: String,
    pub decay_score: f64,
    pub reason: String, // "low_decay_score no_edges age_30d+"
}

impl DecayEngine {
    pub async fn list_prune_candidates(&self, limit: usize) -> Result<Vec<PruneCandidate>, String> { ... }
}
```

### Steps

1. Add method `list_prune_candidates_query` to GraphStorage — SQL JOIN against kg_relationships to find orphan entities.
2. Write DecayEngine with pure decay math + a method that calls the GraphStorage query.
3. Unit test: seed 3 entities (one archival, one with edges, one orphan old); assert only the orphan is a candidate.
4. fmt + clippy + commit: `feat(sleep): DecayEngine — prune candidate selection`

---

## Task 6: Pruner

**Files:**
- Modify: `gateway/gateway-execution/src/sleep/mod.rs`
- Create: `gateway/gateway-execution/src/sleep/pruner.rs`

Pruner takes `Vec<PruneCandidate>` + executes the prune: mark each as `compressed_into=NULL` with a special `properties._pruned = true` flag, remove from `kg_name_index`, write to `kg_compactions` with `operation='prune'`. Never hard-delete (per spec).

Simpler alternative that matches "soft delete" intent: add a `pruned_at TEXT` column to `kg_entities`? Skip — instead, use `compressed_into` pointing at a sentinel value `"__pruned__"` to mark the entity as retired without losing history. Recall queries filter on `compressed_into IS NULL`.

### Steps

1. Add method `mark_pruned` to GraphStorage.
2. Pruner: iterate candidates, call mark_pruned, record in CompactionRepository.
3. Unit test: seed orphan, mark pruned, assert `compressed_into` is set.
4. fmt + clippy + commit: `feat(sleep): Pruner — soft-delete orphan entities with audit`

---

## Task 7: SleepTimeWorker

**Files:**
- Create: `gateway/gateway-execution/src/sleep/worker.rs`

Tokio task that:
- Runs on a 60-min interval (configurable)
- Accepts on-demand trigger via mpsc::Sender<()>
- Per cycle: generate new `run_id` (uuid), call Compactor, call DecayEngine + Pruner, log counts, emit metric event

### Design

```rust
pub struct SleepTimeWorker {
    trigger_tx: mpsc::Sender<()>,
}

impl SleepTimeWorker {
    pub fn start(
        compactor: Arc<Compactor>,
        decay_engine: Arc<DecayEngine>,
        pruner: Arc<Pruner>,
        interval: Duration,
    ) -> Self {
        let (tx, mut rx) = mpsc::channel::<()>(8);

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                tokio::select! {
                    _ = ticker.tick() => {},
                    _ = rx.recv() => {},
                }
                let run_id = format!("sleep-{}", uuid::Uuid::new_v4());
                let _ = run_cycle(&run_id, &compactor, &decay_engine, &pruner).await;
            }
        });

        Self { trigger_tx: tx }
    }

    pub fn trigger(&self) {
        let _ = self.trigger_tx.try_send(());
    }
}

async fn run_cycle(
    run_id: &str,
    compactor: &Arc<Compactor>,
    decay_engine: &Arc<DecayEngine>,
    pruner: &Arc<Pruner>,
) -> Result<(), String> {
    tracing::info!(run_id, "sleep-time worker cycle start");
    let compaction_stats = compactor.run(run_id).await.unwrap_or_default();
    let candidates = decay_engine.list_prune_candidates(100).await.unwrap_or_default();
    let prune_stats = pruner.prune(run_id, &candidates).await.unwrap_or_default();
    tracing::info!(run_id, compaction = ?compaction_stats, prune = ?prune_stats, "cycle done");
    Ok(())
}
```

### Steps

1. Write the worker.
2. Integration test: start worker with 100ms interval, seed a duplicate, wait 200ms, assert merge happened.
3. fmt + clippy + commit: `feat(sleep): SleepTimeWorker — periodic + on-demand orchestration`

---

## Task 8: POST /api/memory/consolidate

**Files:**
- Create: `gateway/src/http/memory.rs` (start with this handler; Task 9 adds the others)
- Modify: `gateway/src/http/mod.rs` — register route

Simple handler — calls `AppState.sleep_time_worker.trigger()` and returns `202 Accepted`.

Commit: `feat(memory): POST /api/memory/consolidate — on-demand sleep-time trigger`

---

## Task 9: GET /api/memory/stats + /api/memory/health

**Files:**
- Modify: `gateway/src/http/memory.rs`
- Modify: `gateway/src/http/mod.rs`

```rust
/// GET /api/memory/stats
pub async fn stats(State(state): State<AppState>) -> Json<MemoryStats> {
    Json(MemoryStats {
        entities: count_entities(&state).await,
        relationships: count_relationships(&state).await,
        facts: count_facts(&state).await,
        wiki_articles: count_wiki(&state).await,
        procedures: count_procedures(&state).await,
        episodes: count_episodes(&state).await,
        goals_active: count_active_goals(&state).await,
        db_size_mb: db_size_mb(&state.paths),
        orphan_ratio: orphan_ratio(&state).await,
    })
}

/// GET /api/memory/health
pub async fn health(State(state): State<AppState>) -> Json<MemoryHealth> {
    Json(MemoryHealth {
        ingestion_queue_depth: count_pending_episodes(&state).await,
        failed_episodes_24h: count_failed_24h(&state).await,
        last_compaction_run: latest_run(&state).await,
    })
}
```

Use existing repo methods where possible; fall back to direct SQL.

Commit: `feat(memory): stats + health endpoints`

---

## Task 10: AppState wiring + final validation + push

**Files:**
- Modify: `gateway/src/state.rs` — construct Compactor, DecayEngine, Pruner, SleepTimeWorker at boot
- Add fields: `pub sleep_time_worker: Option<Arc<SleepTimeWorker>>`, `pub compaction_repo: Option<Arc<CompactionRepository>>`

### Steps

1. In `AppState::new`, after `knowledge_db` + `graph_storage` are built:
   ```rust
   let compaction_repo = Arc::new(CompactionRepository::new(knowledge_db.clone()));
   let compactor = Arc::new(Compactor::new(graph_storage.clone(), compaction_repo.clone(), None));
   let decay_engine = Arc::new(DecayEngine::new(graph_storage.clone(), DecayConfig::default()));
   let pruner = Arc::new(Pruner::new(graph_storage.clone(), compaction_repo.clone()));
   let sleep_worker = Arc::new(SleepTimeWorker::start(
       compactor, decay_engine, pruner, std::time::Duration::from_secs(60 * 60),
   ));
   ```

2. Add fields to struct, set None in minimal/with_components.

3. Full validation:
   ```
   cargo fmt --all
   cargo clippy --all-targets -- -D warnings
   cargo test --workspace --lib
   cargo test -p gateway-execution --test compactor_e2e
   ```

4. Push:
   ```
   git push -u origin feature/memory-v2-phase-4
   ```

Commit: `feat(memory): wire sleep-time worker + compaction in AppState; final validation`

---

## Self-Review

**Spec coverage:**
- ✅ Compactor (Tasks 2-4)
- ✅ Decay scoring (Task 5)
- ✅ Pruning (Task 6) — soft-delete via compressed_into, per spec
- ✅ Sleep-time worker with periodic + on-demand (Task 7)
- ✅ POST /api/memory/consolidate (Task 8)
- ✅ GET /api/memory/stats + /health (Task 9)
- ⏸ Cross-session synthesis — explicitly deferred to Phase 5
- ⏸ Observatory UI tab — backend ready; UI is separate work

**Archival preservation:** every op checks `epistemic_class != 'archival'` before acting. The user's "historical facts never decay" invariant holds.

**Soft-delete only:** Pruner uses `compressed_into = '__pruned__'` sentinel. Never hard-deletes. Audit trail in `kg_compactions`.

**Placeholder scan:** Tasks 2 and 5 have implementation sketches with some `...` — each flagged explicitly as "use whichever SQL JOIN/query shape fits"; the subagent reads current storage.rs and picks the right approach.

**Known risks:**
- Task 3's merge transaction is the riskiest — `kg_relationships` UNIQUE(source, target, type) can collide during re-pointing. Plan mandates deleting duplicates when they arise. Unit test covers a 2-entity, 1-relationship merge; a dedicated test for "merge creates a duplicate relationship" would harden this but adds scope.
- Task 7's `tokio::time::interval` semantics: if the daemon clock is suspended/paused, `interval` can fire burst-catchup. Phase 4 accepts this; if it becomes visible, switch to `sleep` in a loop.
- Task 9's counts can be expensive on large graphs. Each endpoint runs synchronous COUNT(*) queries — fine at 100k entities, would slow past 1M. Mitigation is caching, out of scope for Phase 4.
