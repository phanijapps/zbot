//! Red stubs for the graph-exploration pagination contract (spec
//! observatory-graph-completeness AC1). Today the cross-agent routes report
//! `total` as the page length and carry no `next_offset`; these tests stay red
//! until T2 lands the contract surface.

use std::{net::IpAddr, sync::Arc};

use axum_test::TestServer;
use knowledge_graph::types::{Entity, EntityType};
use serde_json::Value;
use tempfile::TempDir;

use super::create_http_router;
use crate::{config::GatewayConfig, state::AppState, websocket::WebSocketHandler};

fn setup(host: IpAddr) -> (TestServer, TempDir, AppState) {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join("agents")).unwrap();
    std::fs::create_dir_all(dir.path().join("skills")).unwrap();
    let state = AppState::minimal(dir.path().to_path_buf());
    let ws = Arc::new(WebSocketHandler::new(
        state.event_bus().clone(),
        state.runtime().clone(),
    ));
    let router = create_http_router(
        GatewayConfig {
            host,
            ..Default::default()
        },
        state.clone(),
        ws,
    );
    (TestServer::new(router).unwrap(), dir, state)
}

async fn seed_entities(state: &AppState, agent: &str, count: usize) {
    let store = state.kg_store().expect("kg store wired by minimal state");
    for index in 0..count {
        store
            .upsert_entity(
                agent,
                Entity {
                    id: format!("entity-{agent}-{index}"),
                    ..Entity::new(
                        agent.to_string(),
                        EntityType::Concept,
                        format!("Concept {index}"),
                    )
                },
            )
            .await
            .expect("seed entity");
    }
}

// STUB: AC1 — cross-agent pages report the exact scope total, not page length,
// and expose a next offset or explicit exhaustion.
#[tokio::test]
async fn all_entities_total_is_exact_scope_count_with_next_offset() {
    let (server, _dir, state) = setup("127.0.0.1".parse().unwrap());
    seed_entities(&state, "agent-a", 3).await;
    let response = server.get("/api/graph/all/entities?limit=2").await;
    response.assert_status_ok();
    let body: Value = response.json();
    assert_eq!(body["total"].as_u64(), Some(3), "total must be the exact scope count");
    assert!(
        body.get("next_offset").is_some_and(|v| !v.is_null()),
        "a page with more rows must carry next_offset"
    );
}

// STUB: AC1 — an exhausted page is explicit; the returned ids also prove
// offset passthrough (today the route ignores offset and echoes page one).
#[tokio::test]
async fn all_entities_final_page_marks_exhaustion() {
    let (server, _dir, state) = setup("127.0.0.1".parse().unwrap());
    seed_entities(&state, "agent-a", 4).await;
    let response = server.get("/api/graph/all/entities?limit=2&offset=2").await;
    response.assert_status_ok();
    let body: Value = response.json();
    assert_eq!(body["total"].as_u64(), Some(4), "total must be the exact scope count");
    let ids: Vec<&str> = body["entities"]
        .as_array()
        .expect("entities array")
        .iter()
        .map(|e| e["id"].as_str().expect("id"))
        .collect();
    assert_eq!(ids, vec!["entity-agent-a-2", "entity-agent-a-3"], "offset must advance rows");
    assert!(
        body.get("next_offset").is_none_or(|v| v.is_null()),
        "the final page must be explicitly exhausted"
    );
}

// STUB: AC1 — the graph surface is loopback-only: a LAN-bound gateway must
// deny graph reads before any store access. Red today: the routes are
// unguarded and answer 200.
#[tokio::test]
async fn lan_bound_gateway_denies_graph_reads_before_store_access() {
    let (lan, _dir, state) = setup("0.0.0.0".parse().unwrap());
    seed_entities(&state, "agent-a", 1).await;
    let denied = lan.get("/api/graph/all/entities?limit=2").await;
    denied.assert_status(axum::http::StatusCode::FORBIDDEN);
    let body: Value = denied.json();
    assert!(body["error"].as_str().is_some_and(|e| e.len() <= 160));
}

// AC1/AC8 — no /api/graph/* route reaches the store unguarded: on a LAN-bound
// gateway every graph route must deny before any handler/store work.
#[tokio::test]
async fn every_graph_route_denies_on_lan_bind() {
    let (lan, _dir, _state) = setup("0.0.0.0".parse().unwrap());
    for path in [
        "/api/graph/stats",
        "/api/graph/all/entities?limit=10",
        "/api/graph/all/relationships?limit=10",
        "/api/graph/all/search?q=x&limit=10",
        "/api/graph/root/stats",
        "/api/graph/root/entities?limit=10",
        "/api/graph/root/relationships?limit=10",
        "/api/graph/root/search?q=x&limit=10",
        "/api/graph/root/entities/e/neighbors?limit=10",
        "/api/graph/root/entities/e",
        "/api/graph/root/entities/e/subgraph?max_hops=2",
    ] {
        let denied = lan.get(path).await;
        denied.assert_status(axum::http::StatusCode::FORBIDDEN);
    }
    for path in ["/api/graph/ingest/x/progress"] {
        lan.get(path).await.assert_status(axum::http::StatusCode::FORBIDDEN);
    }
    lan.post("/api/graph/reindex")
        .json(&serde_json::json!({}))
        .await
        .assert_status(axum::http::StatusCode::FORBIDDEN);
    lan.post("/api/graph/ingest")
        .json(&serde_json::json!({"source_id": "x", "text": "t"}))
        .await
        .assert_status(axum::http::StatusCode::FORBIDDEN);
}

// AC1 — invalid parameters fail with a bounded body.
#[tokio::test]
async fn invalid_paging_parameters_fail() {
    let (server, _dir, state) = setup("127.0.0.1".parse().unwrap());
    seed_entities(&state, "agent-a", 3).await;
    for path in [
        "/api/graph/all/entities?limit=0",
        "/api/graph/all/entities?limit=1001",
        "/api/graph/all/entities?limit=10&offset=1000001",
        "/api/graph/all/search?q=x&limit=0",
    ] {
        server.get(path).await.assert_status(axum::http::StatusCode::BAD_REQUEST);
    }
    let long_query = "q=".to_string() + &"x".repeat(300);
    server
        .get(&format!("/api/graph/all/search?{long_query}&limit=5"))
        .await
        .assert_status(axum::http::StatusCode::BAD_REQUEST);
    for hops in ["max_hops=0", "max_hops=5"] {
        server
            .get(&format!("/api/graph/root/entities/e/subgraph?{hops}"))
            .await
            .assert_status(axum::http::StatusCode::BAD_REQUEST);
    }
}

// AC3 — projection: oversized names clip at a Unicode boundary with the
// truncation flag; over-budget properties are omitted without losing counts.
#[tokio::test]
async fn projection_flags_truncation_without_losing_counts() {
    use knowledge_graph::types::Entity as DomainEntity;
    use knowledge_graph::types::EntityType;
    let (server, _dir, state) = setup("127.0.0.1".parse().unwrap());
    let store = state.kg_store().expect("kg store");
    let mut long_name = DomainEntity::new(
        "agent-a".to_string(),
        EntityType::Concept,
        "é".repeat(1200),
    );
    long_name.id = "entity-long-name".to_string();
    store.upsert_entity("agent-a", long_name).await.expect("long name");
    let mut deep_props = DomainEntity::new(
        "agent-a".to_string(),
        EntityType::Concept,
        "Deep".to_string(),
    );
    deep_props.id = "entity-deep-props".to_string();
    for index in 0..80 {
        deep_props.properties.insert(
            format!("key{index}"),
            serde_json::json!(format!("v{}", "y".repeat(64))),
        );
    }
    store.upsert_entity("agent-a", deep_props).await.expect("deep props");

    let response = server.get("/api/graph/all/entities?limit=10").await;
    response.assert_status_ok();
    let body: Value = response.json();
    assert_eq!(body["total"].as_u64(), Some(2), "counts stay exact");
    let entities = body["entities"].as_array().expect("entities");
    let long = entities.iter().find(|e| e["id"] == "entity-long-name").expect("long");
    assert_eq!(long["name"].as_str().map(|n| n.chars().count()), Some(1024));
    assert_eq!(long["projection_truncated"], true);
    let deep = entities.iter().find(|e| e["id"] == "entity-deep-props").expect("deep");
    assert_eq!(deep["properties"].as_object().map(|p| p.len()), Some(0));
    assert_eq!(deep["projection_truncated"], true);
}

// AC3/Never-do — a byte-budgeted subgraph must flag truncation, never
// presenting a partial neighborhood as complete.
#[tokio::test]
async fn budgeted_subgraph_flags_truncation() {
    use knowledge_graph::types::{Entity as DomainEntity, EntityType};
    let (server, _dir, state) = setup("127.0.0.1".parse().unwrap());
    let store = state.kg_store().expect("kg store");
    // Two entities with near-row-budget properties force the byte budget to
    // bite on a 2-node subgraph (each projected row alone exceeds the cap
    // only when combined; enforce_page_budget keeps >=1 row and truncates).
    for index in 0..2 {
        let mut wide = DomainEntity::new("agent-a".to_string(), EntityType::Concept, format!("W{index}"));
        wide.id = format!("wide-{index}");
        for entry in 0..60 {
            wide.properties.insert(format!("k{entry}"), serde_json::json!(format!("{}","v".repeat(4000))));
        }
        store.upsert_entity("agent-a", wide).await.expect("wide");
    }
    use knowledge_graph::types::{Relationship, RelationshipType};
    store
        .upsert_relationship(
            "agent-a",
            Relationship::new(
                "agent-a".to_string(),
                "wide-0".to_string(),
                "wide-1".to_string(),
                RelationshipType::RelatedTo,
            ),
        )
        .await
        .expect("edge");
    let response = server
        .get("/api/graph/agent-a/entities/wide-0/subgraph?max_hops=1")
        .await;
    response.assert_status_ok();
    let body: Value = response.json();
    let truncated = body["truncated"].as_bool().unwrap_or(false);
    let entity_rows = body["entities"].as_array().map(|rows| rows.len()).unwrap_or(0);
    // Either the properties fit (2 rows, not truncated) or rows were cut
    // (truncated true with fewer rows than entities exist) — never a silent
    // partial view.
    if truncated {
        assert!(entity_rows < 2, "truncated must mean rows were dropped");
    } else {
        assert_eq!(entity_rows, 2);
    }
}
