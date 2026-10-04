use std::{net::IpAddr, sync::Arc};

use api_logs::{ExecutionLog, LogCategory, LogLevel};
use axum::{
    extract::FromRequestParts,
    http::{Request, StatusCode},
    Json,
};
use axum_test::TestServer;
use execution_state::{AgentExecution, Session, TriggerSource};
use serde_json::Value;
use tempfile::TempDir;

use super::create_http_router;
use super::sessions::LoopbackBind;
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

fn seed_session(state: &AppState, id: &str, mode: &str) {
    let mut session = Session::new_with_source("root", TriggerSource::Web);
    session.id = id.to_owned();
    state.state_service().create_session_from(&session).unwrap();
    state.state_service().set_session_mode(id, mode).unwrap();
}

// STUB: AC2 AC4 AC5 — a persisted session with no evidence is still a 200.
#[tokio::test]
async fn details_returns_persisted_mode_and_honest_empty_arrays() {
    let (server, _dir, state) = setup("127.0.0.1".parse().unwrap());
    seed_session(&state, "sess-test", "deep");
    let response = server.get("/api/sessions/sess-test/details").await;
    response.assert_status_ok();
    let body: Value = response.json();
    assert_eq!(body["sessionId"], "sess-test");
    assert_eq!(body["mode"], "research");
    assert_eq!(body["activity"], serde_json::json!([]));
    assert_eq!(body["sources"], serde_json::json!([]));
}

// STUB: AC2 — legacy sessions never inherit mode from route or title.
#[tokio::test]
async fn details_reports_legacy_missing_mode_as_unknown() {
    let (server, _dir, state) = setup("127.0.0.1".parse().unwrap());
    let mut session = Session::new_with_source("root", TriggerSource::Web);
    session.id = "sess-legacy".to_owned();
    state.state_service().create_session_from(&session).unwrap();
    let response = server.get("/api/sessions/sess-legacy/details").await;
    response.assert_status_ok();
    let body: Value = response.json();
    assert_eq!(body["mode"], "unknown");
}

// STUB: AC4 AC6 — every persisted turn appears, without raw log payloads.
#[tokio::test]
async fn details_reopens_root_and_continuation_activity_redacted() {
    let (server, _dir, state) = setup("127.0.0.1".parse().unwrap());
    seed_session(&state, "sess-test", "deep");
    for (conversation, message) in [
        ("sess-test", "private root result"),
        ("sess-test-cont-a1b2c3d4", "private continuation result"),
    ] {
        let execution = AgentExecution::new_root("sess-test", "root");
        state.state_service().create_execution(&execution).unwrap();
        let log = ExecutionLog::new(
            &execution.id,
            conversation,
            "root",
            LogLevel::Info,
            LogCategory::ToolCall,
            message,
        )
        .with_metadata(serde_json::json!({ "tool_name": "recall" }));
        state.log_service().log(log).unwrap();
    }
    let response = server.get("/api/sessions/sess-test/details").await;
    response.assert_status_ok();
    let body: Value = response.json();
    assert_eq!(body["activity"].as_array().unwrap().len(), 2);
    assert_eq!(body["activity"][0]["kind"], "memory_recall");
    assert_eq!(body["activity"][1]["kind"], "memory_recall");
    assert!(!body.to_string().contains("private"));
}

// STUB: AC12 AC13 — denial precedes a read and uses a bounded error body.
#[tokio::test]
async fn details_denies_lan_bind_and_cross_origin_with_fixed_errors() {
    let (lan, _dir, state) = setup("0.0.0.0".parse().unwrap());
    seed_session(&state, "sess-test", "deep");
    let denied = lan.get("/api/sessions/sess-test/details").await;
    denied.assert_status(StatusCode::FORBIDDEN);
    let body: Value = denied.json();
    assert_eq!(body.as_object().unwrap().len(), 1);
    assert!(body["error"].as_str().unwrap().len() <= 160);

    let (local, _dir, _) = setup("127.0.0.1".parse().unwrap());
    let denied = local
        .get("/api/sessions/sess-test/details")
        .add_header("origin", "https://evil.example")
        .await;
    denied.assert_status(StatusCode::FORBIDDEN);
}

// STUB: AC13 — missing bind proof fails before any handler can read a session.
#[tokio::test]
async fn details_bind_guard_fails_closed_without_config_extension() {
    let (mut parts, _) = Request::builder().uri("/").body(()).unwrap().into_parts();
    let rejected = LoopbackBind::from_request_parts(&mut parts, &())
        .await
        .err()
        .expect("missing bind proof must deny");
    assert_eq!(rejected.0, StatusCode::FORBIDDEN);
    let (_, Json(body)) = rejected;
    assert_eq!(body.error, "session details unavailable");
}

// STUB: AC13 — a same-origin browser remains usable on loopback.
#[tokio::test]
async fn details_allows_same_origin_browser_on_loopback() {
    let (server, _dir, state) = setup("127.0.0.1".parse().unwrap());
    seed_session(&state, "sess-test", "fast");
    let response = server
        .get("/api/sessions/sess-test/details")
        .add_header("host", "localhost")
        .add_header("origin", "http://localhost")
        .await;
    response.assert_status_ok();
}

// STUB: AC12 — invalid and absent session IDs have distinct fixed statuses.
#[tokio::test]
async fn details_rejects_invalid_ids_and_reports_missing_sessions() {
    let (server, _dir, _) = setup("127.0.0.1".parse().unwrap());
    server
        .get("/api/sessions/bad%20id/details")
        .await
        .assert_status(StatusCode::BAD_REQUEST);
    server
        .get("/api/sessions/absent/details")
        .await
        .assert_status(StatusCode::NOT_FOUND);
}

// STUB: AC12 — a storage failure cannot surface the underlying SQL or path.
#[tokio::test]
async fn details_storage_failure_has_fixed_500_body() {
    let (server, dir, _) = setup("127.0.0.1".parse().unwrap());
    let connection = rusqlite::Connection::open(dir.path().join("data/conversations.db")).unwrap();
    connection.execute("DROP TABLE sessions", []).unwrap();
    let response = server.get("/api/sessions/sess-test/details").await;
    response.assert_status(StatusCode::INTERNAL_SERVER_ERROR);
    let body: Value = response.json();
    assert_eq!(
        body,
        serde_json::json!({ "error": "session details unavailable" })
    );
}

// STUB: AC13 — the served API contract must expose the new operation.
#[tokio::test]
async fn served_openapi_contains_session_details() {
    let (server, _dir, _) = setup("127.0.0.1".parse().unwrap());
    let response = server.get("/api/openapi.yaml").await;
    response.assert_status_ok();
    assert!(response.text().contains("getSessionDetails"));
    let json = server.get("/api/openapi.json").await;
    json.assert_status_ok();
    let body: Value = json.json();
    assert_eq!(
        body["paths"]["/api/sessions/{sessionId}/details"]["get"]["operationId"],
        "getSessionDetails"
    );
}

// STUB: AC2 AC14 — exercise actual creation, not mocked ID uniqueness.
#[tokio::test]
async fn independent_chats_coexist_without_mutating_legacy_slot() {
    let (server, _dir, state) = setup("127.0.0.1".parse().unwrap());
    seed_session(&state, "sess-old", "fast");
    let message = zbot_conversation::Message {
        id: "msg-old".into(),
        execution_id: None,
        session_id: "sess-old".into(),
        role: "user".into(),
        content: "Keep this earlier question".into(),
        created_at: "2026-09-27T00:00:00Z".into(),
        token_count: 5,
        tool_calls: None,
        tool_call_id: None,
        seq: 0,
    };
    state.messages().append(&message).unwrap();
    let artifact = execution_state::Artifact::new("sess-old", "/fixture/old.md", "old.md");
    state.state_service().create_artifact(&artifact).unwrap();
    let prior_messages =
        serde_json::to_value(state.messages().replay("sess-old", None, 100).unwrap()).unwrap();
    let prior_artifacts = serde_json::to_value(
        state
            .state_service()
            .list_artifacts_by_session("sess-old")
            .unwrap(),
    )
    .unwrap();
    let old = serde_json::to_value(state.state_service().get_session("sess-old").unwrap()).unwrap();
    let mut settings = state.settings().get_execution_settings().unwrap();
    settings.chat.session_id = Some("sess-old".into());
    settings.chat.conversation_id = Some("chat-old".into());
    state
        .settings()
        .update_execution_settings(settings.clone())
        .unwrap();
    let first = server.post("/api/sessions/chat").await;
    first.assert_status(StatusCode::CREATED);
    let a: Value = first.json();
    let second = server.post("/api/sessions/chat").await;
    second.assert_status(StatusCode::CREATED);
    let b: Value = second.json();
    assert_ne!(a["sessionId"], b["sessionId"]);
    assert_eq!(a["conversationId"], a["sessionId"]);
    assert_eq!(a["created"], true);
    assert_eq!(a["isLive"], false);
    let id = a["sessionId"].as_str().unwrap();
    let stored = state
        .state_service()
        .get_session_with_executions(id)
        .unwrap()
        .unwrap();
    assert_eq!(stored.session.mode.as_deref(), Some("fast"));
    assert_eq!(stored.session.root_agent_id, "root");
    assert!(stored.executions.is_empty());
    let reopened = server.get(&format!("/api/sessions/{id}/chat")).await;
    reopened.assert_status_ok();
    let body: Value = reopened.json();
    assert_eq!(body["sessionId"], a["sessionId"]);
    assert_eq!(body["conversationId"], a["conversationId"]);
    assert_eq!(body["created"], false);
    assert_eq!(body["isLive"], false);
    assert_eq!(
        state
            .settings()
            .get_execution_settings()
            .unwrap()
            .chat
            .session_id,
        settings.chat.session_id
    );
    assert_eq!(
        state
            .settings()
            .get_execution_settings()
            .unwrap()
            .chat
            .conversation_id,
        settings.chat.conversation_id
    );
    assert_eq!(
        serde_json::to_value(state.state_service().get_session("sess-old").unwrap()).unwrap(),
        old
    );
    assert_eq!(
        serde_json::to_value(state.messages().replay("sess-old", None, 100).unwrap()).unwrap(),
        prior_messages
    );
    assert_eq!(
        serde_json::to_value(
            state
                .state_service()
                .list_artifacts_by_session("sess-old")
                .unwrap()
        )
        .unwrap(),
        prior_artifacts
    );
}

// STUB: AC14 — open validates server truth and never coerces mode.
#[tokio::test]
async fn independent_chat_open_rejects_missing_invalid_and_other_modes() {
    let (server, _dir, state) = setup("127.0.0.1".parse().unwrap());
    seed_session(&state, "sess-research", "deep");
    seed_session(&state, "sess-unknown", "future-mode");
    let mut child = Session::new_with_source("root", TriggerSource::Web);
    child.id = "sess-child".into();
    child.parent_session_id = Some("sess-research".into());
    child.mode = Some("fast".into());
    state.state_service().create_session_from(&child).unwrap();
    for id in ["sess-research", "sess-unknown", "sess-child"] {
        let response = server.get(&format!("/api/sessions/{id}/chat")).await;
        response.assert_status(StatusCode::CONFLICT);
        assert_eq!(
            response.json::<Value>(),
            serde_json::json!({"error": "not a root chat session"})
        );
    }
    server
        .get("/api/sessions/missing/chat")
        .await
        .assert_status(StatusCode::NOT_FOUND);
    server
        .get("/api/sessions/invalid%20id/chat")
        .await
        .assert_status(StatusCode::BAD_REQUEST);
    seed_session(&state, "sess-idle-history", "fast");
    let response = server.get("/api/sessions/sess-idle-history/chat").await;
    response.assert_status_ok();
    let body: Value = response.json();
    assert_eq!(body["conversationId"], "sess-idle-history");
    assert_eq!(body["isLive"], false);
}

// STUB: AC14 — active reload must not guess an unrelated Stop/stream key.
#[tokio::test]
async fn independent_chat_open_recovers_new_active_key_but_denies_unknown_legacy_key() {
    let (server, _dir, state) = setup("127.0.0.1".parse().unwrap());
    let response = server.post("/api/sessions/chat").await;
    response.assert_status(StatusCode::CREATED);
    let body: Value = response.json();
    let id = body["sessionId"].as_str().unwrap();
    let execution = AgentExecution::new_root(id, "root");
    state.state_service().create_execution(&execution).unwrap();
    let response = server.get(&format!("/api/sessions/{id}/chat")).await;
    response.assert_status_ok();
    let active: Value = response.json();
    assert_eq!(active["isLive"], true);
    assert_eq!(active["conversationId"], id);
    seed_session(&state, "sess-legacy-active", "fast");
    state
        .state_service()
        .create_execution(&AgentExecution::new_root("sess-legacy-active", "root"))
        .unwrap();
    server
        .get("/api/sessions/sess-legacy-active/chat")
        .await
        .assert_status(StatusCode::CONFLICT);
}

// STUB: AC14 — guards precede persistence even when the database is broken.
#[tokio::test]
async fn independent_chat_guards_and_storage_errors_are_sanitized() {
    let (server, dir, _) = setup("127.0.0.1".parse().unwrap());
    let connection = rusqlite::Connection::open(dir.path().join("data/conversations.db")).unwrap();
    connection.execute("DROP TABLE sessions", []).unwrap();
    server
        .post("/api/sessions/chat")
        .add_header("origin", "https://evil.example")
        .await
        .assert_status(StatusCode::FORBIDDEN);
    server
        .get("/api/sessions/sess-test/chat")
        .add_header("origin", "https://evil.example")
        .await
        .assert_status(StatusCode::FORBIDDEN);
    let failed = server.post("/api/sessions/chat").await;
    failed.assert_status(StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        failed.json::<Value>(),
        serde_json::json!({"error": "chat session unavailable"})
    );
    let (lan, _dir, _) = setup("0.0.0.0".parse().unwrap());
    lan.post("/api/sessions/chat")
        .await
        .assert_status(StatusCode::FORBIDDEN);
    lan.get("/api/sessions/sess-test/chat")
        .await
        .assert_status(StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn served_openapi_contains_independent_chat_lifecycle() {
    let (server, _dir, _) = setup("127.0.0.1".parse().unwrap());
    let response = server.get("/api/openapi.json").await;
    response.assert_status_ok();
    let body: Value = response.json();
    assert_eq!(
        body["paths"]["/api/sessions/chat"]["post"]["operationId"],
        "createChatSession"
    );
    assert_eq!(
        body["paths"]["/api/sessions/{sessionId}/chat"]["get"]["operationId"],
        "openChatSession"
    );
}
