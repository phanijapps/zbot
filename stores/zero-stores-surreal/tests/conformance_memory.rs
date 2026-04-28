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

#[tokio::test]
async fn memory_list_facts_filters_and_paginates() {
    let s = fresh_store().await;
    zero_stores_conformance::memory_list_facts_filters_and_paginates(&s).await;
}

#[tokio::test]
async fn memory_get_by_id_round_trip() {
    let s = fresh_store().await;
    zero_stores_conformance::memory_get_by_id_round_trip(&s).await;
}

#[tokio::test]
async fn memory_delete_fact_removes_it() {
    let s = fresh_store().await;
    zero_stores_conformance::memory_delete_fact_removes_it(&s).await;
}

#[tokio::test]
async fn memory_archive_fact_hides_from_listing() {
    let s = fresh_store().await;
    zero_stores_conformance::memory_archive_fact_hides_from_listing(&s).await;
}

#[tokio::test]
async fn memory_supersede_fact_succeeds() {
    let s = fresh_store().await;
    zero_stores_conformance::memory_supersede_fact_succeeds(&s).await;
}

#[tokio::test]
async fn memory_upsert_typed_fact_round_trip() {
    let s = fresh_store().await;
    zero_stores_conformance::memory_upsert_typed_fact_round_trip(&s).await;
}

#[tokio::test]
async fn memory_hybrid_search_finds_match() {
    let s = fresh_store().await;
    zero_stores_conformance::memory_hybrid_search_finds_match(&s).await;
}
