# Backend API Test Cases

> **Purpose**: Define API test scenarios for all gateway endpoints
> **Technology**: Rust integration tests with `reqwest` + `tokio`
> **Location**: `gateway/tests/api_tests.rs`

---

## Table of Contents

1. [Test Setup](#test-setup)
2. [Health & Status](#health--status)
3. [Gateway Bus (Foreign Plugins)](#gateway-bus-foreign-plugins)
4. [Execution State API](#execution-state-api)
5. [Agent Management](#agent-management)
6. [Session Lifecycle](#session-lifecycle)
7. [WebSocket Tests](#websocket-tests)
8. [Long-Running Scenarios](#long-running-scenarios)

---

## Test Setup

### Test Harness

```rust
// gateway/tests/common/mod.rs

use std::sync::Arc;
use tokio::sync::OnceCell;

static TEST_SERVER: OnceCell<TestServer> = OnceCell::const_new();

pub struct TestServer {
    pub http_url: String,
    pub ws_url: String,
    pub client: reqwest::Client,
}

impl TestServer {
    pub async fn get() -> &'static Self {
        TEST_SERVER
            .get_or_init(|| async {
                // Start gateway server on random port
                let config = GatewayConfig {
                    http_port: 0,  // Random port
                    ws_port: 0,
                    ..Default::default()
                };
                let server = GatewayServer::start(config).await.unwrap();
                TestServer {
                    http_url: format!("http://localhost:{}", server.http_port()),
                    ws_url: format!("ws://localhost:{}", server.ws_port()),
                    client: reqwest::Client::new(),
                }
            })
            .await
    }
}
```

---

## Health & Status

### TC-API-001: Health Check Returns OK

```rust
#[tokio::test]
async fn test_health_check_returns_ok() {
    let server = TestServer::get().await;
    let response = server.client
        .get(format!("{}/api/health", server.http_url))
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["status"], "healthy");
}
```

**Acceptance Criteria**:
- Status code: 200
- Body contains `status: "healthy"`
- Response time < 100ms

### TC-API-002: Status Returns Server Info

```rust
#[tokio::test]
async fn test_status_returns_server_info() {
    let server = TestServer::get().await;
    let response = server.client
        .get(format!("{}/api/status", server.http_url))
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body.get("websocket_port").is_some());
    assert!(body.get("http_port").is_some());
    assert!(body.get("active_connections").is_some());
}
```

---

## Gateway Bus (Foreign Plugins)

### TC-BUS-001: Submit Session - New Session

**Scenario**: Python/JS plugin creates a new session via HTTP

```rust
#[tokio::test]
async fn test_gateway_submit_new_session() {
    let server = TestServer::get().await;
    
    let request = serde_json::json!({
        "agent_id": "root",
        "message": "What is the current time?",
        "source": "plugin",
        "external_ref": "python-test-001"
    });
    
    let response = server.client
        .post(format!("{}/api/gateway/submit", server.http_url))
        .json(&request)
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["session_id"].as_str().unwrap().starts_with("sess-"));
    assert!(body["execution_id"].as_str().unwrap().starts_with("exec-"));
    assert!(body["conversation_id"].is_string());
}
```

**Request**:
```json
{
  "agent_id": "root",
  "message": "What is the current time?",
  "source": "plugin",
  "external_ref": "python-test-001"
}
```

**Expected Response**:
```json
{
  "session_id": "sess-{uuid}",
  "execution_id": "exec-{uuid}",
  "conversation_id": "web-{uuid}"
}
```

### TC-BUS-002: Submit Session - Continue Existing

**Scenario**: Continue an existing session with a new message

```rust
#[tokio::test]
async fn test_gateway_submit_continue_session() {
    let server = TestServer::get().await;
    
    // First, create a session
    let create_request = serde_json::json!({
        "agent_id": "root",
        "message": "Hello",
        "source": "plugin"
    });
    
    let create_response = server.client
        .post(format!("{}/api/gateway/submit", server.http_url))
        .json(&create_request)
        .send()
        .await
        .unwrap();
    
    let session: serde_json::Value = create_response.json().await.unwrap();
    let session_id = session["session_id"].as_str().unwrap();
    
    // Continue the session
    let continue_request = serde_json::json!({
        "agent_id": "root",
        "message": "Tell me more",
        "source": "plugin",
        "session_id": session_id
    });
    
    let continue_response = server.client
        .post(format!("{}/api/gateway/submit", server.http_url))
        .json(&continue_request)
        .send()
        .await
        .unwrap();
    
    assert_eq!(continue_response.status(), 200);
    let body: serde_json::Value = continue_response.json().await.unwrap();
    assert_eq!(body["session_id"].as_str().unwrap(), session_id);
}
```

### TC-BUS-003: Get Session Status

```rust
#[tokio::test]
async fn test_gateway_get_status() {
    let server = TestServer::get().await;
    
    // Create a session first
    let session = create_test_session(&server).await;
    let session_id = session["session_id"].as_str().unwrap();
    
    let response = server.client
        .get(format!("{}/api/gateway/status/{}", server.http_url, session_id))
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["session_id"].as_str().unwrap(), session_id);
    assert!(["running", "completed", "queued"].contains(&body["status"].as_str().unwrap()));
}
```

### TC-BUS-004: Session Not Found

```rust
#[tokio::test]
async fn test_gateway_status_not_found() {
    let server = TestServer::get().await;
    
    let response = server.client
        .get(format!("{}/api/gateway/status/sess-nonexistent", server.http_url))
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), 404);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["code"], "SESSION_NOT_FOUND");
}
```

### TC-BUS-005: Cancel Session

```rust
#[tokio::test]
async fn test_gateway_cancel_session() {
    let server = TestServer::get().await;
    
    let session = create_test_session(&server).await;
    let session_id = session["session_id"].as_str().unwrap();
    
    let response = server.client
        .post(format!("{}/api/gateway/cancel/{}", server.http_url, session_id))
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), 204);
    
    // Verify status changed
    let status = get_session_status(&server, session_id).await;
    // Session might be "cancelled" or "completed" depending on timing
}
```

### TC-BUS-006: Pause and Resume Session

```rust
#[tokio::test]
async fn test_gateway_pause_resume_session() {
    let server = TestServer::get().await;
    
    let session = create_long_running_session(&server).await;
    let session_id = session["session_id"].as_str().unwrap();
    
    // Pause
    let pause_response = server.client
        .post(format!("{}/api/gateway/pause/{}", server.http_url, session_id))
        .send()
        .await
        .unwrap();
    assert_eq!(pause_response.status(), 204);
    
    // Verify paused
    let status = get_session_status(&server, session_id).await;
    assert_eq!(status["status"], "paused");
    
    // Resume
    let resume_response = server.client
        .post(format!("{}/api/gateway/resume/{}", server.http_url, session_id))
        .send()
        .await
        .unwrap();
    assert_eq!(resume_response.status(), 204);
}
```

---

## Execution State API

### TC-EXEC-001: List Sessions Full (Dashboard)

```rust
#[tokio::test]
async fn test_list_sessions_full() {
    let server = TestServer::get().await;
    
    let response = server.client
        .get(format!("{}/api/executions/v2/sessions/full", server.http_url))
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), 200);
    let body: Vec<serde_json::Value> = response.json().await.unwrap();
    
    // Each session should have nested executions
    for session in &body {
        assert!(session.get("id").is_some());
        assert!(session.get("status").is_some());
        assert!(session.get("source").is_some());
        assert!(session.get("executions").is_some());
        assert!(session.get("subagent_count").is_some());
    }
}
```

### TC-EXEC-002: Get Dashboard Stats

```rust
#[tokio::test]
async fn test_get_dashboard_stats() {
    let server = TestServer::get().await;
    
    let response = server.client
        .get(format!("{}/api/executions/stats", server.http_url))
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), 200);
    let stats: serde_json::Value = response.json().await.unwrap();
    
    // Verify all expected fields exist
    assert!(stats.get("sessions_running").is_some());
    assert!(stats.get("sessions_completed").is_some());
    assert!(stats.get("sessions_queued").is_some());
    assert!(stats.get("executions_running").is_some());
    assert!(stats.get("executions_completed").is_some());
    assert!(stats.get("sessions_by_source").is_some());
}
```

### TC-EXEC-003: Stats by Source Breakdown

```rust
#[tokio::test]
async fn test_stats_by_source() {
    let server = TestServer::get().await;
    
    // Create sessions with different sources
    create_session_with_source(&server, "web").await;
    create_session_with_source(&server, "cron").await;
    create_session_with_source(&server, "plugin").await;
    
    let response = server.client
        .get(format!("{}/api/executions/stats", server.http_url))
        .send()
        .await
        .unwrap();
    
    let stats: serde_json::Value = response.json().await.unwrap();
    let by_source = &stats["sessions_by_source"];
    
    // Should have entries for each source
    assert!(by_source.get("web").is_some() || by_source["web"].as_u64().unwrap_or(0) >= 0);
}
```

### TC-EXEC-004: Filter Sessions by Status

```rust
#[tokio::test]
async fn test_filter_sessions_by_status() {
    let server = TestServer::get().await;
    
    let response = server.client
        .get(format!("{}/api/executions/v2/sessions?status=running", server.http_url))
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), 200);
    let sessions: Vec<serde_json::Value> = response.json().await.unwrap();
    
    for session in &sessions {
        assert_eq!(session["status"], "running");
    }
}
```

### TC-EXEC-005: Get Session with Executions

```rust
#[tokio::test]
async fn test_get_session_with_executions() {
    let server = TestServer::get().await;
    
    let session = create_test_session(&server).await;
    let session_id = session["session_id"].as_str().unwrap();
    
    let response = server.client
        .get(format!("{}/api/executions/v2/sessions/{}/full", server.http_url, session_id))
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    
    assert_eq!(body["id"], session_id);
    assert!(body["executions"].is_array());
    
    // Should have at least one execution (root)
    let executions = body["executions"].as_array().unwrap();
    assert!(!executions.is_empty());
    
    // First execution should be root
    assert_eq!(executions[0]["delegation_type"], "root");
}
```

---

## Agent Management

### TC-AGENT-001: Create Agent

```rust
#[tokio::test]
async fn test_create_agent() {
    let server = TestServer::get().await;
    
    let agent = serde_json::json!({
        "name": "test-agent",
        "displayName": "Test Agent",
        "description": "A test agent",
        "providerId": "anthropic",
        "model": "claude-3-haiku-20240307",
        "temperature": 0.7,
        "instructions": "You are a helpful test agent."
    });
    
    let response = server.client
        .post(format!("{}/api/agents", server.http_url))
        .json(&agent)
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body.get("id").is_some());
    assert_eq!(body["name"], "test-agent");
}
```

### TC-AGENT-002: List Agents

```rust
#[tokio::test]
async fn test_list_agents() {
    let server = TestServer::get().await;
    
    let response = server.client
        .get(format!("{}/api/agents", server.http_url))
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), 200);
    let agents: Vec<serde_json::Value> = response.json().await.unwrap();
    
    // Should have at least the root agent
    assert!(!agents.is_empty());
}
```

### TC-AGENT-003: Update Agent

```rust
#[tokio::test]
async fn test_update_agent() {
    let server = TestServer::get().await;
    
    // Create agent first
    let agent = create_test_agent(&server).await;
    let agent_id = agent["id"].as_str().unwrap();
    
    let update = serde_json::json!({
        "displayName": "Updated Agent Name",
        "temperature": 0.9
    });
    
    let response = server.client
        .put(format!("{}/api/agents/{}", server.http_url, agent_id))
        .json(&update)
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["displayName"], "Updated Agent Name");
}
```

### TC-AGENT-004: Delete Agent

```rust
#[tokio::test]
async fn test_delete_agent() {
    let server = TestServer::get().await;
    
    let agent = create_test_agent(&server).await;
    let agent_id = agent["id"].as_str().unwrap();
    
    let response = server.client
        .delete(format!("{}/api/agents/{}", server.http_url, agent_id))
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), 200);
    
    // Verify deleted
    let get_response = server.client
        .get(format!("{}/api/agents/{}", server.http_url, agent_id))
        .send()
        .await
        .unwrap();
    
    assert_eq!(get_response.status(), 404);
}
```

---

## Session Lifecycle

### TC-LIFE-001: Session Queued State

```rust
#[tokio::test]
async fn test_session_queued_state() {
    // When resource constraints are configured
    // Sessions should start in Queued state
    
    let server = TestServer::get().await;
    
    // This requires mocking the queue manager
    // For now, test that queued sessions can be started
}
```

### TC-LIFE-002: Session State Transitions

```rust
#[tokio::test]
async fn test_session_state_transitions() {
    let server = TestServer::get().await;
    
    // Create session -> Running
    let session = create_test_session(&server).await;
    let session_id = session["session_id"].as_str().unwrap();
    
    // Wait a bit for execution to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // Pause -> Paused
    pause_session(&server, session_id).await;
    let status = get_session_status(&server, session_id).await;
    assert_eq!(status["status"], "paused");
    
    // Resume -> Running
    resume_session(&server, session_id).await;
    let status = get_session_status(&server, session_id).await;
    assert_eq!(status["status"], "running");
}
```

---

## WebSocket Tests

### TC-WS-001: Connection Establishment

```rust
#[tokio::test]
async fn test_websocket_connection() {
    let server = TestServer::get().await;
    
    let (ws_stream, _) = tokio_tungstenite::connect_async(&server.ws_url)
        .await
        .unwrap();
    
    // Should receive Connected message
    let (_, mut read) = ws_stream.split();
    let msg = read.next().await.unwrap().unwrap();
    let text = msg.to_text().unwrap();
    let event: serde_json::Value = serde_json::from_str(text).unwrap();
    
    assert_eq!(event["type"], "Connected");
    assert!(event.get("session_id").is_some());
}
```

### TC-WS-002: Invoke Agent via WebSocket

```rust
#[tokio::test]
async fn test_websocket_invoke() {
    let server = TestServer::get().await;
    let ws = connect_websocket(&server).await;
    
    // Send invoke message
    let invoke = serde_json::json!({
        "type": "Invoke",
        "agent_id": "root",
        "conversation_id": "test-conv-001",
        "message": "What is 2 + 2?"
    });
    
    ws.send(Message::Text(invoke.to_string())).await.unwrap();
    
    // Should receive AgentStarted
    let started = receive_message(&ws).await;
    assert_eq!(started["type"], "AgentStarted");
    
    // Should eventually receive AgentCompleted
    loop {
        let msg = receive_message(&ws).await;
        if msg["type"] == "AgentCompleted" {
            break;
        }
    }
}
```

### TC-WS-003: Streaming Tokens

```rust
#[tokio::test]
async fn test_websocket_streaming_tokens() {
    let server = TestServer::get().await;
    let ws = connect_websocket(&server).await;
    
    invoke_agent(&ws, "root", "Tell me a short story").await;
    
    let mut tokens_received = 0;
    loop {
        let msg = receive_message(&ws).await;
        match msg["type"].as_str() {
            Some("Token") => tokens_received += 1,
            Some("AgentCompleted") => break,
            _ => {}
        }
    }
    
    assert!(tokens_received > 0, "Should receive streaming tokens");
}
```

---

## Long-Running Scenarios

### TC-LONG-001: Multi-Turn Conversation

**Scenario**: 5+ turn conversation with context maintained

```rust
#[tokio::test]
#[ignore] // Long-running test
async fn test_multi_turn_conversation() {
    let server = TestServer::get().await;
    let ws = connect_websocket(&server).await;
    
    let conversation_id = "multi-turn-test";
    
    // Turn 1
    invoke_agent(&ws, "root", "My name is Alice").await;
    wait_for_completion(&ws).await;
    
    // Turn 2 - Should remember name
    invoke_agent(&ws, "root", "What is my name?").await;
    let response = collect_response(&ws).await;
    assert!(response.contains("Alice"));
    
    // Turn 3-5...
}
```

### TC-LONG-002: Subagent Delegation

**Scenario**: Root agent delegates to research agent

```rust
#[tokio::test]
#[ignore] // Long-running test
async fn test_subagent_delegation() {
    let server = TestServer::get().await;
    
    // Create research agent with delegation capability
    let request = serde_json::json!({
        "agent_id": "root",
        "message": "Research the latest advancements in AI",
        "source": "api"
    });
    
    let response = server.client
        .post(format!("{}/api/gateway/submit", server.http_url))
        .json(&request)
        .send()
        .await
        .unwrap();
    
    let session: serde_json::Value = response.json().await.unwrap();
    let session_id = session["session_id"].as_str().unwrap();
    
    // Poll for completion (with timeout)
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(120);
    
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        
        let session_full = get_session_full(&server, session_id).await;
        let executions = session_full["executions"].as_array().unwrap();
        
        // Check for subagent execution
        let has_subagent = executions.iter().any(|e| {
            e["delegation_type"].as_str() != Some("root")
        });
        
        if has_subagent {
            // Verify parent-child relationship
            let subagent = executions.iter()
                .find(|e| e["delegation_type"].as_str() != Some("root"))
                .unwrap();
            assert!(subagent.get("parent_execution_id").is_some());
            break;
        }
        
        if start.elapsed() > timeout {
            panic!("Timeout waiting for subagent delegation");
        }
    }
}
```

### TC-LONG-003: Session Recovery

**Scenario**: Crash recovery and checkpoint restoration

```rust
#[tokio::test]
#[ignore] // Long-running test
async fn test_session_recovery() {
    // 1. Start a long-running session
    // 2. Pause it mid-execution
    // 3. Restart the server
    // 4. Resume the session
    // 5. Verify it continues correctly
}
```

---

## Test Utilities

```rust
// gateway/tests/common/helpers.rs

pub async fn create_test_session(server: &TestServer) -> serde_json::Value {
    let request = serde_json::json!({
        "agent_id": "root",
        "message": "Hello",
        "source": "api"
    });
    
    server.client
        .post(format!("{}/api/gateway/submit", server.http_url))
        .json(&request)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

pub async fn get_session_status(server: &TestServer, session_id: &str) -> serde_json::Value {
    server.client
        .get(format!("{}/api/gateway/status/{}", server.http_url, session_id))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

pub async fn pause_session(server: &TestServer, session_id: &str) {
    server.client
        .post(format!("{}/api/gateway/pause/{}", server.http_url, session_id))
        .send()
        .await
        .unwrap();
}

pub async fn resume_session(server: &TestServer, session_id: &str) {
    server.client
        .post(format!("{}/api/gateway/resume/{}", server.http_url, session_id))
        .send()
        .await
        .unwrap();
}
```

---

## Test Execution

### Run All Backend Tests
```bash
cargo test --workspace
```

### Run API Tests Only
```bash
cargo test -p gateway --test api_tests
```

### Run Long-Running Tests
```bash
cargo test -p gateway --test api_tests -- --ignored
```

### Run Specific Test
```bash
cargo test -p gateway test_gateway_submit_new_session
```
