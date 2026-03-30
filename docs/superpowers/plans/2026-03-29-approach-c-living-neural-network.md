# Approach C: Living Neural Network — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add graph-driven recall, temporal decay, predictive recall, session offload, and ward file sync — all Pi 4 safe.

**Architecture:** Extend existing recall pipeline with a `GraphTraversal` trait (SQLite CTE implementation), temporal decay scoring, recall logging for predictive boosting, JSONL.gz session archiving, and post-distillation ward markdown generation. All features configurable via `recall_config.json` with compiled defaults.

**Tech Stack:** Rust (rusqlite WITH RECURSIVE CTE, flate2 for gzip, serde_json), existing SQLite databases, existing React Observatory UI.

**Spec:** `docs/superpowers/specs/2026-03-29-approach-c-living-neural-network-design.md`

---

## File Structure

### New Files (Rust)
| File | Responsibility |
|---|---|
| `services/knowledge-graph/src/traversal.rs` | `GraphTraversal` trait + `SqliteGraphTraversal` impl (2-hop BFS via CTE) |
| `gateway/gateway-database/src/recall_log_repository.rs` | CRUD for `recall_log` table |
| `gateway/gateway-execution/src/pruning.rs` | Fact pruning logic (decay check + archive) |
| `gateway/gateway-execution/src/ward_sync.rs` | Generate ward.md from facts + graph |
| `gateway/gateway-execution/src/archiver.rs` | Session transcript offload to JSONL.gz |

### Modified Files (Rust)
| File | Change |
|---|---|
| `gateway/gateway-database/src/schema.rs` | Migration v13: `recall_log`, `memory_facts_archive`, `sessions.archived` |
| `gateway/gateway-database/src/lib.rs` | Export recall_log_repository |
| `services/knowledge-graph/src/lib.rs` | Export traversal module |
| `services/knowledge-graph/src/storage.rs` | Add indexes for CTE performance |
| `gateway/gateway-services/src/recall_config.rs` | Add graph_traversal, temporal_decay, predictive_recall, session_offload config sections |
| `gateway/gateway-execution/src/recall.rs` | Graph expansion step, temporal decay scoring, predictive boost |
| `gateway/gateway-execution/src/distillation.rs` | Call ward_sync after successful distillation |
| `gateway/src/state.rs` | Wire traversal + recall_log into services |
| `runtime/agent-tools/src/tools/memory.rs` | Log recalled fact keys to recall_log |
| `gateway/gateway-database/src/memory_fact_store.rs` | Log recalled keys in prioritized recall |
| `apps/cli/src/main.rs` | Add `sessions archive` and `sessions restore` subcommands |

---

## Chunk 1: Schema & Config Foundation

### Task 1: Schema Migration v13

**Files:**
- Modify: `gateway/gateway-database/src/schema.rs`

- [ ] **Step 1: Write test for migration v13**

```rust
#[test]
fn test_migration_v13_creates_tables() {
    let conn = Connection::open_in_memory().unwrap();
    initialize_database(&conn).unwrap();

    // recall_log exists
    let has_recall_log: bool = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='recall_log'",
        [], |row| row.get::<_, i64>(0),
    ).unwrap() > 0;
    assert!(has_recall_log);

    // memory_facts_archive exists
    let has_archive: bool = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='memory_facts_archive'",
        [], |row| row.get::<_, i64>(0),
    ).unwrap() > 0;
    assert!(has_archive);

    // sessions.archived column exists
    let has_archived: bool = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('sessions') WHERE name='archived'",
        [], |row| row.get::<_, i64>(0),
    ).unwrap() > 0;
    assert!(has_archived);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --package gateway-database test_migration_v13 -- --nocapture`

- [ ] **Step 3: Implement migration v13**

Increment `SCHEMA_VERSION` to 13. Add migration block:

```rust
if version < 13 {
    // recall_log for predictive recall
    let _ = conn.execute(
        "CREATE TABLE IF NOT EXISTS recall_log (
            session_id TEXT NOT NULL,
            fact_key TEXT NOT NULL,
            recalled_at TEXT NOT NULL,
            PRIMARY KEY (session_id, fact_key)
        )", [],
    );
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_recall_log_session ON recall_log(session_id)", [],
    );

    // memory_facts_archive for pruned facts
    let _ = conn.execute(
        "CREATE TABLE IF NOT EXISTS memory_facts_archive (
            id TEXT PRIMARY KEY,
            agent_id TEXT NOT NULL,
            scope TEXT NOT NULL DEFAULT 'agent',
            category TEXT NOT NULL,
            key TEXT NOT NULL,
            content TEXT NOT NULL,
            confidence REAL NOT NULL DEFAULT 0.8,
            ward_id TEXT NOT NULL DEFAULT '__global__',
            mention_count INTEGER NOT NULL DEFAULT 1,
            source_summary TEXT,
            embedding BLOB,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            archived_at TEXT NOT NULL
        )", [],
    );

    // sessions.archived flag
    let _ = conn.execute(
        "ALTER TABLE sessions ADD COLUMN archived INTEGER NOT NULL DEFAULT 0", [],
    );
}
```

Also update initial `CREATE TABLE` blocks and add `recall_log` + `memory_facts_archive` to `initialize_database()`.

- [ ] **Step 4: Add graph traversal indexes**

In `services/knowledge-graph/src/storage.rs`, find the `initialize_schema` function. Add:

```rust
conn.execute(
    "CREATE INDEX IF NOT EXISTS idx_kg_rel_source ON kg_relationships(source_entity_id)", []
)?;
conn.execute(
    "CREATE INDEX IF NOT EXISTS idx_kg_rel_target ON kg_relationships(target_entity_id)", []
)?;
```

These are critical for CTE performance on Pi.

- [ ] **Step 5: Run tests and check workspace**

Run: `cargo test --package gateway-database test_migration_v13 -- --nocapture`
Run: `cargo check --workspace`

- [ ] **Step 6: Commit**

```bash
git add gateway/gateway-database/src/schema.rs services/knowledge-graph/src/storage.rs
git commit -m "feat(db): schema v13 — recall_log, memory_facts_archive, sessions.archived, graph indexes"
```

---

### Task 2: Extend RecallConfig

**Files:**
- Modify: `gateway/gateway-services/src/recall_config.rs`

- [ ] **Step 1: Write tests for new config sections**

```rust
#[test]
fn test_default_config_has_new_sections() {
    let config = RecallConfig::default();
    assert!(config.graph_traversal.enabled);
    assert_eq!(config.graph_traversal.max_hops, 2);
    assert_eq!(config.graph_traversal.hop_decay, 0.6);
    assert!(config.temporal_decay.enabled);
    assert_eq!(*config.temporal_decay.half_life_days.get("correction").unwrap(), 90.0);
    assert!(config.predictive_recall.enabled);
    assert_eq!(config.predictive_recall.predictive_boost, 1.3);
    assert!(config.session_offload.enabled);
    assert_eq!(config.session_offload.offload_after_days, 7);
}
```

- [ ] **Step 2: Add new config structs and defaults**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphTraversalConfig {
    pub enabled: bool,
    pub max_hops: u8,
    pub hop_decay: f64,
    pub max_graph_facts: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalDecayConfig {
    pub enabled: bool,
    pub half_life_days: HashMap<String, f64>,
    pub prune_threshold: f64,
    pub prune_after_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictiveRecallConfig {
    pub enabled: bool,
    pub min_similar_successes: usize,
    pub predictive_boost: f64,
    pub max_episodes_to_check: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionOffloadConfig {
    pub enabled: bool,
    pub offload_after_days: u32,
    pub keep_session_metadata: bool,
    pub archive_path: String,
}
```

Add all four as fields on `RecallConfig` with defaults in the `Default` impl.

- [ ] **Step 3: Run tests**

Run: `cargo test --package gateway-services recall_config -- --nocapture`

- [ ] **Step 4: Commit**

```bash
git add gateway/gateway-services/src/recall_config.rs
git commit -m "feat(config): add graph_traversal, temporal_decay, predictive_recall, session_offload config"
```

---

### Task 3: Recall Log Repository

**Files:**
- Create: `gateway/gateway-database/src/recall_log_repository.rs`
- Modify: `gateway/gateway-database/src/lib.rs`

- [ ] **Step 1: Write tests**

```rust
#[test]
fn test_log_and_get_keys() {
    let (repo, _dir) = setup();
    repo.log_recall("sess-1", "correction.shell.powershell").unwrap();
    repo.log_recall("sess-1", "domain.finance.spy").unwrap();
    repo.log_recall("sess-2", "correction.shell.powershell").unwrap();

    let keys = repo.get_keys_for_session("sess-1").unwrap();
    assert_eq!(keys.len(), 2);
    assert!(keys.contains(&"correction.shell.powershell".to_string()));

    let keys_multi = repo.get_keys_for_sessions(&["sess-1", "sess-2"]).unwrap();
    // "correction.shell.powershell" appears in both sessions
    assert!(keys_multi.contains_key("correction.shell.powershell"));
    assert_eq!(*keys_multi.get("correction.shell.powershell").unwrap(), 2);
}
```

- [ ] **Step 2: Implement RecallLogRepository**

Methods:
- `log_recall(session_id, fact_key)` — INSERT OR IGNORE (idempotent)
- `get_keys_for_session(session_id)` → `Vec<String>`
- `get_keys_for_sessions(session_ids: &[&str])` → `HashMap<String, usize>` (key → count across sessions)

- [ ] **Step 3: Export from lib.rs**

- [ ] **Step 4: Run tests**

Run: `cargo test --package gateway-database recall_log -- --nocapture`

- [ ] **Step 5: Commit**

```bash
git add gateway/gateway-database/src/recall_log_repository.rs gateway/gateway-database/src/lib.rs
git commit -m "feat(db): add RecallLogRepository — track which facts recalled per session"
```

---

## Chunk 2: Graph Traversal

### Task 4: GraphTraversal Trait + SQLite Implementation

**Files:**
- Create: `services/knowledge-graph/src/traversal.rs`
- Modify: `services/knowledge-graph/src/lib.rs`

- [ ] **Step 1: Write tests**

```rust
#[tokio::test]
async fn test_traverse_2_hops() {
    let storage = create_test_storage().await;
    // Create: Alice -> uses -> Rust -> part_of -> SystemProgramming
    let alice = Entity::new("a1".into(), EntityType::Person, "Alice".into());
    let rust = Entity::new("a1".into(), EntityType::Tool, "Rust".into());
    let systems = Entity::new("a1".into(), EntityType::Concept, "SystemProgramming".into());
    let r1 = Relationship::new("a1".into(), alice.id.clone(), rust.id.clone(), RelationshipType::Uses);
    let r2 = Relationship::new("a1".into(), rust.id.clone(), systems.id.clone(), RelationshipType::PartOf);
    storage.store_knowledge("a1", ExtractedKnowledge {
        entities: vec![alice.clone(), rust, systems],
        relationships: vec![r1, r2],
    }).await.unwrap();

    let traversal = SqliteGraphTraversal::new(storage.clone());
    let result = traversal.traverse(&alice.id, 2).await.unwrap();
    // Should find: Rust (hop 1), SystemProgramming (hop 2)
    assert!(result.len() >= 2);
    assert!(result.iter().any(|n| n.entity.name == "Rust" && n.hop_distance == 1));
    assert!(result.iter().any(|n| n.entity.name == "SystemProgramming" && n.hop_distance == 2));
}

#[tokio::test]
async fn test_traverse_respects_max_hops() {
    // Same graph, but max_hops = 1
    // ...
    let result = traversal.traverse(&alice.id, 1).await.unwrap();
    assert!(result.iter().all(|n| n.hop_distance <= 1));
    assert!(!result.iter().any(|n| n.entity.name == "SystemProgramming"));
}

#[tokio::test]
async fn test_connected_entities_bulk() {
    // ...
    let result = traversal.connected_entities(&["Alice", "Rust"], 1).await.unwrap();
    // Should include both Alice's and Rust's neighbors
}
```

- [ ] **Step 2: Define the trait**

```rust
#[async_trait::async_trait]
pub trait GraphTraversal: Send + Sync {
    async fn traverse(&self, entity_id: &str, max_hops: u8) -> Result<Vec<TraversalNode>, String>;
    async fn connected_entities(&self, names: &[&str], max_hops: u8) -> Result<Vec<TraversalNode>, String>;
}

pub struct TraversalNode {
    pub entity: Entity,
    pub hop_distance: u8,
    pub path: Vec<String>,
    pub relevance: f64,
}
```

- [ ] **Step 3: Implement SqliteGraphTraversal**

Uses `WITH RECURSIVE` CTE on the knowledge_graph.db connection (from `GraphStorage`).

The `traverse` method:
1. Acquires the connection lock from GraphStorage
2. Runs the recursive CTE query bounded by max_hops and LIMIT 20
3. Maps results to `TraversalNode` with `relevance = hop_decay ^ hop_distance`

The `connected_entities` method:
1. Looks up entity IDs by name (case-insensitive) via `find_entity_by_name_global`
2. Calls `traverse` for each, merging results and deduplicating by entity ID

- [ ] **Step 4: Export from lib.rs**

Add `pub mod traversal;` and re-export key types.

- [ ] **Step 5: Run tests**

Run: `cargo test --package knowledge-graph traversal -- --nocapture`

- [ ] **Step 6: Commit**

```bash
git add services/knowledge-graph/src/traversal.rs services/knowledge-graph/src/lib.rs
git commit -m "feat(graph): add GraphTraversal trait + SqliteGraphTraversal with recursive CTE"
```

---

### Task 5: Wire Graph Traversal into Recall

**Files:**
- Modify: `gateway/gateway-execution/src/recall.rs`
- Modify: `gateway/src/state.rs`

- [ ] **Step 1: Add GraphTraversal to MemoryRecall**

Add `traversal: Option<Arc<dyn GraphTraversal>>` field. Wire in state.rs: create `SqliteGraphTraversal` from the existing `GraphStorage`, pass to `MemoryRecall`.

- [ ] **Step 2: Add graph expansion step in recall_with_graph**

After hybrid search returns scored facts, before final scoring:

```rust
if self.config.graph_traversal.enabled {
    if let Some(ref traversal) = self.traversal {
        // Extract entity names from top facts
        let entity_names: Vec<&str> = facts.iter()
            .take(5) // Only expand from top 5 facts
            .filter_map(|sf| extract_entity_name(&sf.fact.content))
            .collect();

        if !entity_names.is_empty() {
            let graph_nodes = traversal.connected_entities(&entity_names, self.config.graph_traversal.max_hops).await?;

            // Find facts connected to graph-discovered entities
            for node in graph_nodes {
                let related_facts = self.memory_repo.search_memory_facts_fts(
                    &node.entity.name, agent_id, 3, None
                )?;
                for sf in related_facts {
                    if seen_keys.insert(sf.fact.key.clone()) {
                        results.push(ScoredFact {
                            score: sf.score * node.relevance, // Apply hop decay
                            fact: sf.fact,
                        });
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 3: Run workspace check and tests**

Run: `cargo check --workspace`
Run: `cargo test --package gateway-execution -- --nocapture`

- [ ] **Step 4: Commit**

```bash
git add gateway/gateway-execution/src/recall.rs gateway/src/state.rs
git commit -m "feat(recall): add graph expansion — 2-hop BFS finds corrections via entity connections"
```

---

## Chunk 3: Temporal Decay & Predictive Recall

### Task 6: Temporal Decay in Recall Scoring

**Files:**
- Modify: `gateway/gateway-execution/src/recall.rs`

- [ ] **Step 1: Write test for decay function**

```rust
#[test]
fn test_temporal_decay() {
    // 0 days old → decay ≈ 1.0
    let now = Utc::now();
    assert!((temporal_decay(now, 30.0) - 1.0).abs() < 0.01);

    // 30 days old with half_life 30 → decay = 0.5
    let thirty_days_ago = now - chrono::Duration::days(30);
    assert!((temporal_decay(thirty_days_ago, 30.0) - 0.5).abs() < 0.05);

    // 90 days old with half_life 90 → decay = 0.5
    let ninety_days_ago = now - chrono::Duration::days(90);
    assert!((temporal_decay(ninety_days_ago, 90.0) - 0.5).abs() < 0.05);
}
```

- [ ] **Step 2: Implement decay function**

```rust
fn temporal_decay(last_seen: DateTime<Utc>, half_life_days: f64) -> f64 {
    let age_days = (Utc::now() - last_seen).num_days().max(0) as f64;
    1.0 / (1.0 + (age_days / half_life_days))
}
```

- [ ] **Step 3: Apply decay in scoring pipeline**

After category weight and ward affinity, before contradiction penalty:

```rust
if self.config.temporal_decay.enabled {
    for sf in &mut results {
        if sf.fact.category == "skill" || sf.fact.category == "agent" {
            continue; // skill/agent indices don't decay
        }
        let half_life = self.config.temporal_decay.half_life_days
            .get(&sf.fact.category)
            .copied()
            .unwrap_or(30.0);
        let last_seen = chrono::DateTime::parse_from_rfc3339(&sf.fact.updated_at)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now());
        let decay = temporal_decay(last_seen, half_life);
        let mention_boost = 1.0 + (sf.fact.mention_count as f64).max(1.0).log2();
        sf.score *= decay * mention_boost;
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --package gateway-execution -- --nocapture`

- [ ] **Step 5: Commit**

```bash
git add gateway/gateway-execution/src/recall.rs
git commit -m "feat(recall): add temporal decay — per-category half-lives, mention count resists decay"
```

---

### Task 7: Predictive Recall

**Files:**
- Modify: `gateway/gateway-execution/src/recall.rs`
- Modify: `runtime/agent-tools/src/tools/memory.rs`
- Modify: `gateway/gateway-database/src/memory_fact_store.rs`
- Modify: `gateway/src/state.rs`

- [ ] **Step 1: Wire RecallLogRepository into services**

Add `recall_log: Option<Arc<RecallLogRepository>>` to `MemoryRecall`. Create in state.rs, pass through.

- [ ] **Step 2: Log recalled keys in memory.recall tool**

In `runtime/agent-tools/src/tools/memory.rs`, after the recall action returns results, log each fact key:

```rust
// After recall returns results:
if let Some(ref recall_log) = self.recall_log {
    let session_id = context.session_id();
    for fact in &results {
        let _ = recall_log.log_recall(session_id, &fact.key);
    }
}
```

Also do the same in `memory_fact_store.rs` `recall_facts_prioritized()`.

- [ ] **Step 3: Add predictive boost in recall scoring**

In `recall_with_graph`, after graph expansion, before final sort:

```rust
if self.config.predictive_recall.enabled {
    if let (Some(ref episode_repo), Some(ref recall_log)) = (&self.episode_repo, &self.recall_log) {
        let query_embedding = self.embed_query(user_message).await;
        if let Some(ref emb) = query_embedding {
            let similar = episode_repo.search_by_similarity(
                agent_id, emb, 0.5, self.config.predictive_recall.max_episodes_to_check
            ).unwrap_or_default();

            let success_session_ids: Vec<&str> = similar.iter()
                .filter(|(ep, _)| ep.outcome == "success")
                .map(|(ep, _)| ep.session_id.as_str())
                .collect();

            if !success_session_ids.is_empty() {
                let key_counts = recall_log.get_keys_for_sessions(&success_session_ids)
                    .unwrap_or_default();

                for sf in &mut results {
                    if let Some(&count) = key_counts.get(&sf.fact.key) {
                        if count >= self.config.predictive_recall.min_similar_successes {
                            sf.score *= self.config.predictive_recall.predictive_boost;
                        }
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo check --workspace`
Run: `cargo test --package gateway-execution -- --nocapture`

- [ ] **Step 5: Commit**

```bash
git add gateway/gateway-execution/src/recall.rs runtime/agent-tools/src/tools/memory.rs gateway/gateway-database/src/memory_fact_store.rs gateway/src/state.rs
git commit -m "feat(recall): add predictive recall — boost facts correlated with past successes"
```

---

## Chunk 4: Session Offload

### Task 8: Session Archiver

**Files:**
- Create: `gateway/gateway-execution/src/archiver.rs`

- [ ] **Step 1: Implement archive_session**

```rust
pub struct SessionArchiver {
    conversation_repo: Arc<ConversationRepository>,
    state_service: Arc<StateService<DatabaseManager>>,
    archive_path: PathBuf,
}

impl SessionArchiver {
    pub fn archive_session(&self, session_id: &str) -> Result<ArchiveResult, String> {
        // 1. Load messages and logs for session
        // 2. Serialize to JSONL
        // 3. Compress with flate2::write::GzEncoder
        // 4. Write to {archive_path}/{session_id}.jsonl.gz
        // 5. DELETE messages and execution_logs WHERE session_id = ?
        // 6. UPDATE sessions SET archived = 1 WHERE id = ?
        // 7. Return stats (messages_archived, logs_archived, file_size)
    }

    pub fn archive_old_sessions(&self, older_than_days: u32) -> Result<Vec<ArchiveResult>, String> {
        // Find sessions: completed_at < now - days, archived = 0,
        // has distillation_runs.status = 'success'
        // Archive each
    }

    pub fn restore_session(&self, session_id: &str) -> Result<RestoreResult, String> {
        // 1. Read {archive_path}/{session_id}.jsonl.gz
        // 2. Decompress with flate2::read::GzDecoder
        // 3. Parse JSONL lines
        // 4. INSERT messages and logs back into SQLite
        // 5. UPDATE sessions SET archived = 0
    }
}
```

- [ ] **Step 2: Verify**

Run: `cargo check --workspace`

- [ ] **Step 3: Commit**

```bash
git add gateway/gateway-execution/src/archiver.rs
git commit -m "feat(storage): add SessionArchiver — JSONL.gz offload for old session transcripts"
```

---

### Task 9: CLI Commands for Archive

**Files:**
- Modify: `apps/cli/src/main.rs`
- Modify: `gateway/src/http/graph.rs`
- Modify: `gateway/src/http/mod.rs`

- [ ] **Step 1: Add gateway endpoints**

```
POST /api/sessions/archive          — archive sessions older than N days
POST /api/sessions/restore/:id      — restore an archived session
GET  /api/sessions/archive/stats    — archive stats (count, total size)
```

- [ ] **Step 2: Add CLI subcommands**

```rust
/// Session management
Sessions {
    #[command(subcommand)]
    action: SessionAction,
},

enum SessionAction {
    Archive {
        #[arg(long, default_value = "7d")]
        older_than: String,
    },
    Restore {
        session_id: String,
    },
}
```

CLI calls gateway endpoints via HTTP.

- [ ] **Step 3: Verify**

Run: `cargo check --workspace`

- [ ] **Step 4: Commit**

```bash
git add apps/cli/src/main.rs gateway/src/http/graph.rs gateway/src/http/mod.rs
git commit -m "feat(cli): add 'zero sessions archive/restore' commands"
```

---

## Chunk 5: Ward File Sync & Pruning

### Task 10: Ward File Sync

**Files:**
- Create: `gateway/gateway-execution/src/ward_sync.rs`
- Modify: `gateway/gateway-execution/src/distillation.rs`

- [ ] **Step 1: Implement ward markdown generation**

```rust
pub fn generate_ward_knowledge_file(
    ward_path: &Path,
    ward_id: &str,
    memory_repo: &MemoryRepository,
    graph_service: Option<&GraphService>,
) -> Result<(), String> {
    // 1. Fetch corrections for this ward: category='correction', ward_id matches
    // 2. Fetch patterns: category='pattern'
    // 3. Fetch domain facts: category='domain'
    // 4. Fetch top entities from graph (if available)
    // 5. Format as markdown
    // 6. Write to {ward_path}/memory/ward.md
    //    - Create memory/ dir if needed
    //    - Add header: "# Ward Knowledge: {ward_id}\n*Auto-generated. Last updated: {date}*"
}
```

- [ ] **Step 2: Call from distillation**

In `distillation.rs`, after successful distillation, if the session has a ward_id:

```rust
if let Some(ward_id) = &ward_id {
    if ward_id != "__global__" {
        if let Err(e) = ward_sync::generate_ward_knowledge_file(
            &ward_path, ward_id, &self.memory_repo, self.graph_service.as_deref()
        ) {
            tracing::warn!("Ward file sync failed: {}", e);
        }
    }
}
```

- [ ] **Step 3: Verify**

Run: `cargo check --workspace`

- [ ] **Step 4: Commit**

```bash
git add gateway/gateway-execution/src/ward_sync.rs gateway/gateway-execution/src/distillation.rs
git commit -m "feat(ward): auto-generate ward/memory/ward.md from distilled knowledge"
```

---

### Task 11: Fact Pruning

**Files:**
- Create: `gateway/gateway-execution/src/pruning.rs`

- [ ] **Step 1: Implement pruning logic**

```rust
pub fn prune_decayed_facts(
    memory_repo: &MemoryRepository,
    config: &TemporalDecayConfig,
) -> Result<PruneResult, String> {
    // 1. Fetch all facts
    // 2. Compute effective score for each: confidence * decay * mention_boost
    // 3. Facts below prune_threshold for prune_after_days:
    //    - INSERT into memory_facts_archive (with archived_at timestamp)
    //    - DELETE from memory_facts
    //    - DELETE from memory_facts_fts (the trigger should handle this)
    // 4. Return PruneResult { pruned_count, archived_count }
}
```

- [ ] **Step 2: Verify**

Run: `cargo check --workspace`

- [ ] **Step 3: Commit**

```bash
git add gateway/gateway-execution/src/pruning.rs
git commit -m "feat(memory): add fact pruning — archive decayed facts to keep SQLite lean"
```

---

## Chunk 6: Integration & Verification

### Task 12: End-to-End Verification

- [ ] **Step 1: Run full workspace tests**

Run: `cargo test --workspace --lib --bins --tests`
Expected: All pass

- [ ] **Step 2: Build UI**

Run: `cd apps/ui && npm run build`
Expected: Success

- [ ] **Step 3: Manual verification — graph traversal**

Start daemon. Send a financial analysis request. Check that recall includes corrections found via graph traversal (not just cosine similarity).

- [ ] **Step 4: Manual verification — temporal decay**

Check that very old domain facts score lower than recent corrections.

- [ ] **Step 5: Manual verification — predictive recall**

After 2+ successful sessions, verify that facts recalled in successful sessions get boosted.

- [ ] **Step 6: Test session archive**

```bash
zero sessions archive --older-than 1d
```

Verify: messages removed from SQLite, `.jsonl.gz` files in archive dir, session metadata preserved.

- [ ] **Step 7: Test session restore**

```bash
zero sessions restore {session_id}
```

Verify: messages re-inserted, archived flag cleared.

- [ ] **Step 8: Verify ward file sync**

After a distillation in a ward, check `{ward_path}/memory/ward.md` exists and contains corrections, patterns, entities.

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "feat: Approach C complete — graph traversal, temporal decay, predictive recall, session offload, ward sync"
```
