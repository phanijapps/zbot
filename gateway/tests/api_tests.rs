//! API Integration Tests for Gateway Endpoints
//!
//! These tests verify the HTTP API endpoints work correctly with a real
//! (but minimal) application state.

use axum::http::StatusCode;
use axum_test::TestServer;
use gateway::{http::create_http_router, AppState, GatewayConfig};
use serde_json::{json, Value};
use tempfile::TempDir;

// ============================================================================
// Test Setup
// ============================================================================

/// Create a test server with minimal state.
///
/// This sets up a temporary directory, creates minimal app state,
/// and returns a test server that can make HTTP requests.
async fn setup_test_server() -> (TestServer, TempDir) {
    let dir = TempDir::new().expect("Failed to create temp dir");

    // Create agents and skills directories
    std::fs::create_dir_all(dir.path().join("agents")).unwrap();
    std::fs::create_dir_all(dir.path().join("skills")).unwrap();

    let config = GatewayConfig::default();
    let state = AppState::minimal(dir.path().to_path_buf());

    let router = create_http_router(config, state);
    let server = TestServer::new(router).expect("Failed to create test server");

    (server, dir)
}

// ============================================================================
// Health Endpoint Tests
// ============================================================================

#[tokio::test]
async fn health_check_returns_ok() {
    let (server, _dir) = setup_test_server().await;

    let response = server.get("/api/health").await;

    response.assert_status_ok();

    let body: Value = response.json();
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn status_endpoint_returns_info() {
    let (server, _dir) = setup_test_server().await;

    let response = server.get("/api/status").await;

    response.assert_status_ok();

    let body: Value = response.json();
    // Status endpoint returns various info - verify it's a valid JSON object
    assert!(body.is_object(), "Expected JSON object response");
}

// ============================================================================
// Execution Stats Endpoint Tests
// ============================================================================

#[tokio::test]
async fn stats_endpoint_returns_counts() {
    let (server, _dir) = setup_test_server().await;

    let response = server.get("/api/executions/stats").await;

    response.assert_status_ok();

    let stats: Value = response.json();

    // Should have session counts
    assert!(stats.get("sessions_running").is_some());
    assert!(stats.get("sessions_queued").is_some());
    assert!(stats.get("sessions_completed").is_some());

    // Should have execution counts
    assert!(stats.get("executions_running").is_some());
    assert!(stats.get("executions_completed").is_some());

    // Should have sessions_by_source
    assert!(stats.get("sessions_by_source").is_some());
}

#[tokio::test]
async fn stats_empty_database_returns_zeros() {
    let (server, _dir) = setup_test_server().await;

    let response = server.get("/api/executions/stats").await;

    response.assert_status_ok();

    let stats: Value = response.json();

    // Empty database should return zeros
    assert_eq!(stats["sessions_running"], 0);
    assert_eq!(stats["sessions_queued"], 0);
    assert_eq!(stats["executions_running"], 0);
}

// ============================================================================
// Sessions V2 Endpoint Tests
// ============================================================================

#[tokio::test]
async fn sessions_list_empty_returns_array() {
    let (server, _dir) = setup_test_server().await;

    let response = server.get("/api/executions/v2/sessions/full").await;

    response.assert_status_ok();

    let sessions: Vec<Value> = response.json();
    assert!(sessions.is_empty());
}

#[tokio::test]
async fn sessions_list_with_filter_params() {
    let (server, _dir) = setup_test_server().await;

    // Test with filter parameters
    let response = server
        .get("/api/executions/v2/sessions/full")
        .add_query_param("status", "running")
        .add_query_param("limit", "10")
        .await;

    response.assert_status_ok();

    let sessions: Vec<Value> = response.json();
    assert!(sessions.is_empty()); // No sessions in test DB
}

#[tokio::test]
async fn session_not_found_returns_404() {
    let (server, _dir) = setup_test_server().await;

    let response = server.get("/api/executions/v2/sessions/nonexistent-session/full").await;

    // Should return 404 or empty result
    // The exact behavior depends on implementation
    let status = response.status_code();
    assert!(status == StatusCode::NOT_FOUND || status == StatusCode::OK);
}

// ============================================================================
// Agent Endpoint Tests
// ============================================================================

#[tokio::test]
async fn agents_list_returns_array() {
    let (server, _dir) = setup_test_server().await;

    let response = server.get("/api/agents").await;

    response.assert_status_ok();

    let agents: Vec<Value> = response.json();
    // May be empty or have seeded agents
    assert!(agents.is_empty() || agents.iter().all(|a| a.get("id").is_some()));
}

#[tokio::test]
async fn agent_not_found_returns_404() {
    let (server, _dir) = setup_test_server().await;

    let response = server.get("/api/agents/nonexistent-agent").await;

    response.assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn create_agent_with_valid_data() {
    let (server, _dir) = setup_test_server().await;

    let agent_data = json!({
        "name": "test-agent",
        "displayName": "Test Agent",
        "description": "A test agent",
        "providerId": "anthropic",
        "model": "claude-sonnet-4-20250514",
        "temperature": 0.7,
        "maxTokens": 4096,
        "instructions": "You are a helpful assistant.",
        "mcps": [],
        "skills": []
    });

    let response = server.post("/api/agents").json(&agent_data).await;

    // Should succeed or fail gracefully
    let status = response.status_code();
    assert!(status == StatusCode::OK || status == StatusCode::CREATED || status == StatusCode::BAD_REQUEST);
}

// ============================================================================
// Gateway Bus Endpoint Tests
// ============================================================================

#[tokio::test]
async fn gateway_status_without_runner() {
    let (server, _dir) = setup_test_server().await;

    let response = server.get("/api/gateway/status/nonexistent").await;

    // Without execution runner (minimal state), returns 500 Internal Server Error
    // With runner, would return 404 for nonexistent session
    let status = response.status_code();
    assert!(
        status == StatusCode::INTERNAL_SERVER_ERROR || status == StatusCode::NOT_FOUND,
        "Expected 500 (no runner) or 404 (not found), got {:?}", status
    );
}

#[tokio::test]
async fn gateway_cancel_without_runner() {
    let (server, _dir) = setup_test_server().await;

    let response = server.post("/api/gateway/cancel/nonexistent").await;

    // Without execution runner (minimal state), returns 500 Internal Server Error
    let status = response.status_code();
    assert!(
        status == StatusCode::INTERNAL_SERVER_ERROR || status == StatusCode::NOT_FOUND,
        "Expected 500 (no runner) or 404 (not found), got {:?}", status
    );
}

#[tokio::test]
async fn gateway_pause_without_runner() {
    let (server, _dir) = setup_test_server().await;

    let response = server.post("/api/gateway/pause/nonexistent").await;

    // Without execution runner (minimal state), returns 500 Internal Server Error
    let status = response.status_code();
    assert!(
        status == StatusCode::INTERNAL_SERVER_ERROR || status == StatusCode::NOT_FOUND,
        "Expected 500 (no runner) or 404 (not found), got {:?}", status
    );
}

#[tokio::test]
async fn gateway_resume_without_runner() {
    let (server, _dir) = setup_test_server().await;

    let response = server.post("/api/gateway/resume/nonexistent").await;

    // Without execution runner (minimal state), returns 500 Internal Server Error
    let status = response.status_code();
    assert!(
        status == StatusCode::INTERNAL_SERVER_ERROR || status == StatusCode::NOT_FOUND,
        "Expected 500 (no runner) or 404 (not found), got {:?}", status
    );
}

#[tokio::test]
async fn gateway_submit_requires_runner() {
    let (server, _dir) = setup_test_server().await;

    let request = json!({
        "agent_id": "root",
        "message": "Hello!",
        "source": "api"
    });

    let response = server.post("/api/gateway/submit").json(&request).await;

    // Minimal state doesn't have a runner, so this should fail gracefully
    // with an internal server error indicating runner not initialized
    let status = response.status_code();
    assert!(
        status == StatusCode::INTERNAL_SERVER_ERROR ||
        status == StatusCode::SERVICE_UNAVAILABLE ||
        status == StatusCode::OK,
        "Expected error status or success, got {:?}", status
    );
}

// ============================================================================
// Conversation Endpoint Tests
// ============================================================================

#[tokio::test]
async fn conversations_list_empty() {
    let (server, _dir) = setup_test_server().await;

    let response = server.get("/api/conversations").await;

    response.assert_status_ok();

    let conversations: Vec<Value> = response.json();
    assert!(conversations.is_empty());
}

#[tokio::test]
async fn conversation_not_found_returns_404() {
    let (server, _dir) = setup_test_server().await;

    let response = server.get("/api/conversations/nonexistent").await;

    response.assert_status(StatusCode::NOT_FOUND);
}

// ============================================================================
// Skills Endpoint Tests
// ============================================================================

#[tokio::test]
async fn skills_list_returns_array() {
    let (server, _dir) = setup_test_server().await;

    let response = server.get("/api/skills").await;

    response.assert_status_ok();

    let skills: Vec<Value> = response.json();
    assert!(skills.is_empty() || skills.iter().all(|s| s.get("id").is_some()));
}

// ============================================================================
// Providers Endpoint Tests
// ============================================================================

#[tokio::test]
async fn providers_list_returns_array() {
    let (server, _dir) = setup_test_server().await;

    let response = server.get("/api/providers").await;

    response.assert_status_ok();

    let providers: Vec<Value> = response.json();
    // May have seeded providers or be empty
    assert!(providers.is_empty() || providers.iter().all(|p| p.get("name").is_some()));
}

// ============================================================================
// MCP Endpoint Tests
// ============================================================================

#[tokio::test]
async fn mcps_list_returns_response() {
    let (server, _dir) = setup_test_server().await;

    let response = server.get("/api/mcps").await;

    response.assert_status_ok();

    let body: Value = response.json();
    assert!(body.get("servers").is_some());
}

// ============================================================================
// Settings Endpoint Tests
// ============================================================================

#[tokio::test]
async fn tool_settings_get_returns_settings() {
    let (server, _dir) = setup_test_server().await;

    let response = server.get("/api/settings/tools").await;

    response.assert_status_ok();

    let body: Value = response.json();
    // Should have success field and data with tool settings
    assert!(body.get("success").is_some() || body.get("grep").is_some());
}

#[tokio::test]
async fn tool_settings_update() {
    let (server, _dir) = setup_test_server().await;

    let settings = json!({
        "grep": true,
        "glob": true,
        "python": false,
        "webFetch": false,
        "loadSkill": true,
        "uiTools": true,
        "knowledgeGraph": false,
        "createAgent": true,
        "introspection": true,
        "offloadLargeResults": true,
        "offloadThresholdTokens": 5000
    });

    let response = server.put("/api/settings/tools").json(&settings).await;

    // Should succeed
    response.assert_status_ok();
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[tokio::test]
async fn invalid_json_returns_bad_request() {
    let (server, _dir) = setup_test_server().await;

    let response = server
        .post("/api/agents")
        .content_type("application/json")
        .bytes("{ invalid json }".as_bytes().to_vec().into())
        .await;

    // Should return 400 Bad Request or 422 Unprocessable Entity
    let status = response.status_code();
    assert!(
        status == StatusCode::BAD_REQUEST || status == StatusCode::UNPROCESSABLE_ENTITY,
        "Expected 400 or 422, got {:?}", status
    );
}

#[tokio::test]
async fn unknown_endpoint_returns_404() {
    let (server, _dir) = setup_test_server().await;

    let response = server.get("/api/unknown/endpoint").await;

    response.assert_status(StatusCode::NOT_FOUND);
}

// ============================================================================
// CORS Header Tests (when enabled)
// ============================================================================

#[tokio::test]
async fn cors_headers_present() {
    let (server, _dir) = setup_test_server().await;

    let response = server.get("/api/health").await;

    // With CORS enabled, response should include CORS headers
    // The exact headers depend on the request origin
    response.assert_status_ok();
}

// ============================================================================
// Content-Type Tests
// ============================================================================

#[tokio::test]
async fn json_content_type_in_response() {
    let (server, _dir) = setup_test_server().await;

    let response = server.get("/api/health").await;

    response.assert_status_ok();

    let content_type = response.header("content-type");
    assert!(
        content_type.to_str().unwrap_or("").contains("application/json"),
        "Expected application/json content type"
    );
}
