//! KgBackfiller — one-shot backfill for pre-existing graph metadata.
//!
//! Commits b816702 (relationship evidence), 1bc21f6 (entity summary), and
//! 5bf3013 (orphan archiver) extended the knowledge-graph schema so that
//! newly-extracted rows carry richer metadata. Entities and relationships
//! written BEFORE those commits landed (a typical warm vault has ~66
//! entities + ~97 edges) never benefit from the new fields unless we
//! rewrite their `properties` JSON.
//!
//! This module runs ONCE at daemon startup:
//!   - Entities lacking a `summary` property get one synthesized from
//!     `description` (when present) or from `"{entity_type} {name}
//!     (backfilled)"`. A `summary_backfilled: true` flag marks the row.
//!   - Relationships with empty `properties` get `extracted_by:
//!     "backfill-legacy"` + `backfilled: true`, plus `source_episode_id`
//!     (first id from `source_episode_ids`) if available. The original
//!     `evidence` text is unrecoverable and left absent; the marker makes
//!     legacy rows filterable.
//!
//! Idempotency via a marker row in `kg_compactions` with
//! `operation = 'backfill'` and `reason = 'kg-metadata-backfill-v1'`.
//! Subsequent runs short-circuit and return `already_done: true`.
//!
//! Updates are batched 500 per transaction; a single malformed-JSON row
//! logs a warning and is skipped rather than aborting the whole pass.
//! Daemon startup treats any error here as non-fatal.

use std::sync::Arc;

use zero_stores_sqlite::KnowledgeDatabase;
use rusqlite::params;
use serde_json::{Map, Value};

/// Marker reason recorded in `kg_compactions.reason` on completion.
const BACKFILL_REASON: &str = "kg-metadata-backfill-v1";

/// `extracted_by` tag written into legacy relationship properties.
const LEGACY_EXTRACTOR_TAG: &str = "backfill-legacy";

/// Maximum length (in chars) for a synthesized summary — matches the
/// limit enforced in `LlmExtractor::parse_entities_response`.
const SUMMARY_MAX_LEN: usize = 200;

/// Rows touched per transaction. Bounds memory and redo-log size.
const BATCH_SIZE: usize = 500;

/// Aggregate counts from one backfill pass.
#[derive(Debug, Default, Clone)]
pub struct KgBackfillStats {
    pub entities_scanned: usize,
    pub entities_updated: usize,
    pub relationships_scanned: usize,
    pub relationships_updated: usize,
    /// `true` when the idempotency marker was already present and the
    /// run did no work.
    pub already_done: bool,
}

/// Backfill driver. Construct with [`KgBackfiller::new`] and invoke
/// [`KgBackfiller::run_once`] at daemon startup.
pub struct KgBackfiller {
    db: Arc<KnowledgeDatabase>,
}

impl KgBackfiller {
    pub fn new(db: Arc<KnowledgeDatabase>) -> Self {
        Self { db }
    }

    /// Run the backfill at most once. Idempotent: second invocation
    /// returns `already_done: true` and performs no updates.
    pub async fn run_once(&self) -> Result<KgBackfillStats, String> {
        self.run_once_blocking()
    }

    /// Synchronous flavor of [`Self::run_once`] for callers that aren't
    /// inside an async context (e.g. `AppState::new`). All the underlying
    /// DB work is blocking anyway; this just exposes it directly.
    pub fn run_once_blocking(&self) -> Result<KgBackfillStats, String> {
        if self.marker_exists()? {
            return Ok(KgBackfillStats {
                already_done: true,
                ..Default::default()
            });
        }

        let mut stats = KgBackfillStats::default();
        self.backfill_entities(&mut stats)?;
        self.backfill_relationships(&mut stats)?;
        self.write_marker()?;
        Ok(stats)
    }

    fn marker_exists(&self) -> Result<bool, String> {
        self.db.with_connection(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM kg_compactions
                 WHERE operation = 'backfill' AND reason = ?1",
                params![BACKFILL_REASON],
                |row| row.get(0),
            )?;
            Ok(count > 0)
        })
    }

    fn write_marker(&self) -> Result<(), String> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        self.db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO kg_compactions
                    (id, run_id, operation, reason, created_at)
                 VALUES (?1, ?2, 'backfill', ?3, ?4)",
                params![id, BACKFILL_REASON, BACKFILL_REASON, now],
            )?;
            Ok(())
        })
    }

    // ---- Entities ---------------------------------------------------------

    fn backfill_entities(&self, stats: &mut KgBackfillStats) -> Result<(), String> {
        let rows = self.load_entity_candidates()?;
        stats.entities_scanned = rows.len();

        for chunk in rows.chunks(BATCH_SIZE) {
            let updated = self.apply_entity_batch(chunk)?;
            stats.entities_updated += updated;
        }
        Ok(())
    }

    /// Select rows whose `properties.summary` is missing or empty.
    fn load_entity_candidates(&self) -> Result<Vec<EntityRow>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, entity_type, name, properties
                 FROM kg_entities
                 WHERE json_extract(properties, '$.summary') IS NULL
                    OR json_extract(properties, '$.summary') = ''",
            )?;
            let rows = stmt
                .query_map([], |row| {
                    Ok(EntityRow {
                        id: row.get(0)?,
                        entity_type: row.get(1)?,
                        name: row.get(2)?,
                        properties: row.get(3)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
    }

    fn apply_entity_batch(&self, rows: &[EntityRow]) -> Result<usize, String> {
        let rows_vec = rows.to_vec();
        self.db.with_connection(move |conn| {
            let tx = conn.unchecked_transaction()?;
            let mut updated = 0usize;
            for row in &rows_vec {
                match build_entity_properties(row) {
                    Ok(new_props) => {
                        tx.execute(
                            "UPDATE kg_entities SET properties = ?1 WHERE id = ?2",
                            params![new_props, row.id],
                        )?;
                        updated += 1;
                    }
                    Err(e) => {
                        tracing::warn!(
                            entity = %row.id,
                            error = %e,
                            "kg_backfill: skipping entity with malformed properties",
                        );
                    }
                }
            }
            tx.commit()?;
            Ok(updated)
        })
    }

    // ---- Relationships ----------------------------------------------------

    fn backfill_relationships(&self, stats: &mut KgBackfillStats) -> Result<(), String> {
        let rows = self.load_relationship_candidates()?;
        stats.relationships_scanned = rows.len();

        for chunk in rows.chunks(BATCH_SIZE) {
            let updated = self.apply_relationship_batch(chunk)?;
            stats.relationships_updated += updated;
        }
        Ok(())
    }

    /// Select relationships with NULL / empty / `{}` properties.
    fn load_relationship_candidates(&self) -> Result<Vec<RelationshipRow>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, properties, source_episode_ids
                 FROM kg_relationships
                 WHERE properties IS NULL
                    OR properties = ''
                    OR properties = '{}'",
            )?;
            let rows = stmt
                .query_map([], |row| {
                    Ok(RelationshipRow {
                        id: row.get(0)?,
                        properties: row.get(1)?,
                        source_episode_ids: row.get(2)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
    }

    fn apply_relationship_batch(&self, rows: &[RelationshipRow]) -> Result<usize, String> {
        let rows_vec = rows.to_vec();
        self.db.with_connection(move |conn| {
            let tx = conn.unchecked_transaction()?;
            let mut updated = 0usize;
            for row in &rows_vec {
                match build_relationship_properties(row) {
                    Ok(new_props) => {
                        tx.execute(
                            "UPDATE kg_relationships SET properties = ?1 WHERE id = ?2",
                            params![new_props, row.id],
                        )?;
                        updated += 1;
                    }
                    Err(e) => {
                        tracing::warn!(
                            relationship = %row.id,
                            error = %e,
                            "kg_backfill: skipping relationship with malformed properties",
                        );
                    }
                }
            }
            tx.commit()?;
            Ok(updated)
        })
    }
}

// ============================================================================
// Row DTOs + pure helpers (testable without a DB)
// ============================================================================

#[derive(Debug, Clone)]
struct EntityRow {
    id: String,
    entity_type: String,
    name: String,
    properties: Option<String>,
}

#[derive(Debug, Clone)]
struct RelationshipRow {
    id: String,
    properties: Option<String>,
    source_episode_ids: Option<String>,
}

/// Construct the new `properties` JSON for an entity row. Preserves any
/// existing keys; adds `summary` + `summary_backfilled: true`.
fn build_entity_properties(row: &EntityRow) -> Result<String, String> {
    let mut map = parse_object(row.properties.as_deref())?;

    let summary = match map.get("description").and_then(|v| v.as_str()) {
        Some(desc) if !desc.trim().is_empty() => truncate_chars(desc.trim(), SUMMARY_MAX_LEN),
        _ => format!("{} {} (backfilled)", row.entity_type, row.name),
    };

    map.insert("summary".to_string(), Value::String(summary));
    map.insert("summary_backfilled".to_string(), Value::Bool(true));

    serde_json::to_string(&Value::Object(map)).map_err(|e| format!("serialize properties: {e}"))
}

/// Construct the new `properties` JSON for a legacy relationship row.
fn build_relationship_properties(row: &RelationshipRow) -> Result<String, String> {
    let mut map = parse_object(row.properties.as_deref())?;
    map.insert(
        "extracted_by".to_string(),
        Value::String(LEGACY_EXTRACTOR_TAG.to_string()),
    );
    map.insert("backfilled".to_string(), Value::Bool(true));

    if let Some(first) = first_episode_id(row.source_episode_ids.as_deref()) {
        map.insert("source_episode_id".to_string(), Value::String(first));
    }

    serde_json::to_string(&Value::Object(map)).map_err(|e| format!("serialize properties: {e}"))
}

/// Parse a JSON object stored as text. NULL / empty / `{}` become an empty
/// map. Non-object JSON (array, string, number) is rejected.
fn parse_object(raw: Option<&str>) -> Result<Map<String, Value>, String> {
    let trimmed = raw.map(str::trim).unwrap_or("");
    if trimmed.is_empty() || trimmed == "{}" {
        return Ok(Map::new());
    }
    let value: Value = serde_json::from_str(trimmed).map_err(|e| format!("invalid json: {e}"))?;
    match value {
        Value::Object(m) => Ok(m),
        other => Err(format!("expected JSON object, got {other:?}")),
    }
}

/// Extract the first non-empty id from a `source_episode_ids` column.
/// Tolerates JSON arrays (`["a","b"]`) and plain comma-separated strings.
fn first_episode_id(raw: Option<&str>) -> Option<String> {
    let trimmed = raw.map(str::trim).unwrap_or("");
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(Value::Array(arr)) = serde_json::from_str::<Value>(trimmed) {
        for v in arr {
            if let Value::String(s) = v {
                let s = s.trim();
                if !s.is_empty() {
                    return Some(s.to_string());
                }
            }
        }
        return None;
    }
    trimmed
        .split(',')
        .map(str::trim)
        .find(|s| !s.is_empty())
        .map(str::to_string)
}

/// Unicode-safe truncation at `max` chars (not bytes).
fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use zero_stores_sqlite::KnowledgeDatabase;
    use gateway_services::VaultPaths;
    use tempfile::TempDir;

    struct Harness {
        _tmp: TempDir,
        db: Arc<KnowledgeDatabase>,
    }

    fn setup() -> Harness {
        let tmp = tempfile::tempdir().expect("tempdir");
        let paths = Arc::new(VaultPaths::new(tmp.path().to_path_buf()));
        std::fs::create_dir_all(paths.conversations_db().parent().expect("parent")).expect("mkdir");
        let db = Arc::new(KnowledgeDatabase::new(paths).expect("knowledge db"));
        Harness { _tmp: tmp, db }
    }

    fn insert_entity(db: &KnowledgeDatabase, id: &str, name: &str, properties: Option<&str>) {
        let now = chrono::Utc::now().to_rfc3339();
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO kg_entities
                    (id, agent_id, entity_type, name, normalized_name, normalized_hash,
                     properties, epistemic_class, confidence, mention_count, access_count,
                     first_seen_at, last_seen_at)
                 VALUES (?1, 'agent', 'Concept', ?2, ?2, ?1, ?3,
                         'current', 0.8, 1, 0, ?4, ?4)",
                params![id, name, properties, now],
            )?;
            Ok(())
        })
        .expect("insert entity");
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_relationship(
        db: &KnowledgeDatabase,
        id: &str,
        src: &str,
        tgt: &str,
        properties: Option<&str>,
        source_episode_ids: Option<&str>,
    ) {
        let now = chrono::Utc::now().to_rfc3339();
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO kg_relationships
                    (id, agent_id, source_entity_id, target_entity_id, relationship_type,
                     properties, epistemic_class, confidence, mention_count, access_count,
                     first_seen_at, last_seen_at, source_episode_ids)
                 VALUES (?1, 'agent', ?2, ?3, 'relates_to', ?4,
                         'current', 0.9, 1, 0, ?5, ?5, ?6)",
                params![id, src, tgt, properties, now, source_episode_ids],
            )?;
            Ok(())
        })
        .expect("insert rel");
    }

    fn entity_properties(db: &KnowledgeDatabase, id: &str) -> Option<String> {
        db.with_connection(|conn| {
            conn.query_row(
                "SELECT properties FROM kg_entities WHERE id = ?1",
                params![id],
                |r| r.get::<_, Option<String>>(0),
            )
        })
        .expect("query entity")
    }

    fn relationship_properties(db: &KnowledgeDatabase, id: &str) -> Option<String> {
        db.with_connection(|conn| {
            conn.query_row(
                "SELECT properties FROM kg_relationships WHERE id = ?1",
                params![id],
                |r| r.get::<_, Option<String>>(0),
            )
        })
        .expect("query rel")
    }

    fn parse_props(raw: &str) -> Map<String, Value> {
        match serde_json::from_str::<Value>(raw).expect("valid json") {
            Value::Object(m) => m,
            _ => panic!("expected object"),
        }
    }

    #[tokio::test]
    async fn entity_without_summary_gets_synthesized_backfill() {
        let h = setup();
        insert_entity(&h.db, "e1", "alpha", Some(r#"{"path": "foo"}"#));
        let bf = KgBackfiller::new(h.db.clone());
        let stats = bf.run_once().await.expect("run");
        assert_eq!(stats.entities_updated, 1);

        let props = parse_props(&entity_properties(&h.db, "e1").expect("props"));
        assert_eq!(props.get("path").and_then(Value::as_str), Some("foo"));
        assert_eq!(
            props.get("summary_backfilled").and_then(Value::as_bool),
            Some(true)
        );
        let summary = props
            .get("summary")
            .and_then(Value::as_str)
            .expect("summary");
        assert!(summary.contains("alpha"), "unexpected summary: {summary}");
        assert!(summary.contains("(backfilled)"));
    }

    #[tokio::test]
    async fn entity_with_description_uses_description_for_summary() {
        let h = setup();
        insert_entity(&h.db, "e1", "alpha", Some(r#"{"description": "a thing"}"#));
        let bf = KgBackfiller::new(h.db.clone());
        bf.run_once().await.expect("run");

        let props = parse_props(&entity_properties(&h.db, "e1").expect("props"));
        assert_eq!(
            props.get("summary").and_then(Value::as_str),
            Some("a thing")
        );
        assert_eq!(
            props.get("summary_backfilled").and_then(Value::as_bool),
            Some(true)
        );
    }

    #[tokio::test]
    async fn entity_with_existing_summary_is_skipped() {
        let h = setup();
        insert_entity(&h.db, "e1", "alpha", Some(r#"{"summary": "already"}"#));
        let bf = KgBackfiller::new(h.db.clone());
        let stats = bf.run_once().await.expect("run");
        assert_eq!(stats.entities_scanned, 0);
        assert_eq!(stats.entities_updated, 0);

        let props = parse_props(&entity_properties(&h.db, "e1").expect("props"));
        assert_eq!(
            props.get("summary").and_then(Value::as_str),
            Some("already")
        );
        assert!(props.get("summary_backfilled").is_none());
    }

    #[tokio::test]
    async fn relationship_with_empty_props_gets_backfill_markers() {
        let h = setup();
        insert_entity(&h.db, "e1", "a", Some(r#"{"summary": "s"}"#));
        insert_entity(&h.db, "e2", "b", Some(r#"{"summary": "s"}"#));
        insert_relationship(&h.db, "r1", "e1", "e2", Some("{}"), None);

        let bf = KgBackfiller::new(h.db.clone());
        let stats = bf.run_once().await.expect("run");
        assert_eq!(stats.relationships_updated, 1);

        let props = parse_props(&relationship_properties(&h.db, "r1").expect("props"));
        assert_eq!(
            props.get("extracted_by").and_then(Value::as_str),
            Some(LEGACY_EXTRACTOR_TAG)
        );
        assert_eq!(props.get("backfilled").and_then(Value::as_bool), Some(true));
    }

    #[tokio::test]
    async fn relationship_with_existing_props_is_skipped() {
        let h = setup();
        insert_entity(&h.db, "e1", "a", Some(r#"{"summary": "s"}"#));
        insert_entity(&h.db, "e2", "b", Some(r#"{"summary": "s"}"#));
        insert_relationship(&h.db, "r1", "e1", "e2", Some(r#"{"evidence": "x"}"#), None);

        let bf = KgBackfiller::new(h.db.clone());
        let stats = bf.run_once().await.expect("run");
        assert_eq!(stats.relationships_scanned, 0);
        assert_eq!(stats.relationships_updated, 0);

        let props = parse_props(&relationship_properties(&h.db, "r1").expect("props"));
        assert_eq!(props.get("evidence").and_then(Value::as_str), Some("x"));
        assert!(props.get("backfilled").is_none());
    }

    #[tokio::test]
    async fn relationship_pulls_first_source_episode_id() {
        let h = setup();
        insert_entity(&h.db, "e1", "a", Some(r#"{"summary": "s"}"#));
        insert_entity(&h.db, "e2", "b", Some(r#"{"summary": "s"}"#));
        insert_relationship(
            &h.db,
            "r1",
            "e1",
            "e2",
            Some("{}"),
            Some(r#"["ep-1","ep-2"]"#),
        );

        let bf = KgBackfiller::new(h.db.clone());
        bf.run_once().await.expect("run");

        let props = parse_props(&relationship_properties(&h.db, "r1").expect("props"));
        assert_eq!(
            props.get("source_episode_id").and_then(Value::as_str),
            Some("ep-1")
        );
    }

    #[tokio::test]
    async fn already_done_second_run_is_noop() {
        let h = setup();
        insert_entity(&h.db, "e1", "alpha", Some(r#"{"path": "foo"}"#));
        let bf = KgBackfiller::new(h.db.clone());
        let first = bf.run_once().await.expect("first");
        assert!(!first.already_done);
        assert_eq!(first.entities_updated, 1);

        let second = bf.run_once().await.expect("second");
        assert!(second.already_done);
        assert_eq!(second.entities_scanned, 0);
        assert_eq!(second.entities_updated, 0);

        let marker_count: i64 =
            h.db.with_connection(|conn| {
                conn.query_row(
                    "SELECT COUNT(*) FROM kg_compactions
                     WHERE operation = 'backfill' AND reason = ?1",
                    params![BACKFILL_REASON],
                    |r| r.get(0),
                )
            })
            .expect("count");
        assert_eq!(marker_count, 1, "marker must be written exactly once");
    }

    #[tokio::test]
    async fn stats_counts_are_accurate() {
        let h = setup();
        // 5 entities: 3 need backfill, 2 already have a summary.
        insert_entity(&h.db, "e1", "a", Some(r#"{"path": "1"}"#));
        insert_entity(&h.db, "e2", "b", None);
        insert_entity(&h.db, "e3", "c", Some(r#"{"description": "d"}"#));
        insert_entity(&h.db, "e4", "d", Some(r#"{"summary": "ok"}"#));
        insert_entity(&h.db, "e5", "e", Some(r#"{"summary": "ok2"}"#));
        // 4 rels: 2 need backfill (NULL + "{}"), 2 already have content.
        insert_relationship(&h.db, "r1", "e1", "e2", None, None);
        insert_relationship(&h.db, "r2", "e2", "e3", Some("{}"), None);
        insert_relationship(&h.db, "r3", "e3", "e4", Some(r#"{"evidence":"x"}"#), None);
        insert_relationship(
            &h.db,
            "r4",
            "e4",
            "e5",
            Some(r#"{"extracted_by":"llm"}"#),
            None,
        );

        let bf = KgBackfiller::new(h.db.clone());
        let stats = bf.run_once().await.expect("run");
        assert_eq!(stats.entities_scanned, 3, "stats={stats:?}");
        assert_eq!(stats.entities_updated, 3);
        assert_eq!(stats.relationships_scanned, 2);
        assert_eq!(stats.relationships_updated, 2);
        assert!(!stats.already_done);
    }

    #[test]
    fn first_episode_id_handles_json_array() {
        assert_eq!(first_episode_id(Some(r#"["a","b"]"#)).as_deref(), Some("a"));
    }

    #[test]
    fn first_episode_id_handles_comma_list() {
        assert_eq!(first_episode_id(Some("a,b,c")).as_deref(), Some("a"));
    }

    #[test]
    fn first_episode_id_handles_empty_and_null() {
        assert!(first_episode_id(None).is_none());
        assert!(first_episode_id(Some("")).is_none());
        assert!(first_episode_id(Some("[]")).is_none());
    }

    #[test]
    fn truncate_chars_is_unicode_safe() {
        let s: String = "é".repeat(250);
        let t = truncate_chars(&s, 200);
        assert_eq!(t.chars().count(), 200);
    }
}
