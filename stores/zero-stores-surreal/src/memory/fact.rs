//! MemoryFactStore implementation backed by the `memory_fact` SurrealDB table.

use std::sync::Arc;

use serde_json::Value;
use surrealdb::engine::any::Any;
use surrealdb::types::SurrealValue;
use surrealdb::Surreal;

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
    let rows: Vec<FactRow> = resp
        .take(0)
        .map_err(|e| format!("recall_facts take: {e}"))?;
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

pub async fn list_memory_facts(
    db: &Arc<Surreal<Any>>,
    agent_id: Option<&str>,
    category: Option<&str>,
    _scope: Option<&str>,
    limit: usize,
    offset: usize,
) -> Result<Vec<Value>, String> {
    // `scope` is accepted for API parity with the SQLite store but is not
    // yet a column on the Surreal `memory_fact` table; ignored for now.
    let mut where_clauses: Vec<&'static str> = vec!["archived = false"];
    if agent_id.is_some() {
        where_clauses.push("agent_id = $a");
    }
    if category.is_some() {
        where_clauses.push("fact_type = $c");
    }
    let where_sql = where_clauses.join(" AND ");
    let q = format!(
        "SELECT id, agent_id, content, fact_type, confidence, \
         created_at, last_used_at \
         FROM memory_fact WHERE {where_sql} \
         ORDER BY created_at DESC LIMIT {limit} START {offset}"
    );
    let mut q = db.query(q);
    if let Some(a) = agent_id {
        q = q.bind(("a", a.to_string()));
    }
    if let Some(c) = category {
        q = q.bind(("c", c.to_string()));
    }
    let mut resp = q.await.map_err(|e| format!("list_memory_facts: {e}"))?;
    let rows: Vec<FactListRow> = resp
        .take(0)
        .map_err(|e| format!("list_memory_facts take: {e}"))?;
    Ok(rows
        .into_iter()
        .map(|r| {
            let id_str = match &r.id.key {
                surrealdb::types::RecordIdKey::String(s) => s.clone(),
                other => format!("{other:?}"),
            };
            // Emit the MemoryFactResponse-compatible shape so the HTTP
            // handler can deserialize directly. Fields not represented on
            // the Surreal table (scope, key, mention_count, source_summary)
            // get sensible defaults.
            serde_json::json!({
                "id": id_str,
                "agent_id": r.agent_id,
                "scope": "session",
                "category": r.fact_type,
                "key": "",
                "content": r.content,
                "confidence": r.confidence.unwrap_or(0.8),
                "mention_count": 0,
                "source_summary": null,
                "created_at": r.created_at
                    .map(|d| d.to_rfc3339())
                    .unwrap_or_default(),
                "updated_at": r.last_used_at
                    .map(|d| d.to_rfc3339())
                    .unwrap_or_default(),
            })
        })
        .collect())
}

#[derive(SurrealValue)]
#[surreal(crate = "surrealdb::types")]
struct FactListRow {
    id: surrealdb::types::RecordId,
    agent_id: String,
    content: String,
    fact_type: String,
    confidence: Option<f64>,
    created_at: Option<chrono::DateTime<chrono::Utc>>,
    last_used_at: Option<chrono::DateTime<chrono::Utc>>,
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
    use crate::{connect, schema::apply_schema, SurrealConfig};

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
        save_fact(
            &db,
            "a1",
            "preference",
            "k1",
            "Loves espresso drinks",
            0.9,
            None,
        )
        .await
        .unwrap();
        let result = recall_facts(&db, "a1", "espresso", 10).await.unwrap();
        let arr = result.as_array().unwrap();
        assert!(!arr.is_empty(), "should find match");
    }

    #[tokio::test]
    async fn list_memory_facts_filters_by_agent_and_category() {
        let db = fresh_db().await;
        save_fact(&db, "a1", "preference", "k1", "Alice fact A", 0.9, None)
            .await
            .unwrap();
        save_fact(&db, "a1", "skill", "k2", "Alice skill", 0.9, None)
            .await
            .unwrap();
        save_fact(&db, "a2", "preference", "k3", "Bob fact", 0.9, None)
            .await
            .unwrap();

        // Filter by agent only
        let a1_facts = list_memory_facts(&db, Some("a1"), None, None, 100, 0)
            .await
            .unwrap();
        assert_eq!(a1_facts.len(), 2);

        // Filter by agent + category
        let a1_pref = list_memory_facts(&db, Some("a1"), Some("preference"), None, 100, 0)
            .await
            .unwrap();
        assert_eq!(a1_pref.len(), 1);
        assert_eq!(a1_pref[0]["category"], "preference");

        // Cross-agent
        let all = list_memory_facts(&db, None, None, None, 100, 0).await.unwrap();
        assert_eq!(all.len(), 3);
    }

    #[tokio::test]
    async fn list_memory_facts_emits_response_compatible_shape() {
        let db = fresh_db().await;
        save_fact(&db, "a1", "preference", "k1", "hello", 0.9, None)
            .await
            .unwrap();
        let rows = list_memory_facts(&db, Some("a1"), None, None, 10, 0)
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        // MemoryFactResponse fields the HTTP handler decodes
        assert!(row.get("id").is_some());
        assert_eq!(row["agent_id"], "a1");
        assert_eq!(row["scope"], "session");
        assert_eq!(row["category"], "preference");
        assert_eq!(row["key"], "");
        assert_eq!(row["content"], "hello");
        assert_eq!(row["mention_count"], 0);
        assert!(row.get("created_at").is_some());
        assert!(row.get("updated_at").is_some());
    }

    #[tokio::test]
    async fn recall_respects_agent_isolation() {
        let db = fresh_db().await;
        save_fact(
            &db,
            "a1",
            "preference",
            "k1",
            "Coffee preference",
            0.9,
            None,
        )
        .await
        .unwrap();
        save_fact(
            &db,
            "a2",
            "preference",
            "k1",
            "Coffee preference",
            0.9,
            None,
        )
        .await
        .unwrap();
        let result = recall_facts(&db, "a1", "coffee", 10).await.unwrap();
        let arr = result.as_array().unwrap();
        assert!(arr.iter().all(|f| f["agent_id"] == "a1"));
    }
}
