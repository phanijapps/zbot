//! MemoryFactStore implementation backed by the `memory_fact` SurrealDB table.

use std::sync::Arc;

use serde_json::Value;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use surrealdb::types::SurrealValue;

#[derive(SurrealValue)]
#[surreal(crate = "surrealdb::types")]
struct CountRow {
    n: i64,
}

#[derive(SurrealValue)]
#[surreal(crate = "surrealdb::types")]
struct FactRow {
    id: surrealdb::types::RecordId,
    agent_id: String,
    content: String,
    fact_type: String,
    confidence: Option<f64>,
}

pub async fn save_fact(
    db: &Arc<Surreal<Any>>,
    agent_id: &str,
    category: &str,
    _key: &str,
    content: &str,
    confidence: f64,
    _session_id: Option<&str>,
) -> Result<Value, String> {
    db.query(
        "CREATE memory_fact SET \
         agent_id = $a, content = $c, fact_type = $ft, confidence = $conf, archived = false",
    )
    .bind(("a", agent_id.to_string()))
    .bind(("c", content.to_string()))
    .bind(("ft", category.to_string()))
    .bind(("conf", confidence))
    .await
    .map_err(|e| format!("save_fact: {e}"))?;
    Ok(serde_json::json!({ "saved": true }))
}

pub async fn recall_facts(
    db: &Arc<Surreal<Any>>,
    agent_id: &str,
    query: &str,
    limit: usize,
) -> Result<Value, String> {
    let q = format!(
        "SELECT id, agent_id, content, fact_type, confidence FROM memory_fact \
         WHERE agent_id = $a AND content @@ $q AND archived = false LIMIT {limit}"
    );
    let mut resp = db
        .query(q)
        .bind(("a", agent_id.to_string()))
        .bind(("q", query.to_string()))
        .await
        .map_err(|e| format!("recall_facts: {e}"))?;
    let rows: Vec<FactRow> = resp.take(0).map_err(|e| format!("recall_facts take: {e}"))?;
    let arr: Vec<Value> = rows
        .into_iter()
        .map(|r| {
            let id_str = match &r.id.key {
                surrealdb::types::RecordIdKey::String(s) => s.clone(),
                other => format!("{other:?}"),
            };
            serde_json::json!({
                "id": id_str,
                "agent_id": r.agent_id,
                "content": r.content,
                "fact_type": r.fact_type,
                "confidence": r.confidence.unwrap_or(0.8),
            })
        })
        .collect();
    Ok(Value::Array(arr))
}

pub async fn count_all_facts(
    db: &Arc<Surreal<Any>>,
    agent_id: Option<&str>,
) -> Result<i64, String> {
    let mut resp = match agent_id {
        Some(a) => db
            .query("SELECT count() AS n FROM memory_fact WHERE agent_id = $a GROUP ALL")
            .bind(("a", a.to_string()))
            .await
            .map_err(|e| format!("count_all_facts: {e}"))?,
        None => db
            .query("SELECT count() AS n FROM memory_fact GROUP ALL")
            .await
            .map_err(|e| format!("count_all_facts: {e}"))?,
    };
    let rows: Vec<CountRow> = resp.take(0).map_err(|e| format!("count take: {e}"))?;
    Ok(rows.first().map(|r| r.n).unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SurrealConfig, connect, schema::apply_schema};

    async fn fresh_db() -> Arc<Surreal<Any>> {
        let cfg = SurrealConfig {
            url: "mem://".into(),
            namespace: "memory_kg".into(),
            database: "main".into(),
            credentials: None,
        };
        let db = connect(&cfg, None).await.unwrap();
        apply_schema(&db).await.unwrap();
        db
    }

    #[tokio::test]
    async fn save_then_count() {
        let db = fresh_db().await;
        save_fact(&db, "a1", "preference", "k1", "Likes coffee", 0.9, None)
            .await
            .unwrap();
        let n = count_all_facts(&db, Some("a1")).await.unwrap();
        assert_eq!(n, 1);
    }

    #[tokio::test]
    async fn recall_finds_match() {
        let db = fresh_db().await;
        save_fact(&db, "a1", "preference", "k1", "Loves espresso drinks", 0.9, None)
            .await
            .unwrap();
        let result = recall_facts(&db, "a1", "espresso", 10).await.unwrap();
        let arr = result.as_array().unwrap();
        assert!(!arr.is_empty(), "should find match");
    }

    #[tokio::test]
    async fn recall_respects_agent_isolation() {
        let db = fresh_db().await;
        save_fact(&db, "a1", "preference", "k1", "Coffee preference", 0.9, None)
            .await
            .unwrap();
        save_fact(&db, "a2", "preference", "k1", "Coffee preference", 0.9, None)
            .await
            .unwrap();
        let result = recall_facts(&db, "a1", "coffee", 10).await.unwrap();
        let arr = result.as_array().unwrap();
        assert!(arr.iter().all(|f| f["agent_id"] == "a1"));
    }
}
