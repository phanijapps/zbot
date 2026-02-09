# daily-sessions

Daily conversation session management with context continuity, message archiving, and system prompt version tracking.

## Build & Test

```bash
cargo test -p daily-sessions    # 16 tests
```

## Key Types

| Type | Purpose |
|------|---------|
| `DailySession` | Daily session for an agent with summary and metadata |
| `SessionMessage` | Message with role, content, tokens, tool calls/results |
| `DaySummary` | UI-friendly session summary |
| `Agent` | Agent metadata with system prompt versioning |
| `SystemPromptCheck` | Result of prompt change detection |

## Public API (DailySessionManager)

| Method | Purpose |
|--------|---------|
| `get_or_create_today()` | Get or create today's session |
| `list_previous_days()` | Browse archived sessions |
| `get_messages()` | Retrieve messages from a session |
| `record_message()` | Store a message |
| `clear_agent_history()` | Delete old sessions |
| `generate_end_of_day_summary()` | LLM-powered summary (stubbed) |

## Trait

```rust
pub trait DailySessionRepository: Send + Sync {
    async fn get_or_create(&self, agent_id: &str, date: NaiveDate) -> Result<DailySession>;
    async fn save_message(&self, session_id: &str, message: SessionMessage) -> Result<()>;
    // ...
}
```

## File Structure

| File | Purpose |
|------|---------|
| `types.rs` | Data types (~15 tests) |
| `manager.rs` | DailySessionManager |
| `repository.rs` | Async repository trait |
| `summary.rs` | Summary generation (~1 test) |
| `cache.rs` | moka caching layer |
| `lib.rs` | Module exports |
