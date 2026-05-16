// ============================================================================
// SQLITE BELIEF STORE
// Implements BeliefStore trait against the v27 kg_beliefs table.
// ============================================================================
//
// A belief row carries its bi-temporal interval (`valid_from` /
// `valid_until`) as RFC3339 strings — matching the convention used by
// `memory_facts`. The `source_fact_ids` column holds a JSON-encoded
// `Vec<String>`; callers see a typed `Vec<String>` via `Belief`.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rusqlite::{params, OptionalExtension};
use zero_stores_domain::Belief;
use zero_stores_traits::BeliefStore;

use crate::KnowledgeDatabase;

/// SQLite-backed `BeliefStore`. Delegates every method to a single
/// `KnowledgeDatabase` pool; each call runs against one short-lived
/// connection so calls compose cleanly with the rest of the memory
/// subsystem.
pub struct SqliteBeliefStore {
    db: Arc<KnowledgeDatabase>,
}

impl SqliteBeliefStore {
    pub fn new(db: Arc<KnowledgeDatabase>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl BeliefStore for SqliteBeliefStore {
    async fn get_belief(
        &self,
        partition_id: &str,
        subject: &str,
        as_of: Option<DateTime<Utc>>,
    ) -> Result<Option<Belief>, String> {
        let cutoff = as_of.unwrap_or_else(Utc::now).to_rfc3339();
        let partition_id = partition_id.to_string();
        let subject = subject.to_string();
        self.db
            .with_connection(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, partition_id, subject, content, confidence,
                        valid_from, valid_until, source_fact_ids,
                        synthesizer_version, reasoning, created_at,
                        updated_at, superseded_by
                 FROM kg_beliefs
                 WHERE partition_id = ?1
                   AND subject = ?2
                   AND (valid_from IS NULL OR valid_from <= ?3)
                   AND (valid_until IS NULL OR valid_until > ?3)
                 ORDER BY COALESCE(valid_from, '') DESC
                 LIMIT 1",
                )?;
                stmt.query_row(params![partition_id, subject, cutoff], row_to_belief)
                    .optional()
            })?
            .transpose()
    }

    async fn list_beliefs(&self, partition_id: &str, limit: usize) -> Result<Vec<Belief>, String> {
        let partition_id = partition_id.to_string();
        // i64 cast is safe — `limit` originates from a usize controlled
        // by the caller and is bounded by realistic UI / API caps.
        let limit_i64 = i64::try_from(limit).unwrap_or(i64::MAX);
        let rows = self.db.with_connection(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, partition_id, subject, content, confidence,
                        valid_from, valid_until, source_fact_ids,
                        synthesizer_version, reasoning, created_at,
                        updated_at, superseded_by
                 FROM kg_beliefs
                 WHERE partition_id = ?1
                 ORDER BY updated_at DESC
                 LIMIT ?2",
            )?;
            let rows = stmt
                .query_map(params![partition_id, limit_i64], row_to_belief)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })?;
        // Each row yields a Result<Belief, String> due to JSON decode in
        // row_to_belief; flatten now.
        rows.into_iter().collect()
    }

    async fn upsert_belief(&self, belief: &Belief) -> Result<(), String> {
        let source_fact_ids_json = serde_json::to_string(&belief.source_fact_ids)
            .map_err(|e| format!("encode source_fact_ids: {e}"))?;
        let valid_from = belief.valid_from.map(|t| t.to_rfc3339());
        let valid_until = belief.valid_until.map(|t| t.to_rfc3339());
        let created_at = belief.created_at.to_rfc3339();
        let updated_at = belief.updated_at.to_rfc3339();

        let id = belief.id.clone();
        let partition_id = belief.partition_id.clone();
        let subject = belief.subject.clone();
        let content = belief.content.clone();
        let confidence = belief.confidence;
        let synthesizer_version = belief.synthesizer_version;
        let reasoning = belief.reasoning.clone();
        let superseded_by = belief.superseded_by.clone();

        self.db.with_connection(move |conn| {
            conn.execute(
                "INSERT INTO kg_beliefs (
                    id, partition_id, subject, content, confidence,
                    valid_from, valid_until, source_fact_ids,
                    synthesizer_version, reasoning, created_at, updated_at,
                    superseded_by
                ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13
                )
                ON CONFLICT(partition_id, subject, valid_from) DO UPDATE SET
                    content = excluded.content,
                    confidence = excluded.confidence,
                    valid_until = excluded.valid_until,
                    source_fact_ids = excluded.source_fact_ids,
                    synthesizer_version = excluded.synthesizer_version,
                    reasoning = excluded.reasoning,
                    updated_at = excluded.updated_at,
                    superseded_by = excluded.superseded_by",
                params![
                    id,
                    partition_id,
                    subject,
                    content,
                    confidence,
                    valid_from,
                    valid_until,
                    source_fact_ids_json,
                    synthesizer_version,
                    reasoning,
                    created_at,
                    updated_at,
                    superseded_by,
                ],
            )?;
            Ok(())
        })
    }

    async fn supersede_belief(
        &self,
        old_id: &str,
        new_id: &str,
        transition_time: DateTime<Utc>,
    ) -> Result<(), String> {
        let old_id = old_id.to_string();
        let new_id = new_id.to_string();
        let ts = transition_time.to_rfc3339();
        self.db.with_connection(move |conn| {
            conn.execute(
                "UPDATE kg_beliefs
                 SET valid_until = ?1,
                     superseded_by = ?2,
                     updated_at = ?1
                 WHERE id = ?3",
                params![ts, new_id, old_id],
            )?;
            Ok(())
        })
    }
}

/// Map one row of `kg_beliefs` to a `Belief`. The `source_fact_ids` JSON
/// decode is fallible — return a `Result<Belief, String>` per row so the
/// caller can fail loud rather than silently dropping rows.
fn row_to_belief(row: &rusqlite::Row) -> rusqlite::Result<Result<Belief, String>> {
    let id: String = row.get(0)?;
    let partition_id: String = row.get(1)?;
    let subject: String = row.get(2)?;
    let content: String = row.get(3)?;
    let confidence: f64 = row.get(4)?;
    let valid_from: Option<String> = row.get(5)?;
    let valid_until: Option<String> = row.get(6)?;
    let source_fact_ids_json: String = row.get(7)?;
    let synthesizer_version: i32 = row.get(8)?;
    let reasoning: Option<String> = row.get(9)?;
    let created_at: String = row.get(10)?;
    let updated_at: String = row.get(11)?;
    let superseded_by: Option<String> = row.get(12)?;

    let source_fact_ids: Vec<String> = match serde_json::from_str(&source_fact_ids_json) {
        Ok(v) => v,
        Err(e) => return Ok(Err(format!("decode source_fact_ids for {id}: {e}"))),
    };

    let parse_dt = |s: &str| {
        DateTime::parse_from_rfc3339(s)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| format!("parse timestamp {s}: {e}"))
    };

    let valid_from = match valid_from.as_deref().map(parse_dt).transpose() {
        Ok(v) => v,
        Err(e) => return Ok(Err(e)),
    };
    let valid_until = match valid_until.as_deref().map(parse_dt).transpose() {
        Ok(v) => v,
        Err(e) => return Ok(Err(e)),
    };
    let created_at = match parse_dt(&created_at) {
        Ok(v) => v,
        Err(e) => return Ok(Err(e)),
    };
    let updated_at = match parse_dt(&updated_at) {
        Ok(v) => v,
        Err(e) => return Ok(Err(e)),
    };

    Ok(Ok(Belief {
        id,
        partition_id,
        subject,
        content,
        confidence,
        valid_from,
        valid_until,
        source_fact_ids,
        synthesizer_version,
        reasoning,
        created_at,
        updated_at,
        superseded_by,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::KnowledgeDatabase;
    use gateway_services::VaultPaths;
    use tempfile::TempDir;

    fn make_store() -> (SqliteBeliefStore, TempDir) {
        let tmp = TempDir::new().expect("tempdir");
        let paths = Arc::new(VaultPaths::new(tmp.path().to_path_buf()));
        std::fs::create_dir_all(paths.conversations_db().parent().unwrap()).unwrap();
        let db = Arc::new(KnowledgeDatabase::new(paths).expect("db"));
        (SqliteBeliefStore::new(db), tmp)
    }

    fn sample_belief(id: &str, subject: &str, valid_from: Option<DateTime<Utc>>) -> Belief {
        let now = Utc::now();
        Belief {
            id: id.to_string(),
            partition_id: "default".to_string(),
            subject: subject.to_string(),
            content: format!("{} content", subject),
            confidence: 0.8,
            valid_from,
            valid_until: None,
            source_fact_ids: vec!["fact-1".to_string()],
            synthesizer_version: 1,
            reasoning: None,
            created_at: now,
            updated_at: now,
            superseded_by: None,
        }
    }

    #[tokio::test]
    async fn upsert_and_get_round_trip() {
        let (store, _tmp) = make_store();
        let b = sample_belief("b1", "user.name", Some(Utc::now()));
        store.upsert_belief(&b).await.unwrap();

        let got = store
            .get_belief("default", "user.name", None)
            .await
            .unwrap()
            .expect("belief present");
        assert_eq!(got.id, "b1");
        assert_eq!(got.subject, "user.name");
        assert_eq!(got.source_fact_ids, vec!["fact-1".to_string()]);
    }

    #[tokio::test]
    async fn upsert_is_idempotent_on_unique_key() {
        let (store, _tmp) = make_store();
        let vf = Utc::now();
        let mut b = sample_belief("b1", "user.name", Some(vf));
        store.upsert_belief(&b).await.unwrap();

        // Upsert again with same (partition, subject, valid_from) but
        // different id/content — the unique constraint triggers UPDATE
        // and the row count stays at 1.
        b.id = "b1-v2".to_string();
        b.content = "second pass".to_string();
        store.upsert_belief(&b).await.unwrap();

        let listed = store.list_beliefs("default", 10).await.unwrap();
        assert_eq!(listed.len(), 1, "no duplicate rows on conflict");
        assert_eq!(listed[0].content, "second pass");
    }

    #[tokio::test]
    async fn get_belief_with_as_of_returns_historical_slice() {
        let (store, _tmp) = make_store();
        let old_vf = DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let old_vu = DateTime::parse_from_rfc3339("2026-03-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let new_vf = DateTime::parse_from_rfc3339("2026-03-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let mut old = sample_belief("b-old", "user.employment", Some(old_vf));
        old.valid_until = Some(old_vu);
        old.content = "Anthropic".to_string();
        old.superseded_by = Some("b-new".to_string());
        let mut new_b = sample_belief("b-new", "user.employment", Some(new_vf));
        new_b.content = "OpenAI".to_string();

        store.upsert_belief(&old).await.unwrap();
        store.upsert_belief(&new_b).await.unwrap();

        // Default → "now" → returns the active belief.
        let cur = store
            .get_belief("default", "user.employment", None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(cur.content, "OpenAI");

        // as_of in the old window → returns the historical belief.
        let past = DateTime::parse_from_rfc3339("2026-02-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let hist = store
            .get_belief("default", "user.employment", Some(past))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(hist.content, "Anthropic");
    }

    #[tokio::test]
    async fn supersede_belief_sets_valid_until_and_pointer() {
        let (store, _tmp) = make_store();
        let vf = DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let b = sample_belief("b-old", "user.x", Some(vf));
        store.upsert_belief(&b).await.unwrap();

        let transition = DateTime::parse_from_rfc3339("2026-03-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        store
            .supersede_belief("b-old", "b-new", transition)
            .await
            .unwrap();

        let listed = store.list_beliefs("default", 10).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].valid_until, Some(transition));
        assert_eq!(listed[0].superseded_by.as_deref(), Some("b-new"));
    }

    #[tokio::test]
    async fn missing_belief_returns_none() {
        let (store, _tmp) = make_store();
        let got = store
            .get_belief("default", "no.such.subject", None)
            .await
            .unwrap();
        assert!(got.is_none());
    }
}
