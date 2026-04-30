//! `SurrealGoalStore` — `GoalStore` impl over `Arc<Surreal<Any>>`.
//!
//! Backs the `goal` table (declared SCHEMALESS in `memory_kg.surql`) so the
//! full `Goal` JSON shape (slots/filled_slots/parent_goal_id/etc.) round-trips
//! without per-column field definitions.
//!
//! Rows are read back as `serde_json::Value` rather than via a `SurrealValue`
//! struct — SCHEMALESS rows can carry `null` literals on optional fields,
//! and the SDK's `SurrealValue` deserializer rejects `null` even on
//! `Option<T>`. JSON deserialization handles `null` -> `Option::None`
//! cleanly without that mismatch.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use zero_stores_domain::Goal;
use zero_stores_traits::GoalStore;

#[derive(Clone)]
pub struct SurrealGoalStore {
    db: Arc<Surreal<Any>>,
}

impl SurrealGoalStore {
    pub fn new(db: Arc<Surreal<Any>>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl GoalStore for SurrealGoalStore {
    async fn get_goal(&self, goal_id: &str) -> Result<Option<Value>, String> {
        let thing = surrealdb::types::RecordId::new(
            "goal",
            surrealdb::types::RecordIdKey::String(goal_id.to_string()),
        );
        let mut resp = self
            .db
            .query("SELECT * FROM ONLY $id")
            .bind(("id", thing))
            .await
            .map_err(|e| format!("get_goal: {e}"))?;
        let row: Option<Value> = resp.take(0).map_err(|e| format!("get_goal take: {e}"))?;
        Ok(row.map(crate::row_value::flatten_record_id))
    }

    async fn list_active_goals(&self, agent_id: &str) -> Result<Vec<Value>, String> {
        let mut resp = self
            .db
            .query(
                "SELECT * FROM goal WHERE agent_id = $a AND state = 'active' \
                 ORDER BY created_at DESC",
            )
            .bind(("a", agent_id.to_string()))
            .await
            .map_err(|e| format!("list_active_goals: {e}"))?;
        let rows: Vec<Value> = resp
            .take(0)
            .map_err(|e| format!("list_active_goals take: {e}"))?;
        Ok(rows
            .into_iter()
            .map(crate::row_value::flatten_record_id)
            .collect())
    }

    async fn create_goal(&self, goal: Value) -> Result<String, String> {
        let typed: Goal = serde_json::from_value(goal).map_err(|e| format!("decode Goal: {e}"))?;
        let thing = surrealdb::types::RecordId::new(
            "goal",
            surrealdb::types::RecordIdKey::String(typed.id.clone()),
        );
        let payload = serde_json::to_value(&typed).map_err(|e| format!("encode Goal: {e}"))?;
        self.db
            .query("CREATE $id CONTENT $g")
            .bind(("id", thing))
            .bind(("g", payload))
            .await
            .map_err(|e| format!("create_goal: {e}"))?;
        Ok(typed.id)
    }

    async fn update_goal_state(&self, goal_id: &str, new_state: &str) -> Result<(), String> {
        let now = chrono::Utc::now().to_rfc3339();
        let completed_at = if new_state == "satisfied" || new_state == "abandoned" {
            Some(now.clone())
        } else {
            None
        };
        let thing = surrealdb::types::RecordId::new(
            "goal",
            surrealdb::types::RecordIdKey::String(goal_id.to_string()),
        );
        // Mirror the SQLite `COALESCE(?, completed_at)` semantics: leave a
        // pre-existing completion stamp untouched on a non-terminal transition.
        if let Some(c) = completed_at {
            self.db
                .query("UPDATE $id SET state = $s, updated_at = $u, completed_at = $c")
                .bind(("id", thing))
                .bind(("s", new_state.to_string()))
                .bind(("u", now))
                .bind(("c", c))
                .await
                .map_err(|e| format!("update_goal_state: {e}"))?;
        } else {
            self.db
                .query("UPDATE $id SET state = $s, updated_at = $u")
                .bind(("id", thing))
                .bind(("s", new_state.to_string()))
                .bind(("u", now))
                .await
                .map_err(|e| format!("update_goal_state: {e}"))?;
        }
        Ok(())
    }

    async fn update_goal_filled_slots(
        &self,
        goal_id: &str,
        filled_slots_json: &str,
    ) -> Result<(), String> {
        let now = chrono::Utc::now().to_rfc3339();
        let thing = surrealdb::types::RecordId::new(
            "goal",
            surrealdb::types::RecordIdKey::String(goal_id.to_string()),
        );
        self.db
            .query("UPDATE $id SET filled_slots = $fs, updated_at = $u")
            .bind(("id", thing))
            .bind(("fs", filled_slots_json.to_string()))
            .bind(("u", now))
            .await
            .map_err(|e| format!("update_goal_filled_slots: {e}"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SurrealConfig, connect, schema::apply_schema};

    async fn fresh_store() -> SurrealGoalStore {
        let cfg = SurrealConfig {
            url: "mem://".into(),
            namespace: "memory_kg".into(),
            database: "main".into(),
            credentials: None,
        };
        let db = connect(&cfg, None).await.expect("connect");
        apply_schema(&db).await.expect("schema");
        SurrealGoalStore::new(db)
    }

    fn sample_goal(id: &str, state: &str) -> Value {
        let now = chrono::Utc::now().to_rfc3339();
        serde_json::json!({
            "id": id,
            "agent_id": "root",
            "ward_id": null,
            "title": format!("goal {id}"),
            "description": "details",
            "state": state,
            "parent_goal_id": null,
            "slots": null,
            "filled_slots": null,
            "created_at": now,
            "updated_at": now,
            "completed_at": null,
        })
    }

    #[tokio::test]
    async fn create_then_get_roundtrip() {
        let store = fresh_store().await;
        let id = store
            .create_goal(sample_goal("g1", "active"))
            .await
            .unwrap();
        assert_eq!(id, "g1");
        let fetched = store.get_goal("g1").await.unwrap().expect("present");
        assert_eq!(fetched["title"], "goal g1");
        assert_eq!(fetched["state"], "active");
    }

    #[tokio::test]
    async fn list_active_filters_state() {
        let store = fresh_store().await;
        store
            .create_goal(sample_goal("ga", "active"))
            .await
            .unwrap();
        store
            .create_goal(sample_goal("gb", "active"))
            .await
            .unwrap();
        store
            .create_goal(sample_goal("gc", "satisfied"))
            .await
            .unwrap();

        let active = store.list_active_goals("root").await.unwrap();
        assert_eq!(active.len(), 2);
        for g in &active {
            assert_eq!(g["state"], "active");
        }
    }

    #[tokio::test]
    async fn update_state_stamps_completed_at_on_terminal() {
        let store = fresh_store().await;
        store
            .create_goal(sample_goal("g1", "active"))
            .await
            .unwrap();
        store.update_goal_state("g1", "satisfied").await.unwrap();
        let fetched = store.get_goal("g1").await.unwrap().expect("present");
        assert_eq!(fetched["state"], "satisfied");
        assert!(fetched["completed_at"].is_string());

        // Non-terminal: blocked must not stamp completed_at.
        store
            .create_goal(sample_goal("g2", "active"))
            .await
            .unwrap();
        store.update_goal_state("g2", "blocked").await.unwrap();
        let fetched = store.get_goal("g2").await.unwrap().expect("present");
        assert_eq!(fetched["state"], "blocked");
        assert!(fetched["completed_at"].is_null());
    }
}
