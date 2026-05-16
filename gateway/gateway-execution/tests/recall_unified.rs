//! End-to-end: construct a `MemoryRecall` with a real `MemoryRepository`,
//! seed one fact, run `recall_unified`, and assert the returned pool carries
//! at least one `ItemKind::Fact`. Covers the wiring that glues embedding,
//! source search, adapter projection, and RRF merge.
//!
//! The test runs without an embedding client — the fact source falls back to
//! FTS5 only, which is enough to prove the pipeline returns a scored item.

use std::sync::Arc;
use tempfile::tempdir;

use gateway_execution::recall::{ItemKind, MemoryRecall};
use gateway_services::{RecallConfig, VaultPaths};
use zero_stores_sqlite::{
    EpisodeRepository, KnowledgeDatabase, MemoryFact, MemoryRepository, SessionEpisode,
    SqliteVecIndex, VectorIndex,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn recall_unified_returns_scored_items_from_facts() {
    let tmp = tempdir().unwrap();
    let paths = Arc::new(VaultPaths::new(tmp.path().to_path_buf()));
    std::fs::create_dir_all(paths.conversations_db().parent().unwrap()).unwrap();
    let db = Arc::new(KnowledgeDatabase::new(paths).expect("knowledge db"));

    let fact_vec: Arc<dyn VectorIndex> = Arc::new(
        SqliteVecIndex::new(db.clone(), "memory_facts_index", "fact_id").expect("vec index init"),
    );
    let memory_repo = Arc::new(MemoryRepository::new(db.clone(), fact_vec));
    // Use min_score=0 so the wiring test is not blocked by the noise filter.
    // Real scores from FTS-only (no embedding client) are below 0.3 by design;
    // the min_score filter is tested separately in the unit tests.
    let config = Arc::new(RecallConfig {
        min_score: 0.0,
        ..RecallConfig::default()
    });

    // Seed one fact so the FTS arm of `search_memory_facts_hybrid` fires.
    let fact = MemoryFact {
        id: "f1".to_string(),
        session_id: None,
        agent_id: "root".to_string(),
        scope: "agent".to_string(),
        category: "pattern".to_string(),
        key: "test.pattern".to_string(),
        content: "tickers are stock symbols".to_string(),
        confidence: 0.9,
        mention_count: 1,
        source_summary: None,
        source_episode_id: None,
        source_ref: None,
        embedding: None,
        ward_id: "__global__".to_string(),
        contradicted_by: None,
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
        expires_at: None,
        valid_from: None,
        valid_until: None,
        superseded_by: None,
        pinned: false,
        epistemic_class: Some("current".to_string()),
    };
    memory_repo.upsert_memory_fact(&fact).unwrap();

    let memory_store: Arc<dyn zero_stores::MemoryFactStore> = Arc::new(
        zero_stores_sqlite::GatewayMemoryFactStore::new(memory_repo.clone(), None),
    );
    let mut recall = MemoryRecall::new(None, config);
    recall.set_memory_store(memory_store);
    let items = recall
        .recall_unified("root", "tickers", None, &[], 10)
        .await
        .expect("recall_unified succeeds");

    assert!(
        items.iter().any(|i| i.kind == ItemKind::Fact),
        "expected at least one fact item, got {items:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn recall_unified_injects_previous_episodes_for_ward() {
    let tmp = tempdir().unwrap();
    let paths = Arc::new(VaultPaths::new(tmp.path().to_path_buf()));
    std::fs::create_dir_all(paths.conversations_db().parent().unwrap()).unwrap();
    let db = Arc::new(KnowledgeDatabase::new(paths).expect("knowledge db"));

    let fact_vec: Arc<dyn VectorIndex> = Arc::new(
        SqliteVecIndex::new(db.clone(), "memory_facts_index", "fact_id").expect("vec index init"),
    );
    let memory_repo = Arc::new(MemoryRepository::new(db.clone(), fact_vec));

    let ep_vec: Arc<dyn VectorIndex> = Arc::new(
        SqliteVecIndex::new(db.clone(), "session_episodes_index", "episode_id")
            .expect("vec index init"),
    );
    let episode_repo = Arc::new(EpisodeRepository::new(db.clone(), ep_vec));

    let ward = "finance";
    let ep = SessionEpisode {
        id: "ep-prev-1".to_string(),
        session_id: "sess-prev-1".to_string(),
        agent_id: "root".to_string(),
        ward_id: ward.to_string(),
        task_summary: "reviewed Q3 earnings".to_string(),
        outcome: "success".to_string(),
        strategy_used: None,
        key_learnings: Some("prefer the 10-Q over press releases".to_string()),
        token_cost: None,
        embedding: None,
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    episode_repo.insert(&ep).unwrap();

    let config = Arc::new(RecallConfig::default());
    let memory_store: Arc<dyn zero_stores::MemoryFactStore> = Arc::new(
        zero_stores_sqlite::GatewayMemoryFactStore::new(memory_repo.clone(), None),
    );
    let episode_store: Arc<dyn zero_stores_traits::EpisodeStore> = Arc::new(
        zero_stores_sqlite::GatewayEpisodeStore::new(episode_repo.clone()),
    );
    let mut recall = MemoryRecall::new(None, config);
    recall.set_memory_store(memory_store);
    recall.set_episode_store(episode_store);

    let items = recall
        .recall_unified("root", "earnings", Some(ward), &[], 10)
        .await
        .expect("recall_unified succeeds");

    assert!(
        items.iter().any(|i| i.kind == ItemKind::Episode),
        "expected at least one episode item for ward={ward}, got {items:?}"
    );
}
