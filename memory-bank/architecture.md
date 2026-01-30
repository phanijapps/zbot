# Agent Zero - Architecture

## Technology Stack

| Layer | Technology |
|-------|------------|
| Frontend | React 19 + TypeScript + Vite |
| UI | Radix UI + Tailwind CSS v4 |
| Backend | Rust daemon (gateway + runtime) |
| Database | SQLite (sqlx) |
| Async | tokio |
| API | HTTP REST + WebSocket |

## System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Web Browser                             │
│  ┌─────────────────────────────────────────────────────┐    │
│  │              React Frontend (Vite)                   │    │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐            │    │
│  │  │   Chat   │ │  Agents  │ │ Providers│            │    │
│  │  └──────────┘ └──────────┘ └──────────┘            │    │
│  │                     │                               │    │
│  │              Transport Layer                        │    │
│  │         (HTTP Client + WebSocket)                   │    │
│  └─────────────────────┬───────────────────────────────┘    │
└─────────────────────────┼───────────────────────────────────┘
                          │
          ┌───────────────┴───────────────┐
          │ HTTP :18791    WebSocket :18790│
          └───────────────┬───────────────┘
                          │
┌─────────────────────────┴───────────────────────────────────┐
│                    Daemon (zerod)                            │
│  ┌─────────────────────────────────────────────────────┐    │
│  │                    Gateway                           │    │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐            │    │
│  │  │ HTTP API │ │WebSocket │ │  Static  │            │    │
│  │  │ (Axum)   │ │ Server   │ │  Files   │            │    │
│  │  └──────────┘ └──────────┘ └──────────┘            │    │
│  └─────────────────────┬───────────────────────────────┘    │
│                        │                                     │
│  ┌─────────────────────┴───────────────────────────────┐    │
│  │               Agent Runtime                          │    │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐            │    │
│  │  │ Executor │ │   LLM    │ │  Tools   │            │    │
│  │  │          │ │  Client  │ │ Registry │            │    │
│  │  └──────────┘ └──────────┘ └──────────┘            │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
                          │
                          ▼
              ~/Documents/agentzero/
```

## Workspace Structure

```
agentzero/
├── src/                       # Frontend (React)
│   ├── features/              # Feature modules
│   │   ├── agent/             # Chat + agent management
│   │   ├── skills/            # Skill management
│   │   ├── integrations/      # Provider management
│   │   └── cron/              # Scheduled tasks
│   ├── services/transport/    # HTTP/WebSocket client
│   └── shared/                # UI components, types
├── crates/                    # Zero Framework
│   ├── zero-core/             # Core traits (Agent, Tool, Session)
│   ├── zero-llm/              # LLM abstractions
│   ├── zero-agent/            # Agent implementations
│   ├── zero-session/          # Session management
│   ├── zero-mcp/              # MCP integration
│   └── zero-app/              # Meta-package
├── application/               # Application crates
│   ├── daemon/                # Main binary (zerod)
│   ├── gateway/               # HTTP + WebSocket server
│   ├── agent-runtime/         # Agent executor
│   ├── agent-tools/           # Built-in tools
│   ├── zero-cli/              # CLI tool
│   └── knowledge-graph/       # Semantic memory
└── dist/                      # Built frontend (served by daemon)
```

## Core Abstractions

```rust
// Agent - invokable AI entity
trait Agent {
    async fn invoke(&self, context: InvocationContext) -> Result<EventStream>;
}

// Tool - callable function with permissions
trait Tool {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> Option<Value>;
    fn permissions(&self) -> ToolPermissions;
    async fn execute(&self, ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value>;
}

// Session - conversation state
trait Session {
    async fn append(&self, event: Event) -> Result<()>;
    async fn events(&self) -> Result<Vec<Event>>;
}

// Llm - language model client
trait Llm {
    async fn generate(&self, request: LlmRequest) -> Result<LlmResponse>;
}
```

## API Endpoints

### HTTP API (port 18791)

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/health` | Health check |
| GET | `/api/status` | Gateway status |
| GET/POST | `/api/agents` | List/create agents |
| GET/PUT/DELETE | `/api/agents/:id` | Agent CRUD |
| GET/POST | `/api/providers` | List/create providers |
| POST | `/api/providers/:id/default` | Set default provider |
| GET/POST | `/api/skills` | List/create skills |

### WebSocket (port 18790)

**Commands (client → server):**
```json
{ "type": "invoke", "agent_id": "root", "conversation_id": "...", "message": "..." }
{ "type": "stop", "conversation_id": "..." }
{ "type": "continue", "conversation_id": "..." }
```

**Events (server → client):**
```json
{ "type": "agent_started", "timestamp": 123, "agent_id": "...", "model": "..." }
{ "type": "token", "timestamp": 123, "content": "..." }
{ "type": "tool_call", "timestamp": 123, "tool_id": "...", "tool_name": "...", "args": {} }
{ "type": "tool_result", "timestamp": 123, "tool_id": "...", "result": "..." }
{ "type": "agent_finished", "timestamp": 123, "final_message": "..." }
{ "type": "error", "timestamp": 123, "error": "...", "recoverable": false }
```

## Storage

**Data Directory**: `~/Documents/agentzero/`

```
agentzero/
├── agents/{name}/
│   ├── config.yaml           # Metadata
│   └── AGENTS.md             # Instructions
├── agents_data/{agent_id}/   # Per-agent workspace
│   ├── outputs/              # Generated files
│   ├── code/                 # Scripts
│   ├── data/                 # Persistent data
│   └── memory.json           # Agent memory
├── skills/{name}/
│   └── SKILL.md              # Skill with frontmatter
├── db/
│   └── sessions.db           # Sessions, messages
├── providers.json            # LLM providers
└── mcps.json                 # MCP server configs
```

## Agent Execution Flow

```
User Message → WebSocket → Gateway → Load Agent Config →
Create LLM Client → Initialize MCPs → Create Tools →
Build Executor → Agent Loop (LLM ↔ Tools) → Stream Events
```

## Key Design Principles

1. **Web + CLI**: No desktop wrapper, browser-based dashboard
2. **Single daemon**: Gateway + runtime in one process
3. **Instructions in AGENTS.md**: Not in config.yaml
4. **Orchestrator-first**: AI plans and routes to capabilities
5. **Tool permissions**: Every tool declares risk level
6. **Streaming-first**: Real-time event streaming via WebSocket
