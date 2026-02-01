# Task 05: External Hook Invocation API

## Context

External hooks need to trigger agent execution when they receive messages from their platforms (WhatsApp message, Telegram update, email, etc.). This task implements the invocation endpoint that hooks call.

### Flow
1. WhatsApp sends webhook to external WhatsApp hook (Node.js service)
2. WhatsApp hook extracts message, sender phone number
3. WhatsApp hook calls `POST /api/hooks/{hook_id}/invoke`
4. Gateway creates conversation, executes agent with HookContext
5. Gateway calls hook's callback_url with response (Task 06)

---

## Specifications (BDD)

### Feature: Hook Invocation

```gherkin
Feature: External Hook Invocation
  As an external hook
  I want to invoke agents via HTTP API
  So that I can trigger agent execution when receiving platform messages

  Background:
    Given hook "whatsapp-prod" is registered with:
      | callback_url     | http://localhost:3000/callback |
      | default_agent_id | root                           |

  Scenario: Invoke agent via hook
    When I call POST /api/hooks/whatsapp-prod/invoke with:
      """
      {
        "source_id": "+1234567890",
        "message": "Hello, what can you do?"
      }
      """
    Then I receive status 202 Accepted
    And the response contains:
      """
      {
        "conversation_id": "conv-uuid-here",
        "status": "processing"
      }
      """
    And the agent is invoked with HookContext:
      | field           | value                  |
      | hook_type       | External { hook_id: "whatsapp-prod" } |
      | source_id       | "+1234567890"          |
      | callback_url    | http://localhost:3000/callback |

  Scenario: Invoke with specific agent
    When I call POST /api/hooks/whatsapp-prod/invoke with:
      """
      {
        "source_id": "+1234567890",
        "message": "Book a meeting",
        "agent_id": "scheduling-agent"
      }
      """
    Then agent "scheduling-agent" is invoked instead of default

  Scenario: Invoke with channel (group chat)
    When I call POST /api/hooks/whatsapp-prod/invoke with:
      """
      {
        "source_id": "+1234567890",
        "channel_id": "group-chat-123",
        "message": "Hello team"
      }
      """
    Then HookContext has:
      | source_id  | +1234567890     |
      | channel_id | group-chat-123  |
    And conversation is scoped to channel

  Scenario: Invoke with metadata
    When I call POST /api/hooks/whatsapp-prod/invoke with:
      """
      {
        "source_id": "+1234567890",
        "message": "Hi",
        "metadata": {
          "platform_message_id": "wamid.xxx",
          "profile_name": "John Doe"
        }
      }
      """
    Then metadata is passed to agent context

  Scenario: Invoke disabled hook fails
    Given hook "whatsapp-prod" is disabled
    When I call POST /api/hooks/whatsapp-prod/invoke
    Then I receive status 403 Forbidden
    And error message "Hook is disabled"

  Scenario: Invoke non-existent hook fails
    When I call POST /api/hooks/unknown-hook/invoke
    Then I receive status 404 Not Found

  Scenario: Continue existing conversation
    Given a conversation exists for source_id "+1234567890"
    When I invoke again with same source_id
    Then the existing conversation is continued
    And conversation history is maintained
```

### Feature: Conversation Mapping

```gherkin
Feature: Conversation Mapping for Hooks
  As the gateway
  I need to map external source_ids to internal conversations
  So that users have continuous conversation history

  Scenario: First message creates conversation
    Given no conversation exists for hook "whatsapp-prod" and source "+1234567890"
    When hook invokes with source_id "+1234567890"
    Then a new conversation is created
    And mapping is stored: (hook_id, source_id) -> conversation_id

  Scenario: Subsequent messages use existing conversation
    Given conversation "conv-123" exists for (whatsapp-prod, +1234567890)
    When hook invokes with source_id "+1234567890"
    Then conversation "conv-123" is used
    And message is added to existing history

  Scenario: Different sources have separate conversations
    Given hook invokes with source_id "+1111111111"
    And hook invokes with source_id "+2222222222"
    Then two separate conversations exist
    And histories are independent

  Scenario: Channel scopes conversation
    Given hook invokes with source "+1234567890" and channel "group-A"
    And hook invokes with source "+1234567890" and channel "group-B"
    Then two separate conversations exist (different channels)
```

---

## Implementation

### File: `application/gateway/src/hooks/external/invocation.rs`

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Request to invoke agent via external hook
#[derive(Clone, Debug, Deserialize)]
pub struct InvokeRequest {
    /// Unique identifier for the source (phone, email, user ID)
    pub source_id: String,

    /// The message to send to the agent
    pub message: String,

    /// Optional: Override the default agent
    pub agent_id: Option<String>,

    /// Optional: Channel within source (group chat, thread)
    pub channel_id: Option<String>,

    /// Optional: Platform-specific metadata
    #[serde(default)]
    pub metadata: HashMap<String, Value>,
}

/// Response from invocation
#[derive(Clone, Debug, Serialize)]
pub struct InvokeResponse {
    /// The conversation ID (can be used to track/continue)
    pub conversation_id: String,

    /// Status of the invocation
    pub status: InvokeStatus,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InvokeStatus {
    /// Agent is processing the request
    Processing,
    /// Request was queued
    Queued,
}
```

### File: `application/gateway/src/hooks/external/mapper.rs`

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Maps (hook_id, source_id, channel_id) -> conversation_id
pub struct ConversationMapper {
    /// In-memory cache of mappings
    mappings: RwLock<HashMap<String, String>>,

    /// Database repository for persistence
    conversation_repo: Arc<crate::database::ConversationRepository>,
}

impl ConversationMapper {
    pub fn new(conversation_repo: Arc<crate::database::ConversationRepository>) -> Self {
        Self {
            mappings: RwLock::new(HashMap::new()),
            conversation_repo,
        }
    }

    /// Get or create a conversation for this hook + source combination
    pub async fn get_or_create(
        &self,
        hook_id: &str,
        source_id: &str,
        channel_id: Option<&str>,
        agent_id: &str,
    ) -> Result<String, String> {
        let key = Self::make_key(hook_id, source_id, channel_id);

        // Check cache first
        {
            let mappings = self.mappings.read().await;
            if let Some(conv_id) = mappings.get(&key) {
                return Ok(conv_id.clone());
            }
        }

        // Check database
        if let Some(conv_id) = self.find_in_database(hook_id, source_id, channel_id).await? {
            // Update cache
            let mut mappings = self.mappings.write().await;
            mappings.insert(key, conv_id.clone());
            return Ok(conv_id);
        }

        // Create new conversation
        let conversation_id = Uuid::new_v4().to_string();

        // Save to database with hook metadata
        self.conversation_repo.get_or_create_conversation(
            &conversation_id,
            agent_id,
        ).await.map_err(|e| e.to_string())?;

        // Store hook mapping in conversation metadata
        self.store_mapping(hook_id, source_id, channel_id, &conversation_id).await?;

        // Update cache
        {
            let mut mappings = self.mappings.write().await;
            mappings.insert(key, conversation_id.clone());
        }

        Ok(conversation_id)
    }

    fn make_key(hook_id: &str, source_id: &str, channel_id: Option<&str>) -> String {
        match channel_id {
            Some(ch) => format!("{}:{}:{}", hook_id, source_id, ch),
            None => format!("{}:{}", hook_id, source_id),
        }
    }

    async fn find_in_database(
        &self,
        hook_id: &str,
        source_id: &str,
        channel_id: Option<&str>,
    ) -> Result<Option<String>, String> {
        // Query conversations table with metadata filter
        // This would query: metadata->>'hook_id' = ? AND metadata->>'source_id' = ?
        // For now, simplified in-memory only
        Ok(None)
    }

    async fn store_mapping(
        &self,
        hook_id: &str,
        source_id: &str,
        channel_id: Option<&str>,
        conversation_id: &str,
    ) -> Result<(), String> {
        // Store in database for persistence
        // Would update conversation metadata with hook info
        Ok(())
    }
}
```

### File: `application/gateway/src/http/hooks.rs` (additions)

```rust
use crate::hooks::external::invocation::{InvokeRequest, InvokeResponse, InvokeStatus};
use crate::hooks::{HookContext, HookType};

/// POST /api/hooks/:id/invoke
pub async fn invoke_hook(
    State(state): State<AppState>,
    Path(hook_id): Path<String>,
    Json(request): Json<InvokeRequest>,
) -> Result<(StatusCode, Json<InvokeResponse>), (StatusCode, String)> {
    // Get hook config
    let hook_config = state.external_hooks.get_config(&hook_id).await
        .ok_or((StatusCode::NOT_FOUND, "Hook not found".into()))?;

    // Check if enabled
    if !hook_config.enabled {
        return Err((StatusCode::FORBIDDEN, "Hook is disabled".into()));
    }

    // Determine which agent to use
    let agent_id = request.agent_id
        .unwrap_or_else(|| hook_config.default_agent_id.clone());

    // Get or create conversation
    let conversation_id = state.conversation_mapper.get_or_create(
        &hook_id,
        &request.source_id,
        request.channel_id.as_deref(),
        &agent_id,
    ).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    // Create hook context
    let mut hook_context = HookContext::external(
        &hook_id,
        &request.source_id,
        &hook_config.callback_url,
    ).with_conversation(&conversation_id);

    if let Some(auth) = &hook_config.callback_auth {
        hook_context = hook_context.with_callback_auth(auth);
    }

    if let Some(channel) = &request.channel_id {
        hook_context = hook_context.with_channel(channel);
    }

    // Add request metadata to hook context
    for (key, value) in request.metadata {
        hook_context = hook_context.with_metadata(key, value);
    }

    // Invoke agent asynchronously (fire and forget)
    let runtime = state.runtime.clone();
    let message = request.message.clone();
    tokio::spawn(async move {
        if let Err(e) = runtime.invoke_with_hook(
            &agent_id,
            &conversation_id,
            &message,
            hook_context,
        ).await {
            tracing::error!(
                hook_id = %hook_id,
                conversation_id = %conversation_id,
                error = %e,
                "Hook invocation failed"
            );
        }
    });

    Ok((StatusCode::ACCEPTED, Json(InvokeResponse {
        conversation_id,
        status: InvokeStatus::Processing,
    })))
}
```

### Update: `application/gateway/src/execution/runner.rs`

Add `invoke_with_hook` method:

```rust
impl ExecutionRunner {
    /// Invoke agent with hook context for response routing
    pub async fn invoke_with_hook(
        &self,
        agent_id: &str,
        conversation_id: &str,
        message: &str,
        hook_context: HookContext,
    ) -> Result<(), String> {
        // Store hook context in execution state
        // This will be available to the respond tool

        // ... existing invoke logic ...

        // When creating executor context, include hook_context
        let mut context = CallbackContext::new();
        context.set_state("hook_context", hook_context.clone());
        context.set_state("agent_id", agent_id.to_string());
        context.set_state("conversation_id", conversation_id.to_string());

        // Execute with context
        executor.execute_stream_with_context(messages, callback, context).await
    }
}
```

---

## Verification

### Unit Tests

```rust
#[tokio::test]
async fn test_conversation_mapper_creates_new() {
    let repo = Arc::new(MockConversationRepository::new());
    let mapper = ConversationMapper::new(repo);

    let conv_id = mapper.get_or_create(
        "whatsapp-prod",
        "+1234567890",
        None,
        "root"
    ).await.unwrap();

    assert!(!conv_id.is_empty());

    // Same source gets same conversation
    let conv_id_2 = mapper.get_or_create(
        "whatsapp-prod",
        "+1234567890",
        None,
        "root"
    ).await.unwrap();

    assert_eq!(conv_id, conv_id_2);
}

#[tokio::test]
async fn test_different_sources_different_conversations() {
    let mapper = ConversationMapper::new(/* ... */);

    let conv1 = mapper.get_or_create("hook", "source-1", None, "root").await.unwrap();
    let conv2 = mapper.get_or_create("hook", "source-2", None, "root").await.unwrap();

    assert_ne!(conv1, conv2);
}
```

### API Tests

```bash
# Invoke via hook
curl -X POST http://localhost:18791/api/hooks/whatsapp-prod/invoke \
  -H "Content-Type: application/json" \
  -d '{
    "source_id": "+1234567890",
    "message": "Hello, what can you do?"
  }'

# Response:
# {
#   "conversation_id": "conv-uuid",
#   "status": "processing"
# }

# Invoke with channel (group chat)
curl -X POST http://localhost:18791/api/hooks/whatsapp-prod/invoke \
  -H "Content-Type: application/json" \
  -d '{
    "source_id": "+1234567890",
    "channel_id": "group-chat-123",
    "message": "Hello team"
  }'

# Invoke with specific agent
curl -X POST http://localhost:18791/api/hooks/whatsapp-prod/invoke \
  -H "Content-Type: application/json" \
  -d '{
    "source_id": "+1234567890",
    "message": "Book a meeting",
    "agent_id": "scheduling-agent"
  }'
```

---

## Dependencies

- Task 01, 04 complete
- ExecutionRunner modifications

## Outputs

- `application/gateway/src/hooks/external/invocation.rs`
- `application/gateway/src/hooks/external/mapper.rs`
- Modified: `http/hooks.rs`, `execution/runner.rs`

## Next Task

Task 06: External Hook Callback & Response Routing
