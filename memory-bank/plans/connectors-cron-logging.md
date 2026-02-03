# Connectors, Cron & Logging Design

## Status: IMPLEMENTED ✅

> **Note**: Phases 1-3 and 5 completed on 2026-02-02. Rolling file logs (Phase 4) deferred.

## Overview

Three interconnected features to enable AgentZero to communicate with external systems:

1. **Connector Registry** - External bridges for bidirectional messaging
2. **Cron Scheduler** - Built-in scheduled agent triggers
3. **Rolling File Logs** - Daemon logging to rotating files

---

## 1. Connector Registry

### Concept

Connectors are **external processes** (any language) that register with Gateway to:
- **Receive** messages from agents at end of execution
- **Trigger** agent sessions via Gateway API

Think of them like MCP servers, but for messaging instead of tools.

### Registration

```
POST /api/connectors
{
  "id": "gmail-bridge",
  "name": "Gmail Bridge",
  "transport": {
    "type": "http",
    "callback_url": "http://localhost:9001/callback",
    "method": "POST",
    "headers": { "Authorization": "Bearer xxx" }
  },
  "metadata": {
    "capabilities": [
      { "name": "send_email", "schema": { "to": "string", "subject": "string", "body": "string" } },
      { "name": "get_contacts", "schema": { "query": "string" } }
    ],
    "contacts": [...],
    "additional_info": {...}
  },
  "enabled": true
}
```

### Transport Types

```rust
pub enum ConnectorTransport {
    Http {
        callback_url: String,
        method: String,  // POST, PUT
        headers: HashMap<String, String>,
    },
    Grpc {
        endpoint: String,
        service: String,
        method: String,
    },
    WebSocket {
        url: String,
    },
    Ipc {
        socket_path: String,
    },
    Cli {
        command: String,
        args: Vec<String>,
    },
}
```

### Inbound Flow (Connector → Agent)

```
External Connector
       │
       ▼
POST /api/gateway/submit
{
  "agent_id": "root",
  "message": "User sent: Hello",
  "source": "connector",
  "connector_id": "gmail-bridge",
  "thread_id": "thread-123",
  "respond_to": ["gmail-bridge"]  // Response routing
}
       │
       ▼
   Gateway creates session
   HookContext { connector_id, thread_id, respond_to }
       │
       ▼
   Agent executes...
       │
       ▼
   respond("Here's my answer")
       │
       ▼
   Gateway dispatches to respond_to connectors
       │
       ▼
POST http://localhost:9001/callback
{
  "session_id": "sess-xxx",
  "thread_id": "thread-123",
  "capability": "send_message",
  "payload": { "text": "Here's my answer" }
}
```

### Response Routing

The `respond_to` field in trigger metadata specifies where responses go:

```json
{
  "respond_to": ["gmail-bridge", "slack-notifier"]
}
```

- If empty/null: response goes to web UI only (default behavior)
- If specified: response dispatched to listed connectors
- Original trigger source NOT automatically included (explicit routing)

### Outbound Dispatch

At end of execution, Gateway:
1. Checks `respond_to` from HookContext
2. For each connector_id:
   - Lookup connector config
   - Build payload with capability + response
   - Dispatch via connector's transport type

```rust
impl ConnectorRegistry {
    pub async fn dispatch(
        &self,
        connector_id: &str,
        capability: &str,
        payload: Value,
        context: &DispatchContext,  // session_id, thread_id, etc.
    ) -> Result<(), ConnectorError> {
        let connector = self.get(connector_id)?;

        match &connector.transport {
            ConnectorTransport::Http { callback_url, method, headers } => {
                // HTTP POST to callback
            },
            ConnectorTransport::Cli { command, args } => {
                // Execute command with payload as stdin/arg
            },
            // ... other transports
        }
    }
}
```

### Disable Outbound

Per-connector and global options:

```json
// Connector config
{ "id": "slack", "outbound_enabled": true }

// Agent config
{ "id": "root", "outbound_enabled": false }  // Disable all outbound for this agent

// Session request
{ "outbound_enabled": false }  // Disable for this session only
```

### HTTP Endpoints

```
POST   /api/connectors              - Register connector
GET    /api/connectors              - List all connectors
GET    /api/connectors/:id          - Get connector details
PUT    /api/connectors/:id          - Update connector
DELETE /api/connectors/:id          - Delete connector
GET    /api/connectors/:id/metadata - Get capabilities + metadata
POST   /api/connectors/:id/test     - Test connectivity
```

### Persistence

Store in `connectors.json` in config directory (like `mcps.json`):

```json
{
  "connectors": [
    {
      "id": "gmail-bridge",
      "name": "Gmail Bridge",
      "transport": { "type": "http", "callback_url": "..." },
      "metadata": { "capabilities": [...] },
      "enabled": true,
      "created_at": "2024-01-01T00:00:00Z"
    }
  ]
}
```

---

## 2. Cron Scheduler

### Concept

Built-in scheduler that triggers agents on a schedule. Uses `respond_to` to route outputs to connectors.

### Cron Job Configuration

```
POST /api/cron
{
  "id": "daily-report",
  "name": "Daily Report Generator",
  "schedule": "0 9 * * *",  // 9am daily (cron syntax)
  "agent_id": "report-agent",
  "message": "Generate the daily sales report",
  "respond_to": ["email-connector", "slack-connector"],
  "enabled": true,
  "metadata": {
    "timezone": "America/New_York"
  }
}
```

### Execution Flow

```
Cron Scheduler (internal)
       │
       │ (schedule matches)
       ▼
Gateway.submit_session({
  agent_id: "report-agent",
  message: "Generate the daily sales report",
  source: TriggerSource::Cron,
  respond_to: ["email-connector", "slack-connector"]
})
       │
       ▼
Agent executes...
       │
       ▼
respond("Here's your daily report: ...")
       │
       ▼
Dispatch to email-connector: { capability: "send_email", payload: {...} }
Dispatch to slack-connector: { capability: "send_message", payload: {...} }
```

### HTTP Endpoints

```
POST   /api/cron              - Create cron job
GET    /api/cron              - List all cron jobs
GET    /api/cron/:id          - Get cron job details
PUT    /api/cron/:id          - Update cron job
DELETE /api/cron/:id          - Delete cron job
POST   /api/cron/:id/trigger  - Manually trigger job
POST   /api/cron/:id/enable   - Enable job
POST   /api/cron/:id/disable  - Disable job
```

### Persistence

Store in `cron_jobs.json`:

```json
{
  "jobs": [
    {
      "id": "daily-report",
      "name": "Daily Report Generator",
      "schedule": "0 9 * * *",
      "agent_id": "report-agent",
      "message": "Generate the daily sales report",
      "respond_to": ["email-connector"],
      "enabled": true,
      "last_run": "2024-01-15T09:00:00Z",
      "next_run": "2024-01-16T09:00:00Z",
      "created_at": "2024-01-01T00:00:00Z"
    }
  ]
}
```

### Implementation

Use `tokio-cron-scheduler` crate for scheduling:

```rust
pub struct CronScheduler {
    scheduler: JobScheduler,
    jobs: RwLock<HashMap<String, CronJobConfig>>,
    gateway_bus: Arc<GatewayBus>,
}

impl CronScheduler {
    pub async fn start(&self) -> Result<()> {
        // Load jobs from config
        // Schedule each enabled job
        // Start scheduler
    }

    pub async fn add_job(&self, config: CronJobConfig) -> Result<()> {
        let job = Job::new_async(config.schedule, |_uuid, _lock| {
            Box::pin(async move {
                // Submit session via gateway bus
            })
        })?;
        self.scheduler.add(job).await?;
    }
}
```

---

## 3. Rolling File Logs

### Concept

Daemon logs to rotating files in addition to stdout. Configurable rotation by size and retention.

### Configuration

Via CLI args or config file:

```bash
zerod --log-dir /var/log/agentzero \
      --log-max-size 10MB \
      --log-max-files 5 \
      --log-level info
```

Or in `daemon.yaml`:

```yaml
logging:
  level: info
  directory: /var/log/agentzero
  max_size_mb: 10
  max_files: 5
  stdout: true  # Also log to stdout
```

### Implementation

Use `tracing-appender` with rolling file:

```rust
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn setup_logging(config: &LogConfig) {
    let file_appender = RollingFileAppender::new(
        Rotation::DAILY,  // or by size
        &config.directory,
        "zerod.log",
    );

    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false);

    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stdout);

    tracing_subscriber::registry()
        .with(file_layer)
        .with(stdout_layer)
        .init();
}
```

### Log Files

```
/var/log/agentzero/
├── zerod.2024-01-15.log
├── zerod.2024-01-14.log
├── zerod.2024-01-13.log
└── zerod.log  (current)
```

---

## Data Flow Summary

```
┌─────────────────────────────────────────────────────────────────────┐
│                        TRIGGER SOURCES                               │
├─────────────────────────────────────────────────────────────────────┤
│  Web UI    │  Connector (External)  │  Cron (Internal)  │  API     │
└─────┬──────┴───────────┬────────────┴─────────┬─────────┴────┬─────┘
      │                  │                      │              │
      └──────────────────┴──────────┬───────────┴──────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│                          GATEWAY                                     │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │  POST /api/gateway/submit                                    │   │
│  │  { agent_id, message, source, respond_to: [...] }           │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                              │                                      │
│                              ▼                                      │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │  Session Created                                             │   │
│  │  HookContext { source, connector_id?, respond_to }          │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                              │                                      │
│                              ▼                                      │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │  Agent Execution                                             │   │
│  │  ...processing...                                            │   │
│  │  respond("Final answer")                                     │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                              │                                      │
│                              ▼                                      │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │  Response Router                                             │   │
│  │  for connector_id in respond_to:                            │   │
│  │      ConnectorRegistry.dispatch(connector_id, payload)       │   │
│  └─────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
                              │
          ┌───────────────────┼───────────────────┐
          ▼                   ▼                   ▼
    ┌──────────┐       ┌──────────┐       ┌──────────┐
    │ Web UI   │       │ Email    │       │ Slack    │
    │ (WS)     │       │ Bridge   │       │ Bridge   │
    └──────────┘       └──────────┘       └──────────┘
```

---

## Implementation Phases

### Phase 1: Connector Infrastructure ✅
- [x] `ConnectorConfig` and `ConnectorTransport` types
- [x] `ConnectorRegistry` with CRUD operations
- [x] Persistence to `connectors.json`
- [x] HTTP endpoints for connector management
- [x] Add `respond_to` field to `SessionRequest`

### Phase 2: Response Routing ✅
- [x] Extend `HookContext` with `respond_to: Vec<String>`
- [x] Implement `ConnectorRegistry.dispatch()` for HTTP transport
- [x] Wire response routing at end of execution
- [x] Add CLI transport

### Phase 3: Cron Scheduler ✅
- [x] `CronJobConfig` type
- [x] `CronScheduler` with tokio-cron-scheduler
- [x] Persistence to `cron_jobs.json`
- [x] HTTP endpoints for cron management
- [x] Wire to gateway submit (always routes to root agent)

### Phase 4: Rolling Logs (Deferred)
- [ ] Add logging config to `GatewayConfig`
- [ ] CLI args for log configuration
- [ ] Setup tracing-appender with rolling files
- [ ] Test log rotation

### Phase 5: UI Integration ✅
- [x] Connector management page (WebConnectorsPanel)
- [x] Cron job management page (WebCronPanel)
- [x] Test connector connectivity from UI

---

## Files to Create/Modify

### New Files
```
gateway/src/connectors/
├── mod.rs           # ConnectorRegistry
├── config.rs        # ConnectorConfig, ConnectorTransport
├── dispatch.rs      # Transport dispatch logic
└── service.rs       # CRUD, persistence

gateway/src/cron/
├── mod.rs           # CronScheduler
├── config.rs        # CronJobConfig
└── service.rs       # CRUD, persistence

gateway/src/http/connectors.rs   # HTTP endpoints
gateway/src/http/cron.rs         # HTTP endpoints
```

### Modified Files
```
gateway/src/http/mod.rs          # Add connector + cron routes
gateway/src/state.rs             # Add ConnectorRegistry, CronScheduler
gateway/src/server.rs            # Start cron scheduler
gateway/src/bus/mod.rs           # Add respond_to to SessionRequest
gateway/src/hooks/context.rs     # Add respond_to to HookContext
apps/daemon/src/main.rs          # Add logging config
```

---

## Dependencies

```toml
# Cargo.toml additions
tokio-cron-scheduler = "0.10"
tracing-appender = "0.2"
```
