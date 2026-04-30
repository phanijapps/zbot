//! MemoryFactStore implementation backed by the `memory_fact` SurrealDB table.

use std::sync::Arc;

use serde_json::Value;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use surrealdb::types::SurrealValue;
use zero_stores_traits::{StrategyFactInsert, StrategyFactMatch};

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
    // Write created_at/last_used_at as ISO-8601 strings to keep the column
    // shape consistent with `upsert_typed_fact` (which writes the typed
    // MemoryFact JSON whose timestamps are strings). The SurrealDB schema's
    // `DEFAULT time::now()` would otherwise coerce these into datetimes,
    // breaking deserialization on read since the table now contains a mix
    // of types.
    let now = chrono::Utc::now().to_rfc3339();
    db.query(
        "CREATE memory_fact SET \
         agent_id = $a, content = $c, fact_type = $ft, confidence = $conf, \
         archived = false, created_at = $now, last_used_at = $now",
    )
    .bind(("a", agent_id.to_string()))
    .bind(("c", content.to_string()))
    .bind(("ft", category.to_string()))
    .bind(("conf", confidence))
    .bind(("now", now))
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
    //
    // SELECT * — the table is SCHEMALESS, so rows may carry any subset of
    // the canonical MemoryFact fields (ward_id, key, pinned, etc.).
    // Returning the whole row lets handlers that filter on ward_id /
    // category / pinned see those fields. Filtering on `archived = false`
    // matches both rows that explicitly set it (the typical case post
    // `upsert_typed_fact` normalization) and absent rows because the
    // typed shape always sets it.
    let mut where_clauses: Vec<&'static str> = vec!["archived = false"];
    if agent_id.is_some() {
        where_clauses.push("agent_id = $a");
    }
    if category.is_some() {
        where_clauses.push("(fact_type = $c OR category = $c)");
    }
    let where_sql = where_clauses.join(" AND ");
    let q = format!(
        "SELECT * FROM memory_fact WHERE {where_sql} \
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
    let rows: Vec<Value> = resp
        .take(0)
        .map_err(|e| format!("list_memory_facts take: {e}"))?;

    // Normalize each row to the MemoryFactResponse-compatible shape so
    // the HTTP handler can deserialize directly into MemoryFact. Rows
    // written by upsert_typed_fact already carry the canonical shape
    // (with `category`); rows from the legacy `save_fact` path carry
    // `fact_type` instead — copy it across when `category` is missing.
    let normalized: Vec<Value> = rows
        .into_iter()
        .map(|mut row| {
            // Surreal returns `id` as a Thing (object); flatten to string.
            row = crate::row_value::flatten_record_id(row);
            if let Some(obj) = row.as_object_mut() {
                if !obj.contains_key("category") {
                    if let Some(ft) = obj.get("fact_type").cloned() {
                        obj.insert("category".to_string(), ft);
                    }
                }
                // Defaults for fields that may be absent on legacy rows
                // so MemoryFact deserialization succeeds.
                obj.entry("scope".to_string())
                    .or_insert_with(|| Value::String("session".to_string()));
                obj.entry("key".to_string())
                    .or_insert_with(|| Value::String(String::new()));
                obj.entry("mention_count".to_string())
                    .or_insert_with(|| Value::from(0));
                obj.entry("source_summary".to_string())
                    .or_insert(Value::Null);
                obj.entry("ward_id".to_string())
                    .or_insert_with(|| Value::String("__global__".to_string()));
                obj.entry("pinned".to_string())
                    .or_insert(Value::Bool(false));
                if !obj.contains_key("updated_at") {
                    if let Some(lu) = obj.get("last_used_at").cloned() {
                        obj.insert("updated_at".to_string(), lu);
                    }
                }
            }
            row
        })
        .collect();

    Ok(normalized)
}

// Accept both string and datetime for created_at/last_used_at since the
// SurrealDB-side memory_fact table is SCHEMALESS — rows written by
// upsert_typed_fact carry MemoryFact.{created_at,updated_at} as ISO strings,
// while older Surreal-native writes (e.g. via save_fact) used datetime
// defaults. Storing as Option<String> tolerates both.
#[derive(SurrealValue)]
#[surreal(crate = "surrealdb::types")]
struct FactListRow {
    id: surrealdb::types::RecordId,
    agent_id: String,
    content: String,
    fact_type: String,
    confidence: Option<f64>,
    created_at: Option<String>,
    last_used_at: Option<String>,
}

pub async fn get_memory_fact_by_id(
    db: &Arc<Surreal<Any>>,
    fact_id: &str,
) -> Result<Option<Value>, String> {
    let thing = surrealdb::types::RecordId::new(
        "memory_fact",
        surrealdb::types::RecordIdKey::String(fact_id.to_string()),
    );
    let mut resp = db
        .query(
            "SELECT id, agent_id, content, fact_type, confidence, \
             created_at, last_used_at FROM ONLY $id",
        )
        .bind(("id", thing))
        .await
        .map_err(|e| format!("get_memory_fact_by_id: {e}"))?;
    let row: Option<FactListRow> = resp
        .take(0)
        .map_err(|e| format!("get_memory_fact_by_id take: {e}"))?;
    Ok(row.map(|r| {
        let id_str = match &r.id.key {
            surrealdb::types::RecordIdKey::String(s) => s.clone(),
            other => format!("{other:?}"),
        };
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
            "created_at": r.created_at.unwrap_or_default(),
            "updated_at": r.last_used_at.unwrap_or_default(),
        })
    }))
}

pub async fn delete_memory_fact(db: &Arc<Surreal<Any>>, fact_id: &str) -> Result<bool, String> {
    let thing = surrealdb::types::RecordId::new(
        "memory_fact",
        surrealdb::types::RecordIdKey::String(fact_id.to_string()),
    );
    let mut resp = db
        .query("DELETE $id RETURN BEFORE")
        .bind(("id", thing))
        .await
        .map_err(|e| format!("delete_memory_fact: {e}"))?;
    let rows: Vec<Value> = resp
        .take(0)
        .map_err(|e| format!("delete_memory_fact take: {e}"))?;
    Ok(!rows.is_empty())
}

pub async fn upsert_typed_fact(
    db: &Arc<Surreal<Any>>,
    mut fact: Value,
    embedding: Option<Vec<f32>>,
) -> Result<(), String> {
    let fact_id = fact
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "MemoryFact missing id".to_string())?
        .to_string();

    // Normalize the row to the Surreal-canonical shape so the existing
    // indexes (`fact_agent_type`, `fact_archived`) and the
    // `list_memory_facts` / `recall_facts` queries match. Callers
    // pass the SQLite `MemoryFact` JSON which uses `category` —
    // mirror it into `fact_type` and default `archived = false`
    // when not set so the row participates in the standard filter.
    if let Some(obj) = fact.as_object_mut() {
        if !obj.contains_key("fact_type") {
            if let Some(cat) = obj.get("category").cloned() {
                obj.insert("fact_type".to_string(), cat);
            }
        }
        if !obj.contains_key("archived") {
            obj.insert("archived".to_string(), Value::Bool(false));
        }
        // `last_used_at` mirrors `updated_at` for compatibility with
        // recall_facts' ORDER BY (when present).
        if !obj.contains_key("last_used_at") {
            if let Some(updated) = obj.get("updated_at").cloned() {
                obj.insert("last_used_at".to_string(), updated);
            }
        }
        if let Some(emb) = embedding {
            obj.insert(
                "embedding".to_string(),
                Value::Array(emb.into_iter().map(|f| serde_json::json!(f)).collect()),
            );
        }
    }

    let thing = surrealdb::types::RecordId::new(
        "memory_fact",
        surrealdb::types::RecordIdKey::String(fact_id),
    );
    db.query("UPSERT $id CONTENT $fact")
        .bind(("id", thing))
        .bind(("fact", fact))
        .await
        .map_err(|e| format!("upsert_typed_fact: {e}"))?;
    Ok(())
}

pub async fn supersede_fact(
    db: &Arc<Surreal<Any>>,
    old_id: &str,
    new_id: &str,
) -> Result<(), String> {
    let thing = surrealdb::types::RecordId::new(
        "memory_fact",
        surrealdb::types::RecordIdKey::String(old_id.to_string()),
    );
    db.query("UPDATE $id SET superseded_by = $new_id, last_used_at = time::now()")
        .bind(("id", thing))
        .bind(("new_id", new_id.to_string()))
        .await
        .map_err(|e| format!("supersede_fact: {e}"))?;
    Ok(())
}

pub async fn archive_fact(db: &Arc<Surreal<Any>>, fact_id: &str) -> Result<bool, String> {
    let thing = surrealdb::types::RecordId::new(
        "memory_fact",
        surrealdb::types::RecordIdKey::String(fact_id.to_string()),
    );
    let mut resp = db
        .query("UPDATE $id SET archived = true RETURN AFTER")
        .bind(("id", thing))
        .await
        .map_err(|e| format!("archive_fact: {e}"))?;
    let rows: Vec<Value> = resp
        .take(0)
        .map_err(|e| format!("archive_fact take: {e}"))?;
    Ok(!rows.is_empty())
}

pub async fn search_memory_facts_hybrid(
    db: &Arc<Surreal<Any>>,
    agent_id: Option<&str>,
    query: &str,
    mode: &str,
    limit: usize,
    ward_id: Option<&str>,
    _query_embedding: Option<&[f32]>,
) -> Result<Vec<Value>, String> {
    // Surreal-side hybrid is FTS-only for now: the @@ FULLTEXT operator
    // gives us keyword retrieval. Vector + RRF fusion is a follow-up
    // (DEFINE INDEX HNSW + KNN scoring blended with FTS rank — non-trivial
    // SurrealQL). semantic-only mode falls back to the same FTS path so
    // search continues to work on Surreal; "match_source" labels reflect
    // the requested mode so callers see consistent shapes across backends.
    let mut clauses: Vec<&'static str> = vec!["archived = false"];
    if !query.is_empty() {
        clauses.push("content @@ $q");
    }
    if agent_id.is_some() {
        clauses.push("agent_id = $a");
    }
    if ward_id.is_some() {
        clauses.push("ward_id = $w");
    }
    let where_sql = clauses.join(" AND ");
    let q_sql = format!(
        "SELECT id, agent_id, content, fact_type, confidence, \
         created_at, last_used_at, ward_id \
         FROM memory_fact WHERE {where_sql} LIMIT {limit}"
    );
    let mut q = db.query(q_sql);
    if !query.is_empty() {
        q = q.bind(("q", query.to_string()));
    }
    if let Some(a) = agent_id {
        q = q.bind(("a", a.to_string()));
    }
    if let Some(w) = ward_id {
        q = q.bind(("w", w.to_string()));
    }
    let mut resp = q
        .await
        .map_err(|e| format!("search_memory_facts_hybrid: {e}"))?;
    let rows: Vec<FactSearchRow> = resp.take(0).map_err(|e| format!("search take: {e}"))?;
    let src = match mode {
        "fts" => "fts",
        "semantic" => "vec",
        _ => "hybrid",
    };
    Ok(rows
        .into_iter()
        .map(|r| {
            let id_str = match &r.id.key {
                surrealdb::types::RecordIdKey::String(s) => s.clone(),
                other => format!("{other:?}"),
            };
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
                "ward_id": r.ward_id.unwrap_or_default(),
                "created_at": r.created_at.unwrap_or_default(),
                "updated_at": r.last_used_at.unwrap_or_default(),
                "match_source": src,
            })
        })
        .collect())
}

#[derive(SurrealValue)]
#[surreal(crate = "surrealdb::types")]
struct FactSearchRow {
    id: surrealdb::types::RecordId,
    agent_id: String,
    content: String,
    fact_type: String,
    confidence: Option<f64>,
    created_at: Option<String>,
    last_used_at: Option<String>,
    ward_id: Option<String>,
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

// ============================================================================
// Sleep-time synthesis (Phase D4)
// ============================================================================

/// Find an existing strategy fact whose embedding's cosine similarity
/// with `embedding` is at or above `threshold`. Pulls up to `scan_limit`
/// candidate facts in `category = "strategy"` for the agent and scores
/// in Rust — Surreal's HNSW isn't yet wired for `memory_fact` so this
/// mirrors the conservative scan used elsewhere in this module.
pub async fn find_strategy_fact_by_similarity(
    db: &Arc<Surreal<Any>>,
    agent_id: &str,
    embedding: &[f32],
    threshold: f32,
    scan_limit: usize,
) -> Result<Option<StrategyFactMatch>, String> {
    let q = format!(
        "SELECT id, source_episode_id, embedding FROM memory_fact \
         WHERE agent_id = $a \
           AND fact_type = 'strategy' \
           AND embedding IS NOT NONE \
           AND (archived = false OR archived IS NONE) \
         LIMIT {scan_limit}"
    );
    let mut resp = db
        .query(q)
        .bind(("a", agent_id.to_string()))
        .await
        .map_err(|e| format!("find_strategy_fact_by_similarity: {e}"))?;
    let rows: Vec<Value> = resp
        .take(0)
        .map_err(|e| format!("find_strategy_fact_by_similarity take: {e}"))?;
    for row in rows {
        let id_raw = match row.get("id").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        let stored: Vec<f32> = match row.get("embedding") {
            Some(Value::Array(arr)) => arr
                .iter()
                .filter_map(|x| x.as_f64().map(|f| f as f32))
                .collect(),
            _ => continue,
        };
        if stored.len() != embedding.len() {
            continue;
        }
        let sim = match crate::similarity::cosine(embedding, &stored) {
            Some(v) => v,
            None => continue,
        };
        if sim >= threshold as f64 {
            let fact_id = strip_thing_prefix(&id_raw);
            let source_episode_id = row
                .get("source_episode_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            return Ok(Some(StrategyFactMatch {
                fact_id,
                source_episode_id,
            }));
        }
    }
    Ok(None)
}

/// Bump an existing strategy fact's `mention_count` and replace its
/// `source_episode_id` CSV. `now_rfc3339` is recorded as `updated_at`.
pub async fn bump_strategy_fact_episodes(
    db: &Arc<Surreal<Any>>,
    fact_id: &str,
    merged_source_episode_id: &str,
    now_rfc3339: &str,
) -> Result<(), String> {
    let thing = surrealdb::types::RecordId::new(
        "memory_fact",
        surrealdb::types::RecordIdKey::String(fact_id.to_string()),
    );
    db.query(
        "UPDATE $id SET \
         mention_count = (mention_count OR 0) + 1, \
         updated_at = $u, \
         last_used_at = $u, \
         source_episode_id = $eids",
    )
    .bind(("id", thing))
    .bind(("u", now_rfc3339.to_string()))
    .bind(("eids", merged_source_episode_id.to_string()))
    .await
    .map_err(|e| format!("bump_strategy_fact_episodes: {e}"))?;
    Ok(())
}

/// Insert a synthesised strategy fact into `memory_fact`. Uses the
/// same row shape as `upsert_typed_fact` so the existing `recall_facts`
/// query continues to surface the row.
pub async fn insert_strategy_fact(
    db: &Arc<Surreal<Any>>,
    req: StrategyFactInsert,
) -> Result<String, String> {
    let id = format!("fact-{}", uuid::Uuid::new_v4());
    let now = chrono::Utc::now().to_rfc3339();
    let row = serde_json::json!({
        "id": id,
        "agent_id": req.agent_id,
        "scope": "agent",
        "category": "strategy",
        "fact_type": "strategy",
        "key": req.key,
        "content": req.content,
        "confidence": req.confidence,
        "mention_count": 1,
        "source_summary": req.source_summary,
        "embedding": req.embedding,
        "ward_id": "__global__",
        "created_at": now,
        "updated_at": now,
        "last_used_at": now,
        "epistemic_class": "convention",
        "source_episode_id": req.source_episode_id,
        "archived": false,
        "pinned": false,
    });
    let thing = surrealdb::types::RecordId::new(
        "memory_fact",
        surrealdb::types::RecordIdKey::String(id.clone()),
    );
    db.query("CREATE $id CONTENT $row")
        .bind(("id", thing))
        .bind(("row", row))
        .await
        .map_err(|e| format!("insert_strategy_fact: {e}"))?;
    Ok(id)
}

/// Strip Surreal's `<table>:<id>` wire prefix and any backticks.
fn strip_thing_prefix(s: &str) -> String {
    let after = match s.find(':') {
        Some(idx) => &s[idx + 1..],
        None => s,
    };
    let cleaned = after.strip_prefix('`').unwrap_or(after);
    cleaned.strip_suffix('`').unwrap_or(cleaned).to_string()
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
        let all = list_memory_facts(&db, None, None, None, 100, 0)
            .await
            .unwrap();
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
