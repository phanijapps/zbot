# zero-core

Core traits, types, and errors for the Zero agent framework.

## Setup

```bash
# Build
cargo build

# Run tests
cargo test

# Run with logging
RUST_LOG=debug cargo test
```

## Code Style

- Use `// ===` comment banners for major sections
- Use `// ----` comment banners for subsections
- Prefer `Arc<T>` for shared state across async tasks
- Use `async_trait` for trait methods with async functions
- Return `Result<T>` from fallible operations (where `Result<T> = core::result::Result<T, ZeroError>`)

## Core Types

- `Agent` - Core agent interface with `invoke()` method
- `Tool` - Tool execution interface with `execute()` method
- `Toolset` - Collection of tools with predicate-based filtering
- `ToolContext` - Context provided to tools during execution
- `Event` - Immutable conversation event (user message, tool call, etc.)
- `Content` - Message content with role and parts (text, function calls, etc.)
- `Part` - Individual content part (Text, FunctionCall, FunctionResponse, Binary)

## Testing

Tests use standard `cargo test`. For integration tests that require async:

```rust
#[tokio::test]
async fn test_async_function() {
    // test code
}
```

## Important Notes

- All errors use `ZeroError` from `error.rs`
- FileSystemContext abstraction for file operations - use this instead of direct fs calls
- Events are immutable - create new events rather than modifying
- Conversation ID should be propagated through ToolContext for conversation-scoped operations
