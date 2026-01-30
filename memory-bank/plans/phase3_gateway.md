# Phase 3: Gateway Extraction Plan

## Goal

Extract the agent runtime from Tauri into a standalone daemon, enabling:
- Headless operation (CLI, scripts)
- Multiple clients (Tauri, web, CLI)
- Decoupled architecture

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         ZERO DAEMON                                  │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │                    application/gateway                         │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐   │  │
│  │  │ WebSocket   │  │   HTTP      │  │   Event Bus         │   │  │
│  │  │  :18790     │  │   :18791    │  │   (broadcast)       │   │  │
│  │  └─────────────┘  └─────────────┘  └─────────────────────┘   │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                                │                                     │
│  ┌─────────────────────────────▼─────────────────────────────────┐  │
│  │                 application/agent-runtime                      │  │
│  │     (existing: executor, sessions, tools, MCPs)               │  │
│  └───────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
         ▲              ▲                    ▲
         │              │                    │
   ┌─────┴────┐  ┌──────┴──────┐  ┌─────────┴─────────┐
   │ zero CLI │  │ Web Client  │  │   Tauri App       │
   │ (future) │  │  (future)   │  │  (refactored)     │
   └──────────┘  └─────────────┘  └───────────────────┘
```

## New Application Crates

### 1. `application/gateway/`

Gateway library providing HTTP and WebSocket APIs.

```
application/gateway/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Public API
│   ├── server.rs           # Server startup/shutdown
│   ├── websocket/
│   │   ├── mod.rs          # WebSocket handler
│   │   ├── messages.rs     # WS message types
│   │   └── session.rs      # WS session management
│   ├── http/
│   │   ├── mod.rs          # HTTP router
│   │   ├── agents.rs       # Agent CRUD endpoints
│   │   ├── conversations.rs # Conversation endpoints
│   │   ├── tools.rs        # Tool endpoints
│   │   └── health.rs       # Health check
│   └── events/
│       ├── mod.rs          # Event bus
│       └── broadcast.rs    # Client broadcast
```

**Dependencies:**
- `axum` - HTTP framework
- `tokio-tungstenite` - WebSocket
- `tower` - Middleware
- `agent-runtime` - Existing runtime

### 2. `application/daemon/`

Standalone binary that runs the gateway.

```
application/daemon/
├── Cargo.toml
├── src/
│   └── main.rs             # Entry point
```

**Features:**
- Starts gateway server
- Loads configuration from vault
- Handles graceful shutdown
- Logging with tracing

## API Design

### WebSocket API (port 18790)

**Connection:**
```
ws://localhost:18790/ws?agent_id={agent_id}
```

**Client → Server Messages:**
```typescript
// Start a conversation
{ "type": "invoke", "conversation_id": "...", "message": "..." }

// Stop execution
{ "type": "stop", "conversation_id": "..." }

// Continue after iteration limit
{ "type": "continue", "conversation_id": "..." }
```

**Server → Client Messages:**
```typescript
// Streaming token
{ "type": "token", "conversation_id": "...", "delta": "..." }

// Tool call
{ "type": "tool_call", "conversation_id": "...", "tool": "...", "args": {...} }

// Tool result
{ "type": "tool_result", "conversation_id": "...", "result": "..." }

// Turn complete
{ "type": "turn_complete", "conversation_id": "..." }

// Error
{ "type": "error", "conversation_id": "...", "message": "..." }

// Iteration update
{ "type": "iteration", "conversation_id": "...", "current": 5, "max": 25 }
```

### HTTP API (port 18791)

**Agents:**
```
GET    /api/agents              # List agents
GET    /api/agents/:id          # Get agent
POST   /api/agents              # Create agent
PUT    /api/agents/:id          # Update agent
DELETE /api/agents/:id          # Delete agent
```

**Conversations:**
```
GET    /api/conversations              # List conversations
GET    /api/conversations/:id          # Get conversation
POST   /api/conversations              # Create conversation
DELETE /api/conversations/:id          # Delete conversation
GET    /api/conversations/:id/messages # List messages
```

**Tools:**
```
GET    /api/tools                      # List available tools
GET    /api/tools/:name                # Get tool schema
```

**Health:**
```
GET    /api/health                     # Health check
GET    /api/status                     # Detailed status
```

## Implementation Steps

### Step 1: Create gateway crate skeleton ✅

1. ✅ Create `application/gateway/Cargo.toml`
2. ✅ Set up module structure
3. ✅ Add to workspace

### Step 2: Implement HTTP endpoints ✅

1. ✅ Health check endpoint
2. ✅ Agent CRUD (stubs ready)
3. ✅ Conversation CRUD (stubs ready)
4. ✅ Tool listing (stubs ready)

### Step 3: Implement WebSocket handler ✅

1. ✅ Connection management
2. ✅ Message parsing
3. ✅ Event streaming (ExecutionRunner converts StreamEvents to GatewayEvents)
4. ✅ Session tracking

### Step 4: Implement event broadcast ✅

1. ✅ Create event bus
2. ✅ Connect executor events to bus (via ExecutionRunner)
3. ✅ Broadcast to connected clients

### Step 5: Create daemon binary ✅

1. ✅ Create `application/daemon/`
2. ✅ Load configuration
3. ✅ Start gateway
4. ✅ Signal handling

### Step 6: Add AgentService and RuntimeService ✅

1. ✅ Create `services/` module with shared services
2. ✅ Implement `AgentService` (agent CRUD with caching)
3. ✅ Implement `RuntimeService` (execution placeholder)
4. ✅ Create `AppState` for shared state
5. ✅ Wire HTTP handlers to services

### Step 6b: Gateway Verification ✅

Verified with curl tests (2026-01-29):
- ✅ `GET /api/health` returns `{"status":"ok","version":"0.1.0"}`
- ✅ `GET /api/status` returns status with agent count
- ✅ `GET /api/agents` lists agents from vault
- ✅ `GET /api/agents/:id` returns single agent
- ✅ `POST /api/agents` creates agent with config.yaml + AGENTS.md
- ✅ `DELETE /api/agents/:id` removes agent directory

Example run:
```bash
cargo run -p daemon -- --config-dir "C:/Users/rampi/AppData/Roaming/zeroagent" --http-port 18792 --ws-port 18793
curl http://127.0.0.1:18792/api/agents
```

### Step 6c: Executor Integration (Phase 3b) ✅

Created `execution/` module for agent invocation:
- ✅ `ExecutionRunner` - manages agent execution lifecycle
- ✅ `ExecutionConfig` - configuration for invocations
- ✅ `ExecutionHandle` - control handle for stop/continue
- ✅ `convert_stream_event()` - converts `StreamEvent` to `GatewayEvent`
- ✅ Updated `RuntimeService` to use `ExecutionRunner`
- ✅ Updated `AppState` to create runtime with execution runner

Files created:
- `application/gateway/src/execution/mod.rs`
- `application/gateway/src/execution/runner.rs`

The execution module uses `AgentExecutor` from `agent-runtime` crate and:
1. Creates executor with agent config (LlmConfig, ToolRegistry, McpManager)
2. Executes with streaming via `execute_stream()`
3. Converts `StreamEvent` variants to `GatewayEvent` variants
4. Broadcasts events to connected WebSocket clients via EventBus
5. Maintains conversation history in memory

### Step 6d: WebSocket Invocation (Phase 3c) ✅

Updated WebSocket handler for full agent invocation support:
- ✅ Added `RuntimeService` to `WebSocketHandler`
- ✅ Implemented `ClientMessage::Invoke` → `RuntimeService.invoke()`
- ✅ Implemented `ClientMessage::Stop` → `RuntimeService.stop()`
- ✅ Implemented `ClientMessage::Continue` → `RuntimeService.continue_execution()`
- ✅ Subscribe to EventBus and forward events to WebSocket clients
- ✅ Added `gateway_event_to_server_message()` converter
- ✅ Updated `ClientMessage::Invoke` to include `agent_id` field
- ✅ Updated `ClientMessage::Continue` to include `additional_iterations` field
- ✅ Added `ServerMessage` variants: `AgentStarted`, `AgentCompleted`, `AgentStopped`, `Thinking`

Files modified:
- `application/gateway/src/websocket/handler.rs` - Full RuntimeService integration
- `application/gateway/src/websocket/messages.rs` - Extended message types
- `application/gateway/src/server.rs` - Pass RuntimeService to WebSocketHandler

WebSocket clients can now:
1. Connect: `ws://localhost:18790`
2. Receive: `{ "type": "connected", "session_id": "..." }`
3. Invoke: `{ "type": "invoke", "agent_id": "...", "conversation_id": "...", "message": "..." }`
4. Receive stream: `token`, `thinking`, `tool_call`, `tool_result`, `turn_complete`
5. Stop: `{ "type": "stop", "conversation_id": "..." }`
6. Continue: `{ "type": "continue", "conversation_id": "...", "additional_iterations": 25 }`

### Step 7: Refactor Tauri to use gateway (In Progress)

#### Step 7a: Add Gateway Client Module ✅

Created gateway client module in Tauri for connecting to the daemon:

Files created:
- `src-tauri/src/domains/gateway_client/mod.rs` - Module exports
- `src-tauri/src/domains/gateway_client/messages.rs` - Message types (mirror gateway messages)
- `src-tauri/src/domains/gateway_client/client.rs` - WebSocket client implementation
- `src-tauri/src/commands/gateway.rs` - Tauri commands for gateway interaction

Features:
- ✅ `GatewayClient` - WebSocket client with async connect/disconnect
- ✅ `GatewayConnection` - Handle for active connection with send/recv
- ✅ `ConnectionState` - Enum tracking Disconnected/Connecting/Connected/Failed
- ✅ Health check via HTTP (`is_gateway_running()`)
- ✅ Message forwarding from gateway to Tauri frontend events

Tauri commands added:
- `is_gateway_running` - Check if daemon is running
- `get_gateway_status` - Get connection status details
- `connect_to_gateway` - Connect to daemon WebSocket
- `disconnect_from_gateway` - Disconnect from daemon
- `execute_agent_via_gateway` - Invoke agent through gateway
- `stop_agent_via_gateway` - Stop execution through gateway
- `continue_agent_via_gateway` - Continue execution through gateway

Dependencies added to `src-tauri/Cargo.toml`:
- `tokio-tungstenite = "0.24"`
- `futures-util = "0.3"`

#### Step 7b: Gateway Mode Settings ✅

Added settings infrastructure for gateway mode:

Backend changes:
- `src-tauri/src/settings.rs` - Added `RuntimeSettings` struct with:
  - `use_gateway: bool` - Toggle between gateway/direct mode
  - `gateway_ws_port: u16` - WebSocket port (default 18790)
  - `gateway_http_port: u16` - HTTP port (default 18791)
  - `auto_start_gateway: bool` - Auto-start daemon on app launch
- Updated `Settings` to include `runtime: RuntimeSettings` with `#[serde(default)]`
- `src-tauri/src/commands/gateway.rs` - Added commands:
  - `initialize_gateway` - Called on app startup to auto-connect if enabled
  - `get_runtime_settings` - Get current runtime settings
  - `update_runtime_settings` - Update settings with reconnection logic

Frontend changes:
- `src/services/gateway.ts` - TypeScript service for gateway interaction
- `src/features/settings/types.ts` - Added `RuntimeSettings` interface
- `src/features/settings/SettingsPanel.tsx` - Added "Runtime" section with:
  - Toggle for "Use Gateway Daemon"
  - Toggle for "Auto-Start Gateway"
  - Port configuration inputs (shown when gateway enabled)

#### Step 7c: Frontend Integration ✅

Updated frontend to support both direct and gateway execution modes:

- `src/domains/agent-runtime/services/ConversationService.ts`:
  - Added `isGatewayMode()` method to check if gateway is enabled and connected
  - Added `resetGatewayCache()` to invalidate cached gateway state after settings change
  - Refactored `executeAgentStream()` to automatically route to gateway or direct based on settings
  - Added `executeViaGateway()` - executes via gateway daemon with event mapping
  - Added `executeViaDirect()` - original direct Tauri IPC execution
  - Added `stopExecution()` - stops via gateway or direct based on mode
  - Added `continueExecution()` - continues via gateway or direct based on mode
  - Added `mapGatewayEvent()` for event format compatibility

- `src/App.tsx`:
  - Added gateway initialization on app startup after vault system init
  - Logs gateway connection status

#### Step 7d: Gateway Status UI & Daemon Auto-Start ✅

Added gateway status indicator and daemon management:

**Backend (Rust):**
- `src-tauri/src/commands/gateway.rs`:
  - Added `start_daemon_process()` internal function to spawn daemon
  - Searches for daemon binary in: app directory, target/debug, target/release, PATH
  - Waits up to 10 seconds for daemon to become ready
  - Added `start_gateway_daemon` Tauri command for manual start
  - Updated `initialize_gateway` to auto-start daemon when `auto_start_gateway` is enabled

**Frontend (TypeScript):**
- `src/services/gateway.ts`:
  - Added `startGatewayDaemon()` function

- `src/core/layout/StatusBar.tsx`:
  - Added gateway status indicator showing:
    - Green "Gateway" when connected
    - Yellow "Connecting..." during connection
    - Yellow "Starting..." during daemon start
    - Orange "Daemon Off" when daemon not running
    - Red "Failed" on connection failure
    - Gray "Disconnected" when disconnected but daemon running
  - Clicking the indicator:
    - Starts daemon if not running
    - Connects if daemon running but not connected
  - Polls gateway status every 10 seconds

#### Step 7 Complete ✅

The gateway integration in Tauri is now complete:
- Settings UI for gateway mode toggle and port configuration
- Automatic routing of agent execution to gateway when enabled
- Gateway status indicator in status bar
- Daemon auto-start capability
- Manual daemon start via UI click

Remaining optional work:
1. Add reconnection logic on connection loss (auto-reconnect)
2. Add gateway status to agent channel panel header
3. Stop daemon on app close (if auto-started)

## Migration Strategy

**Phase 3a: Gateway alongside Tauri (parallel)**
- Daemon runs independently
- Tauri continues using direct calls
- Can test gateway with CLI/curl

**Phase 3b: Tauri uses gateway (integrated)**
- Tauri starts daemon if not running
- All calls go through gateway
- Direct IPC deprecated

**Phase 3c: Gateway primary (complete)**
- Tauri is pure frontend
- All logic in gateway
- CLI fully functional

## Configuration

```yaml
# .agentzero/daemon.yaml
gateway:
  websocket_port: 18790
  http_port: 18791
  host: "127.0.0.1"

runtime:
  max_parallel_agents: 4
  default_max_iterations: 25

logging:
  level: "info"
  format: "json"
```

## Files to Create

| Path | Purpose |
|------|---------|
| `application/gateway/Cargo.toml` | Gateway crate manifest |
| `application/gateway/src/lib.rs` | Public API |
| `application/gateway/src/server.rs` | Server lifecycle |
| `application/gateway/src/websocket/mod.rs` | WebSocket handler |
| `application/gateway/src/websocket/messages.rs` | Message types |
| `application/gateway/src/http/mod.rs` | HTTP router |
| `application/gateway/src/http/agents.rs` | Agent endpoints |
| `application/gateway/src/http/health.rs` | Health endpoint |
| `application/gateway/src/events/mod.rs` | Event bus |
| `application/daemon/Cargo.toml` | Daemon crate manifest |
| `application/daemon/src/main.rs` | Entry point |

## Success Criteria

1. ✅ `cargo run -p daemon` starts gateway
2. ✅ `curl http://localhost:18791/api/health` returns OK
3. ✅ WebSocket connects and receives events (ExecutionRunner broadcasts via EventBus)
4. ✅ Agents can be invoked via RuntimeService.invoke() (requires OPENAI_API_KEY)
5. ✅ Existing Tauri app continues working (independent operation)
6. ✅ No functionality regression

## Dependencies

```toml
# application/gateway/Cargo.toml
[dependencies]
agent-runtime = { path = "../agent-runtime" }
zero-core = { path = "../../crates/zero-core" }
zero-app = { path = "../../crates/zero-app" }

axum = "0.7"
tokio = { version = "1", features = ["full"] }
tokio-tungstenite = "0.21"
tower = "0.4"
tower-http = { version = "0.5", features = ["cors", "trace"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
uuid = { version = "1", features = ["v4"] }
```
