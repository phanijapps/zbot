//! `SurrealDistillationStore` — `DistillationStore` impl over `Arc<Surreal<Any>>`.
//!
//! Backs the `distillation_run` table (declared SCHEMALESS in `memory_kg.surql`).
//! Insert is upsert-by-`session_id` to mirror the SQLite repo's
//! `ON CONFLICT(session_id) DO UPDATE` semantics — a continuation can
//! re-fire distillation without erroring out.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use surrealdb::engine::any::Any;
use surrealdb::Surreal;
use zero_stores_traits::DistillationStore;

#[derive(Clone)]
pub struct SurrealDistillationStore {
    db: Arc<Surreal<Any>>,
}

impl SurrealDistillationStore {
    pub fn new(db: Arc<Surreal<Any>>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl DistillationStore for SurrealDistillationStore {
    async fn insert_run(&self, run: Value) -> Result<(), String> {
        let session_id = run
            .get("session_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "DistillationRun missing session_id".to_string())?
            .to_string();
        // UPSERT keyed by session_id is the simplest way to land both insert
        // and conflict-update on a single row. The unique index on
        // session_id keeps duplicates impossible regardless.
        self.db
            .query(
                "UPDATE distillation_run \
                 SET session_id = $sid, \
                     status = $status, \
                     facts_extracted = $facts, \
                     entities_extracted = $ents, \
                     relationships_extracted = $rels, \
                     episode_created = $ep, \
                     error = $err, \
                     retry_count = $rc, \
                     duration_ms = $dur, \
                     created_at = $ca \
                 WHERE session_id = $sid",
            )
            .bind(("sid", session_id.clone()))
            .bind(("status", string_field(&run, "status").unwrap_or_default()))
            .bind(("facts", i64_field(&run, "facts_extracted")))
            .bind(("ents", i64_field(&run, "entities_extracted")))
            .bind(("rels", i64_field(&run, "relationships_extracted")))
            .bind(("ep", i64_field(&run, "episode_created")))
            .bind(("err", string_field(&run, "error")))
            .bind(("rc", i64_field(&run, "retry_count")))
            .bind(("dur", i64_field(&run, "duration_ms")))
            .bind(("ca", string_field(&run, "created_at").unwrap_or_default()))
            .await
            .map_err(|e| format!("insert_run update: {e}"))?;

        // If the UPDATE matched zero rows, CREATE a new one. SurrealQL
        // doesn't have a single-statement upsert keyed on a non-id column,
        // so the two-step is the canonical pattern.
        let mut existing = self
            .db
            .query("SELECT id FROM distillation_run WHERE session_id = $sid")
            .bind(("sid", session_id.clone()))
            .await
            .map_err(|e| format!("insert_run probe: {e}"))?;
        let probe: Vec<serde_json::Value> = existing
            .take(0)
            .map_err(|e| format!("insert_run probe take: {e}"))?;
        if probe.is_empty() {
            self.db
                .query("CREATE distillation_run CONTENT $r")
                .bind(("r", run))
                .await
                .map_err(|e| format!("insert_run create: {e}"))?;
        }
        Ok(())
    }

    async fn get_run_by_session(&self, session_id: &str) -> Result<Option<Value>, String> {
        let mut resp = self
            .db
            .query("SELECT * FROM distillation_run WHERE session_id = $sid LIMIT 1")
            .bind(("sid", session_id.to_string()))
            .await
            .map_err(|e| format!("get_run_by_session: {e}"))?;
        let rows: Vec<Value> = resp
            .take(0)
            .map_err(|e| format!("get_run_by_session take: {e}"))?;
        Ok(rows
            .into_iter()
            .next()
            .map(crate::row_value::flatten_record_id))
    }

    async fn update_retry(&self, session_id: &str) -> Result<(), String> {
        // Trait contract: bump retry by 1, mark status='retry', no error.
        // Mirrors the SQLite trait impl which passes a fixed retry_count=1
        // because the executor tracks its own counter separately.
        self.db
            .query(
                "UPDATE distillation_run SET \
                 status = 'retry', \
                 retry_count = (retry_count OR 0) + 1, \
                 error = NONE \
                 WHERE session_id = $sid",
            )
            .bind(("sid", session_id.to_string()))
            .await
            .map_err(|e| format!("update_retry: {e}"))?;
        Ok(())
    }

    async fn update_success(
        &self,
        session_id: &str,
        summary: Option<String>,
    ) -> Result<(), String> {
        self.db
            .query(
                "UPDATE distillation_run SET \
                 status = 'success', \
                 error = NONE, \
                 summary = $summary \
                 WHERE session_id = $sid",
            )
            .bind(("sid", session_id.to_string()))
            .bind(("summary", summary))
            .await
            .map_err(|e| format!("update_success: {e}"))?;
        Ok(())
    }
}

fn string_field(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(|x| x.as_str()).map(str::to_string)
}

fn i64_field(v: &Value, key: &str) -> Option<i64> {
    v.get(key).and_then(|x| x.as_i64())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{connect, schema::apply_schema, SurrealConfig};

    async fn fresh_store() -> SurrealDistillationStore {
        let cfg = SurrealConfig {
            url: "mem://".into(),
            namespace: "memory_kg".into(),
            database: "main".into(),
            credentials: None,
        };
        let db = connect(&cfg, None).await.expect("connect");
        apply_schema(&db).await.expect("schema");
        SurrealDistillationStore::new(db)
    }

    fn sample_run(session_id: &str, status: &str) -> Value {
        serde_json::json!({
            "id": format!("dr-{session_id}"),
            "session_id": session_id,
            "status": status,
            "facts_extracted": 0,
            "entities_extracted": 0,
            "relationships_extracted": 0,
            "episode_created": 0,
            "error": null,
            "retry_count": 0,
            "duration_ms": null,
            "created_at": chrono::Utc::now().to_rfc3339(),
        })
    }

    #[tokio::test]
    async fn insert_then_get_by_session() {
        let store = fresh_store().await;
        store
            .insert_run(sample_run("sess-1", "failed"))
            .await
            .unwrap();
        let fetched = store
            .get_run_by_session("sess-1")
            .await
            .unwrap()
            .expect("present");
        assert_eq!(fetched["session_id"], "sess-1");
        assert_eq!(fetched["status"], "failed");
    }

    #[tokio::test]
    async fn insert_is_idempotent_by_session_id() {
        let store = fresh_store().await;
        let mut first = sample_run("sess-same", "failed");
        first["error"] = serde_json::Value::String("initial".to_string());
        store.insert_run(first).await.unwrap();

        let mut second = sample_run("sess-same", "failed");
        second["error"] = serde_json::Value::String("Distillation in progress".to_string());
        second["retry_count"] = serde_json::Value::Number(1.into());
        store
            .insert_run(second)
            .await
            .expect("second insert must upsert, not error");

        let fetched = store
            .get_run_by_session("sess-same")
            .await
            .unwrap()
            .expect("present");
        assert_eq!(fetched["error"], "Distillation in progress");
        assert_eq!(fetched["retry_count"], 1);
    }

    #[tokio::test]
    async fn update_retry_bumps_count_and_status() {
        let store = fresh_store().await;
        store
            .insert_run(sample_run("sess-1", "failed"))
            .await
            .unwrap();
        store.update_retry("sess-1").await.unwrap();
        let fetched = store
            .get_run_by_session("sess-1")
            .await
            .unwrap()
            .expect("present");
        assert_eq!(fetched["status"], "retry");
        assert_eq!(fetched["retry_count"], 1);
    }

    #[tokio::test]
    async fn update_success_marks_status() {
        let store = fresh_store().await;
        store
            .insert_run(sample_run("sess-1", "failed"))
            .await
            .unwrap();
        store
            .update_success("sess-1", Some("done".to_string()))
            .await
            .unwrap();
        let fetched = store
            .get_run_by_session("sess-1")
            .await
            .unwrap()
            .expect("present");
        assert_eq!(fetched["status"], "success");
        assert_eq!(fetched["summary"], "done");
    }
}
