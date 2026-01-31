# Task 06: External Hook Callback & Response Routing

## Context

When an agent uses the `respond()` tool, the gateway needs to route the response back to the originating hook. For external hooks, this means calling the hook's `callback_url`.

### Flow
1. Agent executes and uses `respond("Here's your answer")`
2. Respond tool reads HookContext from execution context
3. Gateway detects external hook (has callback_url)
4. Gateway POSTs to callback_url with response payload
5. External hook receives callback, sends to platform (WhatsApp, Telegram, etc.)

---

## Specifications (BDD)

### Feature: Callback Routing

```gherkin
Feature: External Hook Callback Routing
  As an external hook
  I want to receive agent responses via HTTP callback
  So that I can forward them to my platform

  Background:
    Given hook "whatsapp-prod" is registered with:
      | callback_url  | http://localhost:3000/callback |
      | callback_auth | Bearer secret-token            |

  Scenario: Successful callback delivery
    Given agent is invoked via hook "whatsapp-prod"
    And source_id is "+1234567890"
    When agent uses respond tool with "Hello human!"
    Then gateway sends POST to http://localhost:3000/callback:
      """
      {
        "type": "respond",
        "hook_id": "whatsapp-prod",
        "conversation_id": "conv-uuid",
        "source_id": "+1234567890",
        "channel_id": null,
        "message": "Hello human!",
        "timestamp": "2024-01-30T12:00:00Z"
      }
      """
    And request includes header "Authorization: Bearer secret-token"
    And callback returns 200 OK
    And respond tool returns success

  Scenario: Callback with channel
    Given invocation included channel_id "group-123"
    When agent responds
    Then callback payload includes:
      | channel_id | group-123 |

  Scenario: Callback failure - retry
    Given callback_url returns 500 Internal Server Error
    When agent uses respond tool
    Then gateway retries up to 3 times with exponential backoff
    And if all retries fail, respond tool returns error

  Scenario: Callback timeout
    Given callback_url takes longer than timeout_ms
    When agent uses respond tool
    Then request times out
    And respond tool returns error
    And failure is logged

  Scenario: Callback with attachments (future)
    Given agent uses respond tool with:
      | message     | Here's the file       |
      | attachments | [{"url": "...", "type": "image/png"}] |
    Then callback includes attachments array
```

### Feature: Built-in Hook Routing (No Callback)

```gherkin
Feature: Built-in Hook Response Routing
  As the gateway
  I need to route built-in hook responses directly
  So that Web/CLI/Cron don't need HTTP callbacks

  Scenario: Web hook routes via WebSocket
    Given agent is invoked via Web hook with session_id "sess-123"
    When agent uses respond tool
    Then response is published to EventBus
    And WebSocket session receives Respond event
    And NO HTTP callback is made

  Scenario: CLI hook routes to stdout
    Given agent is invoked via CLI hook
    When agent uses respond tool with "Result: success"
    Then "Result: success" is written to stdout
    And NO HTTP callback is made

  Scenario: Cron hook logs only
    Given agent is invoked via Cron hook
    When agent uses respond tool
    Then response is logged
    And NO HTTP callback is made
```

---

## Implementation

### File: `application/gateway/src/hooks/router.rs`

```rust
use crate::events::EventBus;
use crate::hooks::{
    HookContext, HookType, BuiltinHookType,
    builtin::{WebHook, CronHook},
    external::ExternalHookService,
};
use reqwest::Client;
use serde::Serialize;
use std::sync::Arc;
use std::time::Duration;

/// Routes responses to the appropriate hook
pub struct HookRouter {
    /// For Web hook responses
    web_hook: WebHook,
    /// For Cron hook responses
    cron_hook: CronHook,
    /// For external hook callbacks
    external_hooks: Arc<ExternalHookService>,
    /// HTTP client for callbacks
    http_client: Client,
}

/// Callback payload sent to external hooks
#[derive(Clone, Debug, Serialize)]
pub struct CallbackPayload {
    #[serde(rename = "type")]
    pub payload_type: String,  // "respond"
    pub hook_id: String,
    pub conversation_id: String,
    pub source_id: String,
    pub channel_id: Option<String>,
    pub message: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<Attachment>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Attachment {
    pub url: String,
    pub content_type: String,
    pub filename: Option<String>,
}

impl HookRouter {
    pub fn new(
        event_bus: Arc<EventBus>,
        external_hooks: Arc<ExternalHookService>,
    ) -> Self {
        Self {
            web_hook: WebHook::new(event_bus),
            cron_hook: CronHook::new(),
            external_hooks,
            http_client: Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap(),
        }
    }

    /// Route a response to the appropriate hook
    pub async fn respond(&self, ctx: &HookContext, message: &str) -> Result<(), String> {
        match &ctx.hook_type {
            HookType::Builtin(builtin) => {
                self.route_builtin(builtin, ctx, message).await
            }
            HookType::External { hook_id } => {
                self.route_external(hook_id, ctx, message).await
            }
        }
    }

    async fn route_builtin(
        &self,
        builtin: &BuiltinHookType,
        ctx: &HookContext,
        message: &str,
    ) -> Result<(), String> {
        match builtin {
            BuiltinHookType::Web { .. } => {
                self.web_hook.respond(ctx, message).await
            }
            BuiltinHookType::Cli => {
                // Print to stdout
                println!("{}", message);
                Ok(())
            }
            BuiltinHookType::Cron { .. } => {
                self.cron_hook.respond(ctx, message).await
            }
        }
    }

    async fn route_external(
        &self,
        hook_id: &str,
        ctx: &HookContext,
        message: &str,
    ) -> Result<(), String> {
        let callback_url = ctx.callback_url.as_ref()
            .ok_or("No callback_url in HookContext")?;

        let conversation_id = ctx.conversation_id.as_ref()
            .ok_or("No conversation_id in HookContext")?;

        let payload = CallbackPayload {
            payload_type: "respond".to_string(),
            hook_id: hook_id.to_string(),
            conversation_id: conversation_id.clone(),
            source_id: ctx.source_id.clone(),
            channel_id: ctx.channel_id.clone(),
            message: message.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            attachments: None,
        };

        self.send_callback(callback_url, ctx.callback_auth.as_deref(), &payload).await
    }

    async fn send_callback(
        &self,
        url: &str,
        auth: Option<&str>,
        payload: &CallbackPayload,
    ) -> Result<(), String> {
        let max_retries = 3;
        let mut last_error = String::new();

        for attempt in 0..max_retries {
            if attempt > 0 {
                // Exponential backoff: 1s, 2s, 4s
                let delay = Duration::from_secs(1 << attempt);
                tokio::time::sleep(delay).await;
            }

            let mut request = self.http_client
                .post(url)
                .json(payload);

            if let Some(auth) = auth {
                request = request.header("Authorization", auth);
            }

            match request.send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        tracing::info!(
                            hook_id = %payload.hook_id,
                            url = %url,
                            "Callback delivered successfully"
                        );
                        return Ok(());
                    } else {
                        last_error = format!("Callback returned status {}", response.status());
                        tracing::warn!(
                            hook_id = %payload.hook_id,
                            attempt = attempt + 1,
                            status = %response.status(),
                            "Callback failed, will retry"
                        );
                    }
                }
                Err(e) => {
                    last_error = format!("Callback request failed: {}", e);
                    tracing::warn!(
                        hook_id = %payload.hook_id,
                        attempt = attempt + 1,
                        error = %e,
                        "Callback request failed, will retry"
                    );
                }
            }
        }

        tracing::error!(
            hook_id = %payload.hook_id,
            url = %url,
            error = %last_error,
            "Callback failed after all retries"
        );

        Err(last_error)
    }
}
```

### File: `application/gateway/src/hooks/mod.rs` (updated)

```rust
mod context;
mod types;
mod router;
pub mod builtin;
pub mod external;

pub use context::HookContext;
pub use types::{BuiltinHookType, HookType};
pub use router::{HookRouter, CallbackPayload, Attachment};
pub use external::{ExternalHookConfig, ExternalHookService};
```

### Update: `application/gateway/src/state.rs`

Add HookRouter to AppState:

```rust
pub struct AppState {
    // ... existing fields ...
    pub hook_router: Arc<HookRouter>,
}

impl AppState {
    pub fn new(/* ... */) -> Self {
        let hook_router = Arc::new(HookRouter::new(
            event_bus.clone(),
            external_hooks.clone(),
        ));

        Self {
            // ...
            hook_router,
        }
    }
}
```

---

## Verification

### Unit Tests

```rust
#[tokio::test]
async fn test_builtin_web_routing() {
    let (event_bus, _rx) = create_test_event_bus();
    let external_hooks = Arc::new(ExternalHookService::new("/tmp/hooks.json").await);
    let router = HookRouter::new(event_bus, external_hooks);

    let ctx = HookContext::builtin(
        BuiltinHookType::Web { session_id: "sess-1".into() },
        "sess-1"
    ).with_conversation("conv-1");

    let result = router.respond(&ctx, "Hello").await;
    assert!(result.is_ok());
    // Event should be published to EventBus
}

#[tokio::test]
async fn test_external_callback_routing() {
    // Start mock server
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/callback"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let router = create_test_router();

    let ctx = HookContext::external(
        "test-hook",
        "source-1",
        format!("{}/callback", mock_server.uri()),
    ).with_conversation("conv-1");

    let result = router.respond(&ctx, "Hello external").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_callback_retry_on_failure() {
    let mock_server = MockServer::start().await;

    // First two calls fail, third succeeds
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .up_to_n_times(2)
        .mount(&mock_server)
        .await;

    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let router = create_test_router();
    let ctx = HookContext::external("hook", "src", format!("{}/callback", mock_server.uri()))
        .with_conversation("conv");

    let result = router.respond(&ctx, "retry test").await;
    assert!(result.is_ok());  // Should succeed on 3rd attempt
}
```

### Integration Test

```bash
# Start a simple callback receiver
python3 -c '
from http.server import HTTPServer, BaseHTTPRequestHandler
import json

class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        length = int(self.headers["Content-Length"])
        body = json.loads(self.rfile.read(length))
        print(f"Received callback: {json.dumps(body, indent=2)}")
        self.send_response(200)
        self.end_headers()

HTTPServer(("", 3000), Handler).serve_forever()
' &

# Register hook
curl -X POST http://localhost:18791/api/hooks \
  -H "Content-Type: application/json" \
  -d '{
    "id": "test-hook",
    "name": "Test",
    "callback_url": "http://localhost:3000/callback"
  }'

# Invoke hook (agent will respond, triggering callback)
curl -X POST http://localhost:18791/api/hooks/test-hook/invoke \
  -H "Content-Type: application/json" \
  -d '{
    "source_id": "test-source",
    "message": "Hello"
  }'

# Check callback receiver output
```

---

## Dependencies

- Task 01-05 complete
- `reqwest` for HTTP client
- `wiremock` for testing (optional)

## Outputs

- `application/gateway/src/hooks/router.rs`
- Modified: `state.rs`, `hooks/mod.rs`

## Next Task

Task 07: Respond Tool Integration
