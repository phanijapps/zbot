# gateway-execution

Agent execution engine. Converts runtime stream events to gateway events, manages agent delegation with callbacks, handles continuation turns, and emits lifecycle events.

## Build & Test

```bash
cargo test -p gateway-execution    # 19 tests
```

## Key Types

| Type | Purpose |
|------|---------|
| `ExecutionRunner` | Main execution orchestrator |
| `ExecutionRunnerConfig` | Construction-time input bundle for `ExecutionRunner::with_config` |
| `ExecutionHandle` | Control handle (stop, pause, resume, cancel) |
| `ExecutionConfig` | Execution configuration |
| `GatewayFileSystem` | `FileSystemContext` implementation with ward support |
| `SessionSetup` | Session initialization parameters |
| `DelegationRegistry` | Tracks active delegations |
| `DelegationRequest` / `DelegationContext` | Delegation types |
| `BatchWriter` | Decouples DB writes from streaming (100ms flush, token coalescing) |

## Public API

| Function | Purpose |
|----------|---------|
| `ExecutionRunner::run()` | Run root agent execution |
| `spawn_delegated_agent()` | Spawn a subagent |
| `handle_delegation_success/failure()` | Delegation callbacks |
| `check_and_spawn_continuation()` | Spawn continuation turns |
| `convert_stream_event()` | StreamEvent → GatewayEvent |
| `get_or_create_session()` | Session lifecycle |
| `start_execution()` / `complete_execution()` | Execution lifecycle |

## File Structure

```
gateway-execution/src/
├── lib.rs                  # Public exports
├── runner.rs               # ExecutionRunner
├── config.rs               # ExecutionConfig, GatewayFileSystem
├── events.rs               # Event conversion (StreamEvent → GatewayEvent)
├── handle.rs               # ExecutionHandle
├── lifecycle.rs            # Session/execution lifecycle
├── continuation.rs         # Continuation logic
├── delegation/
│   ├── mod.rs              # Delegation subsystem
│   ├── callback.rs         # Delegation result handling
│   ├── context.rs          # DelegationContext
│   ├── registry.rs         # DelegationRegistry
│   └── spawn.rs            # Spawn delegated agents
└── invoke/
    ├── executor.rs         # Core executor setup
    ├── batch_writer.rs     # BatchWriter (DB write decoupling)
    └── stream.rs           # Stream processing
```

## Dependencies

Depends on most other gateway sub-crates: gateway-events, gateway-database, gateway-services, gateway-connectors, gateway-templates. Also depends on agent-runtime and agent-tools.

## Conventions

### Context structs over positional arguments

Functions that take ≥7 arguments must take a named-field context struct instead of a positional arg list. Rationale:

- Same-type fields (e.g. four consecutive `&str` ids, or three `Option<Arc<…>>`) can be silently swapped on a positional call — the bug only shows up at runtime in the wrong DB row / event / session.
- Adding a new dependency to a positional signature touches every call site, in order. Adding a new field to a context struct is one line per call site, order-independent.
- `#[allow(clippy::too_many_arguments)]` is a confession, not a solution. Don't use it; introduce a `FooContext` / `FooConfig` struct instead.

Established examples:
- Construction: `ExecutionRunner::with_config(ExecutionRunnerConfig { … })`
- Runner internals: `ExecutionRunner::spawn_execution_task(ExecutionTaskArgs { … })`, `ExecutionRunner::create_executor(CreateExecutorArgs { … })`, `spawn_with_notification(request, &SpawnNotificationDeps { … }, done_tx)`, `invoke_continuation(ContinuationArgs { … })`
- Lifecycle: `complete_execution(CompleteExecution { … })`, `crash_execution(CrashExecution { … })`, `stop_execution(StopExecution { … })`, `emit_delegation_completed(DelegationCompletedEvent { … })`
- Delegation: `spawn_execution_task(SpawnContext { … })`, `handle_execution_success(HandleExecutionSuccess { … })`, `handle_execution_failure(HandleExecutionFailure { … })`
- Batch writer: `BatchWrite::SessionMessage(SessionMessage { … })`

When extending any of these, add the new input as a named field to the relevant struct. Don't grow the positional signature.

### Invariant: zero `#[allow(clippy::too_many_arguments)]` inside this crate

The workspace clippy gate runs with `-D warnings`. Any time you catch yourself reaching for `#[allow(clippy::too_many_arguments)]` instead of adding a context struct, stop — the rule above was introduced to replace exactly that reflex. `runner.rs`, `lifecycle.rs`, and `delegation/spawn.rs` used to have nine such attributes combined; they now have zero, and a new one showing up here will fail CI.

### When to simplify vs. test

If a function has low coverage AND high cognitive complexity, simplify first (split, extract, introduce context structs) and add tests for the smaller resulting pieces. Don't test a god method as-is — you pin the wrong behaviour in place.

If a function has low coverage but IS small and single-purpose, just test it.
