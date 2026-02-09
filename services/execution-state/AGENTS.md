# execution-state

Session lifecycle tracking, agent execution state, token consumption, and dashboard statistics. The most critical service crate — used by the executor, web UI, and dashboard.

## Build & Test

```bash
cargo test -p execution-state    # 82 tests
```

## Key Types

| Type | Purpose |
|------|---------|
| `Session` | Top-level user work session (ward_id, status, trigger source) |
| `AgentExecution` | Individual agent participation within a session |
| `SessionStatus` | Queued, Running, Paused, Completed, Crashed |
| `ExecutionStatus` | Queued, Running, Paused, Crashed, Cancelled, Completed |
| `DelegationType` | Root, Sequential, Parallel |
| `TriggerSource` | Web, Cli, Cron, Api, Plugin |
| `DashboardStats` | Pre-computed session/execution metrics |
| `Checkpoint` | Resume point for paused/crashed executions |

## Public API (StateService)

| Method | Purpose |
|--------|---------|
| `create_session()` | Start a new session |
| `start_session()` | Transition QUEUED → RUNNING |
| `get_session()` / `list_sessions()` | Query sessions |
| `get_session_with_executions()` | Full session detail |
| `update_execution_tokens()` | Track token consumption |
| `complete_execution()` / `complete_session()` | Finalize |
| `pause_session()` / `resume_session()` / `cancel_session()` | Control |
| `get_dashboard_stats()` | Pre-computed metrics |

## HTTP Routes

```
GET    /v2/sessions              — List sessions
GET    /v2/sessions/:id          — Get session
GET    /v2/sessions/:id/full     — Session with executions
DELETE /v2/sessions/:id          — Delete session
POST   /sessions/:id/pause       — Pause
POST   /sessions/:id/resume      — Resume
POST   /sessions/:id/cancel      — Cancel
GET    /executions/:id            — Get execution
GET    /executions/:id/children   — Child executions
GET    /stats                     — Dashboard stats
```

## File Structure

| File | Purpose |
|------|---------|
| `types.rs` | All data types (~34 tests) |
| `service.rs` | StateService implementation (~18 tests) |
| `repository.rs` | Database operations (~29 tests) |
| `handlers.rs` | HTTP route handlers |
| `test_utils.rs` | Test helpers |
| `lib.rs` | Route definitions + public exports |

## Trait

```rust
pub trait StateDbProvider {
    fn get_connection(&self) -> Result<PooledConnection>;
}
```

Gateway's `DatabaseManager` implements this trait.
