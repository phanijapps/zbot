# gateway-cron

Cron job configuration and persistence for scheduled agent triggers. Schedule validation via tokio-cron-scheduler.

## Build & Test

```bash
cargo test -p gateway-cron    # 5 tests
```

## Key Types

| Type | Purpose |
|------|---------|
| `CronService` | CRUD for cron jobs |
| `CronJobConfig` | Scheduled job config (agent, schedule, message) |
| `CronJobsStore` | File-based persistence |
| `CreateCronJobRequest` / `UpdateCronJobRequest` | API request types |
| `TriggerResult` | Trigger execution result |

## Public API (CronService)

| Method | Purpose |
|--------|---------|
| `new()` | Create service |
| `list()` / `get()` | Query cron jobs |
| `create()` | Create with schedule validation |
| `update()` / `delete()` | CRUD |
| `load()` / `save()` | Persistence |

## File Structure

| File | Purpose |
|------|---------|
| `lib.rs` | Public exports |
| `service.rs` | CronService (5 tests) |
| `config.rs` | Config types |
