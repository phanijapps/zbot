# Task 04: External Hook Registration API

## Context

External hooks are separate services (Node.js, Python, Go, etc.) that integrate with platforms like WhatsApp, Telegram, Signal, and Email. The gateway provides HTTP APIs for these hooks to:
1. Register themselves
2. Invoke agents
3. Receive responses via callbacks

### Key Principle
**External hooks are NOT part of the gateway binary.** They are independent services that communicate via HTTP. This enables:
- Polyglot development (any language)
- Independent deployment and scaling
- Platform-specific logic isolated from core gateway
- Easy addition of new platforms without gateway changes

---

## Specifications (BDD)

### Feature: External Hook Registration

```gherkin
Feature: External Hook Registration
  As an external hook developer
  I want to register my hook with the gateway
  So that my hook can invoke agents and receive responses

  Scenario: Register a new hook
    When I call POST /api/hooks with:
      """
      {
        "id": "whatsapp-prod",
        "name": "WhatsApp Production",
        "callback_url": "http://localhost:3000/webhook/callback",
        "callback_auth": "Bearer my-secret-token",
        "default_agent_id": "root",
        "timeout_ms": 30000,
        "metadata": {
          "phone_number_id": "123456789"
        }
      }
      """
    Then the hook is saved to hooks.json
    And I receive status 201 Created
    And the response contains:
      """
      {
        "id": "whatsapp-prod",
        "name": "WhatsApp Production",
        "enabled": true
      }
      """

  Scenario: Register with duplicate ID fails
    Given hook "whatsapp-prod" already exists
    When I call POST /api/hooks with id "whatsapp-prod"
    Then I receive status 409 Conflict
    And error message "Hook with ID already exists"

  Scenario: Register with invalid callback_url fails
    When I call POST /api/hooks with callback_url "not-a-url"
    Then I receive status 400 Bad Request
    And error message "Invalid callback_url"

  Scenario: List registered hooks
    Given hooks exist:
      | id            | name              | enabled |
      | whatsapp-prod | WhatsApp Prod     | true    |
      | telegram-bot  | Telegram Bot      | true    |
      | email-inbox   | Email Inbox       | false   |
    When I call GET /api/hooks
    Then I receive all 3 hooks
    And callback_auth is NOT included (secret)

  Scenario: Get hook details
    Given hook "whatsapp-prod" exists
    When I call GET /api/hooks/whatsapp-prod
    Then I receive hook details
    And callback_auth is NOT included

  Scenario: Update hook
    Given hook "whatsapp-prod" exists with enabled=true
    When I call PATCH /api/hooks/whatsapp-prod with:
      """
      { "enabled": false }
      """
    Then the hook is disabled
    And hooks.json is updated

  Scenario: Delete hook
    Given hook "whatsapp-prod" exists
    When I call DELETE /api/hooks/whatsapp-prod
    Then the hook is removed from hooks.json
    And I receive status 204 No Content

  Scenario: Test hook callback
    Given hook "whatsapp-prod" exists with callback_url "http://localhost:3000/callback"
    When I call POST /api/hooks/whatsapp-prod/test
    Then gateway sends test request to callback_url:
      """
      {
        "type": "test",
        "hook_id": "whatsapp-prod",
        "timestamp": "2024-01-30T12:00:00Z"
      }
      """
    And if callback responds 200, I receive status 200 OK
    And if callback fails, I receive status 502 Bad Gateway
```

### Feature: Hook Persistence

```gherkin
Feature: Hook Persistence
  As the gateway
  I need to persist hook registrations
  So that hooks survive gateway restarts

  Scenario: Hooks loaded on startup
    Given hooks.json contains:
      """
      {
        "whatsapp-prod": {
          "id": "whatsapp-prod",
          "name": "WhatsApp",
          "callback_url": "http://localhost:3000/callback",
          "enabled": true
        }
      }
      """
    When the gateway starts
    Then hook "whatsapp-prod" is available

  Scenario: Hooks saved on registration
    When I register a new hook
    Then hooks.json is updated atomically
    And the new hook appears in the file
```

---

## Implementation

### File: `application/gateway/src/hooks/external/mod.rs`

```rust
mod config;
mod service;

pub use config::ExternalHookConfig;
pub use service::ExternalHookService;
```

### File: `application/gateway/src/hooks/external/config.rs`

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use serde_json::Value;

/// Configuration for an external hook
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExternalHookConfig {
    /// Unique identifier (e.g., "whatsapp-prod")
    pub id: String,

    /// Human-readable name
    pub name: String,

    /// URL to call with responses
    pub callback_url: String,

    /// Authorization header value for callbacks (secret, never exposed via API)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub callback_auth: Option<String>,

    /// Default agent to invoke if not specified
    #[serde(default = "default_agent_id")]
    pub default_agent_id: String,

    /// Timeout for callback requests in milliseconds
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,

    /// Whether this hook is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Hook-specific metadata
    #[serde(default)]
    pub metadata: HashMap<String, Value>,
}

fn default_agent_id() -> String { "root".to_string() }
fn default_timeout() -> u64 { 30000 }
fn default_enabled() -> bool { true }

/// Response DTO (excludes secrets)
#[derive(Clone, Debug, Serialize)]
pub struct ExternalHookResponse {
    pub id: String,
    pub name: String,
    pub callback_url: String,
    pub default_agent_id: String,
    pub timeout_ms: u64,
    pub enabled: bool,
    pub metadata: HashMap<String, Value>,
}

impl From<&ExternalHookConfig> for ExternalHookResponse {
    fn from(config: &ExternalHookConfig) -> Self {
        Self {
            id: config.id.clone(),
            name: config.name.clone(),
            callback_url: config.callback_url.clone(),
            default_agent_id: config.default_agent_id.clone(),
            timeout_ms: config.timeout_ms,
            enabled: config.enabled,
            metadata: config.metadata.clone(),
        }
    }
}

/// Request to create a hook
#[derive(Clone, Debug, Deserialize)]
pub struct CreateHookRequest {
    pub id: String,
    pub name: String,
    pub callback_url: String,
    pub callback_auth: Option<String>,
    #[serde(default = "default_agent_id")]
    pub default_agent_id: String,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub metadata: HashMap<String, Value>,
}

impl CreateHookRequest {
    pub fn into_config(self) -> ExternalHookConfig {
        ExternalHookConfig {
            id: self.id,
            name: self.name,
            callback_url: self.callback_url,
            callback_auth: self.callback_auth,
            default_agent_id: self.default_agent_id,
            timeout_ms: self.timeout_ms,
            enabled: true,
            metadata: self.metadata,
        }
    }
}

/// Request to update a hook
#[derive(Clone, Debug, Deserialize)]
pub struct UpdateHookRequest {
    pub name: Option<String>,
    pub callback_url: Option<String>,
    pub callback_auth: Option<String>,
    pub default_agent_id: Option<String>,
    pub timeout_ms: Option<u64>,
    pub enabled: Option<bool>,
}
```

### File: `application/gateway/src/hooks/external/service.rs`

```rust
use super::config::{ExternalHookConfig, ExternalHookResponse, CreateHookRequest, UpdateHookRequest};
use reqwest::Client;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use url::Url;

pub struct ExternalHookService {
    hooks: RwLock<HashMap<String, ExternalHookConfig>>,
    config_path: String,
    http_client: Client,
}

impl ExternalHookService {
    pub async fn new(config_path: &str) -> Self {
        let hooks = Self::load_hooks(config_path).await.unwrap_or_default();

        Self {
            hooks: RwLock::new(hooks),
            config_path: config_path.to_string(),
            http_client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap(),
        }
    }

    /// List all hooks (excludes secrets)
    pub async fn list(&self) -> Vec<ExternalHookResponse> {
        self.hooks.read().await
            .values()
            .map(ExternalHookResponse::from)
            .collect()
    }

    /// Get a hook by ID (excludes secrets)
    pub async fn get(&self, id: &str) -> Option<ExternalHookResponse> {
        self.hooks.read().await
            .get(id)
            .map(ExternalHookResponse::from)
    }

    /// Get full hook config (internal use only, includes secrets)
    pub async fn get_config(&self, id: &str) -> Option<ExternalHookConfig> {
        self.hooks.read().await.get(id).cloned()
    }

    /// Create a new hook
    pub async fn create(&self, request: CreateHookRequest) -> Result<ExternalHookResponse, String> {
        // Validate callback_url
        Url::parse(&request.callback_url)
            .map_err(|_| "Invalid callback_url")?;

        let config = request.into_config();
        let response = ExternalHookResponse::from(&config);

        let mut hooks = self.hooks.write().await;

        if hooks.contains_key(&config.id) {
            return Err("Hook with ID already exists".into());
        }

        hooks.insert(config.id.clone(), config);
        drop(hooks);

        self.save_hooks().await?;
        Ok(response)
    }

    /// Update an existing hook
    pub async fn update(&self, id: &str, request: UpdateHookRequest) -> Result<ExternalHookResponse, String> {
        let mut hooks = self.hooks.write().await;

        let config = hooks.get_mut(id).ok_or("Hook not found")?;

        if let Some(name) = request.name {
            config.name = name;
        }
        if let Some(callback_url) = request.callback_url {
            Url::parse(&callback_url).map_err(|_| "Invalid callback_url")?;
            config.callback_url = callback_url;
        }
        if let Some(callback_auth) = request.callback_auth {
            config.callback_auth = Some(callback_auth);
        }
        if let Some(default_agent_id) = request.default_agent_id {
            config.default_agent_id = default_agent_id;
        }
        if let Some(timeout_ms) = request.timeout_ms {
            config.timeout_ms = timeout_ms;
        }
        if let Some(enabled) = request.enabled {
            config.enabled = enabled;
        }

        let response = ExternalHookResponse::from(&*config);
        drop(hooks);

        self.save_hooks().await?;
        Ok(response)
    }

    /// Delete a hook
    pub async fn delete(&self, id: &str) -> Result<(), String> {
        let mut hooks = self.hooks.write().await;
        hooks.remove(id).ok_or("Hook not found")?;
        drop(hooks);

        self.save_hooks().await
    }

    /// Test hook callback connectivity
    pub async fn test(&self, id: &str) -> Result<(), String> {
        let config = self.get_config(id).await.ok_or("Hook not found")?;

        let payload = json!({
            "type": "test",
            "hook_id": id,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });

        let mut request = self.http_client
            .post(&config.callback_url)
            .json(&payload)
            .timeout(std::time::Duration::from_millis(config.timeout_ms));

        if let Some(auth) = &config.callback_auth {
            request = request.header("Authorization", auth);
        }

        let response = request.send().await
            .map_err(|e| format!("Callback request failed: {}", e))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(format!("Callback returned status {}", response.status()))
        }
    }

    async fn load_hooks(path: &str) -> Result<HashMap<String, ExternalHookConfig>, String> {
        match tokio::fs::read_to_string(path).await {
            Ok(content) => serde_json::from_str(&content).map_err(|e| e.to_string()),
            Err(_) => Ok(HashMap::new()),  // File doesn't exist yet
        }
    }

    async fn save_hooks(&self) -> Result<(), String> {
        let hooks = self.hooks.read().await;
        let content = serde_json::to_string_pretty(&*hooks)
            .map_err(|e| e.to_string())?;
        tokio::fs::write(&self.config_path, content).await
            .map_err(|e| e.to_string())
    }
}
```

### File: `application/gateway/src/http/hooks.rs`

```rust
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use crate::hooks::external::{
    ExternalHookResponse, CreateHookRequest, UpdateHookRequest, ExternalHookService,
};
use crate::state::AppState;

/// GET /api/hooks
pub async fn list_hooks(
    State(state): State<AppState>,
) -> Json<Vec<ExternalHookResponse>> {
    Json(state.external_hooks.list().await)
}

/// GET /api/hooks/:id
pub async fn get_hook(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ExternalHookResponse>, StatusCode> {
    state.external_hooks.get(&id).await
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

/// POST /api/hooks
pub async fn create_hook(
    State(state): State<AppState>,
    Json(request): Json<CreateHookRequest>,
) -> Result<(StatusCode, Json<ExternalHookResponse>), (StatusCode, String)> {
    state.external_hooks.create(request).await
        .map(|r| (StatusCode::CREATED, Json(r)))
        .map_err(|e| {
            if e.contains("already exists") {
                (StatusCode::CONFLICT, e)
            } else {
                (StatusCode::BAD_REQUEST, e)
            }
        })
}

/// PATCH /api/hooks/:id
pub async fn update_hook(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<UpdateHookRequest>,
) -> Result<Json<ExternalHookResponse>, (StatusCode, String)> {
    state.external_hooks.update(&id, request).await
        .map(Json)
        .map_err(|e| {
            if e.contains("not found") {
                (StatusCode::NOT_FOUND, e)
            } else {
                (StatusCode::BAD_REQUEST, e)
            }
        })
}

/// DELETE /api/hooks/:id
pub async fn delete_hook(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    state.external_hooks.delete(&id).await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|_| StatusCode::NOT_FOUND)
}

/// POST /api/hooks/:id/test
pub async fn test_hook(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    state.external_hooks.test(&id).await
        .map(|_| StatusCode::OK)
        .map_err(|e| (StatusCode::BAD_GATEWAY, e))
}
```

### Update: `application/gateway/src/hooks/mod.rs`

```rust
mod context;
mod types;
pub mod builtin;
pub mod external;

pub use context::HookContext;
pub use types::{BuiltinHookType, HookType};
pub use external::{ExternalHookConfig, ExternalHookService};
```

---

## Verification

### Unit Tests

```rust
#[tokio::test]
async fn test_create_hook() {
    let service = ExternalHookService::new("/tmp/test-hooks.json").await;

    let request = CreateHookRequest {
        id: "test-hook".into(),
        name: "Test Hook".into(),
        callback_url: "http://localhost:3000/callback".into(),
        callback_auth: Some("Bearer secret".into()),
        default_agent_id: "root".into(),
        timeout_ms: 5000,
        metadata: HashMap::new(),
    };

    let result = service.create(request).await;
    assert!(result.is_ok());

    let hook = service.get("test-hook").await;
    assert!(hook.is_some());
    assert_eq!(hook.unwrap().name, "Test Hook");
}

#[tokio::test]
async fn test_create_duplicate_fails() {
    let service = ExternalHookService::new("/tmp/test-hooks2.json").await;

    let request = CreateHookRequest { id: "dupe".into(), /* ... */ };
    service.create(request.clone()).await.unwrap();

    let result = service.create(request).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("already exists"));
}
```

### API Tests

```bash
# Create hook
curl -X POST http://localhost:18791/api/hooks \
  -H "Content-Type: application/json" \
  -d '{
    "id": "whatsapp-prod",
    "name": "WhatsApp Production",
    "callback_url": "http://localhost:3000/callback",
    "callback_auth": "Bearer my-secret"
  }'

# List hooks (note: callback_auth is NOT returned)
curl http://localhost:18791/api/hooks

# Get specific hook
curl http://localhost:18791/api/hooks/whatsapp-prod

# Update hook
curl -X PATCH http://localhost:18791/api/hooks/whatsapp-prod \
  -H "Content-Type: application/json" \
  -d '{"enabled": false}'

# Test callback
curl -X POST http://localhost:18791/api/hooks/whatsapp-prod/test

# Delete hook
curl -X DELETE http://localhost:18791/api/hooks/whatsapp-prod
```

---

## Dependencies

- Task 01 complete (HookContext)
- `reqwest` crate for HTTP client
- `url` crate for URL validation
- Add to Cargo.toml: `url = "2.5"`

## Outputs

- `application/gateway/src/hooks/external/mod.rs`
- `application/gateway/src/hooks/external/config.rs`
- `application/gateway/src/hooks/external/service.rs`
- `application/gateway/src/http/hooks.rs`
- `hooks.json` - persisted hook registrations

## Next Task

Task 05: External Hook Invocation API
