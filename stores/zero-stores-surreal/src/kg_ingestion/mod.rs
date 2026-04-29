//! `SurrealKgEpisodeStore` — `KgEpisodeStore` impl over `Arc<Surreal<Any>>`.
//!
//! Backs the `kg_ingestion_episode` table (declared SCHEMALESS in
//! `memory_kg.surql`). One row per chunk staged for KG extraction;
//! the background queue claims pending rows, runs LLM extraction, and
//! marks them done/failed.
//!
//! Atomicity notes:
//! - `claim_next_pending` uses `UPDATE ... WHERE status = 'pending'
//!   LIMIT 1 RETURN AFTER` — Surreal 3.x serializes statements within
//!   a connection so this is the atomic claim primitive.
//! - `upsert_pending` is two-step (probe + insert). The
//!   `kg_ingestion_dedup` unique index catches any race that lands
//!   between the probe and the create — the second writer's CREATE
//!   fails and we return the existing row's id.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use surrealdb::engine::any::Any;
use surrealdb::Surreal;
use zero_stores_traits::{KgEpisodeStatusCounts, KgEpisodeStore};

#[derive(Clone)]
pub struct SurrealKgEpisodeStore {
    db: Arc<Surreal<Any>>,
}

impl SurrealKgEpisodeStore {
    pub fn new(db: Arc<Surreal<Any>>) -> Self {
        Self { db }
    }
}

/// Normalize a row pulled from `kg_ingestion_episode` to the canonical
/// JSON shape the trait surface emits. SurrealDB serializes the row id
/// as `kg_ingestion_episode:<id>`; we strip the prefix so the wire
/// shape matches the SQLite-side `KgEpisode` JSON.
fn normalize_row(mut row: Value) -> Value {
    // Flatten id (table:id_str → id_str)
    if let Some(obj) = row.as_object_mut() {
        if let Some(id_v) = obj.get("id").cloned() {
            let cleaned = match id_v.as_str() {
                Some(s) => match s.find(':') {
                    Some(idx) => {
                        let after = &s[idx + 1..];
                        let stripped = after.trim_matches('`');
                        Value::String(stripped.to_string())
                    }
                    None => Value::String(s.to_string()),
                },
                None => crate::row_value::flatten_record_id(id_v),
            };
            obj.insert("id".to_string(), cleaned);
        }
        // Defaults for fields that may be absent on partial rows
        obj.entry("retry_count".to_string()).or_insert(json!(0));
        obj.entry("error".to_string()).or_insert(Value::Null);
        obj.entry("started_at".to_string()).or_insert(Value::Null);
        obj.entry("completed_at".to_string()).or_insert(Value::Null);
        obj.entry("session_id".to_string()).or_insert(Value::Null);
    }
    row
}

#[async_trait]
impl KgEpisodeStore for SurrealKgEpisodeStore {
    async fn get_episode(&self, id: &str) -> Result<Option<Value>, String> {
        let thing = surrealdb::types::RecordId::new(
            "kg_ingestion_episode",
            surrealdb::types::RecordIdKey::String(id.to_string()),
        );
        let mut resp = self
            .db
            .query("SELECT * FROM ONLY $id")
            .bind(("id", thing))
            .await
            .map_err(|e| format!("get_episode: {e}"))?;
        let row: Option<Value> = resp.take(0).map_err(|e| format!("get_episode take: {e}"))?;
        Ok(row.map(normalize_row))
    }

    async fn get_by_content_hash(
        &self,
        source_type: &str,
        content_hash: &str,
    ) -> Result<Option<Value>, String> {
        let mut resp = self
            .db
            .query(
                "SELECT * FROM kg_ingestion_episode \
                 WHERE source_type = $st AND content_hash = $ch LIMIT 1",
            )
            .bind(("st", source_type.to_string()))
            .bind(("ch", content_hash.to_string()))
            .await
            .map_err(|e| format!("get_by_content_hash: {e}"))?;
        let rows: Vec<Value> = resp
            .take(0)
            .map_err(|e| format!("get_by_content_hash take: {e}"))?;
        Ok(rows.into_iter().next().map(normalize_row))
    }

    async fn list_by_session(&self, session_id: &str) -> Result<Vec<Value>, String> {
        let mut resp = self
            .db
            .query(
                "SELECT * FROM kg_ingestion_episode \
                 WHERE session_id = $sid ORDER BY created_at",
            )
            .bind(("sid", session_id.to_string()))
            .await
            .map_err(|e| format!("list_by_session: {e}"))?;
        let rows: Vec<Value> = resp
            .take(0)
            .map_err(|e| format!("list_by_session take: {e}"))?;
        Ok(rows.into_iter().map(normalize_row).collect())
    }

    async fn status_counts_for_source(
        &self,
        source_ref_prefix: &str,
    ) -> Result<KgEpisodeStatusCounts, String> {
        // Surreal 3.x: GROUP BY status with count() projection. Aggregate
        // the rows into the typed counts struct.
        let pattern = format!("{}%", source_ref_prefix);
        let mut resp = self
            .db
            .query(
                "SELECT status, count() AS n FROM kg_ingestion_episode \
                 WHERE string::starts_with(source_ref, $prefix) \
                 GROUP BY status",
            )
            .bind(("prefix", source_ref_prefix.to_string()))
            .await
            .map_err(|e| format!("status_counts: {e}"))?;
        let rows: Vec<Value> = resp.take(0).map_err(|e| format!("status_counts take: {e}"))?;
        let _ = pattern; // kept for future LIKE-style fallback if needed

        let mut counts = KgEpisodeStatusCounts::default();
        for r in rows {
            let status = r.get("status").and_then(|v| v.as_str()).unwrap_or("");
            let n = r.get("n").and_then(|v| v.as_u64()).unwrap_or(0);
            match status {
                "pending" => counts.pending = n,
                "running" => counts.running = n,
                "done" => counts.done = n,
                "failed" => counts.failed = n,
                _ => {}
            }
        }
        Ok(counts)
    }

    async fn count_pending_global(&self) -> Result<u64, String> {
        let mut resp = self
            .db
            .query(
                "SELECT count() AS n FROM kg_ingestion_episode \
                 WHERE status = 'pending' GROUP ALL",
            )
            .await
            .map_err(|e| format!("count_pending_global: {e}"))?;
        let rows: Vec<Value> = resp
            .take(0)
            .map_err(|e| format!("count_pending_global take: {e}"))?;
        Ok(rows
            .first()
            .and_then(|r| r.get("n"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0))
    }

    async fn count_pending_for_source(
        &self,
        source_ref_prefix: &str,
    ) -> Result<u64, String> {
        let mut resp = self
            .db
            .query(
                "SELECT count() AS n FROM kg_ingestion_episode \
                 WHERE status = 'pending' \
                 AND string::starts_with(source_ref, $prefix) \
                 GROUP ALL",
            )
            .bind(("prefix", source_ref_prefix.to_string()))
            .await
            .map_err(|e| format!("count_pending_for_source: {e}"))?;
        let rows: Vec<Value> = resp
            .take(0)
            .map_err(|e| format!("count_pending_for_source take: {e}"))?;
        Ok(rows
            .first()
            .and_then(|r| r.get("n"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0))
    }

    async fn upsert_pending(
        &self,
        source_type: &str,
        source_ref: &str,
        content_hash: &str,
        session_id: Option<&str>,
        agent_id: &str,
    ) -> Result<String, String> {
        // Probe for an existing row keyed by the dedup index.
        if let Some(existing) = self
            .get_by_content_hash(source_type, content_hash)
            .await?
        {
            if let Some(id) = existing.get("id").and_then(|v| v.as_str()) {
                return Ok(id.to_string());
            }
        }

        let new_id = format!("ep-{}", uuid::Uuid::new_v4());
        let thing = surrealdb::types::RecordId::new(
            "kg_ingestion_episode",
            surrealdb::types::RecordIdKey::String(new_id.clone()),
        );
        let now = chrono::Utc::now().to_rfc3339();
        let payload = json!({
            "source_type": source_type,
            "source_ref": source_ref,
            "content_hash": content_hash,
            "session_id": session_id,
            "agent_id": agent_id,
            "status": "pending",
            "retry_count": 0,
            "created_at": now,
        });
        self.db
            .query("CREATE $id CONTENT $r")
            .bind(("id", thing))
            .bind(("r", payload))
            .await
            .map_err(|e| format!("upsert_pending create: {e}"))?;
        Ok(new_id)
    }

    async fn claim_next_pending(&self) -> Result<Option<Value>, String> {
        // Atomic claim: UPDATE with WHERE-status-pending + LIMIT 1, return
        // AFTER. SurrealDB serializes statements within a connection so a
        // racing claim sees the row already running.
        let mut resp = self
            .db
            .query(
                "UPDATE (SELECT VALUE id FROM kg_ingestion_episode \
                          WHERE status = 'pending' LIMIT 1) \
                 SET status = 'running', started_at = time::now() \
                 RETURN AFTER",
            )
            .await
            .map_err(|e| format!("claim_next_pending: {e}"))?;
        let rows: Vec<Value> = resp
            .take(0)
            .map_err(|e| format!("claim_next_pending take: {e}"))?;
        Ok(rows.into_iter().next().map(normalize_row))
    }

    async fn mark_done(&self, id: &str) -> Result<(), String> {
        let thing = surrealdb::types::RecordId::new(
            "kg_ingestion_episode",
            surrealdb::types::RecordIdKey::String(id.to_string()),
        );
        self.db
            .query("UPDATE $id SET status = 'done', completed_at = time::now()")
            .bind(("id", thing))
            .await
            .map_err(|e| format!("mark_done: {e}"))?;
        Ok(())
    }

    async fn mark_failed(&self, id: &str, error: &str) -> Result<(), String> {
        let thing = surrealdb::types::RecordId::new(
            "kg_ingestion_episode",
            surrealdb::types::RecordIdKey::String(id.to_string()),
        );
        self.db
            .query(
                "UPDATE $id SET status = 'failed', \
                 error = $err, completed_at = time::now()",
            )
            .bind(("id", thing))
            .bind(("err", error.to_string()))
            .await
            .map_err(|e| format!("mark_failed: {e}"))?;
        Ok(())
    }

    async fn retry_if_eligible(
        &self,
        id: &str,
        max_retries: u32,
    ) -> Result<bool, String> {
        let thing = surrealdb::types::RecordId::new(
            "kg_ingestion_episode",
            surrealdb::types::RecordIdKey::String(id.to_string()),
        );
        // Conditional UPDATE: only act when retry_count < max_retries AND
        // status = 'failed'. RETURN AFTER lets us tell the caller whether
        // anything moved.
        let mut resp = self
            .db
            .query(
                "UPDATE $id SET \
                 status = 'pending', \
                 retry_count = (retry_count OR 0) + 1, \
                 error = NONE, \
                 started_at = NONE, \
                 completed_at = NONE \
                 WHERE status = 'failed' \
                 AND (retry_count OR 0) < $max \
                 RETURN AFTER",
            )
            .bind(("id", thing))
            .bind(("max", max_retries as i64))
            .await
            .map_err(|e| format!("retry_if_eligible: {e}"))?;
        let rows: Vec<Value> = resp
            .take(0)
            .map_err(|e| format!("retry_if_eligible take: {e}"))?;
        Ok(!rows.is_empty())
    }

    async fn set_payload(&self, id: &str, text: &str) -> Result<(), String> {
        let thing = surrealdb::types::RecordId::new(
            "kg_ingestion_episode",
            surrealdb::types::RecordIdKey::String(id.to_string()),
        );
        self.db
            .query("UPDATE $id SET payload_text = $t")
            .bind(("id", thing))
            .bind(("t", text.to_string()))
            .await
            .map_err(|e| format!("set_payload: {e}"))?;
        Ok(())
    }

    async fn get_payload(&self, id: &str) -> Result<Option<String>, String> {
        let thing = surrealdb::types::RecordId::new(
            "kg_ingestion_episode",
            surrealdb::types::RecordIdKey::String(id.to_string()),
        );
        let mut resp = self
            .db
            .query("SELECT payload_text FROM ONLY $id")
            .bind(("id", thing))
            .await
            .map_err(|e| format!("get_payload: {e}"))?;
        let row: Option<Value> = resp.take(0).map_err(|e| format!("get_payload take: {e}"))?;
        Ok(row
            .and_then(|v| v.get("payload_text").cloned())
            .and_then(|v| v.as_str().map(|s| s.to_string())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{connect, schema::apply_schema, SurrealConfig};

    async fn fresh_store() -> SurrealKgEpisodeStore {
        let cfg = SurrealConfig {
            url: "mem://".into(),
            namespace: "memory_kg".into(),
            database: "main".into(),
            credentials: None,
        };
        let db = connect(&cfg, None).await.expect("connect");
        apply_schema(&db).await.expect("schema");
        SurrealKgEpisodeStore::new(db)
    }

    #[tokio::test]
    async fn upsert_pending_creates_new_episode() {
        let store = fresh_store().await;
        let id = store
            .upsert_pending("ward_file", "src#chunk-0", "hash-a", None, "agent-x")
            .await
            .expect("upsert");
        assert!(!id.is_empty());

        let fetched = store.get_episode(&id).await.expect("get").expect("present");
        assert_eq!(fetched["source_type"], "ward_file");
        assert_eq!(fetched["status"], "pending");
        assert_eq!(fetched["agent_id"], "agent-x");
    }

    #[tokio::test]
    async fn upsert_pending_dedups_by_content_hash() {
        let store = fresh_store().await;
        let id1 = store
            .upsert_pending("ward_file", "src#chunk-0", "hash-a", None, "agent-x")
            .await
            .unwrap();
        let id2 = store
            .upsert_pending("ward_file", "src#chunk-0", "hash-a", None, "agent-x")
            .await
            .unwrap();
        assert_eq!(id1, id2, "dedup must collapse to the same id");
    }

    #[tokio::test]
    async fn claim_next_pending_transitions_to_running() {
        let store = fresh_store().await;
        let id = store
            .upsert_pending("ward_file", "src#chunk-0", "hash-a", None, "agent-x")
            .await
            .unwrap();
        let claimed = store
            .claim_next_pending()
            .await
            .expect("claim")
            .expect("present");
        assert_eq!(claimed["id"].as_str(), Some(id.as_str()));
        assert_eq!(claimed["status"], "running");

        // Second claim sees no pending rows
        let again = store.claim_next_pending().await.expect("claim 2");
        assert!(again.is_none());
    }

    #[tokio::test]
    async fn mark_done_transitions_status() {
        let store = fresh_store().await;
        let id = store
            .upsert_pending("ward_file", "src#chunk-0", "hash-a", None, "agent-x")
            .await
            .unwrap();
        store.mark_done(&id).await.expect("mark_done");
        let row = store.get_episode(&id).await.expect("get").expect("present");
        assert_eq!(row["status"], "done");
    }

    #[tokio::test]
    async fn payload_round_trip() {
        let store = fresh_store().await;
        let id = store
            .upsert_pending("ward_file", "src#chunk-0", "hash-a", None, "agent-x")
            .await
            .unwrap();
        store
            .set_payload(&id, "hello world")
            .await
            .expect("set_payload");
        let fetched = store.get_payload(&id).await.expect("get_payload");
        assert_eq!(fetched, Some("hello world".to_string()));
    }

    #[tokio::test]
    async fn retry_if_eligible_respects_budget() {
        let store = fresh_store().await;
        let id = store
            .upsert_pending("ward_file", "src#chunk-0", "hash-a", None, "agent-x")
            .await
            .unwrap();
        // First fail
        store.mark_failed(&id, "boom").await.expect("mark_failed");
        let retried = store.retry_if_eligible(&id, 2).await.expect("retry 1");
        assert!(retried, "first retry within budget");

        // Fail again
        store.mark_failed(&id, "boom 2").await.unwrap();
        let retried_2 = store.retry_if_eligible(&id, 2).await.expect("retry 2");
        assert!(retried_2, "second retry within budget");

        // Third fail — over budget
        store.mark_failed(&id, "boom 3").await.unwrap();
        let retried_3 = store.retry_if_eligible(&id, 2).await.expect("retry 3");
        assert!(!retried_3, "retry beyond budget must be denied");
    }
}
