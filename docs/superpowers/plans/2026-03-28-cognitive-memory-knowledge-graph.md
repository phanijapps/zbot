# Cognitive Memory & Knowledge Graph Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the disconnected memory/knowledge pipeline, add episodic memory with execution scoring, build a recall priority engine, retroactive distillation bootstrap, and an Observatory UI for knowledge graph visualization.

**Architecture:** Extend existing SQLite-based memory system with new tables (distillation_runs, session_episodes), add ward_id scoping to memory_facts, build configurable recall priority engine, and create a D3-force Observatory UI. All changes follow existing patterns — version-based schema migrations, `db.with_connection()` repository pattern, Axum HTTP handlers, React feature modules.

**Tech Stack:** Rust (rusqlite, axum, tokio, serde), React 19 + TypeScript, D3-force (new), Vitest + Testing Library

**Spec:** `docs/superpowers/specs/2026-03-28-cognitive-memory-knowledge-graph-design.md`

---

## File Structure

### New Files (Rust)
| File | Responsibility |
|---|---|
| `gateway/gateway-database/src/distillation_repository.rs` | CRUD for `distillation_runs` table |
| `gateway/gateway-database/src/episode_repository.rs` | CRUD + similarity search for `session_episodes` |
| `gateway/gateway-services/src/recall_config.rs` | RecallConfig loading with compiled defaults + JSON merge |
| `gateway/gateway-execution/src/middleware/recall_refresh.rs` | Mid-session automatic recall middleware |

### New Files (TypeScript)
| File | Responsibility |
|---|---|
| `apps/ui/src/features/observatory/ObservatoryPage.tsx` | Main page layout with graph + sidebar + health bar |
| `apps/ui/src/features/observatory/GraphCanvas.tsx` | D3-force graph rendering component |
| `apps/ui/src/features/observatory/EntityDetail.tsx` | Slide-over sidebar for entity details |
| `apps/ui/src/features/observatory/LearningHealthBar.tsx` | Bottom status bar with distillation stats |
| `apps/ui/src/features/observatory/graph-hooks.ts` | Data fetching hooks for graph API |
| `apps/ui/src/features/observatory/index.ts` | Barrel export |

### Modified Files (Rust)
| File | Change |
|---|---|
| `gateway/gateway-database/src/schema.rs` | Migration v11: new tables, ward_id column, UNIQUE constraint update |
| `gateway/gateway-database/src/memory_repository.rs` | Ward_id in queries, UNIQUE constraint update |
| `gateway/gateway-database/src/lib.rs` | Export new repository modules |
| `gateway/gateway-execution/src/distillation.rs` | Health reporting, fallback chain, episode extraction, strategy emergence |
| `gateway/gateway-execution/src/recall.rs` | Priority weights, ward affinity, episode recall, formatted output |
| `gateway/gateway-execution/src/runner.rs` | Delegation recall, continuation query fix |
| `gateway/gateway-execution/src/delegation/spawn.rs` | Recall injection for child agents |
| `gateway/src/http/graph.rs` | Stats, distillation status, cross-agent endpoints |
| `gateway/src/http/mod.rs` | Register new routes |
| `gateway/src/state.rs` | Wire RecallConfig, pass to recall service |
| `runtime/agent-tools/src/tools/memory.rs` | Upgrade recall tool to use priority engine |
| `apps/cli/src/main.rs` | Add `distill --backfill` subcommand |

### Modified Files (TypeScript)
| File | Change |
|---|---|
| `apps/ui/src/App.tsx` | Add /observatory route |
| `apps/ui/src/styles/components.css` | Observatory component classes |
| `apps/ui/package.json` | Add d3-force, d3-selection, d3-zoom dependencies |

---

## Chunk 1: Schema & Config Foundation

### Task 1: Schema Migration v11

**Files:**
- Modify: `gateway/gateway-database/src/schema.rs`

- [ ] **Step 1: Write test for migration v11**

Add to `schema.rs` `#[cfg(test)]` module:

```rust
#[test]
fn test_migration_v11_creates_tables() {
    let conn = Connection::open_in_memory().unwrap();
    initialize_database(&conn).unwrap();

    // Verify distillation_runs exists
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='distillation_runs'",
        [], |row| row.get(0),
    ).unwrap();
    assert_eq!(count, 1);

    // Verify session_episodes exists
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='session_episodes'",
        [], |row| row.get(0),
    ).unwrap();
    assert_eq!(count, 1);

    // Verify memory_facts has ward_id column
    let has_ward_id: bool = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('memory_facts') WHERE name='ward_id'",
        [], |row| row.get::<_, i64>(0),
    ).unwrap() > 0;
    assert!(has_ward_id);

    // Verify UNIQUE constraint includes ward_id
    // Insert two facts with same key but different ward_id — should succeed
    conn.execute(
        "INSERT INTO memory_facts (id, agent_id, scope, category, key, content, confidence, ward_id, created_at, updated_at)
         VALUES ('f1', 'agent1', 'agent', 'domain', 'test.key', 'global fact', 0.8, '__global__', datetime('now'), datetime('now'))",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO memory_facts (id, agent_id, scope, category, key, content, confidence, ward_id, created_at, updated_at)
         VALUES ('f2', 'agent1', 'agent', 'domain', 'test.key', 'ward fact', 0.8, 'my-ward', datetime('now'), datetime('now'))",
        [],
    ).unwrap();

    // Insert duplicate ward_id+key — should fail
    let result = conn.execute(
        "INSERT INTO memory_facts (id, agent_id, scope, category, key, content, confidence, ward_id, created_at, updated_at)
         VALUES ('f3', 'agent1', 'agent', 'domain', 'test.key', 'dupe', 0.8, '__global__', datetime('now'), datetime('now'))",
        [],
    );
    assert!(result.is_err());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --package gateway-database test_migration_v11 -- --nocapture`
Expected: FAIL — migration v11 doesn't exist yet

- [ ] **Step 3: Implement migration v11**

In `schema.rs`, increment `SCHEMA_VERSION` to 11 and add migration block:

```rust
const SCHEMA_VERSION: i32 = 11;

// In migrate_database():
if version < 11 {
    // 1. distillation_runs table
    let _ = conn.execute(
        "CREATE TABLE IF NOT EXISTS distillation_runs (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL UNIQUE,
            status TEXT NOT NULL,
            facts_extracted INTEGER DEFAULT 0,
            entities_extracted INTEGER DEFAULT 0,
            relationships_extracted INTEGER DEFAULT 0,
            episode_created INTEGER DEFAULT 0,
            error TEXT,
            retry_count INTEGER DEFAULT 0,
            duration_ms INTEGER,
            created_at TEXT NOT NULL
        )", [],
    );
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_distillation_runs_status ON distillation_runs(status)", [],
    );

    // 2. session_episodes table
    let _ = conn.execute(
        "CREATE TABLE IF NOT EXISTS session_episodes (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            agent_id TEXT NOT NULL,
            ward_id TEXT NOT NULL DEFAULT '__global__',
            task_summary TEXT NOT NULL,
            outcome TEXT NOT NULL,
            strategy_used TEXT,
            key_learnings TEXT,
            token_cost INTEGER,
            embedding BLOB,
            created_at TEXT NOT NULL
        )", [],
    );
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_session_episodes_agent ON session_episodes(agent_id)", [],
    );
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_session_episodes_ward ON session_episodes(ward_id)", [],
    );
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_session_episodes_outcome ON session_episodes(outcome)", [],
    );

    // 3. Add ward_id to memory_facts and rebuild UNIQUE constraint
    // SQLite doesn't support ALTER CONSTRAINT, so recreate the table
    let _ = conn.execute_batch("
        ALTER TABLE memory_facts ADD COLUMN ward_id TEXT NOT NULL DEFAULT '__global__';
    ");
    // Drop old unique index and create new one including ward_id
    let _ = conn.execute("DROP INDEX IF EXISTS idx_memory_facts_unique", []);
    let _ = conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_memory_facts_unique ON memory_facts(agent_id, scope, ward_id, key)", [],
    );
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_memory_facts_ward ON memory_facts(ward_id)", [],
    );
}
```

Also update the initial `CREATE TABLE memory_facts` in `initialize_database()` to include `ward_id TEXT NOT NULL DEFAULT '__global__'` and update the UNIQUE constraint to `UNIQUE(agent_id, scope, ward_id, key)`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --package gateway-database test_migration_v11 -- --nocapture`
Expected: PASS

- [ ] **Step 5: Run full workspace check**

Run: `cargo check --workspace`
Expected: Success (existing code may need ward_id updates — handled in Task 2)

- [ ] **Step 6: Commit**

```bash
git add gateway/gateway-database/src/schema.rs
git commit -m "feat(db): add schema v11 — distillation_runs, session_episodes, ward_id"
```

---

### Task 2: Distillation Repository

**Files:**
- Create: `gateway/gateway-database/src/distillation_repository.rs`
- Modify: `gateway/gateway-database/src/lib.rs`

- [ ] **Step 1: Write tests for distillation repository**

```rust
// In distillation_repository.rs #[cfg(test)] module
#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatabaseManager;
    use tempfile::TempDir;

    fn setup() -> (DistillationRepository, TempDir) {
        let dir = TempDir::new().unwrap();
        let db = Arc::new(DatabaseManager::new_with_path(dir.path().join("test.db")).unwrap());
        (DistillationRepository::new(db), dir)
    }

    #[test]
    fn test_insert_and_get_run() {
        let (repo, _dir) = setup();
        let run = DistillationRun {
            id: "dr-1".to_string(),
            session_id: "sess-1".to_string(),
            status: "success".to_string(),
            facts_extracted: 5,
            entities_extracted: 3,
            relationships_extracted: 2,
            episode_created: 1,
            error: None,
            retry_count: 0,
            duration_ms: Some(1500),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        repo.insert(&run).unwrap();
        let fetched = repo.get_by_session_id("sess-1").unwrap().unwrap();
        assert_eq!(fetched.facts_extracted, 5);
        assert_eq!(fetched.status, "success");
    }

    #[test]
    fn test_update_retry() {
        let (repo, _dir) = setup();
        let run = DistillationRun {
            id: "dr-2".to_string(),
            session_id: "sess-2".to_string(),
            status: "failed".to_string(),
            facts_extracted: 0,
            entities_extracted: 0,
            relationships_extracted: 0,
            episode_created: 0,
            error: Some("provider timeout".to_string()),
            retry_count: 0,
            duration_ms: None,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        repo.insert(&run).unwrap();
        repo.update_retry("sess-2", "failed", 1, Some("retry failed")).unwrap();
        let fetched = repo.get_by_session_id("sess-2").unwrap().unwrap();
        assert_eq!(fetched.retry_count, 1);
        assert_eq!(fetched.error.as_deref(), Some("retry failed"));
    }

    #[test]
    fn test_get_failed_for_retry() {
        let (repo, _dir) = setup();
        // Insert a failed run with retry_count < 3
        repo.insert(&DistillationRun {
            id: "dr-3".to_string(),
            session_id: "sess-3".to_string(),
            status: "failed".to_string(),
            retry_count: 1,
            ..Default::default()
        }).unwrap();
        // Insert a permanently_failed (should not be returned)
        repo.insert(&DistillationRun {
            id: "dr-4".to_string(),
            session_id: "sess-4".to_string(),
            status: "permanently_failed".to_string(),
            retry_count: 3,
            ..Default::default()
        }).unwrap();
        let retryable = repo.get_failed_for_retry(3).unwrap();
        assert_eq!(retryable.len(), 1);
        assert_eq!(retryable[0].session_id, "sess-3");
    }

    #[test]
    fn test_get_stats() {
        let (repo, _dir) = setup();
        repo.insert(&DistillationRun { id: "1".into(), session_id: "s1".into(), status: "success".into(), facts_extracted: 5, ..Default::default() }).unwrap();
        repo.insert(&DistillationRun { id: "2".into(), session_id: "s2".into(), status: "failed".into(), ..Default::default() }).unwrap();
        repo.insert(&DistillationRun { id: "3".into(), session_id: "s3".into(), status: "skipped".into(), ..Default::default() }).unwrap();
        let stats = repo.get_stats().unwrap();
        assert_eq!(stats.success_count, 1);
        assert_eq!(stats.failed_count, 1);
        assert_eq!(stats.skipped_count, 1);
        assert_eq!(stats.total_facts, 5);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --package gateway-database distillation -- --nocapture`
Expected: FAIL — module doesn't exist

- [ ] **Step 3: Implement DistillationRepository**

```rust
// gateway/gateway-database/src/distillation_repository.rs
use std::sync::Arc;
use crate::DatabaseManager;
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DistillationRun {
    pub id: String,
    pub session_id: String,
    pub status: String,
    pub facts_extracted: i32,
    pub entities_extracted: i32,
    pub relationships_extracted: i32,
    pub episode_created: i32,
    pub error: Option<String>,
    pub retry_count: i32,
    pub duration_ms: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct DistillationStats {
    pub success_count: i64,
    pub failed_count: i64,
    pub skipped_count: i64,
    pub permanently_failed_count: i64,
    pub total_facts: i64,
    pub total_entities: i64,
    pub total_relationships: i64,
    pub total_episodes: i64,
}

pub struct DistillationRepository {
    db: Arc<DatabaseManager>,
}

impl DistillationRepository {
    pub fn new(db: Arc<DatabaseManager>) -> Self {
        Self { db }
    }

    pub fn insert(&self, run: &DistillationRun) -> Result<(), String> {
        self.db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO distillation_runs (id, session_id, status, facts_extracted, entities_extracted, relationships_extracted, episode_created, error, retry_count, duration_ms, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![run.id, run.session_id, run.status, run.facts_extracted, run.entities_extracted, run.relationships_extracted, run.episode_created, run.error, run.retry_count, run.duration_ms, run.created_at],
            )?;
            Ok(())
        })
    }

    pub fn get_by_session_id(&self, session_id: &str) -> Result<Option<DistillationRun>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, session_id, status, facts_extracted, entities_extracted, relationships_extracted, episode_created, error, retry_count, duration_ms, created_at FROM distillation_runs WHERE session_id = ?1"
            )?;
            let mut rows = stmt.query_map([session_id], |row| {
                Ok(DistillationRun {
                    id: row.get(0)?, session_id: row.get(1)?, status: row.get(2)?,
                    facts_extracted: row.get(3)?, entities_extracted: row.get(4)?,
                    relationships_extracted: row.get(5)?, episode_created: row.get(6)?,
                    error: row.get(7)?, retry_count: row.get(8)?,
                    duration_ms: row.get(9)?, created_at: row.get(10)?,
                })
            })?;
            match rows.next() {
                Some(Ok(run)) => Ok(Some(run)),
                Some(Err(e)) => Err(e.to_string()),
                None => Ok(None),
            }
        })
    }

    pub fn update_retry(&self, session_id: &str, status: &str, retry_count: i32, error: Option<&str>) -> Result<(), String> {
        self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE distillation_runs SET status = ?1, retry_count = ?2, error = ?3 WHERE session_id = ?4",
                params![status, retry_count, error, session_id],
            )?;
            Ok(())
        })
    }

    pub fn update_success(&self, session_id: &str, facts: i32, entities: i32, rels: i32, episode: bool, duration_ms: i64) -> Result<(), String> {
        self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE distillation_runs SET status = 'success', facts_extracted = ?1, entities_extracted = ?2, relationships_extracted = ?3, episode_created = ?4, duration_ms = ?5 WHERE session_id = ?6",
                params![facts, entities, rels, episode as i32, duration_ms, session_id],
            )?;
            Ok(())
        })
    }

    pub fn get_failed_for_retry(&self, max_retries: i32) -> Result<Vec<DistillationRun>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, session_id, status, facts_extracted, entities_extracted, relationships_extracted, episode_created, error, retry_count, duration_ms, created_at
                 FROM distillation_runs WHERE status = 'failed' AND retry_count < ?1 ORDER BY created_at ASC"
            )?;
            let rows = stmt.query_map([max_retries], |row| {
                Ok(DistillationRun {
                    id: row.get(0)?, session_id: row.get(1)?, status: row.get(2)?,
                    facts_extracted: row.get(3)?, entities_extracted: row.get(4)?,
                    relationships_extracted: row.get(5)?, episode_created: row.get(6)?,
                    error: row.get(7)?, retry_count: row.get(8)?,
                    duration_ms: row.get(9)?, created_at: row.get(10)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
        })
    }

    pub fn get_undistilled_session_ids(&self) -> Result<Vec<String>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT s.id FROM sessions s
                 LEFT JOIN distillation_runs dr ON s.id = dr.session_id
                 WHERE s.status = 'completed' AND dr.id IS NULL
                 ORDER BY s.created_at ASC"
            )?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
        })
    }

    pub fn get_stats(&self) -> Result<DistillationStats, String> {
        self.db.with_connection(|conn| {
            let stats = conn.query_row(
                "SELECT
                    COALESCE(SUM(CASE WHEN status='success' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN status='failed' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN status='skipped' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN status='permanently_failed' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(facts_extracted), 0),
                    COALESCE(SUM(entities_extracted), 0),
                    COALESCE(SUM(relationships_extracted), 0),
                    COALESCE(SUM(episode_created), 0)
                 FROM distillation_runs",
                [],
                |row| Ok(DistillationStats {
                    success_count: row.get(0)?, failed_count: row.get(1)?,
                    skipped_count: row.get(2)?, permanently_failed_count: row.get(3)?,
                    total_facts: row.get(4)?, total_entities: row.get(5)?,
                    total_relationships: row.get(6)?, total_episodes: row.get(7)?,
                }),
            )?;
            Ok(stats)
        })
    }
}
```

- [ ] **Step 4: Export from lib.rs**

Add to `gateway/gateway-database/src/lib.rs`:
```rust
pub mod distillation_repository;
pub use distillation_repository::{DistillationRepository, DistillationRun, DistillationStats};
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --package gateway-database distillation -- --nocapture`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add gateway/gateway-database/src/distillation_repository.rs gateway/gateway-database/src/lib.rs
git commit -m "feat(db): add DistillationRepository — CRUD for distillation_runs"
```

---

### Task 3: Episode Repository

**Files:**
- Create: `gateway/gateway-database/src/episode_repository.rs`
- Modify: `gateway/gateway-database/src/lib.rs`

- [ ] **Step 1: Write tests for episode repository**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (EpisodeRepository, TempDir) {
        let dir = TempDir::new().unwrap();
        let db = Arc::new(DatabaseManager::new_with_path(dir.path().join("test.db")).unwrap());
        (EpisodeRepository::new(db), dir)
    }

    #[test]
    fn test_insert_and_get_episode() {
        let (repo, _dir) = setup();
        let episode = SessionEpisode {
            id: "ep-1".to_string(),
            session_id: "sess-1".to_string(),
            agent_id: "root".to_string(),
            ward_id: "__global__".to_string(),
            task_summary: "Analyze SPY options".to_string(),
            outcome: "success".to_string(),
            strategy_used: Some("delegated to data-analyst".to_string()),
            key_learnings: Some("delegation works well for financial analysis".to_string()),
            token_cost: Some(180000),
            embedding: None,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        repo.insert(&episode).unwrap();
        let fetched = repo.get_by_session_id("sess-1").unwrap().unwrap();
        assert_eq!(fetched.outcome, "success");
        assert_eq!(fetched.task_summary, "Analyze SPY options");
    }

    #[test]
    fn test_search_by_agent() {
        let (repo, _dir) = setup();
        repo.insert(&SessionEpisode {
            id: "ep-1".into(), session_id: "s1".into(), agent_id: "root".into(),
            ward_id: "__global__".into(), task_summary: "task 1".into(),
            outcome: "success".into(), ..Default::default()
        }).unwrap();
        repo.insert(&SessionEpisode {
            id: "ep-2".into(), session_id: "s2".into(), agent_id: "data-analyst".into(),
            ward_id: "__global__".into(), task_summary: "task 2".into(),
            outcome: "failed".into(), ..Default::default()
        }).unwrap();
        let results = repo.get_by_agent("root", None, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].agent_id, "root");
    }

    #[test]
    fn test_get_successful_by_agent() {
        let (repo, _dir) = setup();
        repo.insert(&SessionEpisode {
            id: "ep-1".into(), session_id: "s1".into(), agent_id: "root".into(),
            ward_id: "__global__".into(), task_summary: "t1".into(),
            outcome: "success".into(), ..Default::default()
        }).unwrap();
        repo.insert(&SessionEpisode {
            id: "ep-2".into(), session_id: "s2".into(), agent_id: "root".into(),
            ward_id: "__global__".into(), task_summary: "t2".into(),
            outcome: "failed".into(), ..Default::default()
        }).unwrap();
        let results = repo.get_successful_by_agent("root", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].outcome, "success");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --package gateway-database episode -- --nocapture`
Expected: FAIL

- [ ] **Step 3: Implement EpisodeRepository**

Implement `EpisodeRepository` with methods: `insert`, `get_by_session_id`, `get_by_agent`, `get_successful_by_agent`, `search_by_similarity` (brute-force cosine on embedding column — same pattern as `memory_repository.rs:393-429`).

The `search_by_similarity` method loads all episode embeddings for the agent, computes cosine similarity in Rust, returns top-K above threshold.

- [ ] **Step 4: Export from lib.rs**

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --package gateway-database episode -- --nocapture`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add gateway/gateway-database/src/episode_repository.rs gateway/gateway-database/src/lib.rs
git commit -m "feat(db): add EpisodeRepository — CRUD and similarity search for session_episodes"
```

---

### Task 4: RecallConfig Service

**Files:**
- Create: `gateway/gateway-services/src/recall_config.rs`
- Modify: `gateway/gateway-services/src/lib.rs`

- [ ] **Step 1: Write tests for config loading**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default_config() {
        let config = RecallConfig::default();
        assert_eq!(config.category_weights.get("correction"), Some(&1.5));
        assert_eq!(config.ward_affinity_boost, 1.3);
        assert_eq!(config.max_recall_tokens, 3000);
    }

    #[test]
    fn test_load_missing_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("recall_config.json");
        let config = RecallConfig::load_from_path(&path);
        assert_eq!(config.category_weights.get("correction"), Some(&1.5)); // defaults
    }

    #[test]
    fn test_load_partial_override() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("recall_config.json");
        std::fs::write(&path, r#"{"ward_affinity_boost": 2.0}"#).unwrap();
        let config = RecallConfig::load_from_path(&path);
        assert_eq!(config.ward_affinity_boost, 2.0); // overridden
        assert_eq!(config.category_weights.get("correction"), Some(&1.5)); // default preserved
    }

    #[test]
    fn test_load_corrupted_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("recall_config.json");
        std::fs::write(&path, "not json {{{").unwrap();
        let config = RecallConfig::load_from_path(&path);
        assert_eq!(config.ward_affinity_boost, 1.3); // falls back to defaults
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --package gateway-services recall_config -- --nocapture`
Expected: FAIL

- [ ] **Step 3: Implement RecallConfig**

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidSessionRecallConfig {
    pub enabled: bool,
    pub every_n_turns: usize,
    pub min_novelty_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecallConfig {
    pub category_weights: HashMap<String, f64>,
    pub ward_affinity_boost: f64,
    pub max_recall_tokens: usize,
    pub vector_weight: f64,
    pub bm25_weight: f64,
    pub max_facts: usize,
    pub max_episodes: usize,
    pub high_confidence_threshold: f64,
    pub mid_session_recall: MidSessionRecallConfig,
}

impl Default for RecallConfig {
    fn default() -> Self {
        let mut weights = HashMap::new();
        weights.insert("correction".into(), 1.5);
        weights.insert("strategy".into(), 1.4);
        weights.insert("user".into(), 1.3);
        weights.insert("instruction".into(), 1.2);
        weights.insert("domain".into(), 1.0);
        weights.insert("pattern".into(), 0.9);
        weights.insert("ward".into(), 0.8);
        weights.insert("skill".into(), 0.7);
        weights.insert("agent".into(), 0.7);

        Self {
            category_weights: weights,
            ward_affinity_boost: 1.3,
            max_recall_tokens: 3000,
            vector_weight: 0.7,
            bm25_weight: 0.3,
            max_facts: 10,
            max_episodes: 3,
            high_confidence_threshold: 0.9,
            mid_session_recall: MidSessionRecallConfig {
                enabled: true,
                every_n_turns: 5,
                min_novelty_score: 0.3,
            },
        }
    }
}

impl RecallConfig {
    pub fn load_from_path(path: &Path) -> Self {
        let defaults = Self::default();
        if !path.exists() {
            tracing::info!("No recall_config.json found, using compiled defaults");
            return defaults;
        }
        match std::fs::read_to_string(path) {
            Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(user_val) => {
                    let default_val = serde_json::to_value(&defaults).unwrap();
                    let merged = deep_merge(default_val, user_val);
                    serde_json::from_value(merged).unwrap_or_else(|e| {
                        tracing::warn!("Failed to parse merged recall_config.json: {}, using defaults", e);
                        defaults
                    })
                }
                Err(e) => {
                    tracing::warn!("Corrupted recall_config.json: {}, using defaults", e);
                    defaults
                }
            },
            Err(e) => {
                tracing::warn!("Failed to read recall_config.json: {}, using defaults", e);
                defaults
            }
        }
    }

    pub fn category_weight(&self, category: &str) -> f64 {
        *self.category_weights.get(category).unwrap_or(&1.0)
    }
}

fn deep_merge(base: serde_json::Value, overlay: serde_json::Value) -> serde_json::Value {
    match (base, overlay) {
        (serde_json::Value::Object(mut base_map), serde_json::Value::Object(overlay_map)) => {
            for (key, value) in overlay_map {
                let base_val = base_map.remove(&key).unwrap_or(serde_json::Value::Null);
                base_map.insert(key, deep_merge(base_val, value));
            }
            serde_json::Value::Object(base_map)
        }
        (_, overlay) => overlay,
    }
}
```

- [ ] **Step 4: Export from lib.rs**

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --package gateway-services recall_config -- --nocapture`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add gateway/gateway-services/src/recall_config.rs gateway/gateway-services/src/lib.rs
git commit -m "feat(config): add RecallConfig with compiled defaults and JSON merge"
```

---

## Chunk 2: Distillation Pipeline Fix

### Task 5: Distillation Health Reporting

**Files:**
- Modify: `gateway/gateway-execution/src/distillation.rs`
- Modify: `gateway/src/state.rs`

- [ ] **Step 1: Add DistillationRepository to SessionDistiller**

In `distillation.rs`, add `distillation_repo: Arc<DistillationRepository>` field to `SessionDistiller`. Update constructor to accept it.

- [ ] **Step 2: Wrap distill() with health reporting**

Before the existing distillation logic, insert a `distillation_runs` record with status `'failed'` (optimistic failure — update to success on completion). After successful distillation, call `distillation_repo.update_success(session_id, facts, entities, rels, episode, duration_ms)`. On error, the initial failed record stays.

Use `std::time::Instant` to measure duration_ms.

- [ ] **Step 3: Wire DistillationRepository in state.rs**

In `AppState::new()`, create `DistillationRepository::new(db_manager.clone())` and pass to `SessionDistiller`.

- [ ] **Step 4: Run workspace check**

Run: `cargo check --workspace`
Expected: Success

- [ ] **Step 5: Commit**

```bash
git add gateway/gateway-execution/src/distillation.rs gateway/src/state.rs
git commit -m "feat(distillation): add health reporting — write distillation_runs on every attempt"
```

---

### Task 6: Distillation Fallback Chain

**Files:**
- Modify: `gateway/gateway-execution/src/distillation.rs`

- [ ] **Step 1: Add provider fallback to distillation**

Currently `create_llm_client()` (distillation.rs:149-173) picks the default provider. Modify to accept a list of providers and try each in order. On failure, move to next. On all-fail, record error.

```rust
async fn create_llm_client_with_fallback(
    provider_service: &ProviderService,
) -> Result<Box<dyn LlmClient>, String> {
    let providers = provider_service.list_providers().await?;
    let mut last_error = String::new();
    for provider in &providers {
        match provider_service.create_client(provider).await {
            Ok(client) => return Ok(client),
            Err(e) => {
                tracing::warn!("Distillation provider {} failed: {}", provider.id, e);
                last_error = e.to_string();
            }
        }
    }
    Err(format!("All providers failed. Last error: {}", last_error))
}
```

- [ ] **Step 2: Run workspace check**

Run: `cargo check --workspace`
Expected: Success

- [ ] **Step 3: Commit**

```bash
git add gateway/gateway-execution/src/distillation.rs
git commit -m "feat(distillation): add provider fallback chain — try all providers in order"
```

---

### Task 7: Distillation Status API Endpoint

**Files:**
- Modify: `gateway/src/http/graph.rs`
- Modify: `gateway/src/http/mod.rs`

- [ ] **Step 1: Add GET /api/distillation/status endpoint**

```rust
#[derive(Serialize)]
pub struct DistillationStatusResponse {
    pub total_sessions: i64,
    pub distilled: DistillationStats,
    pub pending_count: i64,
}

pub async fn distillation_status(State(state): State<AppState>) -> impl IntoResponse {
    let stats = state.distillation_repo.get_stats();
    let total = state.state_repo.count_completed_sessions();
    let pending = total - (stats.success_count + stats.failed_count + stats.skipped_count + stats.permanently_failed_count);
    Json(DistillationStatusResponse { total_sessions: total, distilled: stats, pending_count: pending })
}
```

- [ ] **Step 2: Register route in mod.rs**

Add `.route("/api/distillation/status", get(graph::distillation_status))`

- [ ] **Step 3: Run workspace check**

Run: `cargo check --workspace`
Expected: Success

- [ ] **Step 4: Commit**

```bash
git add gateway/src/http/graph.rs gateway/src/http/mod.rs
git commit -m "feat(api): add GET /api/distillation/status endpoint"
```

---

## Chunk 3: Episodic Memory & Strategy

### Task 8: Extended Distillation — Episode Extraction

**Files:**
- Modify: `gateway/gateway-execution/src/distillation.rs`

- [ ] **Step 1: Extend the distillation prompt**

Add to the `DEFAULT_DISTILLATION_PROMPT` (or vault config prompt):

```
## Episode Assessment
In addition to facts, entities, and relationships, assess the session as a whole:
- task_summary: What was the user trying to accomplish? (1-2 sentences)
- outcome: Did the agent complete the goal? One of: success, partial, failed
- strategy_used: What approach was taken? (e.g., "delegated to data-analyst for technicals")
- key_learnings: What went well or poorly? (1-2 sentences)
```

Add `strategy` to the list of allowed fact categories.

- [ ] **Step 2: Parse episode from LLM response**

Extend the `ExtractedData` struct to include an optional `episode` field:

```rust
#[derive(Debug, Deserialize)]
struct ExtractedEpisode {
    task_summary: String,
    outcome: String,
    strategy_used: Option<String>,
    key_learnings: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ExtractedData {
    facts: Vec<ExtractedFact>,
    entities: Vec<ExtractedEntity>,
    relationships: Vec<ExtractedRelationship>,
    episode: Option<ExtractedEpisode>,
}
```

- [ ] **Step 3: Store episode in database**

After storing facts and entities, if `episode` is present:
1. Embed the `task_summary` using the embedding client
2. Create a `SessionEpisode` and insert via `EpisodeRepository`
3. Set `episode_created = 1` in the distillation run

- [ ] **Step 4: Run workspace check**

Run: `cargo check --workspace`
Expected: Success

- [ ] **Step 5: Commit**

```bash
git add gateway/gateway-execution/src/distillation.rs
git commit -m "feat(distillation): extract episode (task summary, outcome, strategy) during distillation"
```

---

### Task 9: Strategy Emergence

**Files:**
- Modify: `gateway/gateway-execution/src/distillation.rs`

- [ ] **Step 1: Add strategy emergence after episode creation**

After successfully creating an episode with outcome `'success'`:
1. Call `episode_repo.search_by_similarity(agent_id, &task_summary_embedding, 0.7, 10)`
2. Filter results: `outcome == 'success'` only
3. If 2+ similar successful episodes share a similar strategy pattern:
   - Write/upsert a `memory_facts` entry with `category = 'strategy'`
   - Key: `strategy.{inferred_task_type}` (extracted from task_summary)
   - Content: The common strategy pattern
   - Confidence: 0.9

- [ ] **Step 2: Run workspace check**

Run: `cargo check --workspace`
Expected: Success

- [ ] **Step 3: Commit**

```bash
git add gateway/gateway-execution/src/distillation.rs
git commit -m "feat(distillation): add strategy emergence from repeated successful episodes"
```

---

## Chunk 4: Recall Priority Engine

### Task 10: Priority Scoring in Recall

**Files:**
- Modify: `gateway/gateway-execution/src/recall.rs`
- Modify: `gateway/src/state.rs`

- [ ] **Step 1: Add RecallConfig to MemoryRecall**

Add `config: Arc<RecallConfig>` field. Update constructor and state.rs wiring.

- [ ] **Step 2: Apply category weights and ward affinity**

In the `recall()` method, after getting base scores from hybrid search, apply:

```rust
let category_weight = self.config.category_weight(&fact.category);
let ward_affinity = if fact.ward_id == current_ward_id { self.config.ward_affinity_boost } else { 1.0 };
scored_fact.score *= category_weight * ward_affinity;
```

- [ ] **Step 3: Add episodic recall**

After fact recall, query `episode_repo.search_by_similarity()` for top-K episodes matching the user's query. Merge into the formatted output.

- [ ] **Step 4: Update formatted output**

Structure the recall output as:
```
## Recalled Knowledge
### Corrections & Preferences
### Relevant Past Experiences
### Domain Context
```

Apply `max_recall_tokens` budget, trimming lowest-scored items first.

- [ ] **Step 5: Wire ward_id into recall queries**

Pass `current_ward_id` through the recall chain. In `memory_repository.rs`, update hybrid search to filter by `WHERE ward_id = '__global__' OR ward_id = ?`.

- [ ] **Step 6: Run workspace check**

Run: `cargo check --workspace`
Expected: Success

- [ ] **Step 7: Commit**

```bash
git add gateway/gateway-execution/src/recall.rs gateway/gateway-database/src/memory_repository.rs gateway/src/state.rs
git commit -m "feat(recall): add priority scoring — category weights, ward affinity, episodic recall"
```

---

### Task 11: Delegation Recall Injection

**Files:**
- Modify: `gateway/gateway-execution/src/delegation/spawn.rs`
- Modify: `gateway/gateway-execution/src/runner.rs`

- [ ] **Step 1: Inject recall at delegation spawn**

In `spawn_delegated_agent()`, before the child agent's first LLM call:
1. Run `recall.recall_with_graph(child_agent_id, &delegation_task, 5)` using the child's agent_id
2. Inject result as system message at position 0 in child's history

- [ ] **Step 2: Fix continuation query**

In `runner.rs` line ~1415, replace hardcoded `"[continuation - recall recent learnings]"` with the actual continuation message.

- [ ] **Step 3: Run workspace check**

Run: `cargo check --workspace`
Expected: Success

- [ ] **Step 4: Commit**

```bash
git add gateway/gateway-execution/src/delegation/spawn.rs gateway/gateway-execution/src/runner.rs
git commit -m "feat(recall): inject recall at delegation spawn, fix continuation query"
```

---

### Task 12: Mid-Session Recall Middleware

**Files:**
- Create: `gateway/gateway-execution/src/middleware/recall_refresh.rs`
- Modify: `runtime/agent-runtime/src/middleware/mod.rs`

- [ ] **Step 1: Implement recall refresh middleware**

```rust
pub struct RecallRefreshMiddleware {
    recall: Arc<MemoryRecall>,
    config: Arc<RecallConfig>,
    injected_keys: Mutex<HashSet<String>>,
    turn_count: AtomicUsize,
}
```

On each turn:
1. Increment `turn_count`
2. If `turn_count % config.mid_session_recall.every_n_turns != 0`, skip
3. Run recall with latest user message
4. Filter results: exclude keys in `injected_keys` set
5. Filter results: only include facts with `score >= min_novelty_score`
6. If any novel facts remain, inject as system message
7. Add their keys to `injected_keys`

- [ ] **Step 2: Register in middleware pipeline**

Add to the middleware chain in the executor/runner where other middleware is registered.

- [ ] **Step 3: Run workspace check**

Run: `cargo check --workspace`
Expected: Success

- [ ] **Step 4: Commit**

```bash
git add gateway/gateway-execution/src/middleware/recall_refresh.rs runtime/agent-runtime/src/middleware/mod.rs
git commit -m "feat(middleware): add mid-session recall refresh — auto-inject novel facts every N turns"
```

---

### Task 13: Upgrade Memory Recall Tool

**Files:**
- Modify: `runtime/agent-tools/src/tools/memory.rs`

- [ ] **Step 1: Upgrade action_recall to use priority engine**

Replace the basic hybrid search in the `recall` action with a call to the full `MemoryRecall` service (with priority weights, ward affinity, episodic lookup).

The memory tool already has access to the fact store. Add a reference to `MemoryRecall` (or pass the RecallConfig so it can apply weights locally).

- [ ] **Step 2: Run workspace check**

Run: `cargo check --workspace`
Expected: Success

- [ ] **Step 3: Commit**

```bash
git add runtime/agent-tools/src/tools/memory.rs
git commit -m "feat(tools): upgrade memory.recall to use full priority engine"
```

---

## Chunk 5: Retroactive Bootstrap

### Task 14: CLI Backfill Command

**Files:**
- Modify: `apps/cli/src/main.rs`

- [ ] **Step 1: Add Distill subcommand to CLI**

```rust
#[derive(Subcommand)]
enum Commands {
    // ... existing commands

    /// Manage distillation
    Distill {
        #[command(subcommand)]
        action: DistillAction,
    },
}

#[derive(Subcommand)]
enum DistillAction {
    /// Retroactively distill all undistilled sessions
    Backfill {
        /// Max concurrent LLM calls
        #[arg(long, default_value = "2")]
        concurrency: usize,
    },
}
```

- [ ] **Step 2: Implement backfill handler**

The backfill handler:
1. Connects to gateway via HTTP: `GET /api/distillation/undistilled` (new endpoint)
2. For each session, calls `POST /api/distillation/trigger/:session_id` (new endpoint)
3. Reports progress to stdout

Alternatively, if the CLI can access the database directly (check existing patterns), it can use the `SessionDistiller` directly.

- [ ] **Step 3: Add gateway endpoints for backfill**

Add to `gateway/src/http/graph.rs`:
- `GET /api/distillation/undistilled` — returns list of undistilled session IDs
- `POST /api/distillation/trigger/:session_id` — triggers distillation for a specific session

- [ ] **Step 4: Run workspace check**

Run: `cargo check --workspace`
Expected: Success

- [ ] **Step 5: Commit**

```bash
git add apps/cli/src/main.rs gateway/src/http/graph.rs gateway/src/http/mod.rs
git commit -m "feat(cli): add 'zero distill --backfill' command for retroactive distillation"
```

---

## Chunk 6: Observatory UI

### Task 15: Install D3 Dependencies

**Files:**
- Modify: `apps/ui/package.json`

- [ ] **Step 1: Install D3 packages**

Run: `cd apps/ui && npm install d3-force d3-selection d3-zoom && npm install -D @types/d3-force @types/d3-selection @types/d3-zoom`

- [ ] **Step 2: Commit**

```bash
git add apps/ui/package.json apps/ui/package-lock.json
git commit -m "deps(ui): add d3-force, d3-selection, d3-zoom for Observatory"
```

---

### Task 16: Graph Stats API Endpoint

**Files:**
- Modify: `gateway/src/http/graph.rs`
- Modify: `gateway/src/http/mod.rs`

- [ ] **Step 1: Add GET /api/graph/stats endpoint**

Returns aggregate counts: total entities, total relationships, entities by type, entities by agent.

```rust
pub async fn graph_stats(State(state): State<AppState>) -> impl IntoResponse {
    match &state.graph_service {
        Some(gs) => {
            let entity_count = gs.count_entities().await.unwrap_or(0);
            let rel_count = gs.count_relationships().await.unwrap_or(0);
            let distillation = state.distillation_repo.get_stats().unwrap_or_default();
            let episode_count = /* query session_episodes count */;
            let fact_count = state.memory_repo.count_facts().unwrap_or(0);
            Json(json!({
                "entities": entity_count,
                "relationships": rel_count,
                "facts": fact_count,
                "episodes": episode_count,
                "distillation": distillation,
            })).into_response()
        }
        None => (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "Graph service not available"}))).into_response(),
    }
}
```

- [ ] **Step 2: Add GET /api/graph/all/entities endpoint**

Cross-agent entity listing for Observatory "All Agents" mode. Filter by optional `ward_id` and `entity_type` query params.

- [ ] **Step 3: Register routes**

```rust
.route("/api/graph/stats", get(graph::graph_stats))
.route("/api/graph/all/entities", get(graph::all_entities))
.route("/api/distillation/status", get(graph::distillation_status))
```

- [ ] **Step 4: Run workspace check and test**

Run: `cargo check --workspace && cargo test --package gateway`
Expected: Success

- [ ] **Step 5: Commit**

```bash
git add gateway/src/http/graph.rs gateway/src/http/mod.rs
git commit -m "feat(api): add graph stats and cross-agent entity endpoints for Observatory"
```

---

### Task 17: Observatory Data Hooks

**Files:**
- Create: `apps/ui/src/features/observatory/graph-hooks.ts`

- [ ] **Step 1: Create data fetching hooks**

```typescript
import { useState, useEffect, useCallback } from 'react';

export interface GraphEntity {
  id: string;
  name: string;
  entity_type: string;
  agent_id: string;
  mention_count: number;
  properties?: Record<string, string>;
  first_seen_at: string;
  last_seen_at: string;
}

export interface GraphRelationship {
  id: string;
  source_entity_id: string;
  target_entity_id: string;
  relationship_type: string;
  mention_count: number;
}

export interface GraphStats {
  entities: number;
  relationships: number;
  facts: number;
  episodes: number;
  distillation: {
    success_count: number;
    failed_count: number;
    skipped_count: number;
    total_facts: number;
  };
}

export function useGraphEntities(agentId?: string, wardId?: string) {
  const [entities, setEntities] = useState<GraphEntity[]>([]);
  const [relationships, setRelationships] = useState<GraphRelationship[]>([]);
  const [loading, setLoading] = useState(true);

  const fetchData = useCallback(async () => {
    setLoading(true);
    const url = agentId
      ? `/api/graph/${agentId}/entities`
      : `/api/graph/all/entities`;
    const params = new URLSearchParams();
    if (wardId) params.set('ward_id', wardId);

    const [entRes, relRes] = await Promise.all([
      fetch(`${url}?${params}`),
      fetch(agentId ? `/api/graph/${agentId}/relationships` : `/api/graph/all/relationships`),
    ]);
    setEntities(await entRes.json());
    setRelationships(await relRes.json());
    setLoading(false);
  }, [agentId, wardId]);

  useEffect(() => { fetchData(); }, [fetchData]);
  return { entities, relationships, loading, refetch: fetchData };
}

export function useGraphStats() {
  const [stats, setStats] = useState<GraphStats | null>(null);
  useEffect(() => {
    fetch('/api/graph/stats').then(r => r.json()).then(setStats);
  }, []);
  return stats;
}

export function useDistillationStatus() {
  const [status, setStatus] = useState<any>(null);
  useEffect(() => {
    fetch('/api/distillation/status').then(r => r.json()).then(setStatus);
  }, []);
  return status;
}
```

- [ ] **Step 2: Commit**

```bash
git add apps/ui/src/features/observatory/graph-hooks.ts
git commit -m "feat(ui): add Observatory data fetching hooks"
```

---

### Task 18: GraphCanvas Component

**Files:**
- Create: `apps/ui/src/features/observatory/GraphCanvas.tsx`

- [ ] **Step 1: Implement D3-force graph component**

React component that:
1. Takes `entities` and `relationships` as props
2. Creates a D3 force simulation with forceLink, forceManyBody, forceCenter
3. Renders SVG with circles (nodes) and lines (edges)
4. Node size proportional to `mention_count`
5. Node color by `entity_type` (use CSS custom properties from theme.css)
6. Click handler calls `onEntitySelect(entity)`
7. Zoom/pan via d3-zoom
8. Search highlight via `highlightTerm` prop

Use `useRef` for the SVG element. Use `useEffect` to create/update the simulation when data changes. Follow existing React patterns — no inline styles, use semantic CSS classes.

- [ ] **Step 2: Add observatory CSS classes to components.css**

```css
/* Observatory */
.observatory { display: flex; flex-direction: column; height: 100%; }
.observatory__toolbar { display: flex; justify-content: space-between; align-items: center; padding: var(--spacing-3); border-bottom: 1px solid var(--border-primary); }
.observatory__main { display: flex; flex: 1; overflow: hidden; }
.observatory__canvas { flex: 1; position: relative; }
.observatory__canvas svg { width: 100%; height: 100%; }
.observatory__sidebar { width: 280px; border-left: 1px solid var(--border-primary); overflow-y: auto; padding: var(--spacing-4); }
.observatory__health { border-top: 1px solid var(--border-primary); padding: var(--spacing-3); }

.graph-node { cursor: pointer; transition: opacity 0.2s; }
.graph-node:hover { opacity: 0.8; }
.graph-node--person { fill: var(--color-indigo-500, #6366f1); }
.graph-node--concept { fill: var(--color-amber-500, #f59e0b); }
.graph-node--agent { fill: var(--color-emerald-500, #10b981); }
.graph-node--tool { fill: var(--color-emerald-500, #10b981); }
.graph-node--project { fill: var(--color-red-500, #ef4444); }
.graph-node--strategy { fill: var(--color-violet-500, #8b5cf6); }
.graph-node--selected { stroke: var(--accent-primary); stroke-width: 2px; }
.graph-node--dimmed { opacity: 0.2; }

.graph-edge { stroke: var(--border-primary); }
.graph-label { fill: var(--text-primary); font-size: 10px; pointer-events: none; text-anchor: middle; }
```

- [ ] **Step 3: Commit**

```bash
git add apps/ui/src/features/observatory/GraphCanvas.tsx apps/ui/src/styles/components.css
git commit -m "feat(ui): add GraphCanvas D3-force component with entity type coloring"
```

---

### Task 19: EntityDetail Sidebar + LearningHealthBar

**Files:**
- Create: `apps/ui/src/features/observatory/EntityDetail.tsx`
- Create: `apps/ui/src/features/observatory/LearningHealthBar.tsx`

- [ ] **Step 1: Implement EntityDetail**

Slide-over sidebar that shows:
- Entity name, type badge, mention count
- Connections grouped by relationship type (fetched via `/api/graph/:agent_id/entities/:id/connections`)
- Related memory facts
- Timeline: first_seen_at, last_seen_at

Follow existing slide-over/card pattern from ARCHITECTURE.md.

- [ ] **Step 2: Implement LearningHealthBar**

Bottom bar with:
- Sessions distilled / total (from useDistillationStatus hook)
- Facts, entities, relationships, episodes counts (from useGraphStats hook)
- Failed and skipped counts

- [ ] **Step 3: Commit**

```bash
git add apps/ui/src/features/observatory/EntityDetail.tsx apps/ui/src/features/observatory/LearningHealthBar.tsx
git commit -m "feat(ui): add EntityDetail sidebar and LearningHealthBar components"
```

---

### Task 20: ObservatoryPage + Routing

**Files:**
- Create: `apps/ui/src/features/observatory/ObservatoryPage.tsx`
- Create: `apps/ui/src/features/observatory/index.ts`
- Modify: `apps/ui/src/App.tsx`

- [ ] **Step 1: Implement ObservatoryPage**

Main page that composes:
- Toolbar (agent filter pills, ward scope pills, search input)
- GraphCanvas (main area)
- EntityDetail sidebar (shown on entity click)
- LearningHealthBar (bottom)

State: `selectedEntity`, `agentFilter`, `wardFilter`, `searchTerm`

- [ ] **Step 2: Create barrel export**

`index.ts`:
```typescript
export { ObservatoryPage } from './ObservatoryPage';
```

- [ ] **Step 3: Add route to App.tsx**

Add `/observatory` route pointing to `ObservatoryPage`. Check existing routing pattern in App.tsx and follow it.

- [ ] **Step 4: Run UI build**

Run: `cd apps/ui && npm run build`
Expected: Success

- [ ] **Step 5: Commit**

```bash
git add apps/ui/src/features/observatory/ apps/ui/src/App.tsx
git commit -m "feat(ui): add Observatory page — knowledge graph visualization with D3-force"
```

---

## Chunk 7: Integration & Verification

### Task 21: Update memory_repository.rs for ward_id

**Files:**
- Modify: `gateway/gateway-database/src/memory_repository.rs`
- Modify: `gateway/gateway-database/src/memory_fact_store.rs`

- [ ] **Step 1: Update MemoryFact struct**

Add `pub ward_id: String` field (default `"__global__"`).

- [ ] **Step 2: Update all SQL queries**

- `upsert_memory_fact`: include `ward_id` in INSERT and ON CONFLICT clause (update the UNIQUE key to `agent_id, scope, ward_id, key`)
- `search_memory_facts_hybrid`: add `AND (ward_id = '__global__' OR ward_id = ?ward)` filter
- `search_memory_facts_fts`: same ward filter via JOIN
- `get_memory_facts`: include `ward_id` in SELECT

- [ ] **Step 3: Update GatewayMemoryFactStore**

Pass `ward_id` through `save_fact()` method. Default to `"__global__"` if not specified.

- [ ] **Step 4: Run existing tests**

Run: `cargo test --package gateway-database -- --nocapture`
Expected: PASS (existing tests should work with default ward_id)

- [ ] **Step 5: Commit**

```bash
git add gateway/gateway-database/src/memory_repository.rs gateway/gateway-database/src/memory_fact_store.rs
git commit -m "feat(db): add ward_id to memory queries — filter by __global__ + current ward"
```

---

### Task 22: End-to-End Verification

- [ ] **Step 1: Run full Rust workspace tests**

Run: `cargo test --workspace`
Expected: All tests pass

- [ ] **Step 2: Run full UI build**

Run: `cd apps/ui && npm run build`
Expected: Success

- [ ] **Step 3: Run UI tests**

Run: `cd apps/ui && npm test -- --run`
Expected: All tests pass

- [ ] **Step 4: Manual smoke test**

1. Start daemon: `cargo run --package zerod`
2. Open UI at localhost:3000
3. Navigate to /observatory — should show empty graph with health bar
4. Send a message to an agent, let it complete
5. Check /api/distillation/status — should show the session being distilled
6. Check /observatory — should start showing entities after distillation

- [ ] **Step 5: Run backfill**

```bash
zero distill --backfill
```

Expected: Processes existing sessions, populates knowledge graph

- [ ] **Step 6: Verify Observatory populated**

Refresh /observatory — should now show entities, relationships, and a populated learning health bar.

- [ ] **Step 7: Final commit**

```bash
git add -A
git commit -m "feat: cognitive memory & knowledge graph — complete implementation"
```

---

## Chunk 8: Correctness Hardening (Addendum)

Added after model council review. These are surgical improvements that harden the system for correctness under failure.

### Task 23: Ward-Entry Recall Trigger

**Files:**
- Modify: `runtime/agent-tools/src/tools/ward.rs`

- [ ] **Step 1: Read the ward tool implementation**

Read `runtime/agent-tools/src/tools/ward.rs` to understand:
- How ward switching works
- What context/state the tool has access to
- Where the "ward switched successfully" result is returned

- [ ] **Step 2: Add MemoryRecall to ward tool context**

The ward tool needs access to `MemoryRecall` to trigger recall on ward entry. Check how other tools receive service references (look at how the memory tool gets `MemoryFactStore`). Follow the same pattern to pass `MemoryRecall` to the ward tool.

- [ ] **Step 3: Trigger recall after ward switch**

After a successful ward switch (the tool returns a success result):

```rust
// After ward switch succeeds:
if let Some(recall) = &self.memory_recall {
    let ward_id = new_ward_name;
    let query = format!("ward {} project context", ward_id);
    match recall.recall(&agent_id, &query, Some(ward_id), 5).await {
        Ok(result) if !result.facts.is_empty() => {
            // Inject as system message via execution context
            // The tool result can include a "recall_context" field
            // that the executor injects as a system message
            tracing::info!(
                ward = %ward_id,
                facts = result.facts.len(),
                "Ward-entry recall injected {} facts",
                result.facts.len()
            );
        }
        _ => {} // No facts for this ward yet — that's fine
    }
}
```

The exact injection mechanism depends on how the tool communicates back to the executor. Options:
- Append recall context to the tool result text
- Use a side-channel (if the tool has access to the message history)
- Return a structured result that the executor parses

Read the executor's tool result handling to determine the best approach.

- [ ] **Step 4: Verify**

Run: `cargo check --workspace`

- [ ] **Step 5: Commit**

```bash
git add runtime/agent-tools/src/tools/ward.rs
git commit -m "feat(recall): trigger ward-scoped recall on ward entry"
```

---

### Task 24: Schema Migration v12 — Contradiction Support

**Files:**
- Modify: `gateway/gateway-database/src/schema.rs`

- [ ] **Step 1: Add migration v12**

Increment `SCHEMA_VERSION` to 12. Add migration block:

```rust
if version < 12 {
    let _ = conn.execute(
        "ALTER TABLE memory_facts ADD COLUMN contradicted_by TEXT",
        [],
    );
}
```

Also update the initial `CREATE TABLE memory_facts` to include `contradicted_by TEXT`.

- [ ] **Step 2: Add test**

```rust
#[test]
fn test_migration_v12_adds_contradicted_by() {
    let conn = Connection::open_in_memory().unwrap();
    initialize_database(&conn).unwrap();
    let has_col: bool = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('memory_facts') WHERE name='contradicted_by'",
        [], |row| row.get::<_, i64>(0),
    ).unwrap() > 0;
    assert!(has_col);
}
```

- [ ] **Step 3: Verify**

Run: `cargo test --package gateway-database schema -- --nocapture`

- [ ] **Step 4: Commit**

```bash
git add gateway/gateway-database/src/schema.rs
git commit -m "feat(db): schema v12 — add contradicted_by column to memory_facts"
```

---

### Task 25: Contradiction Detection on Write

**Files:**
- Modify: `gateway/gateway-database/src/memory_repository.rs`
- Modify: `gateway/gateway-database/src/memory_fact_store.rs`
- Modify: `gateway/gateway-services/src/recall_config.rs`
- Modify: `gateway/gateway-execution/src/recall.rs`

- [ ] **Step 1: Add contradiction config to RecallConfig**

In `recall_config.rs`, add to the struct and defaults:
```rust
pub contradiction_penalty: f64,              // default: 0.7
pub contradiction_similarity_threshold: f64,  // default: 0.8
```

- [ ] **Step 2: Add contradicted_by to MemoryFact struct**

In `memory_repository.rs`, add `pub contradicted_by: Option<String>` to `MemoryFact`. Update `row_to_memory_fact` and all queries.

- [ ] **Step 3: Implement contradiction check in memory_fact_store**

In `memory_fact_store.rs`, after a successful `upsert_memory_fact`:

```rust
// Check for contradictions with semantically similar facts
if let Some(embedding) = &fact_embedding {
    let similar = self.memory_repo.search_memory_facts_vector(
        embedding, agent_id, 0.8, 5, None
    )?;
    for similar_fact in &similar {
        if similar_fact.key != new_fact.key && similar_fact.category == new_fact.category {
            // Potential contradiction — reduce old fact's confidence
            self.memory_repo.mark_contradicted(
                &similar_fact.id, &new_fact.key
            )?;
            tracing::info!(
                old_key = %similar_fact.key,
                new_key = %new_fact.key,
                "Contradiction detected — reduced confidence on '{}'",
                similar_fact.key
            );
        }
    }
}
```

- [ ] **Step 4: Add mark_contradicted to MemoryRepository**

```rust
pub fn mark_contradicted(&self, fact_id: &str, contradicted_by: &str) -> Result<(), String> {
    self.db.with_connection(|conn| {
        conn.execute(
            "UPDATE memory_facts SET contradicted_by = ?1, confidence = MAX(0.1, confidence - 0.15) WHERE id = ?2",
            params![contradicted_by, fact_id],
        )?;
        Ok(())
    })
}
```

- [ ] **Step 5: Apply contradiction penalty in recall scoring**

In `recall.rs`, after category weight and ward affinity:
```rust
if fact.contradicted_by.is_some() {
    scored_fact.score *= self.config.contradiction_penalty;
}
```

- [ ] **Step 6: Verify**

Run: `cargo test --package gateway-database -- --nocapture`
Run: `cargo check --workspace`

- [ ] **Step 7: Commit**

```bash
git add gateway/gateway-database/src/memory_repository.rs gateway/gateway-database/src/memory_fact_store.rs gateway/gateway-services/src/recall_config.rs gateway/gateway-execution/src/recall.rs gateway/gateway-database/src/schema.rs
git commit -m "feat(memory): add contradiction detection — flag and penalize conflicting facts"
```

---

### Task 26: Failure Clustering in Episodes

**Files:**
- Modify: `gateway/gateway-execution/src/distillation.rs`

- [ ] **Step 1: Add try_cluster_failures after episode creation**

In `distillation.rs`, after `store_episode()` when the episode outcome is `failed` or `partial`:

```rust
if episode.outcome == "failed" || episode.outcome == "partial" {
    if let Err(e) = self.try_cluster_failures(agent_id, &episode, ward_id).await {
        tracing::warn!(error = %e, "Failure clustering failed (non-fatal)");
    }
}
```

- [ ] **Step 2: Implement try_cluster_failures**

```rust
async fn try_cluster_failures(
    &self,
    agent_id: &str,
    episode: &ExtractedEpisode,
    ward_id: &str,
) -> Result<(), String> {
    let episode_repo = self.episode_repo.as_ref()
        .ok_or("No episode repository")?;

    // Embed the task summary
    let embedding = self.embed_text(&episode.task_summary).await
        .ok_or("Failed to embed task summary")?;

    // Find similar episodes
    let similar = episode_repo.search_by_similarity(agent_id, &embedding, 0.6, 20)?;

    // Filter to failures only (excluding the current episode)
    let failures: Vec<_> = similar.iter()
        .filter(|(ep, _score)| ep.outcome == "failed" || ep.outcome == "partial")
        .collect();

    if failures.len() < 3 {
        return Ok(()); // Not enough failures to cluster
    }

    // Extract common pattern from key_learnings
    let learnings: Vec<&str> = failures.iter()
        .filter_map(|(ep, _)| ep.key_learnings.as_deref())
        .collect();

    let pattern_summary = if let Some(latest) = learnings.first() {
        format!("Recurring failure ({} episodes): {}", failures.len(), latest)
    } else {
        return Ok(());
    };

    // Write correction fact
    let key = format!("correction.recurring.{}", sanitize_task_type(&episode.task_summary));
    let confidence = (0.85 + 0.02 * failures.len() as f64).min(0.98);

    let fact = MemoryFact {
        id: format!("fact-{}", uuid::Uuid::new_v4()),
        agent_id: agent_id.to_string(),
        scope: "agent".to_string(),
        ward_id: ward_id.to_string(),
        category: "correction".to_string(),
        key,
        content: pattern_summary,
        confidence,
        mention_count: failures.len() as i32,
        source_summary: Some("Clustered from repeated failures".to_string()),
        embedding: Some(embedding),
        ..Default::default()
    };

    self.memory_repo.upsert_memory_fact(&fact)?;
    tracing::info!(
        cluster_size = failures.len(),
        key = %fact.key,
        "Failure cluster detected — wrote correction fact"
    );

    Ok(())
}
```

- [ ] **Step 3: Verify**

Run: `cargo check --workspace`

- [ ] **Step 4: Commit**

```bash
git add gateway/gateway-execution/src/distillation.rs
git commit -m "feat(distillation): add failure clustering — auto-generate corrections from repeated failures"
```

---

### Task 27: End-to-End Verification (Addendum)

- [ ] **Step 1: Run full workspace tests**

Run: `cargo test --workspace --lib --bins --tests`

- [ ] **Step 2: Verify contradiction detection**

Insert two contradictory facts via API or test, verify that the older one gets `contradicted_by` set and confidence reduced.

- [ ] **Step 3: Verify failure clustering**

Check that after 3+ similar failed episodes, a `correction.recurring.*` fact exists in memory_facts.

- [ ] **Step 4: Verify ward-entry recall**

Switch to a ward that has facts, verify recall fires and facts are available in context.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat: correctness hardening — ward recall, contradiction detection, failure clustering"
```
