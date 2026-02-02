# Runner.rs Modularization Plan

## Current State

`gateway/src/execution/runner.rs` - **1805 lines** (god file)

### Line Count Breakdown

| Section | Lines | Description |
|---------|-------|-------------|
| DelegationRequest | ~10 | Delegation request struct |
| GatewayFileSystem | ~50 | FileSystemContext impl |
| ExecutionConfig | ~46 | Execution configuration |
| ExecutionHandle | ~89 | Handle for controlling execution |
| ExecutionRunner impl | ~843 | Main runner with invoke, controls |
| spawn_delegated_agent | ~612 | Delegated subagent execution |
| convert_stream_event | ~78 | Event conversion helper |

### Problem Areas

1. **ExecutionRunner.invoke()** (~400 lines) - Does too much:
   - Session/execution creation
   - Agent loading
   - Provider setup
   - MCP initialization
   - Tool registry building
   - Executor creation
   - Event streaming
   - Message persistence
   - Status updates

2. **spawn_delegated_agent()** (~612 lines) - Duplicates invoke logic:
   - Nearly identical setup to invoke()
   - Separate event handling
   - Callback mechanism
   - Status updates

3. **Tight coupling** - Everything depends on everything

## Target Structure

```
gateway/src/execution/
├── mod.rs                 # Re-exports, DelegationRegistry
├── runner.rs              # ExecutionRunner struct, high-level API (~200 lines)
├── config.rs              # ExecutionConfig, GatewayFileSystem (~100 lines)
├── handle.rs              # ExecutionHandle (~100 lines)
├── invoke/
│   ├── mod.rs             # invoke() orchestration (~150 lines)
│   ├── setup.rs           # Agent/provider/MCP setup (~200 lines)
│   ├── executor.rs        # Executor creation & config (~150 lines)
│   └── stream.rs          # Stream event handling (~200 lines)
├── delegation/
│   ├── mod.rs             # Re-exports, DelegationRegistry
│   ├── context.rs         # DelegationContext (existing)
│   ├── request.rs         # DelegationRequest, spawning (~100 lines)
│   └── handler.rs         # Delegated execution logic (~400 lines)
├── lifecycle.rs           # Session/execution state management (~150 lines)
└── events.rs              # Event conversion, emission helpers (~100 lines)
```

**Target: ~1500 lines total across 12 files, avg 125 lines/file**

## Module Responsibilities

### runner.rs (Core)
```rust
pub struct ExecutionRunner {
    // Dependencies only
    handles: Arc<RwLock<HashMap<String, ExecutionHandle>>>,
    delegation_tx: mpsc::UnboundedSender<DelegationRequest>,
    // ... services
}

impl ExecutionRunner {
    pub fn new(...) -> Self
    pub async fn invoke(...) -> Result<...>  // Delegates to invoke module
    pub async fn stop(...) -> Result<...>
    pub async fn pause(...) -> Result<...>
    pub async fn resume(...) -> Result<...>
    pub async fn cancel(...) -> Result<...>
    pub fn delegation_registry(&self) -> Arc<DelegationRegistry>
}
```

### config.rs
```rust
pub struct ExecutionConfig { ... }
pub struct GatewayFileSystem { ... }
impl FileSystemContext for GatewayFileSystem { ... }
```

### handle.rs
```rust
pub struct ExecutionHandle { ... }
impl ExecutionHandle {
    pub fn stop(&self)
    pub fn pause(&self)
    pub fn resume(&self)
    pub fn cancel(&self)
    // ... status queries
}
```

### invoke/mod.rs
```rust
pub async fn execute_agent(
    runner: &ExecutionRunner,
    config: ExecutionConfig,
    message: &str,
) -> Result<(ExecutionHandle, String), String>
```

### invoke/setup.rs
```rust
pub struct AgentSetup {
    pub agent: Agent,
    pub provider: Provider,
    pub tool_registry: ToolRegistry,
    pub mcp_manager: McpManager,
}

pub async fn setup_agent(
    agent_id: &str,
    services: &Services,
    config_dir: &Path,
) -> Result<AgentSetup, String>
```

### invoke/executor.rs
```rust
pub fn build_executor(
    setup: &AgentSetup,
    config: &ExecutionConfig,
) -> Result<AgentExecutor, String>
```

### invoke/stream.rs
```rust
pub fn handle_stream_event(
    event: StreamEvent,
    context: &StreamContext,
) -> Option<GatewayEvent>

pub struct StreamContext {
    agent_id: String,
    conversation_id: String,
    session_id: String,
    execution_id: String,
    // ... callbacks for delegation, logging
}
```

### delegation/request.rs
```rust
pub struct DelegationRequest { ... }

pub async fn spawn_delegation(
    runner: &ExecutionRunner,
    request: DelegationRequest,
) -> Result<String, String>
```

### delegation/handler.rs
```rust
pub async fn execute_delegated_agent(
    request: DelegationRequest,
    services: &Services,
    event_bus: Arc<EventBus>,
) -> Result<String, String>
```

### lifecycle.rs
```rust
pub fn create_session_and_execution(
    state_service: &StateService,
    agent_id: &str,
    session_id: Option<&str>,
) -> Result<(Session, AgentExecution), String>

pub fn complete_execution(...)
pub fn try_complete_session(...)
pub fn crash_execution(...)
```

### events.rs
```rust
pub fn convert_stream_event(
    event: StreamEvent,
    agent_id: &str,
    conversation_id: &str,
    session_id: &str,
) -> GatewayEvent

pub async fn emit_agent_started(...)
pub async fn emit_agent_completed(...)
pub async fn emit_delegation_started(...)
```

## Shared Traits

### ExecutionServices
```rust
pub struct ExecutionServices {
    pub agent_service: Arc<AgentService>,
    pub provider_service: Arc<ProviderService>,
    pub mcp_service: Arc<McpService>,
    pub settings_service: Arc<SettingsService>,
    pub state_service: Arc<StateService<DatabaseManager>>,
    pub log_service: Arc<LogService<DatabaseManager>>,
    pub conversation_repo: ConversationRepository,
    pub event_bus: Arc<EventBus>,
}
```

This allows passing all services as a single reference.

## Refactoring Strategy

### Phase 1: Extract Non-Breaking Modules
1. Move ExecutionConfig + GatewayFileSystem → `config.rs`
2. Move ExecutionHandle → `handle.rs`
3. Move convert_stream_event → `events.rs`
4. Move DelegationRequest → `delegation/request.rs`

### Phase 2: Extract Setup Logic
1. Create `invoke/setup.rs` with agent/provider/MCP setup
2. Create `invoke/executor.rs` with executor building
3. Refactor invoke() to use these modules

### Phase 3: Extract Stream Handling
1. Create `invoke/stream.rs` with event handling
2. Create StreamContext struct
3. Refactor event callbacks to use stream module

### Phase 4: Extract Delegation
1. Move spawn_delegated_agent → `delegation/handler.rs`
2. Refactor to reuse setup/executor modules
3. Eliminate duplication between invoke and delegation

### Phase 5: Extract Lifecycle
1. Create `lifecycle.rs` with state management
2. Move session/execution creation, completion, crash handling
3. Centralize status transitions

### Phase 6: Final Cleanup
1. Slim down runner.rs to high-level API only
2. Add comprehensive documentation
3. Add unit tests for each module

## Success Criteria

- [ ] No file > 300 lines
- [ ] Each module has single responsibility
- [ ] Shared logic extracted (no duplication between invoke/delegation)
- [ ] Clear dependency graph (no circular deps)
- [ ] Easy to add orchestration features
- [ ] Testable in isolation

## Benefits for Orchestration

After modularization:
1. **Delegation handler** can be enhanced independently
2. **Lifecycle module** can add "waiting for delegations" state
3. **Events module** can add callback injection
4. **Stream handler** can pause/resume cleanly
5. **Clear boundaries** make orchestration implementation safer

## Estimated Effort

| Phase | Files | Estimated Time |
|-------|-------|----------------|
| Phase 1 | 4 | 1-2 hours |
| Phase 2 | 2 | 2-3 hours |
| Phase 3 | 1 | 2-3 hours |
| Phase 4 | 2 | 3-4 hours |
| Phase 5 | 1 | 2-3 hours |
| Phase 6 | All | 2-3 hours |
| **Total** | **12** | **12-18 hours** |
