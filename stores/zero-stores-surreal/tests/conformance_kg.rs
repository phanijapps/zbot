use std::sync::Arc;

use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use zero_stores_surreal::{SurrealConfig, SurrealKgStore, connect, schema::apply_schema};

async fn fresh_store() -> SurrealKgStore {
    let cfg = SurrealConfig {
        url: "mem://".into(),
        namespace: "memory_kg".into(),
        database: "main".into(),
        credentials: None,
    };
    let db: Arc<Surreal<Any>> = connect(&cfg, None).await.expect("connect");
    apply_schema(&db).await.expect("schema");
    SurrealKgStore::new(db)
}

#[tokio::test]
async fn entity_round_trip() {
    let s = fresh_store().await;
    zero_stores_conformance::entity_round_trip(&s).await;
}

#[tokio::test]
async fn upsert_increments_mention_count() {
    let s = fresh_store().await;
    zero_stores_conformance::upsert_increments_mention_count(&s).await;
}

#[tokio::test]
async fn bump_mention_increases_count() {
    let s = fresh_store().await;
    zero_stores_conformance::bump_mention_increases_count(&s).await;
}

#[tokio::test]
async fn resolve_exact_match() {
    let s = fresh_store().await;
    zero_stores_conformance::resolve_exact_match(&s).await;
}

#[tokio::test]
async fn resolve_via_alias() {
    let s = fresh_store().await;
    zero_stores_conformance::resolve_via_alias(&s).await;
}

#[tokio::test]
async fn resolve_no_match() {
    let s = fresh_store().await;
    zero_stores_conformance::resolve_no_match(&s).await;
}

#[tokio::test]
async fn relationship_round_trip() {
    let s = fresh_store().await;
    zero_stores_conformance::relationship_round_trip(&s).await;
}

#[tokio::test]
async fn store_knowledge_writes_both() {
    let s = fresh_store().await;
    zero_stores_conformance::store_knowledge_writes_both(&s).await;
}

#[tokio::test]
async fn neighbors_outgoing() {
    let s = fresh_store().await;
    zero_stores_conformance::neighbors_outgoing(&s).await;
}

#[tokio::test]
async fn neighbors_incoming() {
    let s = fresh_store().await;
    zero_stores_conformance::neighbors_incoming(&s).await;
}

#[tokio::test]
async fn traverse_respects_max_hops() {
    let s = fresh_store().await;
    zero_stores_conformance::traverse_respects_max_hops(&s).await;
}

#[tokio::test]
async fn fts_finds_match() {
    let s = fresh_store().await;
    zero_stores_conformance::fts_finds_match(&s).await;
}

#[tokio::test]
async fn reindex_idempotent_when_dim_matches() {
    let s = fresh_store().await;
    zero_stores_conformance::reindex_idempotent_when_dim_matches(&s).await;
}

#[tokio::test]
async fn stats_reflects_writes() {
    let s = fresh_store().await;
    zero_stores_conformance::stats_reflects_writes(&s).await;
}

#[tokio::test]
async fn graph_stats_per_agent() {
    let s = fresh_store().await;
    zero_stores_conformance::graph_stats_per_agent(&s).await;
}

#[tokio::test]
async fn mark_archival_sets_class() {
    let s = fresh_store().await;
    zero_stores_conformance::mark_archival_sets_class(&s).await;
}

#[tokio::test]
async fn list_entities_respects_agent() {
    let s = fresh_store().await;
    zero_stores_conformance::list_entities_respects_agent(&s).await;
}
