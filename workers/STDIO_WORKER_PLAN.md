# Plan: STDIO Bridge Workers (MCP-Style)

**Status**: Planned
**Priority**: Medium
**Depends On**: Existing BridgeRegistry, OutboxRepository

---

## Overview

Add STDIO transport for bridge workers, following the same pattern as MCP servers. Gateway spawns worker processes and communicates via stdin/stdout using newline-delimited JSON.

```
                    ┌─────────────────────────────────────┐
                    │            Gateway                  │
                    │                                     │
   spawn ─────────► │  ┌──────────────┐                  │
                    │  │ slack-worker │ stdin/stdout     │
                    │  │ (subprocess) │◄──────────────►  │
                    │  └──────────────┘                  │
                    │                                     │
   spawn ─────────► │  ┌──────────────┐                  │
                    │  │discord-worker│ stdin/stdout     │
                    │  │ (subprocess) │◄──────────────►  │
                    │  └──────────────┘                  │
                    │                                     │
                    │         BridgeRegistry             │
                    └─────────────────────────────────────┘
```

---

## Config Format

**File:** `~/.agentzero/bridge_workers.json`

```json
{
  "workers": {
    "slack-worker": {
      "adapter_id": "slack-worker",
      "command": "/usr/local/bin/slack-bridge",
      "args": ["--mode", "stdio"],
      "env": {
        "SLACK_TOKEN": "${SLACK_BOT_TOKEN}"
      },
      "auto_restart": true,
      "restart_delay_ms": 5000
    },
    "discord-worker": {
      "adapter_id": "discord-worker",
      "command": "node",
      "args": ["/opt/bridges/discord-worker.js"],
      "env": {
        "DISCORD_TOKEN": "${DISCORD_TOKEN}"
      }
    }
  }
}
```

---

## Transport Comparison

| Transport | Who Initiates | Multiple Workers | Cross-Platform |
|-----------|---------------|------------------|----------------|
| WebSocket | Worker connects | Yes | Yes |
| STDIO | Gateway spawns | Yes | Yes |

Both use the same protocol. Only framing differs:
- **WebSocket**: WS frames
- **STDIO**: Newline-delimited JSON

---

## Implementation Phases

### Phase 1: Worker Config

**File:** `gateway-bridge/src/worker_config.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeWorkerConfig {
    pub adapter_id: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default = "default_auto_restart")]
    pub auto_restart: bool,
    #[serde(default = "default_restart_delay")]
    pub restart_delay_ms: u64,
}
```

**File:** `gateway-bridge/src/config_loader.rs`

- Load from `~/.agentzero/bridge_workers.json`
- Expand environment variables in values

### Phase 2: STDIO Worker Process

**File:** `gateway-bridge/src/stdio_worker.rs`

```rust
pub struct StdioWorker {
    config: BridgeWorkerConfig,
    child: Option<Child>,
    stdin: Box<dyn AsyncWrite + Unpin + Send>,
    stdout: Box<dyn AsyncRead + Unpin + Send>,
    registry: Arc<BridgeRegistry>,
    outbox: Arc<OutboxRepository>,
    state: WorkerState,
}

impl StdioWorker {
    pub async fn spawn(&mut self) -> Result<()>;
    pub async fn run(&mut self) -> Result<()>;
    pub async fn send(&mut self, msg: &BridgeServerMessage) -> Result<()>;
    async fn handle_message(&mut self, msg: WorkerMessage) -> Result<()>;
    pub async fn restart(&mut self) -> Result<()>;
}
```

Key responsibilities:
- Spawn subprocess with configured command/args/env
- Write newline-delimited JSON to stdin
- Read newline-delimited JSON from stdout
- Handle Hello handshake
- Route messages to BridgeRegistry/OutboxRepository
- Auto-restart on crash

### Phase 3: Worker Manager

**File:** `gateway-bridge/src/worker_manager.rs`

```rust
pub struct WorkerManager {
    workers: HashMap<String, StdioWorker>,
    registry: Arc<BridgeRegistry>,
    outbox: Arc<OutboxRepository>,
}

impl WorkerManager {
    pub async fn start_all(&mut self, paths: &SharedVaultPaths) -> Result<()>;
    pub async fn start_worker(&mut self, adapter_id: &str) -> Result<()>;
    pub async fn stop_worker(&mut self, adapter_id: &str) -> Result<()>;
    pub async fn restart_worker(&mut self, adapter_id: &str) -> Result<()>;
    pub fn get_status(&self, adapter_id: &str) -> Option<WorkerStatus>;
}
```

### Phase 4: Gateway Integration

**File:** `gateway/src/state.rs`

```rust
// Initialize STDIO workers
let worker_manager = Arc::new(WorkerManager::new(
    bridge_registry.clone(),
    bridge_outbox.clone(),
));

// Spawn workers on startup
worker_manager.start_all(&paths).await?;

AppState {
    // ...existing fields...
    bridge_worker_manager: Some(worker_manager),
}
```

### Phase 5: HTTP API

**File:** `gateway/src/http/bridge_workers.rs`

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/bridge/workers` | GET | List all workers with status |
| `/api/bridge/workers/:id/restart` | POST | Restart a worker |
| `/api/bridge/workers/:id/logs` | GET | Get recent worker logs |

### Phase 6: Reference Implementation

**File:** `workers/echo_worker_stdio/worker.js`

```javascript
const readline = require('readline');

const ADAPTER_ID = process.env.ADAPTER_ID || 'echo-worker-stdio';
let buffer = '';

process.stdin.on('data', (data) => {
  buffer += data.toString();
  const lines = buffer.split('\n');
  buffer = lines.pop();

  for (const line of lines) {
    if (line.trim()) {
      handleMessage(JSON.parse(line));
    }
  }
});

// Send Hello on startup
send(buildHello());

function send(msg) {
  process.stdout.write(JSON.stringify(msg) + '\n');
}

// Same message handlers as WebSocket worker...
```

---

## Files to Create/Modify

| File | Action | Description |
|------|--------|-------------|
| `gateway-bridge/src/worker_config.rs` | Create | Config struct |
| `gateway-bridge/src/config_loader.rs` | Create | Load from JSON |
| `gateway-bridge/src/stdio_worker.rs` | Create | STDIO worker process |
| `gateway-bridge/src/worker_manager.rs` | Create | Manage all workers |
| `gateway-bridge/src/lib.rs` | Modify | Export new modules |
| `gateway/src/state.rs` | Modify | Initialize WorkerManager |
| `gateway/src/http/bridge_workers.rs` | Create | HTTP API |
| `gateway/src/http/mod.rs` | Modify | Add routes |
| `workers/echo_worker_stdio/` | Create | Reference worker |
| `workers/BRIDGE_WORKER_SPEC.md` | Modify | Document STDIO mode |

---

## Protocol (Same as WebSocket)

| Direction | Type | Description |
|-----------|------|-------------|
| W→S | `hello` | Handshake with adapter_id, capabilities, resources |
| S→W | `hello_ack` | Acknowledge registration |
| S→W | `ping` | Heartbeat |
| W→S | `pong` | Heartbeat response |
| S→W | `outbox_item` | Push message for delivery |
| W→S | `ack` / `fail` | Acknowledge delivery |
| S→W | `resource_query` | Query a resource |
| W→S | `resource_response` | Resource data |
| S→W | `capability_invoke` | Invoke a capability |
| W→S | `capability_response` | Invocation result |
| W→S | `inbound` | User message to trigger agent |

**Only difference:** Framing is newline-delimited JSON instead of WebSocket frames.

---

## Cross-Platform Support

| Platform | subprocess + stdin/stdout |
|----------|---------------------------|
| Windows | ✅ tokio::process::Command |
| Linux | ✅ tokio::process::Command |
| macOS | ✅ tokio::process::Command |

Uses `tokio::process::Command` which works on all platforms.

---

## Effort Estimate

| Phase | Effort |
|-------|--------|
| Phase 1: Config | 0.5 day |
| Phase 2: STDIO Worker | 1 day |
| Phase 3: Worker Manager | 0.5 day |
| Phase 4: Gateway Integration | 0.5 day |
| Phase 5: HTTP API | 0.5 day |
| Phase 6: Reference Worker | 0.5 day |
| **Total** | **3-4 days** |

---

## Dependencies

- Existing `gateway-bridge` crate (protocol, registry, outbox)
- `tokio::process` for subprocess management
- `tokio::io` for async stdin/stdout
- Same as MCP implementation pattern in `runtime/agent-runtime/src/mcp/stdio.rs`
