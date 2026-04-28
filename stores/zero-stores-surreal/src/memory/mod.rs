//! `SurrealMemoryStore` — `MemoryFactStore` impl over `Arc<Surreal<Any>>`.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use surrealdb::engine::any::Any;
use surrealdb::Surreal;
use zero_stores_traits::MemoryFactStore;

mod fact;

#[derive(Clone)]
pub struct SurrealMemoryStore {
    db: Arc<Surreal<Any>>,
}

impl SurrealMemoryStore {
    pub fn new(db: Arc<Surreal<Any>>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl MemoryFactStore for SurrealMemoryStore {
    async fn save_fact(
        &self,
        agent_id: &str,
        category: &str,
        key: &str,
        content: &str,
        confidence: f64,
        session_id: Option<&str>,
    ) -> Result<Value, String> {
        fact::save_fact(
            &self.db, agent_id, category, key, content, confidence, session_id,
        )
        .await
    }

    async fn recall_facts(
        &self,
        agent_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Value, String> {
        fact::recall_facts(&self.db, agent_id, query, limit).await
    }

    async fn count_all_facts(&self, agent_id: Option<&str>) -> Result<i64, String> {
        fact::count_all_facts(&self.db, agent_id).await
    }

    async fn list_memory_facts(
        &self,
        agent_id: Option<&str>,
        category: Option<&str>,
        scope: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Value>, String> {
        fact::list_memory_facts(&self.db, agent_id, category, scope, limit, offset).await
    }

    async fn get_memory_fact_by_id(&self, fact_id: &str) -> Result<Option<Value>, String> {
        fact::get_memory_fact_by_id(&self.db, fact_id).await
    }

    async fn delete_memory_fact(&self, fact_id: &str) -> Result<bool, String> {
        fact::delete_memory_fact(&self.db, fact_id).await
    }
}
