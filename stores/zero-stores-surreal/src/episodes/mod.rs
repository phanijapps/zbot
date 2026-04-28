//! `SurrealEpisodeStore` — `EpisodeStore` impl over `Arc<Surreal<Any>>`.
//!
//! Backs the `episode` table (declared SCHEMALESS in `memory_kg.surql`).
//! Embeddings live inline on the row as `array<float>` — vector-similarity
//! search uses brute-force cosine over fetched candidates rather than the
//! lazy-defined HNSW index, mirroring the conservative approach in
//! `memory/fact.rs` (HNSW for memory_fact is also a follow-up).

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use surrealdb::engine::any::Any;
use surrealdb::Surreal;
use zero_stores_domain::SessionEpisode;
use zero_stores_traits::{EpisodeStats, EpisodeStore};

#[derive(Clone)]
pub struct SurrealEpisodeStore {
    db: Arc<Surreal<Any>>,
}

impl SurrealEpisodeStore {
    pub fn new(db: Arc<Surreal<Any>>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl EpisodeStore for SurrealEpisodeStore {
    async fn list_by_ward(&self, ward_id: &str, limit: usize) -> Result<Vec<Value>, String> {
        let q = format!(
            "SELECT * FROM episode WHERE ward_id = $w \
             ORDER BY created_at DESC LIMIT {limit}"
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
        Ok(rows.into_iter().map(row_to_episode_value).collect())
    }

    async fn insert_episode(
        &self,
        episode: Value,
        embedding: Option<Vec<f32>>,
    ) -> Result<String, String> {
        let mut typed: SessionEpisode = serde_json::from_value(episode)
            .map_err(|e| format!("decode SessionEpisode: {e}"))?;
        if embedding.is_some() {
            typed.embedding = embedding;
        }

        // UNIQUE(session_id) on the SQLite side is mirrored by the unique
        // index on `episode.session_id`; on conflict the SQLite repo
        // updates the mutable fields and returns the *existing* row's id.
        // Replicate that: probe by session_id first; if hit, UPDATE in
        // place; otherwise CREATE.
        let mut probe = self
            .db
            .query("SELECT id FROM episode WHERE session_id = $sid LIMIT 1")
            .bind(("sid", typed.session_id.clone()))
            .await
            .map_err(|e| format!("insert_episode probe: {e}"))?;
        let existing: Vec<Value> = probe
            .take(0)
            .map_err(|e| format!("insert_episode probe take: {e}"))?;

        let persisted_id = match existing.into_iter().next() {
            Some(prev) => {
                let flat = crate::row_value::flatten_record_id(prev);
                let prev_id = flat["id"].as_str().unwrap_or_default().to_string();
                let thing = surrealdb::types::RecordId::new(
                    "episode",
                    surrealdb::types::RecordIdKey::String(prev_id.clone()),
                );
                self.db
                    .query(
                        "UPDATE $id SET \
                         task_summary = $ts, \
                         outcome = $oc, \
                         strategy_used = $st, \
                         key_learnings = $kl, \
                         token_cost = $tc, \
                         embedding = $emb",
                    )
                    .bind(("id", thing))
                    .bind(("ts", typed.task_summary.clone()))
                    .bind(("oc", typed.outcome.clone()))
                    .bind(("st", typed.strategy_used.clone()))
                    .bind(("kl", typed.key_learnings.clone()))
                    .bind(("tc", typed.token_cost))
                    .bind(("emb", typed.embedding.clone()))
                    .await
                    .map_err(|e| format!("insert_episode update: {e}"))?;
                prev_id
            }
            None => {
                let thing = surrealdb::types::RecordId::new(
                    "episode",
                    surrealdb::types::RecordIdKey::String(typed.id.clone()),
                );
                let payload = build_episode_payload(&typed);
                self.db
                    .query("CREATE $id CONTENT $e")
                    .bind(("id", thing))
                    .bind(("e", payload))
                    .await
                    .map_err(|e| format!("insert_episode create: {e}"))?;
                typed.id.clone()
            }
        };
        Ok(persisted_id)
    }

    async fn search_episodes_by_similarity(
        &self,
        agent_id: &str,
        embedding: &[f32],
        threshold: f32,
        limit: usize,
    ) -> Result<Vec<Value>, String> {
        // No HNSW index yet — scan rows with embeddings for this agent and
        // score in Rust. For the SCHEMALESS table the embedding is stored
        // inline on the row. Caller is required to pass an L2-normalized
        // vector (same contract as the SQLite path).
        let mut resp = self
            .db
            .query("SELECT * FROM episode WHERE agent_id = $a")
            .bind(("a", agent_id.to_string()))
            .await
            .map_err(|e| format!("search_episodes_by_similarity: {e}"))?;
        let rows: Vec<Value> = resp
            .take(0)
            .map_err(|e| format!("search_episodes_by_similarity take: {e}"))?;

        let mut scored: Vec<(Value, f64)> = rows
            .into_iter()
            .filter_map(|r| {
                let emb_arr = r.get("embedding")?.as_array()?;
                let row_emb: Vec<f32> = emb_arr
                    .iter()
                    .filter_map(|x| x.as_f64().map(|f| f as f32))
                    .collect();
                let score = crate::similarity::cosine(embedding, &row_emb)?;
                if score >= threshold as f64 {
                    Some((row_to_episode_value(r), score))
                } else {
                    None
                }
            })
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit);
        Ok(scored
            .into_iter()
            .map(|(ep, score)| {
                serde_json::json!({
                    "episode": ep,
                    "score": score,
                })
            })
            .collect())
    }

    async fn fetch_recent_successful_by_ward(
        &self,
        ward_id: &str,
        limit: usize,
    ) -> Result<Vec<Value>, String> {
        // 14-day window matches the SQLite repo's literal cutoff.
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(14)).to_rfc3339();
        let q = format!(
            "SELECT * FROM episode \
             WHERE ward_id = $w \
               AND outcome IN ['success', 'partial'] \
               AND created_at > $cutoff \
             ORDER BY created_at DESC LIMIT {limit}"
        );
        let mut resp = self
            .db
            .query(q)
            .bind(("w", ward_id.to_string()))
            .bind(("cutoff", cutoff))
            .await
            .map_err(|e| format!("fetch_recent_successful_by_ward: {e}"))?;
        let rows: Vec<Value> = resp
            .take(0)
            .map_err(|e| format!("fetch_recent_successful_by_ward take: {e}"))?;
        Ok(rows.into_iter().map(row_to_episode_value).collect())
    }

    async fn episode_stats(&self) -> Result<EpisodeStats, String> {
        let mut resp = self
            .db
            .query("SELECT count() AS n FROM episode GROUP ALL")
            .await
            .map_err(|e| format!("episode_stats: {e}"))?;
        let rows: Vec<Value> = resp
            .take(0)
            .map_err(|e| format!("episode_stats take: {e}"))?;
        let total = rows
            .first()
            .and_then(|v| v.get("n"))
            .and_then(|n| n.as_i64())
            .unwrap_or(0);
        Ok(EpisodeStats { total })
    }
}

/// Strip the `embedding` field (callers don't expect it on read — domain
/// type marks it `serde(skip)`) and flatten the record id.
fn row_to_episode_value(row: Value) -> Value {
    let mut flat = crate::row_value::flatten_record_id(row);
    if let Some(obj) = flat.as_object_mut() {
        obj.remove("embedding");
    }
    flat
}

fn build_episode_payload(ep: &SessionEpisode) -> Value {
    serde_json::json!({
        "session_id": ep.session_id,
        "agent_id": ep.agent_id,
        "ward_id": ep.ward_id,
        "task_summary": ep.task_summary,
        "outcome": ep.outcome,
        "strategy_used": ep.strategy_used,
        "key_learnings": ep.key_learnings,
        "token_cost": ep.token_cost,
        "embedding": ep.embedding,
        "created_at": ep.created_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{connect, schema::apply_schema, SurrealConfig};

    async fn fresh_store() -> SurrealEpisodeStore {
        let cfg = SurrealConfig {
            url: "mem://".into(),
            namespace: "memory_kg".into(),
            database: "main".into(),
            credentials: None,
        };
        let db = connect(&cfg, None).await.expect("connect");
        apply_schema(&db).await.expect("schema");
        SurrealEpisodeStore::new(db)
    }

    fn normalized(v: Vec<f32>) -> Vec<f32> {
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm < 1e-9 {
            v
        } else {
            v.into_iter().map(|x| x / norm).collect()
        }
    }

    fn sample_episode(id: &str, session_id: &str, agent: &str, ward: &str, outcome: &str) -> Value {
        serde_json::json!({
            "id": id,
            "session_id": session_id,
            "agent_id": agent,
            "ward_id": ward,
            "task_summary": format!("task for {session_id}"),
            "outcome": outcome,
            "strategy_used": "direct",
            "key_learnings": "learning",
            "token_cost": 100,
            "created_at": chrono::Utc::now().to_rfc3339(),
        })
    }

    #[tokio::test]
    async fn insert_then_list_by_ward() {
        let store = fresh_store().await;
        store
            .insert_episode(
                sample_episode("ep-1", "sess-1", "root", "wardA", "success"),
                None,
            )
            .await
            .unwrap();
        store
            .insert_episode(
                sample_episode("ep-2", "sess-2", "root", "wardA", "failed"),
                None,
            )
            .await
            .unwrap();
        let rows = store.list_by_ward("wardA", 10).await.unwrap();
        assert_eq!(rows.len(), 2);
    }

    #[tokio::test]
    async fn insert_is_idempotent_by_session() {
        let store = fresh_store().await;
        let first = store
            .insert_episode(
                sample_episode("ep-first", "sess-same", "root", "wardA", "partial"),
                None,
            )
            .await
            .unwrap();
        // Second insert for the same session_id with a different `id`
        // must update in place and return the original `id`.
        let second = store
            .insert_episode(
                sample_episode("ep-second-ghost", "sess-same", "root", "wardA", "success"),
                None,
            )
            .await
            .expect("second insert must upsert, not fail");
        assert_eq!(first, "ep-first");
        assert_eq!(second, "ep-first");

        let rows = store.list_by_ward("wardA", 10).await.unwrap();
        assert_eq!(rows.len(), 1, "must not duplicate the row");
        assert_eq!(rows[0]["outcome"], "success");
    }

    #[tokio::test]
    async fn similarity_search_finds_match() {
        let store = fresh_store().await;
        let emb = normalized(
            (0..16)
                .map(|i| if i == 0 { 1.0_f32 } else { 0.0_f32 })
                .collect(),
        );
        store
            .insert_episode(
                sample_episode("ep-1", "sess-1", "agent-x", "wardA", "success"),
                Some(emb.clone()),
            )
            .await
            .unwrap();
        let results = store
            .search_episodes_by_similarity("agent-x", &emb, 0.5, 10)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0]["score"].as_f64().unwrap() > 0.99);
    }

    #[tokio::test]
    async fn episode_stats_counts_rows() {
        let store = fresh_store().await;
        let stats = store.episode_stats().await.unwrap();
        assert_eq!(stats.total, 0);
        store
            .insert_episode(
                sample_episode("ep-1", "sess-1", "root", "wardA", "success"),
                None,
            )
            .await
            .unwrap();
        let stats = store.episode_stats().await.unwrap();
        assert_eq!(stats.total, 1);
    }
}
