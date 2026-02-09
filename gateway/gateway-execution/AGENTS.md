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
