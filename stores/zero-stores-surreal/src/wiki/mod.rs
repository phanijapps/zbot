//! `SurrealWikiStore` — `WikiStore` impl over `Arc<Surreal<Any>>`.
//!
//! Backs the `wiki_doc` table (declared SCHEMALESS in `memory_kg.surql`).
//! Hybrid search performs FTS via the `@@` operator (against
//! `wiki_doc_title_fts` / `wiki_doc_content_fts`) and unions the result
//! with brute-force cosine over inline embeddings, fused via reciprocal
//! rank — same fusion algorithm as the SQLite path.
//!
//! Per-row identity is `(ward_id, title)`. We de-dup defensively in
//! `upsert_article` rather than declaring a unique compound index — the
//! SQLite path is the source of truth for this schema constraint.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use surrealdb::engine::any::Any;
use surrealdb::Surreal;
use zero_stores_domain::WikiArticle;
use zero_stores_traits::{WikiStats, WikiStore};

#[derive(Clone)]
pub struct SurrealWikiStore {
    db: Arc<Surreal<Any>>,
}

impl SurrealWikiStore {
    pub fn new(db: Arc<Surreal<Any>>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl WikiStore for SurrealWikiStore {
    async fn list_articles(&self, ward_id: &str) -> Result<Vec<Value>, String> {
        let mut resp = self
            .db
            .query("SELECT * FROM wiki_doc WHERE ward_id = $w ORDER BY title")
            .bind(("w", ward_id.to_string()))
            .await
            .map_err(|e| format!("list_articles: {e}"))?;
        let rows: Vec<Value> = resp
            .take(0)
            .map_err(|e| format!("list_articles take: {e}"))?;
        Ok(rows.into_iter().map(row_to_article_value).collect())
    }

    async fn get_article(&self, ward_id: &str, title: &str) -> Result<Option<Value>, String> {
        let mut resp = self
            .db
            .query("SELECT * FROM wiki_doc WHERE ward_id = $w AND title = $t LIMIT 1")
            .bind(("w", ward_id.to_string()))
            .bind(("t", title.to_string()))
            .await
            .map_err(|e| format!("get_article: {e}"))?;
        let rows: Vec<Value> = resp.take(0).map_err(|e| format!("get_article take: {e}"))?;
        Ok(rows.into_iter().next().map(row_to_article_value))
    }

    async fn upsert_article(
        &self,
        article: Value,
        embedding: Option<Vec<f32>>,
    ) -> Result<(), String> {
        let mut typed: WikiArticle =
            serde_json::from_value(article).map_err(|e| format!("decode WikiArticle: {e}"))?;
        if embedding.is_some() {
            typed.embedding = embedding;
        }

        // Mirror the SQLite `ON CONFLICT(ward_id, title) DO UPDATE` path:
        // probe by (ward_id, title) and either update the existing row
        // (incrementing version) or create a new one with the supplied id.
        let mut probe = self
            .db
            .query("SELECT id, version FROM wiki_doc WHERE ward_id = $w AND title = $t LIMIT 1")
            .bind(("w", typed.ward_id.clone()))
            .bind(("t", typed.title.clone()))
            .await
            .map_err(|e| format!("upsert_article probe: {e}"))?;
        let existing: Vec<Value> = probe
            .take(0)
            .map_err(|e| format!("upsert_article probe take: {e}"))?;

        match existing.into_iter().next() {
            Some(prev) => {
                let flat = crate::row_value::flatten_record_id(prev);
                let prev_id = flat["id"].as_str().unwrap_or_default().to_string();
                let prev_version = flat.get("version").and_then(|v| v.as_i64()).unwrap_or(1);
                let thing = surrealdb::types::RecordId::new(
                    "wiki_doc",
                    surrealdb::types::RecordIdKey::String(prev_id),
                );
                self.db
                    .query(
                        "UPDATE $id SET \
                         content = $c, \
                         tags = $tags, \
                         source_fact_ids = $sfi, \
                         version = $ver, \
                         updated_at = $u, \
                         embedding = $emb",
                    )
                    .bind(("id", thing))
                    .bind(("c", typed.content.clone()))
                    .bind(("tags", typed.tags.clone()))
                    .bind(("sfi", typed.source_fact_ids.clone()))
                    .bind(("ver", prev_version + 1))
                    .bind(("u", typed.updated_at.clone()))
                    .bind(("emb", typed.embedding.clone()))
                    .await
                    .map_err(|e| format!("upsert_article update: {e}"))?;
            }
            None => {
                let thing = surrealdb::types::RecordId::new(
                    "wiki_doc",
                    surrealdb::types::RecordIdKey::String(typed.id.clone()),
                );
                let payload = build_wiki_payload(&typed);
                self.db
                    .query("CREATE $id CONTENT $w")
                    .bind(("id", thing))
                    .bind(("w", payload))
                    .await
                    .map_err(|e| format!("upsert_article create: {e}"))?;
            }
        }
        Ok(())
    }

    async fn delete_article(&self, ward_id: &str, title: &str) -> Result<bool, String> {
        let mut resp = self
            .db
            .query("DELETE wiki_doc WHERE ward_id = $w AND title = $t RETURN BEFORE")
            .bind(("w", ward_id.to_string()))
            .bind(("t", title.to_string()))
            .await
            .map_err(|e| format!("delete_article: {e}"))?;
        let rows: Vec<Value> = resp
            .take(0)
            .map_err(|e| format!("delete_article take: {e}"))?;
        Ok(!rows.is_empty())
    }

    async fn search_wiki_hybrid(
        &self,
        ward_id: Option<&str>,
        query: &str,
        limit: usize,
        query_embedding: Option<&[f32]>,
    ) -> Result<Vec<Value>, String> {
        let fts_rows = self.fts_branch(ward_id, query).await?;
        let vec_rows = self.vec_branch(ward_id, query_embedding).await?;
        Ok(fuse_rrf(fts_rows, vec_rows, limit))
    }

    async fn wiki_stats(&self) -> Result<WikiStats, String> {
        let mut resp = self
            .db
            .query("SELECT count() AS n FROM wiki_doc GROUP ALL")
            .await
            .map_err(|e| format!("wiki_stats: {e}"))?;
        let rows: Vec<Value> = resp.take(0).map_err(|e| format!("wiki_stats take: {e}"))?;
        let total = rows
            .first()
            .and_then(|v| v.get("n"))
            .and_then(|n| n.as_i64())
            .unwrap_or(0);
        Ok(WikiStats { total })
    }
}

impl SurrealWikiStore {
    /// Run the FTS branch of hybrid search and return raw rows.
    /// Empty query short-circuits to an empty vec.
    async fn fts_branch(&self, ward_id: Option<&str>, query: &str) -> Result<Vec<Value>, String> {
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let q = match ward_id {
            Some(_) => self
                .db
                .query(
                    "SELECT * FROM wiki_doc \
                     WHERE ward_id = $w \
                       AND (title @@ $q OR content @@ $q) \
                     LIMIT 50",
                )
                .bind(("q", query.to_string()))
                .bind(("w", ward_id.unwrap().to_string())),
            None => self
                .db
                .query(
                    "SELECT * FROM wiki_doc \
                     WHERE title @@ $q OR content @@ $q LIMIT 50",
                )
                .bind(("q", query.to_string())),
        };
        let mut resp = q
            .await
            .map_err(|e| format!("search_wiki_hybrid fts: {e}"))?;
        resp.take(0)
            .map_err(|e| format!("search_wiki_hybrid fts take: {e}"))
    }

    /// Run the vector branch — fetch ward-scoped rows and score in Rust.
    /// Returns `(row, score)` pairs.
    async fn vec_branch(
        &self,
        ward_id: Option<&str>,
        query_embedding: Option<&[f32]>,
    ) -> Result<Vec<(Value, f64)>, String> {
        let Some(emb) = query_embedding else {
            return Ok(Vec::new());
        };
        let q = match ward_id {
            Some(_) => self
                .db
                .query("SELECT * FROM wiki_doc WHERE ward_id = $w")
                .bind(("w", ward_id.unwrap().to_string())),
            None => self.db.query("SELECT * FROM wiki_doc"),
        };
        let mut resp = q
            .await
            .map_err(|e| format!("search_wiki_hybrid vec: {e}"))?;
        let candidates: Vec<Value> = resp
            .take(0)
            .map_err(|e| format!("search_wiki_hybrid vec take: {e}"))?;
        Ok(candidates
            .into_iter()
            .filter_map(|r| {
                let emb_arr = r.get("embedding")?.as_array()?;
                let row_emb: Vec<f32> = emb_arr
                    .iter()
                    .filter_map(|x| x.as_f64().map(|f| f as f32))
                    .collect();
                let score = crate::similarity::cosine(emb, &row_emb)?;
                Some((r, score))
            })
            .collect())
    }
}

/// Reciprocal-rank fusion (RRF k=60) — same as the SQLite path. Keys are
/// the row's id (post flatten); the row Value flows through unchanged so
/// `row_to_article_value` strips the embedding on the way out.
fn fuse_rrf(fts_rows: Vec<Value>, vec_rows: Vec<(Value, f64)>, limit: usize) -> Vec<Value> {
    let mut scored: std::collections::HashMap<String, (f32, &'static str, Value)> =
        std::collections::HashMap::new();
    for (rank, row) in fts_rows.into_iter().enumerate() {
        let key = id_key(&row);
        let s = 1.0 / (60.0 + rank as f32);
        scored.entry(key).or_insert((0.0, "fts", row)).0 += s;
    }
    let mut sorted_vec = vec_rows;
    sorted_vec.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    for (rank, (row, _)) in sorted_vec.into_iter().enumerate() {
        let key = id_key(&row);
        let s = 1.0 / (60.0 + rank as f32);
        let slot = scored.entry(key).or_insert((0.0, "vec", row));
        slot.0 += s;
        if slot.1 == "fts" {
            slot.1 = "hybrid";
        }
    }

    let mut ranked: Vec<(f32, &'static str, Value)> = scored
        .into_iter()
        .map(|(_, (s, src, row))| (s, src, row))
        .collect();
    ranked.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    ranked.truncate(limit);

    ranked
        .into_iter()
        .map(|(score, src, row)| {
            let article = row_to_article_value(row);
            serde_json::json!({
                "article": article,
                "score": score,
                "match_source": src,
            })
        })
        .collect()
}

/// Read the id key off a raw Surreal row Value.
fn id_key(row: &Value) -> String {
    row.get("id")
        .map(|v| {
            if let Some(s) = v.as_str() {
                if let Some((_, k)) = s.split_once(':') {
                    k.trim_matches('`').to_string()
                } else {
                    s.trim_matches('`').to_string()
                }
            } else {
                v.to_string()
            }
        })
        .unwrap_or_default()
}

/// Strip the `embedding` field (WikiArticle marks it `serde(skip)`) and
/// flatten the record id.
fn row_to_article_value(row: Value) -> Value {
    let mut flat = crate::row_value::flatten_record_id(row);
    if let Some(obj) = flat.as_object_mut() {
        obj.remove("embedding");
    }
    flat
}

fn build_wiki_payload(a: &WikiArticle) -> Value {
    serde_json::json!({
        "agent_id": a.agent_id,
        "ward_id": a.ward_id,
        "title": a.title,
        "content": a.content,
        "tags": a.tags,
        "source_fact_ids": a.source_fact_ids,
        "version": a.version,
        "embedding": a.embedding,
        "created_at": a.created_at,
        "updated_at": a.updated_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{connect, schema::apply_schema, SurrealConfig};

    async fn fresh_store() -> SurrealWikiStore {
        let cfg = SurrealConfig {
            url: "mem://".into(),
            namespace: "memory_kg".into(),
            database: "main".into(),
            credentials: None,
        };
        let db = connect(&cfg, None).await.expect("connect");
        apply_schema(&db).await.expect("schema");
        SurrealWikiStore::new(db)
    }

    fn sample_article(id: &str, ward: &str, title: &str, content: &str) -> Value {
        let now = chrono::Utc::now().to_rfc3339();
        serde_json::json!({
            "id": id,
            "ward_id": ward,
            "agent_id": "root",
            "title": title,
            "content": content,
            "tags": null,
            "source_fact_ids": null,
            "version": 1,
            "created_at": now,
            "updated_at": now,
        })
    }

    #[tokio::test]
    async fn upsert_then_get_article() {
        let store = fresh_store().await;
        store
            .upsert_article(
                sample_article("a1", "wardA", "topic-1", "hello world"),
                None,
            )
            .await
            .unwrap();
        let fetched = store
            .get_article("wardA", "topic-1")
            .await
            .unwrap()
            .expect("present");
        assert_eq!(fetched["title"], "topic-1");
        assert_eq!(fetched["content"], "hello world");
    }

    #[tokio::test]
    async fn upsert_increments_version_on_conflict() {
        let store = fresh_store().await;
        store
            .upsert_article(sample_article("a1", "wardA", "topic-1", "v1"), None)
            .await
            .unwrap();
        // Second upsert with a different `id` — must update the existing
        // row in place and bump version.
        store
            .upsert_article(sample_article("a2", "wardA", "topic-1", "v2"), None)
            .await
            .unwrap();

        let rows = store.list_articles("wardA").await.unwrap();
        assert_eq!(rows.len(), 1, "must not duplicate the row");
        assert_eq!(rows[0]["content"], "v2");
        assert_eq!(rows[0]["version"], 2);
    }

    #[tokio::test]
    async fn list_and_delete_article() {
        let store = fresh_store().await;
        store
            .upsert_article(sample_article("a1", "wardA", "topic-1", "x"), None)
            .await
            .unwrap();
        store
            .upsert_article(sample_article("a2", "wardA", "topic-2", "y"), None)
            .await
            .unwrap();
        let rows = store.list_articles("wardA").await.unwrap();
        assert_eq!(rows.len(), 2);

        let deleted = store.delete_article("wardA", "topic-1").await.unwrap();
        assert!(deleted);
        assert_eq!(store.list_articles("wardA").await.unwrap().len(), 1);
        assert!(!store.delete_article("wardA", "nope").await.unwrap());
    }

    #[tokio::test]
    async fn wiki_stats_counts_rows() {
        let store = fresh_store().await;
        assert_eq!(store.wiki_stats().await.unwrap().total, 0);
        store
            .upsert_article(sample_article("a1", "wardA", "topic-1", "x"), None)
            .await
            .unwrap();
        assert_eq!(store.wiki_stats().await.unwrap().total, 1);
    }

    #[tokio::test]
    async fn search_wiki_hybrid_fts_finds_match() {
        let store = fresh_store().await;
        store
            .upsert_article(sample_article("a1", "wardA", "coffee brewing", "How to brew pour-over coffee"), None)
            .await
            .unwrap();
        store
            .upsert_article(sample_article("a2", "wardA", "tea guide", "Herbal tea at night"), None)
            .await
            .unwrap();
        let results = store
            .search_wiki_hybrid(Some("wardA"), "coffee", 10, None)
            .await
            .unwrap();
        assert!(
            results.iter().any(|r| r["article"]["title"].as_str().unwrap_or("").contains("coffee")),
            "FTS should find the coffee article"
        );
    }

    #[tokio::test]
    async fn search_wiki_hybrid_empty_query_returns_nothing() {
        let store = fresh_store().await;
        store
            .upsert_article(sample_article("a1", "wardA", "topic", "content"), None)
            .await
            .unwrap();
        let results = store
            .search_wiki_hybrid(Some("wardA"), "", 10, None)
            .await
            .unwrap();
        assert!(results.is_empty(), "empty query should return nothing");
    }

    #[tokio::test]
    async fn search_wiki_hybrid_with_embedding() {
        let store = fresh_store().await;
        let mut article = sample_article("a1", "wardA", "embedded topic", "content with vector");
        article.as_object_mut().unwrap().insert(
            "embedding".to_string(),
            serde_json::json!([1.0, 0.0, 0.0]),
        );
        store.upsert_article(article, None).await.unwrap();
        let query_emb: Vec<f32> = vec![1.0, 0.0, 0.0];
        let results = store
            .search_wiki_hybrid(Some("wardA"), "topic", 10, Some(&query_emb))
            .await
            .unwrap();
        assert!(!results.is_empty(), "should find article via embedding");
        let found = &results[0];
        assert!(found.get("score").is_some());
        assert!(found.get("match_source").is_some());
    }

    #[tokio::test]
    async fn search_wiki_hybrid_no_ward_filter() {
        let store = fresh_store().await;
        store
            .upsert_article(sample_article("a1", "wardA", "global topic", "shared content"), None)
            .await
            .unwrap();
        let results = store
            .search_wiki_hybrid(None, "shared", 10, None)
            .await
            .unwrap();
        assert!(!results.is_empty(), "should find without ward filter");
    }
}
