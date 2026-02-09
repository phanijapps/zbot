# gateway-database

SQLite connection pool manager with WAL mode, r2d2 pooling, and ConversationRepository for message persistence.

## Build & Test

```bash
cargo test -p gateway-database    # 4 tests
```

## Key Types

| Type | Purpose |
|------|---------|
| `DatabaseManager` | r2d2 connection pool wrapper (max 8, min idle 2) |
| `PragmaCustomizer` | Applies WAL + performance pragmas to pooled connections |
| `ConversationRepository` | CRUD for conversation messages |
| `Message` | Message record (role, content, tool calls, timestamp) |

## Public API

| Method | Purpose |
|--------|---------|
| `DatabaseManager::new()` | Create pool, initialize schema |
| `with_connection()` | Borrow connection from pool |
| `ConversationRepository::add_message()` | Store message |
| `get_messages()` | Get messages for conversation |
| `get_recent_messages()` | Get N most recent messages |

## Trait Implementations

`DatabaseManager` implements both `StateDbProvider` (execution-state) and `DbProvider` (api-logs), bridging services to the connection pool.

## File Structure

| File | Purpose |
|------|---------|
| `lib.rs` | Public exports |
| `connection.rs` | DatabaseManager, PragmaCustomizer, trait impls |
| `repository.rs` | ConversationRepository, Message CRUD (4 tests) |
| `schema.rs` | Schema migrations |
