use std::sync::Arc;

use surrealdb::engine::any::Any;
use surrealdb::Surreal;
use zero_stores_surreal::{connect, schema::apply_schema, SurrealConfig, SurrealMemoryStore};

async fn fresh_store() -> SurrealMemoryStore {
    let cfg = SurrealConfig {
        url: "mem://".into(),
        namespace: "memory_kg".into(),
        database: "main".into(),
        credentials: None,
    };
    let db: Arc<Surreal<Any>> = connect(&cfg, None).await.expect("connect");
    apply_schema(&db).await.expect("schema");
    SurrealMemoryStore::new(db)
}

#[tokio::test]
async fn memory_save_and_count() {
    let s = fresh_store().await;
    zero_stores_conformance::memory_save_and_count(&s).await;
}

#[tokio::test]
async fn memory_recall_finds_match() {
    let s = fresh_store().await;
    zero_stores_conformance::memory_recall_finds_match(&s).await;
}

#[tokio::test]
async fn memory_recall_respects_agent_isolation() {
    let s = fresh_store().await;
    zero_stores_conformance::memory_recall_respects_agent_isolation(&s).await;
}
