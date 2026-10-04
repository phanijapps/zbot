//! Red stubs for the graph-exploration pagination contract (spec
//! observatory-graph-completeness AC1). Today the cross-agent routes report
//! `total` as the page length and carry no `nextOffset`; these tests stay red
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
        "a page with more rows must carry nextOffset"
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
