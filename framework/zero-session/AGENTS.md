# zero-session

Session management for the Zero framework.

## Setup

```bash
# Build
cargo build

# Run tests
cargo test
```

## Code Style

- Sessions are append-only logs of events
- Use `MutexSession` wrapper for thread safety
- Clone sessions for read-only operations

## Session Trait

```rust
#[async_trait]
pub trait Session: Send + Sync {
    async fn append(&self, event: Event) -> Result<()>;
    async fn events(&self) -> Result<Vec<Event>>;
}
```

## Implementations

### InMemorySession

Simple in-memory session storing events in a `Vec<Event>` wrapped in `Mutex`.

### MutexSession

Thread-safe wrapper around any session implementation.

## Events

Events represent conversation state changes:
- User messages
- Assistant messages
- Tool calls
- Tool responses
- Error events

Events are immutable - create new events for updates.

## Testing

Use `tokio::test` for async session operations.

## Important Notes

- Sessions are the source of truth for conversation history
- Agents should append every exchange to the session
- Clone sessions when you need read-only access
