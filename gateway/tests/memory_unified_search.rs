//! Tests for `POST /api/memory/search` unified hybrid search (Task 6 —
//! Memory Tab Command Deck).
//!
//! Seeds one item in each of the four content types (facts, wiki, procedures,
//! episodes) in a shared ward and asserts the unified handler fans out across
//! all four, returning per-type `hits` arrays and `latency_ms`.

mod common;

use common::{make_episode_repo, make_procedure_repo, make_wiki_repo, now_iso, setup};
use gateway::AppState;
use gateway_database::{MemoryFact, Procedure, SessionEpisode, WikiArticle};
use serde_json::{json, Value};

const TEST_WARD: &str = "maritime-vessel-tracking";

fn seed_all_four_types(state: &AppState) {
    let now = now_iso();

    let fact = MemoryFact {
        id: "fact-hormuz".to_string(),
        session_id: None,
        agent_id: "agent:root".to_string(),
        scope: "agent".to_string(),
        category: "pattern".to_string(),
        key: "mar.hormuz".to_string(),
        content: "Strait of Hormuz transit".to_string(),
        confidence: 0.9,
        mention_count: 1,
        source_summary: None,
        embedding: None,
        ward_id: TEST_WARD.to_string(),
        contradicted_by: None,
        created_at: now.clone(),
        updated_at: now.clone(),
        expires_at: None,
        valid_from: None,
        valid_until: None,
        superseded_by: None,
        pinned: false,
        epistemic_class: Some("current".to_string()),
        source_episode_id: None,
        source_ref: None,
    };
    state
        .memory_repo
        .as_ref()
        .expect("memory_repo")
        .upsert_memory_fact(&fact)
        .expect("upsert fact");

    let wiki = make_wiki_repo(state);
    let article = WikiArticle {
        id: "wiki-hormuz".to_string(),
        ward_id: TEST_WARD.to_string(),
        agent_id: "agent:root".to_string(),
        title: "Hormuz".to_string(),
        content: "Narrow strait between Oman and Iran.".to_string(),
        tags: None,
        source_fact_ids: None,
        embedding: None,
        version: 1,
        created_at: now.clone(),
        updated_at: now.clone(),
    };
    wiki.upsert_article(&article).expect("upsert wiki");

    let proc_repo = make_procedure_repo(state);
    let proc = Procedure {
        id: "proc-hormuz".to_string(),
        agent_id: "agent:root".to_string(),
        ward_id: Some(TEST_WARD.to_string()),
        name: "track-hormuz".to_string(),
        description: "Track vessels in Hormuz".to_string(),
        trigger_pattern: None,
        steps: "[]".to_string(),
        parameters: None,
        success_count: 0,
        failure_count: 0,
        avg_duration_ms: None,
        avg_token_cost: None,
        last_used: None,
        embedding: None,
        created_at: now.clone(),
        updated_at: now.clone(),
    };
    proc_repo.upsert_procedure(&proc).expect("upsert proc");

    let ep_repo = make_episode_repo(state);
    let ep = SessionEpisode {
        id: "ep-hormuz".to_string(),
        session_id: "sess-h".to_string(),
        agent_id: "agent:root".to_string(),
        ward_id: TEST_WARD.to_string(),
        task_summary: "Monitored Hormuz traffic".to_string(),
        outcome: "success".to_string(),
        strategy_used: None,
        key_learnings: None,
        token_cost: None,
        embedding: None,
        created_at: now.clone(),
    };
    ep_repo.insert(&ep).expect("insert episode");
}

fn assert_block(body: &Value, key: &str) {
    let block = &body[key];
    assert!(block.is_object(), "{key} should be an object, got: {block}");
    assert!(
        block["hits"].is_array(),
        "{key}.hits should be array, got: {block}"
    );
    assert!(
        block["latency_ms"].is_number(),
        "{key}.latency_ms should be number, got: {block}"
    );
}

#[tokio::test]
async fn searches_all_four_types_in_parallel() {
    let (server, _dir, state) = setup();
    seed_all_four_types(&state);

    let response = server
        .post("/api/memory/search")
        .json(&json!({
            "query": "hormuz",
            "mode": "hybrid",
            "types": ["facts", "wiki", "procedures", "episodes"],
            "ward_ids": [TEST_WARD],
            "limit": 10
        }))
        .await;

    response.assert_status_ok();
    let body: Value = response.json();

    assert_block(&body, "facts");
    assert_block(&body, "wiki");
    assert_block(&body, "procedures");
    assert_block(&body, "episodes");
}

#[tokio::test]
async fn mode_fts_skips_procedures_and_returns_empty() {
    let (server, _dir, state) = setup();
    seed_all_four_types(&state);

    let response = server
        .post("/api/memory/search")
        .json(&json!({
            "query": "hormuz",
            "mode": "fts",
            "types": ["facts", "wiki", "procedures", "episodes"],
            "ward_ids": [TEST_WARD],
            "limit": 10
        }))
        .await;

    response.assert_status_ok();
    let body: Value = response.json();

    assert_block(&body, "facts");
    assert_block(&body, "wiki");
    assert_block(&body, "procedures");
    assert_block(&body, "episodes");

    let procs = body["procedures"]["hits"].as_array().expect("procs arr");
    assert!(
        procs.is_empty(),
        "procedures must be empty in fts mode (no FTS index), got: {procs:?}"
    );

    // Facts and wiki should find the seeded "hormuz" content via FTS.
    let facts = body["facts"]["hits"].as_array().expect("facts arr");
    assert!(!facts.is_empty(), "facts should have hits via FTS");
    let wiki = body["wiki"]["hits"].as_array().expect("wiki arr");
    assert!(!wiki.is_empty(), "wiki should have hits via FTS");
}
