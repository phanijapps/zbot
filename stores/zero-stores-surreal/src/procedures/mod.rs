//! `SurrealProcedureStore` — `ProcedureStore` impl over `Arc<Surreal<Any>>`.
//!
//! Backs the `procedure` table (declared SCHEMALESS in `memory_kg.surql`).
//! Embeddings are inline `array<float>`; similarity search is brute-force
//! cosine over rows scoped to the agent — same trade-off as
//! `episodes::mod.rs` (HNSW for these tables is a follow-up).

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use surrealdb::engine::any::Any;
use surrealdb::Surreal;
use zero_stores_domain::Procedure;
use zero_stores_traits::{ProcedureStats, ProcedureStore};

#[derive(Clone)]
pub struct SurrealProcedureStore {
    db: Arc<Surreal<Any>>,
}

impl SurrealProcedureStore {
    pub fn new(db: Arc<Surreal<Any>>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl ProcedureStore for SurrealProcedureStore {
    async fn list_by_ward(&self, ward_id: &str, limit: usize) -> Result<Vec<Value>, String> {
        let q = format!(
            "SELECT * FROM procedure WHERE ward_id = $w \
             ORDER BY name LIMIT {limit}"
        );
        let mut resp = self
            .db
            .query(q)
            .bind(("w", ward_id.to_string()))
            .await
            .map_err(|e| format!("list_by_ward: {e}"))?;
        let rows: Vec<Value> = resp
            .take(0)
            .map_err(|e| format!("list_by_ward take: {e}"))?;
        Ok(rows.into_iter().map(row_to_procedure_value).collect())
    }

    async fn upsert_procedure(
        &self,
        procedure: Value,
        embedding: Option<Vec<f32>>,
    ) -> Result<(), String> {
        let mut typed: Procedure =
            serde_json::from_value(procedure).map_err(|e| format!("decode Procedure: {e}"))?;
        if embedding.is_some() {
            typed.embedding = embedding;
        }
        let thing = surrealdb::types::RecordId::new(
            "procedure",
            surrealdb::types::RecordIdKey::String(typed.id.clone()),
        );
        let payload = build_procedure_payload(&typed);
        self.db
            .query("UPSERT $id CONTENT $p")
            .bind(("id", thing))
            .bind(("p", payload))
            .await
            .map_err(|e| format!("upsert_procedure: {e}"))?;
        Ok(())
    }

    async fn search_procedures_by_similarity(
        &self,
        embedding: &[f32],
        agent_id: &str,
        ward_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Value>, String> {
        // ward filter is optional — narrow at fetch time when present.
        let q = match ward_id {
            Some(_) => self
                .db
                .query("SELECT * FROM procedure WHERE agent_id = $a AND ward_id = $w")
                .bind(("a", agent_id.to_string()))
                .bind(("w", ward_id.unwrap().to_string())),
            None => self
                .db
                .query("SELECT * FROM procedure WHERE agent_id = $a")
                .bind(("a", agent_id.to_string())),
        };
        let mut resp = q
            .await
            .map_err(|e| format!("search_procedures_by_similarity: {e}"))?;
        let rows: Vec<Value> = resp
            .take(0)
            .map_err(|e| format!("search_procedures_by_similarity take: {e}"))?;

        let mut scored: Vec<(Value, f64)> = rows
            .into_iter()
            .filter_map(|r| {
                let emb_arr = r.get("embedding")?.as_array()?;
                let row_emb: Vec<f32> = emb_arr
                    .iter()
                    .filter_map(|x| x.as_f64().map(|f| f as f32))
                    .collect();
                let score = crate::similarity::cosine(embedding, &row_emb)?;
                Some((row_to_procedure_value(r), score))
            })
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit);
        Ok(scored
            .into_iter()
            .map(|(p, score)| {
                serde_json::json!({
                    "procedure": p,
                    "score": score,
                })
            })
            .collect())
    }

    async fn increment_success(
        &self,
        id: &str,
        duration_ms: Option<i64>,
        token_cost: Option<i64>,
    ) -> Result<(), String> {
        // Read-modify-write: SurrealQL's IF expressions are picky around
        // mixed numeric/None arithmetic. Computing the new running averages
        // in Rust keeps the SQL trivial and matches the SQLite behaviour
        // (running-mean update only on the present sample) exactly.
        let thing = surrealdb::types::RecordId::new(
            "procedure",
            surrealdb::types::RecordIdKey::String(id.to_string()),
        );
        let now = chrono::Utc::now().to_rfc3339();
        let mut resp = self
            .db
            .query("SELECT success_count, avg_duration_ms, avg_token_cost FROM ONLY $id")
            .bind(("id", thing.clone()))
            .await
            .map_err(|e| format!("increment_success read: {e}"))?;
        let row: Option<Value> = resp
            .take(0)
            .map_err(|e| format!("increment_success read take: {e}"))?;
        let (prev_succ, prev_dur, prev_tc) = match row {
            Some(v) => (
                v.get("success_count").and_then(|x| x.as_i64()).unwrap_or(1),
                v.get("avg_duration_ms").and_then(|x| x.as_i64()),
                v.get("avg_token_cost").and_then(|x| x.as_i64()),
            ),
            None => return Ok(()), // row gone — nothing to do
        };
        let new_succ = prev_succ + 1;
        let new_dur = match (prev_dur, duration_ms) {
            (None, Some(d)) => Some(d),
            (Some(p), Some(d)) => Some((p * (new_succ - 1) + d) / new_succ),
            (existing, None) => existing,
        };
        let new_tc = match (prev_tc, token_cost) {
            (None, Some(c)) => Some(c),
            (Some(p), Some(c)) => Some((p * (new_succ - 1) + c) / new_succ),
            (existing, None) => existing,
        };
        self.db
            .query(
                "UPDATE $id SET \
                 success_count = $sc, \
                 last_used = $now, \
                 updated_at = $now, \
                 avg_duration_ms = $dur, \
                 avg_token_cost = $tc",
            )
            .bind(("id", thing))
            .bind(("sc", new_succ))
            .bind(("now", now))
            .bind(("dur", new_dur))
            .bind(("tc", new_tc))
            .await
            .map_err(|e| format!("increment_success write: {e}"))?;
        Ok(())
    }

    async fn increment_failure(&self, id: &str) -> Result<(), String> {
        let thing = surrealdb::types::RecordId::new(
            "procedure",
            surrealdb::types::RecordIdKey::String(id.to_string()),
        );
        let now = chrono::Utc::now().to_rfc3339();
        // Same pattern as `increment_success`: read-then-write keeps SQL trivial.
        let mut resp = self
            .db
            .query("SELECT failure_count FROM ONLY $id")
            .bind(("id", thing.clone()))
            .await
            .map_err(|e| format!("increment_failure read: {e}"))?;
        let row: Option<Value> = resp
            .take(0)
            .map_err(|e| format!("increment_failure read take: {e}"))?;
        let prev = row
            .and_then(|v| v.get("failure_count").and_then(|x| x.as_i64()))
            .unwrap_or(0);
        self.db
            .query("UPDATE $id SET failure_count = $fc, updated_at = $now")
            .bind(("id", thing))
            .bind(("fc", prev + 1))
            .bind(("now", now))
            .await
            .map_err(|e| format!("increment_failure: {e}"))?;
        Ok(())
    }

    async fn procedure_stats(&self) -> Result<ProcedureStats, String> {
        let mut resp = self
            .db
            .query("SELECT count() AS n FROM procedure GROUP ALL")
            .await
            .map_err(|e| format!("procedure_stats: {e}"))?;
        let rows: Vec<Value> = resp
            .take(0)
            .map_err(|e| format!("procedure_stats take: {e}"))?;
        let total = rows
            .first()
            .and_then(|v| v.get("n"))
            .and_then(|n| n.as_i64())
            .unwrap_or(0);
        Ok(ProcedureStats { total })
    }
}

/// Strip the `embedding` field (Procedure marks it `serde(skip)`) and
/// flatten the record id.
fn row_to_procedure_value(row: Value) -> Value {
    let mut flat = crate::row_value::flatten_record_id(row);
    if let Some(obj) = flat.as_object_mut() {
        obj.remove("embedding");
    }
    flat
}

fn build_procedure_payload(p: &Procedure) -> Value {
    serde_json::json!({
        "agent_id": p.agent_id,
        "ward_id": p.ward_id,
        "name": p.name,
        "description": p.description,
        "trigger_pattern": p.trigger_pattern,
        "steps": p.steps,
        "parameters": p.parameters,
        "success_count": p.success_count,
        "failure_count": p.failure_count,
        "avg_duration_ms": p.avg_duration_ms,
        "avg_token_cost": p.avg_token_cost,
        "last_used": p.last_used,
        "embedding": p.embedding,
        "created_at": p.created_at,
        "updated_at": p.updated_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{connect, schema::apply_schema, SurrealConfig};

    async fn fresh_store() -> SurrealProcedureStore {
        let cfg = SurrealConfig {
            url: "mem://".into(),
            namespace: "memory_kg".into(),
            database: "main".into(),
            credentials: None,
        };
        let db = connect(&cfg, None).await.expect("connect");
        apply_schema(&db).await.expect("schema");
        SurrealProcedureStore::new(db)
    }

    fn normalized(v: Vec<f32>) -> Vec<f32> {
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm < 1e-9 {
            v
        } else {
            v.into_iter().map(|x| x / norm).collect()
        }
    }

    fn sample_proc(id: &str, agent: &str, ward: Option<&str>) -> Value {
        let now = chrono::Utc::now().to_rfc3339();
        serde_json::json!({
            "id": id,
            "agent_id": agent,
            "ward_id": ward,
            "name": format!("proc-{id}"),
            "description": "test procedure",
            "trigger_pattern": null,
            "steps": "[\"s1\",\"s2\"]",
            "parameters": null,
            "success_count": 1,
            "failure_count": 0,
            "avg_duration_ms": null,
            "avg_token_cost": null,
            "last_used": null,
            "created_at": now,
            "updated_at": now,
        })
    }

    #[tokio::test]
    async fn upsert_and_list_by_ward() {
        let store = fresh_store().await;
        store
            .upsert_procedure(sample_proc("p1", "root", Some("__global__")), None)
            .await
            .unwrap();
        store
            .upsert_procedure(sample_proc("p2", "root", Some("__global__")), None)
            .await
            .unwrap();
        let rows = store.list_by_ward("__global__", 10).await.unwrap();
        assert_eq!(rows.len(), 2);
    }

    #[tokio::test]
    async fn similarity_search_returns_matches() {
        let store = fresh_store().await;
        let emb = normalized(
            (0..16)
                .map(|i| if i == 0 { 1.0_f32 } else { 0.0_f32 })
                .collect(),
        );
        store
            .upsert_procedure(
                sample_proc("p1", "root", Some("__global__")),
                Some(emb.clone()),
            )
            .await
            .unwrap();
        let results = store
            .search_procedures_by_similarity(&emb, "root", Some("__global__"), 5)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0]["score"].as_f64().unwrap() > 0.99);
    }

    #[tokio::test]
    async fn increment_success_and_failure() {
        let store = fresh_store().await;
        store
            .upsert_procedure(sample_proc("p1", "root", Some("__global__")), None)
            .await
            .unwrap();
        store
            .increment_success("p1", Some(100), Some(500))
            .await
            .unwrap();
        store.increment_failure("p1").await.unwrap();
        let rows = store.list_by_ward("__global__", 10).await.unwrap();
        let row = &rows[0];
        assert_eq!(row["success_count"], 2);
        assert_eq!(row["failure_count"], 1);
    }
}
