# zero-session

Session and state management for the Zero framework.

## What It Provides

Concrete implementations of the `Session` and `State` traits defined in `zero-core::context`.

## Key Exports

```rust
pub use zero_core::context::{Session, State};         // re-exported traits

pub use service::{InMemorySessionService, SessionService};
pub use session::{InMemorySession, MutexSession};
pub use state::{validate_key, InMemoryState};
```

## Session Trait (defined in zero-core)

```rust
pub trait Session: Send + Sync {
    fn id(&self) -> &str;
    fn app_name(&self) -> &str;
    fn user_id(&self) -> &str;
    fn state(&self) -> &dyn State;
    fn conversation_history(&self) -> Vec<Content>;  // message history
}
```

## Implementations

| Type | Purpose |
|------|---------|
| `InMemorySession` | In-memory session: ID, app name, user ID, state, `Vec<Content>` history |
| `MutexSession` | Thread-safe wrapper around any `Session` impl (interior-mutable) |
| `InMemoryState` | `HashMap<String, Value>` state store implementing `State` |

## SessionService

```rust
pub trait SessionService: Send + Sync {
    async fn create_session(app_name, user_id) -> Result<Arc<dyn Session>>;
    async fn get_session(session_id) -> Result<Option<Arc<dyn Session>>>;
    async fn delete_session(session_id) -> Result<bool>;
    async fn list_sessions(user_id) -> Result<Vec<Arc<dyn Session>>>;
}
```

`InMemorySessionService` implements this with an `RwLock<HashMap>`.

## Intra-Repo Dependencies

- `zero-core` — `Session`, `State`, `Content` traits and types

## Notes

- Sessions hold conversation history as `Vec<Content>` (role + parts), not event logs.
- State is a key-value store; use the `KEY_PREFIX_*` constants from `zero-core` for scoping.
- Use `validate_key()` before setting state keys to enforce naming conventions.
