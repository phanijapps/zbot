# Gateway

Network layer providing HTTP and WebSocket APIs for agent interaction. Decomposed into 10 focused sub-crates with a thin shell for HTTP routes, WebSocket handler, and AppState wiring.

## Crate Structure

```
gateway/
├── gateway-events/      # EventBus, GatewayEvent, HookContext, HookType
├── gateway-database/    # DatabaseManager (r2d2 pool, WAL), ConversationRepository
├── gateway-templates/   # System prompt assembly, shard injection
├── gateway-connectors/  # ConnectorRegistry, dispatch to external services
├── gateway-services/    # AgentService, ProviderService, McpService, SkillService, SettingsService
├── gateway-execution/   # ExecutionRunner, delegation, lifecycle, streaming, BatchWriter
├── gateway-hooks/       # Hook trait, HookRegistry, CliHook, CronHook, NoOpHook
├── gateway-cron/        # CronJobConfig, CronService, CRUD types
├── gateway-bus/         # GatewayBus trait, SessionRequest, SessionHandle
├── gateway-ws-protocol/ # ClientMessage, ServerMessage, SubscriptionScope
├── src/                 # Thin shell: HTTP routes, WS handler, AppState
│   ├── http/            #   REST API routes (agents, providers, skills, webhooks, etc.)
│   ├── websocket/       #   WebSocket handler + subscription manager
│   ├── bus/             #   HttpGatewayBus (composes execution runner with bus trait)
│   ├── hooks/           #   WebHook (depends on WS module)
│   └── state.rs         #   AppState (wires all services together)
└── templates/           # System prompt templates (embedded at compile time)
    ├── instructions_starter.md
    └── shards/
        ├── tooling_skills.md
        └── memory_learning.md
```

## Sub-Crate Responsibilities

| Crate | Purpose | Tests |
|-------|---------|-------|
| `gateway-events` | Event bus (broadcast), GatewayEvent types, HookContext | 8 |
| `gateway-database` | r2d2 connection pool, schema migrations, ConversationRepository | 4 |
| `gateway-templates` | Prompt assembly from INSTRUCTIONS.md + embedded shards | 10 |
| `gateway-connectors` | HTTP/CLI connectors for dispatching agent responses | 10 |
| `gateway-services` | Config services with RwLock caching (agents, providers, MCPs, skills, settings) | 5 |
| `gateway-execution` | Execution engine: runner, delegation, lifecycle, stream processing, BatchWriter | 19 |
| `gateway-hooks` | Hook trait + registry (CLI, Cron, NoOp implementations) | 6 |
| `gateway-cron` | Cron job config types and CRUD service | 5 |
| `gateway-bus` | GatewayBus trait, SessionRequest/Handle types | 21 |
| `gateway-ws-protocol` | WebSocket message types (Client/Server), subscription scopes | 7 |
| `gateway` (shell) | HTTP routes, WS handler, AppState, static files | 54 |

## Dependency Direction

```
gateway-events (foundation — no gateway deps)
    ├── gateway-database
    ├── gateway-templates
    ├── gateway-connectors
    ├── gateway-services
    ├── gateway-hooks
    ├── gateway-cron
    ├── gateway-bus
    ├── gateway-ws-protocol
    └── gateway-execution (depends on most above)
            │
            └── gateway (thin shell — composes everything)
```

## Ports

| Port | Protocol | Purpose |
|------|----------|---------|
| 18791 | HTTP | REST API + Static files |
| 18790 | WebSocket | Real-time streaming |

## HTTP Endpoints

### Agents
- `GET /api/agents` — List agents
- `POST /api/agents` — Create agent
- `GET /api/agents/:id` — Get agent
- `PUT /api/agents/:id` — Update agent
- `DELETE /api/agents/:id` — Delete agent

### Providers
- `GET /api/providers` — List providers
- `POST /api/providers` — Create provider
- `POST /api/providers/:id/default` — Set default
- `POST /api/providers/test` — Test connection

### Skills / MCPs / Connectors / Cron
- `GET|POST /api/skills` — List/create skills
- `GET|POST /api/mcps` — List/create MCP configs
- `GET|POST|PUT|DELETE /api/connectors[/:id]` — Connector CRUD
- `GET|POST|PUT|DELETE /api/cron[/:id]` — Cron job CRUD

### Sessions & Gateway
- `POST /api/gateway/submit` — Submit agent request
- `GET /api/gateway/status/:session_id` — Session status
- `POST /api/gateway/cancel/:session_id` — Cancel session
- `GET /api/executions/v2/sessions/full` — All sessions with executions
- `GET /api/logs/sessions` — Execution log sessions

## WebSocket Protocol

```typescript
// Client → Server
{ type: "invoke", agent_id, conversation_id, message, session_id? }
{ type: "stop", conversation_id }
{ type: "continue", conversation_id }
{ type: "subscribe", conversation_id, scope: "all"|"session"|"execution:{id}" }
{ type: "unsubscribe", conversation_id }
{ type: "end_session", session_id }

// Server → Client
{ type: "agent_started", agent_id, conversation_id, session_id, execution_id }
{ type: "token", agent_id, conversation_id, delta }
{ type: "tool_call", agent_id, conversation_id, tool_id, tool_name, args }
{ type: "tool_result", agent_id, conversation_id, tool_id, result, error? }
{ type: "agent_completed", agent_id, conversation_id, result }
{ type: "ward_changed", session_id, ward_id }
{ type: "delegation_started", parent_agent_id, child_agent_id, ... }
{ type: "delegation_completed", child_agent_id, result?, ... }
{ type: "error", message }
```

## Key Patterns

- **BatchWriter**: Decouples DB writes from streaming callback (100ms flush, token coalescing)
- **r2d2 pool**: 8 connections, 2 min idle, WAL mode SQLite
- **RwLock caching**: Provider, MCP, Settings services cache config files
- **Synchronous event publish**: `publish_sync` for token ordering preservation
- **Session ward_id**: Persisted to DB on WardChanged, restored on continuation/delegation

## Dependencies

- `runtime/*` — Agent executor, LLM client, tools
- `services/*` — Execution state, API logs
- `framework/zero-core` — FileSystemContext trait
