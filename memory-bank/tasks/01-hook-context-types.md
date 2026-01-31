# Task 01: Hook Context and Types

## Context

AgentZero needs a unified way to track the origin of every agent invocation so responses can be routed back correctly. This is the foundation for the entire hook system.

### What We're Building
A `HookContext` struct that travels with every agent invocation, containing:
- Where the request came from (CLI, Web, Cron, or external hook)
- Who sent it (source_id like phone number, email, session)
- How to respond (callback URL for external, direct for built-in)

### Why This Matters
Without this context, an agent has no way to know if it should:
- Print to stdout (CLI)
- Send WebSocket event (Web)
- Log only (Cron)
- Call external webhook (WhatsApp, Telegram)

---

## Specifications (BDD)

### Feature: Hook Context Creation

```gherkin
Feature: Hook Context Creation
  As the gateway
  I need to create a HookContext for every invocation
  So that responses can be routed correctly

  Scenario: Create Web hook context
    Given a WebSocket connection with session_id "sess-123"
    When the user sends a message via WebSocket
    Then a HookContext is created with:
      | field       | value                    |
      | hook_type   | Web { session_id: "sess-123" } |
      | source_id   | "sess-123"               |
      | is_builtin  | true                     |

  Scenario: Create CLI hook context
    Given a CLI invocation
    When the user runs "agentzero chat 'hello'"
    Then a HookContext is created with:
      | field       | value     |
      | hook_type   | Cli       |
      | source_id   | "cli"     |
      | is_builtin  | true      |

  Scenario: Create Cron hook context
    Given a cron job with id "daily-report"
    When the cron triggers
    Then a HookContext is created with:
      | field       | value                      |
      | hook_type   | Cron { job_id: "daily-report" } |
      | source_id   | "cron:daily-report"        |
      | is_builtin  | true                       |

  Scenario: Create external hook context
    Given an external hook "whatsapp-prod" is registered
    And it has callback_url "http://localhost:3000/callback"
    When the hook invokes the gateway with source_id "+1234567890"
    Then a HookContext is created with:
      | field        | value                     |
      | hook_type    | External { hook_id: "whatsapp-prod" } |
      | source_id    | "+1234567890"             |
      | is_builtin   | false                     |
      | callback_url | "http://localhost:3000/callback" |
```

### Feature: Hook Context Serialization

```gherkin
Feature: Hook Context Serialization
  As the execution system
  I need to serialize/deserialize HookContext
  So that it can be passed through the execution pipeline

  Scenario: Serialize to JSON
    Given a HookContext for Web with session_id "abc"
    When I serialize it to JSON
    Then the JSON contains:
      """
      {
        "hook_type": { "type": "web", "session_id": "abc" },
        "source_id": "abc",
        "is_builtin": true
      }
      """

  Scenario: Deserialize from JSON
    Given JSON:
      """
      {
        "hook_type": { "type": "external", "hook_id": "telegram-bot" },
        "source_id": "chat:12345",
        "callback_url": "http://localhost:4000/webhook"
      }
      """
    When I deserialize it
    Then I get a HookContext with hook_type External
    And callback_url is "http://localhost:4000/webhook"
```

---

## Implementation

### File: `application/gateway/src/hooks/mod.rs`

```rust
//! Hook system for routing agent invocations and responses.
//!
//! Built-in hooks (CLI, Web, Cron) are handled directly.
//! External hooks connect via HTTP APIs.

mod context;
mod types;

pub use context::HookContext;
pub use types::{BuiltinHookType, ExternalHookConfig, HookType};
```

### File: `application/gateway/src/hooks/types.rs`

```rust
use serde::{Deserialize, Serialize};

/// Types of built-in hooks (part of gateway binary)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BuiltinHookType {
    /// Command line interface - responds to stdout
    Cli,
    /// Web dashboard - responds via WebSocket
    Web { session_id: String },
    /// Scheduled job - logs only, no response
    Cron { job_id: String },
}

/// Configuration for an external hook
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExternalHookConfig {
    /// Unique identifier for this hook
    pub hook_id: String,
    /// Human-readable name
    pub name: String,
    /// URL to call with responses
    pub callback_url: String,
    /// Authorization header value for callbacks
    pub callback_auth: Option<String>,
    /// Default agent to invoke
    pub default_agent_id: String,
    /// Timeout for callback requests (ms)
    pub timeout_ms: u64,
}

/// Union of all hook types
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "category", rename_all = "snake_case")]
pub enum HookType {
    /// Built-in hook (CLI, Web, Cron)
    Builtin(BuiltinHookType),
    /// External hook (WhatsApp, Telegram, etc.)
    External { hook_id: String },
}

impl HookType {
    pub fn is_builtin(&self) -> bool {
        matches!(self, HookType::Builtin(_))
    }

    pub fn type_name(&self) -> &str {
        match self {
            HookType::Builtin(BuiltinHookType::Cli) => "cli",
            HookType::Builtin(BuiltinHookType::Web { .. }) => "web",
            HookType::Builtin(BuiltinHookType::Cron { .. }) => "cron",
            HookType::External { hook_id } => hook_id,
        }
    }
}
```

### File: `application/gateway/src/hooks/context.rs`

```rust
use super::types::HookType;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Context that travels with every agent invocation.
/// Used to route responses back to the correct destination.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HookContext {
    /// Type of hook (builtin or external)
    pub hook_type: HookType,

    /// Unique identifier for the source
    /// - CLI: "cli"
    /// - Web: session_id
    /// - Cron: "cron:{job_id}"
    /// - External: platform-specific (phone number, chat_id, email)
    pub source_id: String,

    /// Optional channel within source (group chat, thread)
    pub channel_id: Option<String>,

    /// Conversation ID in gateway's database
    pub conversation_id: Option<String>,

    /// For external hooks: URL to call with response
    pub callback_url: Option<String>,

    /// For external hooks: Auth header for callback
    pub callback_auth: Option<String>,

    /// Hook-specific metadata
    pub metadata: HashMap<String, Value>,

    /// When this invocation was created
    pub created_at: DateTime<Utc>,
}

impl HookContext {
    /// Create context for a built-in hook
    pub fn builtin(hook_type: super::types::BuiltinHookType, source_id: impl Into<String>) -> Self {
        Self {
            hook_type: HookType::Builtin(hook_type),
            source_id: source_id.into(),
            channel_id: None,
            conversation_id: None,
            callback_url: None,
            callback_auth: None,
            metadata: HashMap::new(),
            created_at: Utc::now(),
        }
    }

    /// Create context for an external hook
    pub fn external(
        hook_id: impl Into<String>,
        source_id: impl Into<String>,
        callback_url: impl Into<String>,
    ) -> Self {
        Self {
            hook_type: HookType::External {
                hook_id: hook_id.into(),
            },
            source_id: source_id.into(),
            channel_id: None,
            conversation_id: None,
            callback_url: Some(callback_url.into()),
            callback_auth: None,
            metadata: HashMap::new(),
            created_at: Utc::now(),
        }
    }

    pub fn with_channel(mut self, channel_id: impl Into<String>) -> Self {
        self.channel_id = Some(channel_id.into());
        self
    }

    pub fn with_conversation(mut self, conversation_id: impl Into<String>) -> Self {
        self.conversation_id = Some(conversation_id.into());
        self
    }

    pub fn with_callback_auth(mut self, auth: impl Into<String>) -> Self {
        self.callback_auth = Some(auth.into());
        self
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Check if this is a built-in hook
    pub fn is_builtin(&self) -> bool {
        self.hook_type.is_builtin()
    }

    /// Check if this hook expects a callback (external hooks)
    pub fn expects_callback(&self) -> bool {
        self.callback_url.is_some()
    }
}
```

### File: `application/gateway/src/lib.rs` (modification)

Add to existing module declarations:
```rust
pub mod hooks;
```

---

## Verification

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_web_context() {
        let ctx = HookContext::builtin(
            BuiltinHookType::Web { session_id: "sess-123".into() },
            "sess-123"
        );

        assert!(ctx.is_builtin());
        assert!(!ctx.expects_callback());
        assert_eq!(ctx.source_id, "sess-123");
    }

    #[test]
    fn test_external_context() {
        let ctx = HookContext::external(
            "whatsapp-prod",
            "+1234567890",
            "http://localhost:3000/callback"
        ).with_callback_auth("Bearer token123");

        assert!(!ctx.is_builtin());
        assert!(ctx.expects_callback());
        assert_eq!(ctx.callback_url, Some("http://localhost:3000/callback".into()));
        assert_eq!(ctx.callback_auth, Some("Bearer token123".into()));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let ctx = HookContext::builtin(
            BuiltinHookType::Cron { job_id: "daily".into() },
            "cron:daily"
        );

        let json = serde_json::to_string(&ctx).unwrap();
        let parsed: HookContext = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.source_id, "cron:daily");
        assert!(matches!(
            parsed.hook_type,
            HookType::Builtin(BuiltinHookType::Cron { .. })
        ));
    }
}
```

### Build Verification

```bash
cd application/gateway
cargo build
cargo test hooks
```

---

## Dependencies

- `chrono` - timestamps (already in Cargo.toml)
- `serde`, `serde_json` - serialization (already in Cargo.toml)

## Outputs

- `application/gateway/src/hooks/mod.rs`
- `application/gateway/src/hooks/types.rs`
- `application/gateway/src/hooks/context.rs`

## Next Task

Task 02: Web Hook (Built-in) - Route WebSocket responses through hook system
