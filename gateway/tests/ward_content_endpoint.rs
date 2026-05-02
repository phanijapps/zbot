//! Tests for `GET /api/wards/:ward_id/content` aggregator (Task 5 — Memory Tab
//! Command Deck).
//!
//! Seeds one item in each of the four content types (facts, wiki, procedures,
//! episodes) for a single ward and asserts that the aggregator handler returns
//! all four arrays with server-computed `age_bucket` annotations and matching
//! `counts`.

mod common;

use common::{make_episode_repo, make_procedure_repo, make_wiki_repo, now_iso, setup};
use serde_json::Value;
use zero_stores_domain::{MemoryFact, Procedure, SessionEpisode, WikiArticle};

const TEST_WARD: &str = "literature-library";

#[tokio::test]
async fn returns_four_content_types_with_age_buckets() {
    let (server, _dir, state) = setup();

    let now = now_iso();

    // Fact
    let fact = MemoryFact {
        id: "fact-1".to_string(),
        session_id: None,
        agent_id: "agent-1".to_string(),
        scope: "agent".to_string(),
        category: "pattern".to_string(),
        key: "lit.genre".to_string(),
        content: "fantasy novels".to_string(),
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
    let fact_v = serde_json::to_value(&fact).expect("encode MemoryFact");
    futures::executor::block_on(
        state
            .memory_store
            .as_ref()
            .expect("memory_store")
            .upsert_typed_fact(fact_v, None),
    )
    .expect("upsert fact");

    // Wiki (including an __index__ article to drive summary)
    let wiki = make_wiki_repo(&state);
    let index_article = WikiArticle {
        id: "wiki-index".to_string(),
        ward_id: TEST_WARD.to_string(),
        agent_id: "agent-1".to_string(),
        title: "__index__".to_string(),
        content: "Books, authors, and reading notes.\nSecond line.".to_string(),
        tags: None,
        source_fact_ids: None,
        embedding: None,
        version: 1,
        created_at: now.clone(),
        updated_at: now.clone(),
    };
    wiki.upsert_article(&index_article).expect("upsert index");
    let regular_article = WikiArticle {
        id: "wiki-1".to_string(),
        ward_id: TEST_WARD.to_string(),
        agent_id: "agent-1".to_string(),
        title: "Tolkien".to_string(),
        content: "LOTR author.".to_string(),
        tags: None,
        source_fact_ids: None,
        embedding: None,
        version: 1,
        created_at: now.clone(),
        updated_at: now.clone(),
    };
    wiki.upsert_article(&regular_article).expect("upsert wiki");

    // Procedure
    let proc_repo = make_procedure_repo(&state);
    let proc = Procedure {
        id: "proc-1".to_string(),
        agent_id: "agent-1".to_string(),
        ward_id: Some(TEST_WARD.to_string()),
        name: "recommend-book".to_string(),
        description: "Suggest a book".to_string(),
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

    // Episode
    let ep_repo = make_episode_repo(&state);
    let ep = SessionEpisode {
        id: "ep-1".to_string(),
        session_id: "sess-1".to_string(),
        agent_id: "agent-1".to_string(),
        ward_id: TEST_WARD.to_string(),
        task_summary: "Reviewed Tolkien".to_string(),
        outcome: "success".to_string(),
        strategy_used: None,
        key_learnings: None,
        token_cost: None,
        embedding: None,
        created_at: now.clone(),
    };
    ep_repo.insert(&ep).expect("insert episode");

    let response = server.get(&format!("/api/wards/{TEST_WARD}/content")).await;
    response.assert_status_ok();
    let body: Value = response.json();

    assert_eq!(body["ward_id"], TEST_WARD);
    let facts = body["facts"].as_array().expect("facts array");
    let wiki_arr = body["wiki"].as_array().expect("wiki array");
    let procs = body["procedures"].as_array().expect("procedures array");
    let eps = body["episodes"].as_array().expect("episodes array");

    assert_eq!(facts.len(), 1, "body={body}");
    assert_eq!(wiki_arr.len(), 2);
    assert_eq!(procs.len(), 1);
    assert_eq!(eps.len(), 1);

    assert_eq!(body["counts"]["facts"], 1);
    assert_eq!(body["counts"]["wiki"], 2);
    assert_eq!(body["counts"]["procedures"], 1);
    assert_eq!(body["counts"]["episodes"], 1);

    for item in facts
        .iter()
        .chain(wiki_arr.iter())
        .chain(procs.iter())
        .chain(eps.iter())
    {
        let bucket = item["age_bucket"].as_str().expect("age_bucket str");
        assert!(
            matches!(bucket, "today" | "last_7_days" | "historical"),
            "unexpected bucket: {bucket}"
        );
    }

    // summary derived from __index__ article (first non-empty line)
    assert_eq!(
        body["summary"]["description"].as_str(),
        Some("Books, authors, and reading notes.")
    );
}

#[tokio::test]
async fn unknown_ward_returns_empty_arrays_and_zero_counts() {
    let (server, _dir, _state) = setup();

    let response = server.get("/api/wards/nope/content").await;
    response.assert_status_ok();
    let body: Value = response.json();

    assert_eq!(body["facts"].as_array().map(|a| a.len()), Some(0));
    assert_eq!(body["wiki"].as_array().map(|a| a.len()), Some(0));
    assert_eq!(body["procedures"].as_array().map(|a| a.len()), Some(0));
    assert_eq!(body["episodes"].as_array().map(|a| a.len()), Some(0));

    assert_eq!(body["counts"]["facts"], 0);
    assert_eq!(body["counts"]["wiki"], 0);
    assert_eq!(body["counts"]["procedures"], 0);
    assert_eq!(body["counts"]["episodes"], 0);

    // Summary fallback: title = ward_id
    assert_eq!(body["summary"]["title"].as_str(), Some("nope"));
}
