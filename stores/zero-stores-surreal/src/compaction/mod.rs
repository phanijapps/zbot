//! `SurrealCompactionStore` — `CompactionStore` impl over Arc<Surreal<Any>>.
//!
//! Backs the `kg_compaction_run` SCHEMALESS table. Each maintenance
//! event (merge / synthesis / pattern / prune / archival) becomes one
//! row tagged with `run_id` so Observatory queries like
//! "how many merges did the last sleep cycle do?" stay one query.
//!
//! Implementation note: the trait was designed with default no-op
//! impls, so we only override the operations actually useful for
//! Surreal. Each method is one CREATE statement — no transaction
//! ceremony needed.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;
use surrealdb::engine::any::Any;
use surrealdb::Surreal;
use zero_stores_traits::{CompactionRunSummary, CompactionStore};

#[derive(Clone)]
pub struct SurrealCompactionStore {
    db: Arc<Surreal<Any>>,
}

impl SurrealCompactionStore {
    pub fn new(db: Arc<Surreal<Any>>) -> Self {
        Self { db }
    }

    async fn insert_row(
        &self,
        operation: &str,
        run_id: &str,
        entity_id: Option<&str>,
        relationship_id: Option<&str>,
        merged_into: Option<&str>,
        reason: &str,
    ) -> Result<String, String> {
        let id = uuid::Uuid::new_v4().to_string();
        let payload = json!({
            "id": id.clone(),
            "run_id": run_id,
            "operation": operation,
            "entity_id": entity_id,
            "relationship_id": relationship_id,
            "merged_into": merged_into,
            "reason": reason,
            "created_at": chrono::Utc::now().to_rfc3339(),
        });
        let thing = surrealdb::types::RecordId::new(
            "kg_compaction_run",
            surrealdb::types::RecordIdKey::String(id.clone()),
        );
        self.db
            .query("CREATE $id CONTENT $r")
            .bind(("id", thing))
            .bind(("r", payload))
            .await
            .map_err(|e| format!("compaction insert: {e}"))?;
        Ok(id)
    }
}

#[async_trait]
impl CompactionStore for SurrealCompactionStore {
    async fn record_merge(
        &self,
        run_id: &str,
        loser_entity_id: &str,
        winner_entity_id: &str,
        reason: &str,
    ) -> Result<String, String> {
        self.insert_row(
            "merge",
            run_id,
            Some(loser_entity_id),
            None,
            Some(winner_entity_id),
            reason,
        )
        .await
    }

    async fn record_synthesis(
        &self,
        run_id: &str,
        fact_id: &str,
        reason: &str,
    ) -> Result<String, String> {
        self.insert_row("synthesis", run_id, Some(fact_id), None, None, reason)
            .await
    }

    async fn record_pattern(
        &self,
        run_id: &str,
        procedure_id: &str,
        reason: &str,
    ) -> Result<String, String> {
        self.insert_row("pattern", run_id, Some(procedure_id), None, None, reason)
            .await
    }

    async fn record_prune(
        &self,
        run_id: &str,
        entity_id: Option<&str>,
        relationship_id: Option<&str>,
        reason: &str,
    ) -> Result<String, String> {
        self.insert_row("prune", run_id, entity_id, relationship_id, None, reason)
            .await
    }

    async fn record_archival(
        &self,
        run_id: &str,
        entity_id: &str,
        reason: &str,
    ) -> Result<String, String> {
        self.insert_row("archival", run_id, Some(entity_id), None, None, reason)
            .await
    }

    async fn latest_run_summary(&self) -> Result<Option<CompactionRunSummary>, String> {
        // Latest row globally — its run_id and created_at give the
        // "most recent run" anchor. Cheaper than a per-run aggregate
        // since rows are append-only and an index on run_id exists.
        let mut resp = self
            .db
            .query(
                "SELECT run_id, created_at FROM kg_compaction_run \
                 ORDER BY created_at DESC LIMIT 1",
            )
            .await
            .map_err(|e| format!("latest_run_summary head: {e}"))?;
        let head: Vec<serde_json::Value> = resp
            .take(0)
            .map_err(|e| format!("latest_run_summary head take: {e}"))?;
        let Some(row) = head.first() else {
            return Ok(None);
        };
        let run_id = row
            .get("run_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let latest_at = row
            .get("created_at")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let mut resp = self
            .db
            .query(
                "SELECT operation, count() AS n FROM kg_compaction_run \
                 WHERE run_id = $rid GROUP BY operation",
            )
            .bind(("rid", run_id.clone()))
            .await
            .map_err(|e| format!("latest_run_summary counts: {e}"))?;
        let counts: Vec<serde_json::Value> = resp
            .take(0)
            .map_err(|e| format!("latest_run_summary counts take: {e}"))?;
        let mut merges = 0u64;
        let mut prunes = 0u64;
        for c in counts {
            let op = c.get("operation").and_then(|v| v.as_str()).unwrap_or("");
            let n = c.get("n").and_then(|v| v.as_u64()).unwrap_or(0);
            match op {
                "merge" => merges = n,
                "prune" | "archival" => prunes += n,
                _ => {}
            }
        }
        Ok(Some(CompactionRunSummary {
            run_id,
            latest_at,
            merges,
            prunes,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{connect, schema::apply_schema, SurrealConfig};

    async fn fresh_store() -> SurrealCompactionStore {
        let cfg = SurrealConfig {
            url: "mem://".into(),
            namespace: "memory_kg".into(),
            database: "main".into(),
            credentials: None,
        };
        let db = connect(&cfg, None).await.expect("connect");
        apply_schema(&db).await.expect("schema");
        SurrealCompactionStore::new(db)
    }

    #[tokio::test]
    async fn record_merge_persists_audit_row() {
        let store = fresh_store().await;
        let id = store
            .record_merge("run-1", "ent-loser", "ent-winner", "cosine 0.95")
            .await
            .expect("record_merge");
        assert!(!id.is_empty());
    }

    #[tokio::test]
    async fn latest_run_summary_aggregates_per_run() {
        let store = fresh_store().await;
        store.record_merge("run-1", "a", "b", "x").await.unwrap();
        store.record_merge("run-1", "c", "d", "y").await.unwrap();
        store
            .record_prune("run-1", Some("e"), None, "z")
            .await
            .unwrap();

        let summary = store
            .latest_run_summary()
            .await
            .expect("summary")
            .expect("present");
        assert_eq!(summary.run_id, "run-1");
        assert_eq!(summary.merges, 2);
        assert_eq!(summary.prunes, 1);
    }
}
