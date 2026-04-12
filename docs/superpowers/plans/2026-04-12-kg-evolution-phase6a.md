# Knowledge Graph Evolution — Phase 6a Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add episode-based provenance + ward artifact indexer so the graph captures domain content (people, places, events) from structured ward files that previously were invisible to the knowledge graph.

**Architecture:** Two components: (1) `kg_episodes` table records every extraction event with provenance, and (2) `WardArtifactIndexer` scans ward directories post-session, parses structured JSON/CSV collections, and produces entities tagged with `epistemic_class = archival` and `source_ref` pointing to the file.

**Tech Stack:** Rust (gateway-execution, gateway-database, knowledge-graph), SQLite, serde_json for structured file parsing.

**Spec:** `docs/superpowers/specs/2026-04-12-knowledge-graph-evolution-design.md` — Phase 6a

**Branch:** `feature/sentient` (continues from Phase 5)

---

## File Structure

| Action | File | Responsibility |
|--------|------|----------------|
| MODIFY | `gateway/gateway-database/src/schema.rs` | Migration v21: kg_episodes table + new columns on facts/entities/relationships |
| CREATE | `gateway/gateway-database/src/kg_episode_repository.rs` | CRUD for kg_episodes (content_hash dedup) |
| MODIFY | `gateway/gateway-database/src/lib.rs` | Export KgEpisodeRepository |
| CREATE | `gateway/gateway-execution/src/ward_artifact_indexer.rs` | Scan ward → parse structured files → emit entities with episodes |
| MODIFY | `gateway/gateway-execution/src/lib.rs` | Export ward_artifact_indexer |
| MODIFY | `gateway/gateway-execution/src/runner.rs` | Call indexer after distillation |
| MODIFY | `services/knowledge-graph/src/types.rs` | Add epistemic_class, source_episode_ids, source_ref fields |
| MODIFY | `services/knowledge-graph/src/storage.rs` | Support new columns in INSERT/SELECT |
| MODIFY | `gateway/gateway-database/src/memory_repository.rs` | Add epistemic_class, source_episode_id, source_ref fields to MemoryFact |
| MODIFY | `gateway/src/state.rs` | Wire KgEpisodeRepository and WardArtifactIndexer |

---

### Task 1: Migration v21 — Episodes Table + Provenance Columns

**Files:**
- Modify: `gateway/gateway-database/src/schema.rs`

- [ ] **Step 1: Read current schema version**

Open `gateway/gateway-database/src/schema.rs` and locate `SCHEMA_VERSION`. It should be 20 (set in Phase 4). Increment to 21.

- [ ] **Step 2: Add the v20→v21 migration block**

Add inside `migrate_database()` after the v19→v20 block:

```rust
if version < 21 {
    // kg_episodes: every extraction has a source episode for provenance
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS kg_episodes (
            id TEXT PRIMARY KEY,
            source_type TEXT NOT NULL,
            source_ref TEXT NOT NULL,
            content_hash TEXT NOT NULL,
            session_id TEXT,
            agent_id TEXT NOT NULL,
            created_at TEXT NOT NULL,
            UNIQUE(content_hash, source_type)
        );
        CREATE INDEX IF NOT EXISTS idx_episodes_session ON kg_episodes(session_id);
        CREATE INDEX IF NOT EXISTS idx_episodes_source ON kg_episodes(source_type, source_ref);",
    )?;

    // Add provenance + epistemic_class columns (ALTER TABLE is idempotent-ish
    // via let _ = so re-running is safe).
    let _ = conn.execute(
        "ALTER TABLE memory_facts ADD COLUMN epistemic_class TEXT DEFAULT 'current'",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE memory_facts ADD COLUMN source_episode_id TEXT",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE memory_facts ADD COLUMN source_ref TEXT",
        [],
    );
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_facts_class ON memory_facts(agent_id, epistemic_class)",
        [],
    );
}
```

- [ ] **Step 3: Update fresh-install schema**

Find the block that creates `memory_facts` for brand-new databases. Add the three columns to its `CREATE TABLE` statement. Add the `kg_episodes` CREATE TABLE + indexes after it.

- [ ] **Step 4: Update the schema version test assertion**

Find the test that asserts `SCHEMA_VERSION == 20` and bump to 21.

- [ ] **Step 5: Verify compilation**

Run: `cargo check --package gateway-database`
Expected: Clean.

- [ ] **Step 6: Run schema tests**

Run: `cargo test --package gateway-database -- schema`
Expected: All pass, including the updated version assertion.

- [ ] **Step 7: Commit**

```bash
git add gateway/gateway-database/src/schema.rs
git commit -m "feat(db): migration v21 — kg_episodes table + provenance columns on memory_facts"
```

---

### Task 2: Schema Migration for Knowledge Graph Tables

**Files:**
- Modify: `services/knowledge-graph/src/storage.rs` (or wherever the graph tables are defined)

The graph tables `kg_entities` and `kg_relationships` live in a separate SQLite file (`knowledge_graph.db`). They need parallel additions.

- [ ] **Step 1: Find the graph schema initialization**

Locate where `kg_entities` and `kg_relationships` are created. This is typically in `services/knowledge-graph/src/storage.rs` — look for `CREATE TABLE IF NOT EXISTS kg_entities`.

- [ ] **Step 2: Add columns to kg_entities**

In the CREATE TABLE block, add:

```sql
aliases TEXT,
epistemic_class TEXT DEFAULT 'current',
source_episode_ids TEXT,
valid_from TEXT,
valid_until TEXT,
confidence REAL DEFAULT 0.8
```

Also add an ALTER TABLE migration path in the storage init for existing databases:

```rust
let _ = conn.execute("ALTER TABLE kg_entities ADD COLUMN aliases TEXT", []);
let _ = conn.execute("ALTER TABLE kg_entities ADD COLUMN epistemic_class TEXT DEFAULT 'current'", []);
let _ = conn.execute("ALTER TABLE kg_entities ADD COLUMN source_episode_ids TEXT", []);
let _ = conn.execute("ALTER TABLE kg_entities ADD COLUMN valid_from TEXT", []);
let _ = conn.execute("ALTER TABLE kg_entities ADD COLUMN valid_until TEXT", []);
let _ = conn.execute("ALTER TABLE kg_entities ADD COLUMN confidence REAL DEFAULT 0.8", []);
conn.execute_batch(
    "CREATE INDEX IF NOT EXISTS idx_entities_class ON kg_entities(agent_id, epistemic_class);"
)?;
```

- [ ] **Step 3: Add columns to kg_relationships**

Same pattern for `kg_relationships`:

```rust
let _ = conn.execute("ALTER TABLE kg_relationships ADD COLUMN valid_at TEXT", []);
let _ = conn.execute("ALTER TABLE kg_relationships ADD COLUMN invalidated_at TEXT", []);
let _ = conn.execute("ALTER TABLE kg_relationships ADD COLUMN epistemic_class TEXT DEFAULT 'current'", []);
let _ = conn.execute("ALTER TABLE kg_relationships ADD COLUMN source_episode_ids TEXT", []);
let _ = conn.execute("ALTER TABLE kg_relationships ADD COLUMN confidence REAL DEFAULT 0.8", []);
conn.execute_batch(
    "CREATE INDEX IF NOT EXISTS idx_rels_valid_at ON kg_relationships(valid_at);"
)?;
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check --package knowledge-graph`

- [ ] **Step 5: Commit**

```bash
git add services/knowledge-graph/src/storage.rs
git commit -m "feat(kg): add epistemic_class, provenance, and bitemporal columns to graph tables"
```

---

### Task 3: KgEpisodeRepository

**Files:**
- Create: `gateway/gateway-database/src/kg_episode_repository.rs`
- Modify: `gateway/gateway-database/src/lib.rs`

- [ ] **Step 1: Read existing repository patterns**

Read `gateway/gateway-database/src/memory_repository.rs` and `wiki_repository.rs` (lines 1–50 of each). Match:
- Constructor takes `Arc<DatabaseManager>`
- Methods use `self.db.with_connection(|conn| { ... })`
- Rows mapped via a private `row_to_*` helper

- [ ] **Step 2: Create kg_episode_repository.rs**

```rust
//! Repository for kg_episodes — records every extraction event for
//! provenance tracking. Facts, entities, and relationships reference
//! an episode ID so we can always answer "where did this come from?"

use crate::connection::DatabaseManager;
use rusqlite::params;
use std::sync::Arc;

/// The source system that produced an episode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EpisodeSource {
    ToolResult,
    WardFile,
    Session,
    Distillation,
    UserInput,
}

impl EpisodeSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ToolResult => "tool_result",
            Self::WardFile => "ward_file",
            Self::Session => "session",
            Self::Distillation => "distillation",
            Self::UserInput => "user_input",
        }
    }
}

/// A provenance record: one extraction event from one source.
#[derive(Debug, Clone)]
pub struct KgEpisode {
    pub id: String,
    pub source_type: String,
    pub source_ref: String,
    pub content_hash: String,
    pub session_id: Option<String>,
    pub agent_id: String,
    pub created_at: String,
}

pub struct KgEpisodeRepository {
    db: Arc<DatabaseManager>,
}

impl KgEpisodeRepository {
    pub fn new(db: Arc<DatabaseManager>) -> Self {
        Self { db }
    }

    /// Insert an episode. Returns Ok(true) if inserted, Ok(false) if a duplicate
    /// (same content_hash + source_type) already exists.
    pub fn upsert_episode(&self, ep: &KgEpisode) -> Result<bool, String> {
        self.db.with_connection(|conn| {
            let changed = conn.execute(
                "INSERT OR IGNORE INTO kg_episodes \
                 (id, source_type, source_ref, content_hash, session_id, agent_id, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    ep.id,
                    ep.source_type,
                    ep.source_ref,
                    ep.content_hash,
                    ep.session_id,
                    ep.agent_id,
                    ep.created_at,
                ],
            )?;
            Ok(changed > 0)
        })
    }

    /// Look up an episode by content_hash + source_type. Used for dedup
    /// before extraction: if content hasn't changed, skip re-extraction.
    pub fn get_by_content_hash(
        &self,
        content_hash: &str,
        source_type: &str,
    ) -> Result<Option<KgEpisode>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, source_type, source_ref, content_hash, session_id, agent_id, created_at \
                 FROM kg_episodes WHERE content_hash = ?1 AND source_type = ?2",
            )?;
            let result = stmt
                .query_row(params![content_hash, source_type], Self::row_to_episode)
                .optional()?;
            Ok(result)
        })
    }

    /// Get all episodes for a session.
    pub fn list_by_session(&self, session_id: &str) -> Result<Vec<KgEpisode>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, source_type, source_ref, content_hash, session_id, agent_id, created_at \
                 FROM kg_episodes WHERE session_id = ?1 ORDER BY created_at",
            )?;
            let rows = stmt
                .query_map(params![session_id], Self::row_to_episode)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
    }

    /// Get a single episode by ID.
    pub fn get(&self, id: &str) -> Result<Option<KgEpisode>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, source_type, source_ref, content_hash, session_id, agent_id, created_at \
                 FROM kg_episodes WHERE id = ?1",
            )?;
            let result = stmt
                .query_row(params![id], Self::row_to_episode)
                .optional()?;
            Ok(result)
        })
    }

    fn row_to_episode(row: &rusqlite::Row) -> rusqlite::Result<KgEpisode> {
        Ok(KgEpisode {
            id: row.get(0)?,
            source_type: row.get(1)?,
            source_ref: row.get(2)?,
            content_hash: row.get(3)?,
            session_id: row.get(4)?,
            agent_id: row.get(5)?,
            created_at: row.get(6)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gateway_services::VaultPaths;
    use tempfile::TempDir;

    fn setup_test_db() -> Arc<DatabaseManager> {
        let temp_dir = TempDir::new().unwrap();
        let paths = Arc::new(VaultPaths::new(temp_dir.path().to_path_buf()));
        let _ = temp_dir.keep();
        Arc::new(DatabaseManager::new(paths).unwrap())
    }

    fn sample_episode() -> KgEpisode {
        KgEpisode {
            id: "ep-1".into(),
            source_type: "ward_file".into(),
            source_ref: "timeline.json".into(),
            content_hash: "abc123".into(),
            session_id: Some("sess-1".into()),
            agent_id: "root".into(),
            created_at: "2026-04-12T00:00:00Z".into(),
        }
    }

    #[test]
    fn upsert_and_get_by_id() {
        let db = setup_test_db();
        let repo = KgEpisodeRepository::new(db);
        let ep = sample_episode();
        let inserted = repo.upsert_episode(&ep).unwrap();
        assert!(inserted);
        let fetched = repo.get("ep-1").unwrap().unwrap();
        assert_eq!(fetched.source_type, "ward_file");
        assert_eq!(fetched.source_ref, "timeline.json");
    }

    #[test]
    fn duplicate_content_hash_returns_false() {
        let db = setup_test_db();
        let repo = KgEpisodeRepository::new(db);
        let ep = sample_episode();
        assert!(repo.upsert_episode(&ep).unwrap());
        // Different ID, same content_hash + source_type → conflict → no insert
        let ep2 = KgEpisode { id: "ep-2".into(), ..ep };
        assert!(!repo.upsert_episode(&ep2).unwrap());
    }

    #[test]
    fn get_by_content_hash() {
        let db = setup_test_db();
        let repo = KgEpisodeRepository::new(db);
        let ep = sample_episode();
        repo.upsert_episode(&ep).unwrap();
        let found = repo
            .get_by_content_hash("abc123", "ward_file")
            .unwrap()
            .unwrap();
        assert_eq!(found.id, "ep-1");
        // Wrong source_type → no match
        assert!(repo.get_by_content_hash("abc123", "tool_result").unwrap().is_none());
    }

    #[test]
    fn list_by_session_returns_in_order() {
        let db = setup_test_db();
        let repo = KgEpisodeRepository::new(db);
        for i in 0..3 {
            let ep = KgEpisode {
                id: format!("ep-{i}"),
                content_hash: format!("hash-{i}"),
                created_at: format!("2026-04-12T00:00:0{i}Z"),
                ..sample_episode()
            };
            repo.upsert_episode(&ep).unwrap();
        }
        let eps = repo.list_by_session("sess-1").unwrap();
        assert_eq!(eps.len(), 3);
        assert_eq!(eps[0].id, "ep-0");
        assert_eq!(eps[2].id, "ep-2");
    }

    #[test]
    fn get_missing_returns_none() {
        let db = setup_test_db();
        let repo = KgEpisodeRepository::new(db);
        assert!(repo.get("nonexistent").unwrap().is_none());
    }

    #[test]
    fn episode_source_as_str_roundtrip() {
        assert_eq!(EpisodeSource::ToolResult.as_str(), "tool_result");
        assert_eq!(EpisodeSource::WardFile.as_str(), "ward_file");
        assert_eq!(EpisodeSource::Session.as_str(), "session");
        assert_eq!(EpisodeSource::Distillation.as_str(), "distillation");
        assert_eq!(EpisodeSource::UserInput.as_str(), "user_input");
    }
}
```

- [ ] **Step 3: Export from lib.rs**

In `gateway/gateway-database/src/lib.rs`, add:

```rust
pub mod kg_episode_repository;
pub use kg_episode_repository::{EpisodeSource, KgEpisode, KgEpisodeRepository};
```

- [ ] **Step 4: Run tests**

Run: `cargo test --package gateway-database -- kg_episode_repository`
Expected: 6 tests pass.

- [ ] **Step 5: Quality checks**

Run: `cargo fmt --all && cargo clippy --package gateway-database -- -D warnings`
Expected: Clean.

- [ ] **Step 6: Commit**

```bash
git add gateway/gateway-database/src/kg_episode_repository.rs gateway/gateway-database/src/lib.rs
git commit -m "feat(db): KgEpisodeRepository with content-hash dedup and 6 tests"
```

---

### Task 4: Ward Artifact Indexer — Core Logic

**Files:**
- Create: `gateway/gateway-execution/src/ward_artifact_indexer.rs`
- Modify: `gateway/gateway-execution/src/lib.rs`

This is the high-value module. It scans a ward for structured JSON/CSV files and extracts entities from collections.

**Design guardrails for cognitive complexity ≤ 15**:
- Main entry point `index_ward()` delegates to small helpers
- File-type dispatch via `detect_artifact_kind()` (early return)
- Schema detection via `detect_collection_schema()` (returns enum)
- Entity extraction per kind is a separate function

- [ ] **Step 1: Create ward_artifact_indexer.rs**

```rust
//! Ward Artifact Indexer — scans a ward for structured files (JSON/CSV)
//! after a session completes, parses collection-of-objects schemas, and
//! emits entities + relationships tagged `epistemic_class = archival`
//! with `source_ref` pointing to the originating file.
//!
//! Zero LLM cost. Domain content that previously lived only in ward files
//! (timeline.json, people.json, etc.) now reaches the knowledge graph.

use gateway_database::{EpisodeSource, KgEpisode, KgEpisodeRepository};
use knowledge_graph::{Entity, EntityType, ExtractedKnowledge, GraphStorage};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Entry point: index every structured file in the ward directory.
///
/// Returns the number of entities created. Errors in individual files are
/// logged as warnings — indexing is best-effort, never crashes the pipeline.
pub async fn index_ward(
    ward_path: &Path,
    session_id: &str,
    agent_id: &str,
    episode_repo: &KgEpisodeRepository,
    graph: &Arc<GraphStorage>,
) -> usize {
    let mut created = 0_usize;
    let files = collect_structured_files(ward_path);

    for file_path in files {
        match index_one_file(&file_path, session_id, agent_id, episode_repo, graph).await {
            Ok(n) => created += n,
            Err(e) => tracing::warn!(
                file = ?file_path,
                error = %e,
                "Failed to index ward artifact"
            ),
        }
    }

    tracing::info!(
        ward = ?ward_path,
        entities = created,
        "Ward artifact indexing complete"
    );
    created
}

/// Collect all structured files (.json, .csv, .yaml) in the ward recursively,
/// skipping common noise directories.
fn collect_structured_files(ward_path: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let _ = walk(ward_path, &mut files);
    files
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if should_skip_dir(&path) {
                continue;
            }
            walk(&path, out)?;
        } else if is_structured_file(&path) {
            out.push(path);
        }
    }
    Ok(())
}

fn should_skip_dir(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|n| n.to_str()),
        Some("node_modules") | Some(".git") | Some("__pycache__") | Some(".venv") | Some("tmp")
    )
}

fn is_structured_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("json") | Some("JSON")
    )
    // YAML and CSV supported in a later task; JSON is the common case.
}

/// Index a single file. Returns the number of entities created.
async fn index_one_file(
    file_path: &Path,
    session_id: &str,
    agent_id: &str,
    episode_repo: &KgEpisodeRepository,
    graph: &Arc<GraphStorage>,
) -> Result<usize, String> {
    let content = std::fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read {:?}: {e}", file_path))?;

    let content_hash = compute_hash(&content);

    // Dedup: skip if we've already indexed this exact content
    if episode_repo
        .get_by_content_hash(&content_hash, EpisodeSource::WardFile.as_str())
        .map_err(|e| format!("Dedup check failed: {e}"))?
        .is_some()
    {
        tracing::debug!(file = ?file_path, "Skipping already-indexed ward file");
        return Ok(0);
    }

    // Parse JSON (other formats in later tasks)
    let value: Value = serde_json::from_str(&content)
        .map_err(|e| format!("JSON parse failed for {:?}: {e}", file_path))?;

    // Create the episode record
    let source_ref = file_path.to_string_lossy().to_string();
    let episode = KgEpisode {
        id: format!("ep-{}", uuid::Uuid::new_v4()),
        source_type: EpisodeSource::WardFile.as_str().to_string(),
        source_ref: source_ref.clone(),
        content_hash,
        session_id: Some(session_id.to_string()),
        agent_id: agent_id.to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    episode_repo
        .upsert_episode(&episode)
        .map_err(|e| format!("Episode insert failed: {e}"))?;

    // Extract entities based on the detected schema
    let schema = detect_collection_schema(&value);
    let entities = extract_entities(&value, schema, agent_id, &episode.id, &source_ref);
    let count = entities.len();

    if count > 0 {
        let knowledge = ExtractedKnowledge {
            entities,
            relationships: vec![], // relationships in a follow-up task
        };
        graph
            .store_knowledge(agent_id, knowledge)
            .await
            .map_err(|e| format!("Graph store failed: {e}"))?;
    }

    Ok(count)
}

/// Hash file content for dedup.
fn compute_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// What kind of collection is this JSON?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CollectionSchema {
    /// `[{name: "...", ...}, ...]` — person or organization list
    NamedObjectArray,
    /// `[{date: "...", ...}, ...]` or `[{year: ..., ...}, ...]` — timeline
    DatedObjectArray,
    /// `{key1: {...}, key2: {...}}` — map of named objects
    NamedObjectMap,
    /// Not a known collection schema — skip
    Unknown,
}

/// Heuristically detect the collection schema.
fn detect_collection_schema(value: &Value) -> CollectionSchema {
    match value {
        Value::Array(items) => detect_array_schema(items),
        Value::Object(obj) => {
            if obj.values().all(|v| v.is_object()) && obj.len() >= 2 {
                CollectionSchema::NamedObjectMap
            } else {
                CollectionSchema::Unknown
            }
        }
        _ => CollectionSchema::Unknown,
    }
}

fn detect_array_schema(items: &[Value]) -> CollectionSchema {
    if items.is_empty() {
        return CollectionSchema::Unknown;
    }
    let sample = items.iter().take(5).collect::<Vec<_>>();
    let all_objects = sample.iter().all(|v| v.is_object());
    if !all_objects {
        return CollectionSchema::Unknown;
    }

    let has_date_field = sample.iter().any(|v| {
        v.as_object()
            .map(|o| o.keys().any(|k| is_date_key(k)))
            .unwrap_or(false)
    });
    let has_name_field = sample.iter().any(|v| {
        v.as_object()
            .map(|o| o.keys().any(|k| is_name_key(k)))
            .unwrap_or(false)
    });

    if has_date_field {
        CollectionSchema::DatedObjectArray
    } else if has_name_field {
        CollectionSchema::NamedObjectArray
    } else {
        CollectionSchema::Unknown
    }
}

fn is_date_key(key: &str) -> bool {
    matches!(
        key.to_lowercase().as_str(),
        "date" | "year" | "start_date" | "when" | "timestamp"
    )
}

fn is_name_key(key: &str) -> bool {
    matches!(
        key.to_lowercase().as_str(),
        "name" | "title" | "label"
    )
}

/// Extract entities from a parsed JSON value given the detected schema.
fn extract_entities(
    value: &Value,
    schema: CollectionSchema,
    agent_id: &str,
    episode_id: &str,
    source_ref: &str,
) -> Vec<Entity> {
    match schema {
        CollectionSchema::NamedObjectArray => extract_named_array(value, agent_id, episode_id, source_ref),
        CollectionSchema::DatedObjectArray => extract_dated_array(value, agent_id, episode_id, source_ref),
        CollectionSchema::NamedObjectMap => extract_named_map(value, agent_id, episode_id, source_ref),
        CollectionSchema::Unknown => Vec::new(),
    }
}

fn extract_named_array(
    value: &Value,
    agent_id: &str,
    episode_id: &str,
    source_ref: &str,
) -> Vec<Entity> {
    let Some(arr) = value.as_array() else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|item| item.as_object())
        .filter_map(|obj| {
            let name = obj
                .get("name")
                .or(obj.get("title"))
                .or(obj.get("label"))
                .and_then(|v| v.as_str())?;
            Some(build_entity(
                name,
                guess_type_from_source_ref(source_ref),
                obj,
                agent_id,
                episode_id,
                source_ref,
            ))
        })
        .collect()
}

fn extract_dated_array(
    value: &Value,
    agent_id: &str,
    episode_id: &str,
    source_ref: &str,
) -> Vec<Entity> {
    let Some(arr) = value.as_array() else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|item| item.as_object())
        .filter_map(|obj| {
            // A dated entry without a name becomes an Event with a synthesized name
            let name = derive_event_name(obj)?;
            Some(build_entity(
                &name,
                EntityType::from_str("event"),
                obj,
                agent_id,
                episode_id,
                source_ref,
            ))
        })
        .collect()
}

fn extract_named_map(
    value: &Value,
    agent_id: &str,
    episode_id: &str,
    source_ref: &str,
) -> Vec<Entity> {
    let Some(obj) = value.as_object() else {
        return Vec::new();
    };
    obj.iter()
        .filter_map(|(key, val)| {
            let props = val.as_object()?;
            Some(build_entity(
                key,
                guess_type_from_source_ref(source_ref),
                props,
                agent_id,
                episode_id,
                source_ref,
            ))
        })
        .collect()
}

fn derive_event_name(obj: &serde_json::Map<String, Value>) -> Option<String> {
    // Prefer an explicit title/name; otherwise build "YYYY: brief"
    if let Some(name) = obj
        .get("name")
        .or(obj.get("title"))
        .and_then(|v| v.as_str())
    {
        return Some(name.to_string());
    }
    let year = obj
        .get("year")
        .or(obj.get("date"))
        .and_then(|v| v.as_str().or_else(|| v.as_i64().map(|_| "unknown").or(Some(""))))
        .unwrap_or("");
    let brief = obj
        .get("description")
        .or(obj.get("event"))
        .or(obj.get("summary"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .chars()
        .take(40)
        .collect::<String>();
    if brief.is_empty() && year.is_empty() {
        return None;
    }
    Some(format!("{}: {}", year, brief).trim().to_string())
}

fn guess_type_from_source_ref(source_ref: &str) -> EntityType {
    let lower = source_ref.to_lowercase();
    if lower.contains("people") || lower.contains("person") {
        EntityType::Person
    } else if lower.contains("org") || lower.contains("company") {
        EntityType::Organization
    } else if lower.contains("place") || lower.contains("location") {
        // EntityType::Place is added in Phase 6b; fall back to Concept for now
        EntityType::Concept
    } else if lower.contains("timeline") || lower.contains("event") {
        // EntityType::Event is added in Phase 6b
        EntityType::Concept
    } else {
        EntityType::Concept
    }
}

fn build_entity(
    name: &str,
    entity_type: EntityType,
    properties: &serde_json::Map<String, Value>,
    agent_id: &str,
    episode_id: &str,
    source_ref: &str,
) -> Entity {
    let mut entity = Entity::new(agent_id.to_string(), entity_type, name.to_string());
    // Store original properties
    for (k, v) in properties {
        entity.properties.insert(k.clone(), v.clone());
    }
    // Stamp provenance
    entity
        .properties
        .insert("_source_episode_id".to_string(), Value::String(episode_id.to_string()));
    entity
        .properties
        .insert("_source_ref".to_string(), Value::String(source_ref.to_string()));
    entity
        .properties
        .insert("_epistemic_class".to_string(), Value::String("archival".to_string()));
    entity
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_named_object_array() {
        let v: Value = serde_json::from_str(r#"[{"name": "A"}, {"name": "B"}]"#).unwrap();
        assert_eq!(detect_collection_schema(&v), CollectionSchema::NamedObjectArray);
    }

    #[test]
    fn detect_dated_object_array() {
        let v: Value = serde_json::from_str(r#"[{"date": "1937", "event": "x"}]"#).unwrap();
        assert_eq!(detect_collection_schema(&v), CollectionSchema::DatedObjectArray);
    }

    #[test]
    fn detect_named_object_map() {
        let v: Value = serde_json::from_str(r#"{"A": {"x": 1}, "B": {"y": 2}}"#).unwrap();
        assert_eq!(detect_collection_schema(&v), CollectionSchema::NamedObjectMap);
    }

    #[test]
    fn detect_unknown_primitive() {
        let v: Value = serde_json::from_str(r#""just a string""#).unwrap();
        assert_eq!(detect_collection_schema(&v), CollectionSchema::Unknown);
    }

    #[test]
    fn detect_unknown_array_of_primitives() {
        let v: Value = serde_json::from_str(r#"[1, 2, 3]"#).unwrap();
        assert_eq!(detect_collection_schema(&v), CollectionSchema::Unknown);
    }

    #[test]
    fn extract_named_array_produces_entities() {
        let v: Value = serde_json::from_str(
            r#"[{"name": "Alice", "role": "founder"}, {"name": "Bob", "role": "CEO"}]"#,
        )
        .unwrap();
        let entities = extract_entities(&v, CollectionSchema::NamedObjectArray, "root", "ep-1", "people.json");
        assert_eq!(entities.len(), 2);
        assert_eq!(entities[0].name, "Alice");
        assert_eq!(entities[1].name, "Bob");
        assert!(entities[0].properties.contains_key("role"));
        assert!(entities[0].properties.contains_key("_source_ref"));
        assert_eq!(
            entities[0].properties.get("_epistemic_class").unwrap().as_str(),
            Some("archival")
        );
    }

    #[test]
    fn extract_dated_array_produces_events() {
        let v: Value = serde_json::from_str(
            r#"[{"date": "1937", "event": "Ahmedabad Session"}]"#,
        )
        .unwrap();
        let entities = extract_entities(&v, CollectionSchema::DatedObjectArray, "root", "ep-1", "timeline.json");
        assert_eq!(entities.len(), 1);
    }

    #[test]
    fn guess_type_from_people_filename() {
        assert!(matches!(
            guess_type_from_source_ref("/ward/foo/people.json"),
            EntityType::Person
        ));
    }

    #[test]
    fn is_date_key_recognizes_common_names() {
        assert!(is_date_key("date"));
        assert!(is_date_key("Year"));
        assert!(is_date_key("start_date"));
        assert!(!is_date_key("description"));
    }

    #[test]
    fn compute_hash_is_deterministic() {
        let h1 = compute_hash("hello world");
        let h2 = compute_hash("hello world");
        let h3 = compute_hash("different");
        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
    }
}
```

- [ ] **Step 2: Add `sha2` dependency if not present**

Check `gateway/gateway-execution/Cargo.toml`. If `sha2` is not there, add:

```toml
sha2 = { workspace = true }
```

And ensure it's in workspace `Cargo.toml` dependencies. If missing, add `sha2 = "0.10"`.

- [ ] **Step 3: Export the module**

In `gateway/gateway-execution/src/lib.rs`, add:

```rust
pub mod ward_artifact_indexer;
```

- [ ] **Step 4: Run tests**

Run: `cargo test --package gateway-execution -- ward_artifact_indexer`
Expected: 10 tests pass.

- [ ] **Step 5: Quality checks**

Run: `cargo fmt --all && cargo clippy --package gateway-execution -- -D warnings`
Expected: Clean. No function should exceed cognitive complexity 15.

- [ ] **Step 6: Commit**

```bash
git add gateway/gateway-execution/src/ward_artifact_indexer.rs gateway/gateway-execution/src/lib.rs gateway/gateway-execution/Cargo.toml Cargo.toml
git commit -m "feat(kg): WardArtifactIndexer — scan ward, parse JSON collections, emit archival entities with episode provenance"
```

---

### Task 5: Wire Indexer Into Execution Pipeline

**Files:**
- Modify: `gateway/src/state.rs`
- Modify: `gateway/gateway-execution/src/runner.rs`

- [ ] **Step 1: Wire KgEpisodeRepository in state.rs**

In `gateway/src/state.rs`, near where other repositories are constructed:

```rust
use gateway_database::KgEpisodeRepository;

let kg_episode_repo = Arc::new(KgEpisodeRepository::new(db_manager.clone()));
```

Pass this to the runner or make it available via AppState. Follow the same pattern used for `MemoryRepository` and `WardWikiRepository`.

- [ ] **Step 2: Add field to ExecutionRunner**

In `gateway/gateway-execution/src/runner.rs`, add:

```rust
kg_episode_repo: Option<Arc<KgEpisodeRepository>>,
```

Plus a setter `set_kg_episode_repo()`.

- [ ] **Step 3: Call indexer after distillation**

Find the distillation spawn block in `spawn_execution_task` (or wherever distillation is fired on session completion). After distillation returns, add:

```rust
// Index ward artifacts (new in Phase 6a). Runs after distillation so
// facts/entities from the LLM extraction are already persisted when
// the artifact indexer adds structured entities on top.
if let (Some(ref ward_id), Some(ref episode_repo), Some(ref graph)) =
    (&session.ward_id, &kg_episode_repo_clone, &graph_storage_clone)
{
    let ward_path = paths.vault_dir().join("wards").join(ward_id);
    let n = ward_artifact_indexer::index_ward(
        &ward_path,
        &session_id_clone,
        &agent_id_clone,
        episode_repo,
        graph,
    ).await;
    tracing::info!(ward = %ward_id, indexed_entities = n, "Ward artifact indexing complete");
}
```

Clone the repos into the spawn closure before the tokio::spawn block.

- [ ] **Step 4: Verify compilation**

Run: `cargo check --workspace`
Expected: Clean.

- [ ] **Step 5: Quality checks**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`
Expected: Clean.

- [ ] **Step 6: Commit**

```bash
git add gateway/src/state.rs gateway/gateway-execution/src/runner.rs
git commit -m "feat(kg): wire WardArtifactIndexer into post-distillation pipeline"
```

---

### Task 6: Final Checks + Push

- [ ] **Step 1: Run all tests**

Run: `cargo test --workspace --lib --bins --tests`
Expected: All pass. No regressions.

- [ ] **Step 2: Final fmt + clippy**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`
Expected: Clean.

- [ ] **Step 3: UI checks**

Run: `cd apps/ui && npm run build && npm run lint`
Expected: Clean (no UI changes in Phase 6a, but verify no regressions).

- [ ] **Step 4: Cognitive complexity audit**

Run: `cargo clippy --package gateway-execution --package gateway-database --package knowledge-graph -- -W clippy::cognitive_complexity`

Clippy has its own cognitive_complexity lint (threshold default 25). SonarQube's threshold is 15 — stricter. Review any functions flagged and refactor by extracting helpers.

For Phase 6a specifically, audit:
- `index_ward()` — should be ≤ 10 (delegates heavily)
- `index_one_file()` — should be ≤ 12 (one main path, dedup early return)
- `detect_collection_schema()` — ≤ 8 (match + single helper)
- `extract_entities()` — ≤ 5 (trivial dispatch)

- [ ] **Step 5: Push**

```bash
git push
```
