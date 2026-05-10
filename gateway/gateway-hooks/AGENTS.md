# gateway-hooks

Unified abstraction for all inbound message triggers. Hook implementations handle response routing back to the origin (CLI, Web, Cron, etc.).

## Build & Test

```bash
cargo test -p gateway-hooks    # 6 tests
```

## Key Types

| Type | Purpose |
|------|---------|
| `Hook` trait | Interface for responding back to message origin |
| `HookRegistry` | Central registry with EventBus integration |
| `CliHook` | CLI terminal hook |
| `CronHook` | Scheduled task hook |
| `NoOpHook` | No-op hook (testing/fallback) |
| `Attachment` / `ResponseFormat` | Response types |

## Hook Trait

```rust
#[async_trait]
pub trait Hook: Send + Sync {
    fn hook_type(&self) -> HookType;
    fn can_handle(&self, ctx: &HookContext) -> bool;
    async fn respond(
        &self,
        ctx: &HookContext,
        message: &str,
        format: ResponseFormat,
        attachments: Option<Vec<Attachment>>,
    ) -> Result<(), String>;
}
```

`HookContext` and `HookType` are re-exported from `gateway-events`.

## Public API (HookRegistry)

| Method | Purpose |
|--------|---------|
| `register()` | Add a hook implementation |
| `get()` | Get hook by type |
| `respond()` | Route response to correct hook |
| `event_bus()` | Access the event bus |

## File Structure

| File | Purpose |
|------|---------|
| `lib.rs` | Hook trait, public exports |
| `registry.rs` | HookRegistry, Attachment, ResponseFormat |
| `cli.rs` | CliHook (2 tests) |
| `cron.rs` | CronHook (2 tests) |
