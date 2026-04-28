//! `SurrealRecallLogStore` — `RecallLogStore` impl over `Arc<Surreal<Any>>`.
//!
//! Backs the `recall_log` table (declared SCHEMALESS in `memory_kg.surql`).
//! `(session_id, fact_key)` is the dedup key; `log_recall` is idempotent
//! via a unique compound index.

use std::sync::Arc;

use async_trait::async_trait;
use surrealdb::engine::any::Any;
use surrealdb::types::SurrealValue;
use surrealdb::Surreal;
use zero_stores_traits::RecallLogStore;

#[derive(Clone)]
pub struct SurrealRecallLogStore {
    db: Arc<Surreal<Any>>,
}

impl SurrealRecallLogStore {
    pub fn new(db: Arc<Surreal<Any>>) -> Self {
        Self { db }
    }
}

#[derive(SurrealValue)]
#[surreal(crate = "surrealdb::types")]
struct KeyRow {
    fact_key: String,
}

#[async_trait]
impl RecallLogStore for SurrealRecallLogStore {
    async fn log_recall(&self, session_id: &str, fact_key: &str) -> Result<(), String> {
        // The unique index on (session_id, fact_key) makes a duplicate INSERT
        // fail with "already exists" — swallow that error so the call is
        // idempotent (matches the SQLite `INSERT OR IGNORE` semantics).
        let now = chrono::Utc::now().to_rfc3339();
        let result = self
            .db
            .query(
                "CREATE recall_log SET \
                 session_id = $sid, fact_key = $fk, recalled_at = $ts",
            )
            .bind(("sid", session_id.to_string()))
            .bind(("fk", fact_key.to_string()))
            .bind(("ts", now))
            .await;
        match result {
            Ok(_) => Ok(()),
            Err(e) => {
                let msg = format!("{e}");
                if msg.contains("already") || msg.contains("Index") || msg.contains("unique") {
                    Ok(())
                } else {
                    Err(format!("log_recall: {e}"))
                }
            }
        }
    }

    async fn get_keys_for_session(&self, session_id: &str) -> Result<Vec<String>, String> {
        let mut resp = self
            .db
            .query("SELECT fact_key FROM recall_log WHERE session_id = $sid")
            .bind(("sid", session_id.to_string()))
            .await
            .map_err(|e| format!("get_keys_for_session: {e}"))?;
        let rows: Vec<KeyRow> = resp
            .take(0)
            .map_err(|e| format!("get_keys_for_session take: {e}"))?;
        Ok(rows.into_iter().map(|r| r.fact_key).collect())
    }

    async fn get_keys_for_sessions(
        &self,
        session_ids: &[String],
    ) -> Result<Vec<String>, String> {
        if session_ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut resp = self
            .db
            .query("SELECT fact_key FROM recall_log WHERE session_id IN $sids")
            .bind(("sids", session_ids.to_vec()))
            .await
            .map_err(|e| format!("get_keys_for_sessions: {e}"))?;
        let rows: Vec<KeyRow> = resp
            .take(0)
            .map_err(|e| format!("get_keys_for_sessions take: {e}"))?;
        // Trait contract is "list of distinct keys" (not per-key counts), so
        // collapse via HashSet on the way out.
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut out = Vec::new();
        for r in rows {
            if seen.insert(r.fact_key.clone()) {
                out.push(r.fact_key);
            }
        }
        Ok(out)
    }
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
        let db = connect(&cfg, None).await.expect("connect");
        apply_schema(&db).await.expect("schema");
        db
    }

    #[tokio::test]
    async fn log_and_get_keys_for_session() {
        let db = fresh_db().await;
        let store = SurrealRecallLogStore::new(db);
        store.log_recall("sess-1", "user::name").await.unwrap();
        store.log_recall("sess-1", "user::email").await.unwrap();
        let keys = store.get_keys_for_session("sess-1").await.unwrap();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"user::name".to_string()));
        assert!(keys.contains(&"user::email".to_string()));
    }

    #[tokio::test]
    async fn log_recall_is_idempotent() {
        let db = fresh_db().await;
        let store = SurrealRecallLogStore::new(db);
        store.log_recall("sess-1", "user::name").await.unwrap();
        store.log_recall("sess-1", "user::name").await.unwrap();
        let keys = store.get_keys_for_session("sess-1").await.unwrap();
        assert_eq!(keys.len(), 1);
    }

    #[tokio::test]
    async fn get_keys_for_sessions_distinct_across_sessions() {
        let db = fresh_db().await;
        let store = SurrealRecallLogStore::new(db);
        store.log_recall("sess-1", "user::name").await.unwrap();
        store.log_recall("sess-2", "user::name").await.unwrap();
        store.log_recall("sess-2", "user::email").await.unwrap();

        let sessions = vec!["sess-1".to_string(), "sess-2".to_string()];
        let keys = store.get_keys_for_sessions(&sessions).await.unwrap();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"user::name".to_string()));
        assert!(keys.contains(&"user::email".to_string()));

        // Empty input
        let empty = store.get_keys_for_sessions(&[]).await.unwrap();
        assert!(empty.is_empty());
    }
}
