# Task 01: Hook Context Foundation

## Objective
Create the foundational `HookContext` struct and `HookType` enum that represents the origin of any agent invocation.

## Background
AgentZero needs a unified way to track where agent invocations come from (CLI, Web, Cron, WhatsApp, Telegram, etc.) so responses can be routed back to the correct channel automatically.

## Current State
- Agent invocations go through `ExecutionRunner.invoke()` in `application/gateway/src/execution/runner.rs`
- No tracking of invocation source exists
- Responses stream via WebSocket only

## Deliverables

### 1. Create `application/gateway/src/hooks/mod.rs`
```rust
mod context;
mod registry;

pub use context::{HookContext, HookType};
pub use registry::HookRegistry;
```

### 2. Create `application/gateway/src/hooks/context.rs`
```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Represents the origin of an agent invocation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HookContext {
    /// Type of hook that triggered this invocation
    pub hook_type: HookType,
    /// Unique identifier for the source (phone number, email, session ID)
    pub source_id: String,
    /// Optional channel within source (group chat, thread)
    pub channel_id: Option<String>,
    /// Hook-specific metadata
    pub metadata: HashMap<String, Value>,
    /// When this invocation was triggered
    pub created_at: DateTime<Utc>,
}

impl HookContext {
    pub fn new(hook_type: HookType, source_id: impl Into<String>) -> Self {
        Self {
            hook_type,
            source_id: source_id.into(),
            channel_id: None,
            metadata: HashMap::new(),
            created_at: Utc::now(),
        }
    }

    pub fn with_channel(mut self, channel_id: impl Into<String>) -> Self {
        self.channel_id = Some(channel_id.into());
        self
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

/// Types of hooks that can trigger agent invocations
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HookType {
    /// Command line interface
    Cli,
    /// Web dashboard via WebSocket
    Web { session_id: String },
    /// Scheduled cron job
    Cron { job_id: String },
    /// WhatsApp Business API
    WhatsApp { phone_number_id: String },
    /// Telegram Bot
    Telegram { bot_id: String, chat_id: i64 },
    /// Signal messenger
    Signal { number: String },
    /// Email integration
    Email { account_id: String },
    /// Generic webhook
    Webhook { endpoint_id: String },
}

impl HookType {
    /// Get a string identifier for this hook type
    pub fn type_name(&self) -> &'static str {
        match self {
            HookType::Cli => "cli",
            HookType::Web { .. } => "web",
            HookType::Cron { .. } => "cron",
            HookType::WhatsApp { .. } => "whatsapp",
            HookType::Telegram { .. } => "telegram",
            HookType::Signal { .. } => "signal",
            HookType::Email { .. } => "email",
            HookType::Webhook { .. } => "webhook",
        }
    }
}
```

### 3. Update `application/gateway/src/lib.rs`
Add module declaration:
```rust
pub mod hooks;
```

## Verification
1. Build: `cargo build -p agentzero-gateway`
2. Test serialization:
```rust
#[test]
fn test_hook_context_serialization() {
    let ctx = HookContext::new(
        HookType::WhatsApp { phone_number_id: "123".into() },
        "+1234567890"
    );
    let json = serde_json::to_string(&ctx).unwrap();
    let parsed: HookContext = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.source_id, "+1234567890");
}
```

## Dependencies
- `chrono` (already in Cargo.toml)
- `serde`, `serde_json` (already in Cargo.toml)

## Next Task
Task 02: Hook Trait and Registry
