---
description: Rust patterns and conventions for AgentZero backend development
---

# AgentZero Rust Development Guide

Use this skill when working on the Rust backend. This captures the architecture, patterns, and conventions used in this codebase.

## Layered Architecture

```
framework/          # Core abstractions (zero-core, zero-prompt)
    ↓
runtime/           # Execution engine (agent-runtime)
    ↓
services/          # Domain services (api-logs, execution-state, agent-tools)
    ↓
gateway/           # HTTP/WebSocket server, orchestration
    ↓
apps/              # Entry points (zerod daemon)
```

**Key principle**: Lower layers never depend on higher layers. Gateway orchestrates everything.

## Crate Organization

```
gateway/
├── src/
│   ├── lib.rs              # Public exports
│   ├── server.rs           # Server startup
│   ├── state.rs            # AppState (shared state)
│   ├── database/           # DB connection and schema
│   │   ├── connection.rs   # DatabaseManager
│   │   ├── repository.rs   # ConversationRepository
│   │   └── schema.rs       # SQL schema initialization
│   ├── events/             # Event bus
│   ├── execution/          # Agent execution
│   │   └── runner.rs       # ExecutionRunner
│   ├── http/               # HTTP endpoints
│   ├── websocket/          # WebSocket handlers
│   └── services/           # Gateway services
│       ├── agents.rs       # AgentService
│       ├── runtime.rs      # RuntimeService
│       └── ...
```

## Shared State Pattern

Use `Arc<T>` for shared state across async tasks:

```rust
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct AppState {
    pub db: Arc<DatabaseManager>,
    pub conversation_repo: Arc<ConversationRepository>,
    pub event_bus: Arc<EventBus>,
    pub agent_service: Arc<AgentService>,
    // ... more services
}

impl AppState {
    pub fn new(/* deps */) -> Self {
        Self {
            db: Arc::new(DatabaseManager::new(/* ... */)),
            // ...
        }
    }
}
```

## Error Handling Pattern

Use `Result<T, String>` for simple error propagation in services:

```rust
pub fn do_something(&self, id: &str) -> Result<SomeType, String> {
    let data = self.repo.get(id)
        .map_err(|e| format!("Failed to get data: {}", e))?;

    // Process data...

    Ok(result)
}
```

For database operations, wrap errors:

```rust
pub fn create(&self, item: &Item) -> Result<(), String> {
    let conn = self.db.get_connection()
        .map_err(|e| format!("Database connection failed: {}", e))?;

    conn.execute(
        "INSERT INTO items (id, name) VALUES (?1, ?2)",
        params![item.id, item.name],
    ).map_err(|e| format!("Database operation failed: {}", e))?;

    Ok(())
}
```

## Async Patterns

### Async methods with Arc<Self>
```rust
impl ExecutionRunner {
    pub async fn invoke(
        &self,
        config: ExecutionConfig,
        message: String,
    ) -> Result<ExecutionHandle, String> {
        // Clone Arcs for spawned tasks
        let conversation_repo = self.conversation_repo.clone();
        let event_bus = self.event_bus.clone();

        tokio::spawn(async move {
            // Use cloned Arcs here
            conversation_repo.add_message(/* ... */);
        });

        Ok(handle)
    }
}
```

### Atomic Flags for Execution Control
```rust
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

pub struct ExecutionHandle {
    stop_flag: Arc<AtomicBool>,
    pause_flag: Arc<AtomicBool>,
    cancel_flag: Arc<AtomicBool>,
    iteration: Arc<AtomicU32>,
}

impl ExecutionHandle {
    pub fn pause(&self) {
        self.pause_flag.store(true, Ordering::SeqCst);
    }

    pub fn is_paused(&self) -> bool {
        self.pause_flag.load(Ordering::SeqCst)
    }

    pub fn resume(&self) {
        self.pause_flag.store(false, Ordering::SeqCst);
    }
}
```

### Pause Loop Pattern
```rust
// In execution loop
while self.handle.is_paused() {
    if self.handle.is_cancelled() {
        return Ok(/* cancelled result */);
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
}
```

## Repository Pattern

Data access through repository structs:

```rust
pub struct ConversationRepository {
    db: Arc<DatabaseManager>,
}

impl ConversationRepository {
    pub fn new(db: Arc<DatabaseManager>) -> Self {
        Self { db }
    }

    pub fn get_or_create_conversation(
        &self,
        conversation_id: &str,
        agent_id: &str,
    ) -> Result<(), String> {
        let conn = self.db.get_connection()
            .map_err(|e| format!("Database connection failed: {}", e))?;

        conn.execute(
            "INSERT OR IGNORE INTO conversations (id, agent_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![conversation_id, agent_id, now, now],
        ).map_err(|e| format!("Database operation failed: {}", e))?;

        Ok(())
    }
}
```

## Service Pattern

Business logic in service structs:

```rust
pub struct StateService<D: DatabaseProvider> {
    repo: ExecutionSessionRepository<D>,
}

impl<D: DatabaseProvider> StateService<D> {
    pub fn new(db: Arc<D>) -> Self {
        Self {
            repo: ExecutionSessionRepository::new(db),
        }
    }

    pub fn create_session(
        &self,
        conversation_id: &str,
        agent_id: &str,
        parent_session_id: Option<String>,
    ) -> Result<ExecutionSession, String> {
        let session = ExecutionSession::new(conversation_id, agent_id, parent_session_id);
        self.repo.insert(&session)?;
        Ok(session)
    }

    pub fn start_session(&self, session_id: &str) -> Result<(), String> {
        self.repo.update_status(session_id, ExecutionStatus::Running)?;
        self.repo.set_started_at(session_id)?;
        Ok(())
    }
}
```

## Tracing for Logging

Use `tracing` crate consistently:

```rust
use tracing::{info, warn, error};

// With structured fields
tracing::info!(
    session_id = %session_id,
    conversation_id = %config.conversation_id,
    agent_id = %config.agent_id,
    "Execution session started"
);

// Warnings with context
tracing::warn!("Failed to create execution session: {}", e);

// Error logging
tracing::error!(error = %e, "Critical failure in execution");
```

## Foreign Key Considerations

When working with SQLite foreign keys:

```rust
// ALWAYS ensure parent record exists before creating child
// Example: conversation must exist before execution_session

// Ensure conversation exists first (required for FK constraint)
if let Err(e) = self.conversation_repo.get_or_create_conversation(
    &config.conversation_id,
    &config.agent_id,
) {
    tracing::warn!("Failed to ensure conversation exists: {}", e);
}

// Now safe to create execution session
let session = self.state_service.create_session(
    &config.conversation_id,
    &config.agent_id,
    None,
)?;
```

## Database Schema Pattern

Define schema in `gateway/src/database/schema.rs`:

```rust
pub fn initialize_schema(conn: &Connection) -> rusqlite::Result<()> {
    // Create parent tables first
    conn.execute(
        "CREATE TABLE IF NOT EXISTS conversations (
            id TEXT PRIMARY KEY,
            agent_id TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
        [],
    )?;

    // Then child tables with FK references
    conn.execute(
        "CREATE TABLE IF NOT EXISTS execution_sessions (
            id TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'queued',
            FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
        )",
        [],
    )?;

    // Create indexes
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_sessions_status
         ON execution_sessions(status)",
        [],
    )?;

    Ok(())
}
```

## WebSocket Message Handling

Define messages with serde:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Invoke {
        agent_id: String,
        conversation_id: String,
        message: String,
    },
    Pause { session_id: String },
    Resume { session_id: String },
    Cancel { session_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    SessionPaused { session_id: String },
    SessionResumed { session_id: String },
    Error { message: String },
}
```

## Module Organization

Each crate should have clear public exports:

```rust
// lib.rs
#![warn(missing_docs)]

//! Crate description here.

mod types;
mod repository;
mod service;

pub use types::{ExecutionSession, ExecutionStatus};
pub use service::StateService;
```

## Testing Patterns

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        let session = ExecutionSession::new("conv-1", "agent-1", None);
        assert_eq!(session.status, ExecutionStatus::Queued);
        assert!(session.started_at.is_none());
    }

    #[tokio::test]
    async fn test_async_operation() {
        // Async test
    }
}
```

## Checklist for New Features

1. **Types**: Define in `types.rs` with Serialize/Deserialize
2. **Repository**: Data access in `repository.rs`
3. **Service**: Business logic in `service.rs`
4. **Schema**: Database tables in `gateway/src/database/schema.rs`
5. **Integration**: Wire into `AppState` and relevant handlers
6. **FK constraints**: Ensure parent records exist before children
7. **Logging**: Add tracing with structured fields
8. **Docs**: Add `#![warn(missing_docs)]` and document public APIs

## Common Pitfalls

1. **FK violations**: Always create parent records before children
2. **Arc cloning**: Clone Arc before moving into async blocks
3. **Atomic ordering**: Use `Ordering::SeqCst` for flags checked across tasks
4. **Database connections**: Get fresh connection for each operation
5. **Error context**: Always include context in error messages
