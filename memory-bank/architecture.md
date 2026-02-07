# Agent Zero — Technical Architecture

## System Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           CLIENTS                                        │
├─────────────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────┐       ┌─────────────────────────┐          │
│  │     Web Dashboard       │       │          CLI            │          │
│  │    (React + Vite)       │       │        (zero)           │          │
│  │    localhost:3000       │       │                         │          │
│  └───────────┬─────────────┘       └───────────┬─────────────┘          │
│              │ HTTP/WebSocket                   │ HTTP/WebSocket         │
└──────────────┼──────────────────────────────────┼────────────────────────┘
               │                                  │
               └────────────────┬─────────────────┘
                                │
┌───────────────────────────────┴─────────────────────────────────────────┐
│                           DAEMON (zerod)                                 │
├─────────────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │                         GATEWAY                                  │    │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐              │    │
│  │  │  HTTP API   │  │  WebSocket  │  │   Static    │              │    │
│  │  │   :18791    │  │   :18790    │  │   Files     │              │    │
│  │  │   (Axum)    │  │  (tokio-    │  │  (tower)    │              │    │
│  │  │             │  │  tungstenite)│  │             │              │    │
│  │  └──────┬──────┘  └──────┬──────┘  └─────────────┘              │    │
│  │         │                │                                       │    │
│  │         └────────┬───────┘                                       │    │
│  │                  │                                               │    │
│  │         ┌────────┴────────┐                                      │    │
│  │         │    Event Bus    │ ◄─── Broadcast streaming events      │    │
│  │         └────────┬────────┘                                      │    │
│  └──────────────────┼───────────────────────────────────────────────┘    │
│                     │                                                    │
│  ┌──────────────────┴───────────────────────────────────────────────┐    │
│  │                      AGENT RUNTIME                                │    │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐              │    │
│  │  │  Executor   │  │ LLM Client  │  │    Tool     │              │    │
│  │  │   (loop)    │──│  (OpenAI    │  │  Registry   │              │    │
│  │  │             │  │ compatible) │  │             │              │    │
│  │  └──────┬──────┘  └─────────────┘  └──────┬──────┘              │    │
│  │         │                                  │                     │    │
│  │         │         ┌─────────────┐         │                     │    │
│  │         └─────────│ MCP Manager │─────────┘                     │    │
│  │                   └─────────────┘                               │    │
│  └──────────────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         DATA LAYER                                       │
├─────────────────────────────────────────────────────────────────────────┤
│  ~/Documents/agentzero/                                                  │
│  ├── conversations.db          # SQLite: conversations, messages        │
│  ├── INSTRUCTIONS.md           # Custom system prompt (auto-created)    │
│  ├── agents/{name}/            # Agent configurations                   │
│  │   ├── config.yaml           #   Model, provider, temperature         │
│  │   └── AGENTS.md             #   System instructions                  │
│  ├── agents_data/{id}/         # Per-agent runtime data                 │
│  │   └── memory.json           #   Persistent key-value storage         │
│  ├── agents_data/shared/       # Cross-agent shared memory (file-locked)│
│  │   ├── user_info.json        #   User preferences                     │
│  │   ├── workspace.json        #   Project paths (auto-injected)        │
│  │   ├── patterns.json         #   Learned patterns/conventions         │
│  │   └── session_summaries.json#   Distilled learnings                  │
│  ├── wards/                    # Code Wards (persistent project dirs)   │
│  │   ├── .venv/                #   Shared Python venv for all wards     │
│  │   ├── scratch/              #   Default ward for quick tasks         │
│  │   └── {ward-name}/          #   Agent-named project directories      │
│  │       └── .ward_memory.json #     Per-ward context                   │
│  ├── skills/{name}/            # Skill definitions                      │
│  │   └── SKILL.md              #   Instructions + frontmatter           │
│  ├── providers.json            # LLM provider configurations            │
│  └── mcps.json                 # MCP server configurations              │
└─────────────────────────────────────────────────────────────────────────┘
```

## Technology Stack

| Layer | Technology | Purpose |
|-------|------------|---------|
| Frontend | React 19 + TypeScript | UI components |
| Build | Vite | Fast dev server, bundling |
| UI | Tailwind CSS v4 + Radix UI | Styling, accessible primitives |
| HTTP Server | Axum | Async HTTP framework |
| WebSocket | tokio-tungstenite | Real-time streaming |
| Async Runtime | tokio | Async I/O |
| Database | SQLite (rusqlite + r2d2 pool) | Conversation persistence (WAL mode) |
| Serialization | serde + serde_json | JSON handling |

## Crate Structure

### Layer Overview

```
agentzero/
├── framework/      # Core abstractions (publishable)
├── runtime/        # Execution engine
├── services/       # Standalone data services
├── gateway/        # HTTP/WebSocket server
├── apps/           # Applications (daemon, cli, ui)
└── dist/           # Frontend build output
```

### Framework (`framework/`)

Core abstractions that can be used independently:

```
framework/
├── zero-core/           # Core traits: Agent, Tool, Toolset, Event
├── zero-llm/            # LLM abstractions and OpenAI client
├── zero-tool/           # Tool registry and execution
├── zero-session/        # Session and state management
├── zero-agent/          # Agent implementations (LLM, workflow)
├── zero-mcp/            # Model Context Protocol integration
├── zero-prompt/         # Template rendering
├── zero-middleware/     # Message preprocessing pipelines
└── zero-app/            # Convenience prelude
```

### Runtime (`runtime/`)

Execution engine:

```
runtime/
├── agent-runtime/       # Executor, LLM loop, middleware
└── agent-tools/         # Built-in tool implementations
```

### Services (`services/`)

Standalone data services:

```
services/
├── api-logs/            # Execution logging (SQLite)
├── knowledge-graph/     # Entity extraction
├── search-index/        # Full-text search (Tantivy)
├── session-archive/     # Parquet archival
└── daily-sessions/      # Session management
```

### Gateway (`gateway/`)

Network layer, decomposed into focused crates:

```
gateway/
├── gateway-events/      # EventBus, GatewayEvent, HookContext
├── gateway-database/    # DatabaseManager, pool, schema, ConversationRepository
├── gateway-templates/   # Prompt assembly, shard injection
├── gateway-connectors/  # ConnectorRegistry, dispatch (Discord, Telegram, Slack)
├── gateway-services/    # AgentService, ProviderService, McpService, SkillService, SettingsService
├── gateway-execution/   # ExecutionRunner, delegation, lifecycle, streaming, BatchWriter
├── gateway-hooks/       # Hook trait, HookRegistry, CliHook, CronHook
├── gateway-cron/        # CronJobConfig, CronService
├── gateway-bus/         # GatewayBus trait, SessionRequest, SessionHandle
├── gateway-ws-protocol/ # ClientMessage, ServerMessage, SubscriptionScope
├── src/                 # Thin shell: HTTP routes, WebSocket handler, AppState
└── templates/           # System prompt templates (embedded at compile time)
```

### Apps (`apps/`)

Runnable applications:

```
apps/
├── daemon/              # Main binary (zerod)
└── zero-cli/            # CLI tool with TUI
```

## Core Abstractions

### Agent Trait
```rust
#[async_trait]
pub trait Agent: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;

    async fn invoke(
        &self,
        context: InvocationContext,
    ) -> Result<EventStream>;
}
```

### Tool Trait
```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> Option<Value>;
    fn permissions(&self) -> ToolPermissions;

    async fn execute(
        &self,
        ctx: Arc<dyn ToolContext>,
        args: Value,
    ) -> Result<Value>;
}
```

### LLM Client
```rust
#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn chat_completion_stream(
        &self,
        messages: &[ChatMessage],
        tools: Option<&[Value]>,
        callback: &mut dyn FnMut(StreamEvent),
    ) -> Result<()>;
}
```

## Session Management Architecture

Sessions are the top-level container for user interactions. A session groups multiple agent executions (turns) together, enabling multi-turn conversations with context preservation.

### Session Lifecycle

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         SESSION LIFECYCLE                                │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│   User sends first message (no session_id)                              │
│        │                                                                │
│        ▼                                                                │
│   ┌─────────────────┐                                                   │
│   │ Create Session  │ ──► sess-{uuid} created in DB                     │
│   │ (status=running)│     source = web|cli|api|cron|plugin              │
│   └────────┬────────┘                                                   │
│            │                                                            │
│            ▼                                                            │
│   ┌─────────────────┐                                                   │
│   │ Create Root     │ ──► exec-{uuid} created, parent=null              │
│   │ Execution       │                                                   │
│   └────────┬────────┘                                                   │
│            │                                                            │
│            ▼                                                            │
│   ┌─────────────────┐                                                   │
│   │ agent_started   │ ──► Frontend receives session_id                  │
│   │ event emitted   │     Frontend stores in localStorage               │
│   └────────┬────────┘                                                   │
│            │                                                            │
│            ▼                                                            │
│   User sends follow-up message (WITH session_id)                        │
│        │                                                                │
│        ▼                                                                │
│   ┌─────────────────┐                                                   │
│   │ Lookup existing │ ──► Same session reused                           │
│   │ Session         │     New execution created under same session      │
│   └────────┬────────┘                                                   │
│            │                                                            │
│            ▼                                                            │
│   User sends /new command                                               │
│        │                                                                │
│        ▼                                                                │
│   ┌─────────────────┐                                                   │
│   │ Clear session_id│ ──► localStorage cleared                          │
│   │ from frontend   │     Next message creates new session              │
│   └─────────────────┘                                                   │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### Session vs Execution vs Conversation

| Concept | Scope | Purpose |
|---------|-------|---------|
| **Session** | User work session | Groups all activity until `/new` command |
| **Execution** | Single agent turn | One agent processing one request |
| **Conversation** | Message thread | Persists chat history for context |

### Session and Execution States

**Session Status:**
| Status | Description |
|--------|-------------|
| `queued` | Created but not yet started |
| `running` | Actively processing |
| `paused` | Paused by user or server shutdown |
| `completed` | Successfully finished |
| `crashed` | Failed with error or unexpected interruption |

**Execution Status:**
| Status | Description |
|--------|-------------|
| `queued` | Created but not yet started |
| `running` | Actively executing |
| `paused` | Paused (session paused or waiting) |
| `completed` | Successfully finished |
| `crashed` | Failed with error |
| `cancelled` | Cancelled by user or parent |

### Server Shutdown Behavior

The server handles session states differently based on shutdown type:

**Graceful Shutdown (Ctrl+C):**
- All running sessions are marked as `paused`
- All running/queued executions are marked as `paused`
- Sessions can be resumed when the server restarts

**Unexpected Crash:**
- Sessions remain in `running` state in the database
- On startup, any sessions still in `running` state are marked as `crashed`
- This indicates they were interrupted unexpectedly

```
Graceful Shutdown:
  Server receives SIGINT/SIGTERM
       │
       ▼
  mark_running_as_paused()  ──► Sessions: running → paused
       │                        Executions: running/queued → paused
       ▼
  Shutdown HTTP/WebSocket servers

Startup Recovery:
  Server starts
       │
       ▼
  mark_running_as_crashed()  ──► Only sessions still in "running" state
       │                         (unexpected crash) marked as crashed
       ▼
  Normal operation
```

### Frontend Session Persistence

The frontend stores session state in localStorage:

```typescript
// Keys used for session persistence
const WEB_SESSION_ID_KEY = 'agentzero_web_session_id';
const WEB_CONV_ID_KEY = 'agentzero_web_conv_id';

// On agent_started event, store session_id
localStorage.setItem(WEB_SESSION_ID_KEY, event.session_id);

// On subsequent messages, include session_id
{ type: "invoke", session_id: storedSessionId, ... }

// On /new command, clear session
localStorage.removeItem(WEB_SESSION_ID_KEY);
```

### Trigger Sources

Sessions track their origin for analytics and filtering:

| Source | Description |
|--------|-------------|
| `web` | Web dashboard (default) |
| `cli` | Command-line interface |
| `api` | External API call |
| `cron` | Scheduled task |
| `plugin` | Plugin/extension initiated |

## Execution Flow

```
User Message
     │
     ▼
┌─────────────────┐
│   WebSocket     │ ◄── { type: "invoke", session_id?, message: "..." }
│   Handler       │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│   Session       │
│   Resolution    │
├─────────────────┤
│ if session_id { │
│   lookup(id)    │ ──► Reuse existing session
│ } else {        │
│   create_new()  │ ──► New session + execution
│ }               │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│   Execution     │
│   Runner        │
├─────────────────┤
│ 1. Load agent   │
│ 2. Load history │ ◄── SQLite (by conversation_id)
│ 3. Create LLM   │
│ 4. Build tools  │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│   Agent         │
│   Executor      │
├─────────────────┤
│ while !done {   │
│   llm.call()    │──► Stream tokens ──► WebSocket ──► UI
│   if tool_call {│
│     execute()   │──► Stream result ──► WebSocket ──► UI
│   }             │
│ }               │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Save Messages  │ ──► SQLite
│  Update Session │ ──► Status, timestamps
└─────────────────┘
```

## API Reference

### HTTP Endpoints (port 18791)

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/health` | Health check |
| GET | `/api/status` | Daemon status |
| GET | `/api/agents` | List agents |
| POST | `/api/agents` | Create agent |
| GET | `/api/agents/:id` | Get agent |
| PUT | `/api/agents/:id` | Update agent |
| DELETE | `/api/agents/:id` | Delete agent |
| GET | `/api/providers` | List providers |
| POST | `/api/providers` | Create provider |
| POST | `/api/providers/:id/default` | Set default |
| POST | `/api/providers/test` | Test connection |
| GET | `/api/skills` | List skills |
| POST | `/api/skills` | Create skill |
| GET | `/api/logs/sessions` | List execution sessions |
| GET | `/api/logs/sessions/:id` | Get session with logs |
| DELETE | `/api/logs/sessions/:id` | Delete session |
| **Operations Dashboard** | | |
| GET | `/api/executions/stats/counts` | Dashboard statistics |
| GET | `/api/executions/v2/sessions/full` | Sessions with executions |
| GET | `/api/executions/v2/sessions/:id` | Single session details |
| POST | `/api/gateway/submit` | Submit new agent request |
| GET | `/api/gateway/status/:session_id` | Get session status |
| POST | `/api/gateway/cancel/:session_id` | Cancel running session |

### WebSocket Protocol (port 18790)

**Client Commands:**
```typescript
// Invoke agent (session_id optional - if omitted, new session created)
{
  type: "invoke",
  agent_id: string,
  conversation_id: string,
  message: string,
  session_id?: string  // Include to continue existing session
}

// Stop execution
{ type: "stop", conversation_id: string }

// Continue after max iterations
{ type: "continue", conversation_id: string }

// Subscribe to events with scope filtering
{
  type: "subscribe",
  conversation_id: string,  // Session ID to subscribe to
  scope: "all" | "session" | "execution:{exec_id}"
}
// Scopes:
// - "all": All events (backward compatible, includes subagent internal events)
// - "session": Root execution events + delegation lifecycle markers only
// - "execution:{id}": All events for a specific execution (debug view)

// Unsubscribe
{ type: "unsubscribe", conversation_id: string }
```

**Subscription Response:**
```typescript
// Subscription confirmed
{
  type: "subscribed",
  conversation_id: string,
  current_sequence: number,
  root_execution_ids?: string[]  // For session scope, list of root execution IDs
}
```

**Server Events:**
```typescript
// Agent started processing (IMPORTANT: contains session_id for client to store)
{
  type: "agent_started",
  agent_id: string,
  conversation_id: string,
  session_id: string,      // Client should store this for subsequent messages
  execution_id: string     // Unique execution within session
}

// Streaming token
{ type: "token", agent_id: string, conversation_id: string, delta: string }

// Tool being called
{ type: "tool_call", agent_id: string, conversation_id: string,
  tool_id: string, tool_name: string, args: object }

// Tool result
{ type: "tool_result", agent_id: string, conversation_id: string,
  tool_id: string, result: string, error?: string }

// Agent finished
{ type: "agent_completed", agent_id: string, conversation_id: string,
  result: string }

// Error occurred
{ type: "error", agent_id?: string, conversation_id?: string,
  message: string }
```

## Database Schema

### conversations
```sql
CREATE TABLE conversations (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    title TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    metadata TEXT
);
```

### messages
```sql
CREATE TABLE messages (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    role TEXT NOT NULL,           -- user, assistant, tool
    content TEXT NOT NULL,
    created_at TEXT NOT NULL,
    token_count INTEGER DEFAULT 0,
    tool_calls TEXT,              -- JSON array
    tool_results TEXT,            -- JSON array
    FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
);
```

### execution_logs
```sql
CREATE TABLE execution_logs (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,        -- Groups logs for one agent invocation
    conversation_id TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    parent_session_id TEXT,          -- For delegated agents, links to parent
    timestamp TEXT NOT NULL,
    level TEXT NOT NULL,             -- debug, info, warn, error
    category TEXT NOT NULL,          -- session, tool_call, tool_result, delegation, error
    message TEXT NOT NULL,
    metadata TEXT,                   -- JSON with tool args, results, etc.
    duration_ms INTEGER
);
```

## Built-in Tools

### Core Tools (Always Enabled)

| Tool | Description | Permissions |
|------|-------------|-------------|
| `shell` | Run shell command | Dangerous |
| `read` | Read file contents | Safe |
| `write` | Write content to file | Moderate |
| `edit` | Edit file contents | Moderate |
| `memory` | Persistent key-value store (shared/private/ward) | Safe |
| `ward` | Manage code wards (use, list, create, info) | Safe |
| `todo` | Task management | Safe |
| `list_skills` | List available skills | Safe |
| `load_skill` | Load skill instructions | Safe |
| `grep` | Search file contents | Safe |
| `glob` | Find files by pattern | Safe |

### Action Tools (Always Enabled)

| Tool | Description | Permissions |
|------|-------------|-------------|
| `respond` | Send response to user | Safe |
| `delegate_to_agent` | Delegate task to subagent | Safe |
| `list_agents` | List available agents | Safe |

### Optional Tools (Configurable)

| Tool | Description | Permissions |
|------|-------------|-------------|
| `python` | Execute Python code | Dangerous |
| `web_fetch` | Fetch web content | Moderate |
| `ui_tools` | UI manipulation tools | Moderate |
| `knowledge_graph` | Entity-relationship storage | Safe |
| `create_agent` | Create new agents | Moderate |
| `introspection` | Agent introspection | Safe |

## Design Decisions

### Why No Desktop Wrapper?
- Browsers are more capable than custom webviews
- Easier deployment (no native installers)
- Better developer experience (standard web tools)
- Cross-platform without platform-specific builds

### Why Single Daemon?
- Simpler deployment and debugging
- Shared state without IPC complexity
- Single port configuration
- Memory efficiency

### Why SQLite?
- Zero configuration
- Portable (single file)
- ACID transactions
- Fast for local workloads

### Why Rust?
- Memory safety without GC
- Excellent async story (tokio)
- Great tooling (cargo, clippy)
- Single binary distribution

### Why Instructions in AGENTS.md?
- Human-readable and editable
- Version control friendly
- Markdown rendering in UI
- Separates behavior from configuration

## System Prompt Architecture

The system prompt is composed of a base template plus automatically injected shards:

```
┌─────────────────────────────────────────┐
│ INSTRUCTIONS.md (user-customizable)     │
│                                         │
│ Your custom agent instructions...       │
├─────────────────────────────────────────┤
│ # --- SYSTEM INJECTED ---               │
├─────────────────────────────────────────┤
│ ENVIRONMENT                             │
│ - OS: windows (x86_64)                  │
│ - Shell: PowerShell/cmd syntax          │
├─────────────────────────────────────────┤
│ SAFETY (shard)                          │
│ - Never exfiltrate secrets              │
│ - Confirm before dangerous operations   │
├─────────────────────────────────────────┤
│ TOOLING & SKILLS (shard)                │
│ - Skills-first approach                 │
│ - Delegation patterns                   │
├─────────────────────────────────────────┤
│ MEMORY & LEARNING (shard)               │
│ - Shared memory usage                   │
│ - Pattern recording                     │
└─────────────────────────────────────────┘
```

### Shards

Required shards are automatically appended to custom instructions:

| Shard | Purpose |
|-------|---------|
| `safety` | Security rules (secrets, confirmations) |
| `tooling_skills` | Skills-first approach, delegation |
| `memory_learning` | Shared memory patterns |

### Environment Injection

OS and architecture are detected at runtime and injected:
- **Windows**: PowerShell/cmd syntax hints
- **macOS/Linux**: Unix shell syntax hints

This ensures the agent uses correct shell commands for the platform.

## Connectors

Connectors are external services that receive agent responses. When an agent execution completes, AgentZero can dispatch the response to one or more configured connectors.

### Connector Flow

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   Trigger       │────▶│   AgentZero     │────▶│   Connector     │
│ (Cron/API/Web)  │     │   Gateway       │     │   (Your Service)│
└─────────────────┘     └─────────────────┘     └─────────────────┘
                              │
                              │ respond_to: ["my-connector"]
                              ▼
                        ┌─────────────────┐
                        │  HTTP POST to   │
                        │  your endpoint  │
                        └─────────────────┘
```

### Transport Types

| Type | Description | Use Case |
|------|-------------|----------|
| `http` | HTTP POST to callback URL | Webhooks, external APIs |
| `cli` | Execute local command | Scripts, local integrations |

### Connector Payload

When dispatching to connectors, Gateway sends:

```json
{
  "context": {
    "session_id": "sess-abc123",
    "thread_id": null,
    "agent_id": "root",
    "timestamp": "2024-01-15T09:00:00Z"
  },
  "capability": "respond",
  "payload": {
    "message": "The agent's response text",
    "execution_id": "exec-xyz789",
    "conversation_id": "conv-abc123"
  }
}
```

### Connector API

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/connectors` | List all connectors |
| GET | `/api/connectors/:id` | Get connector by ID |
| POST | `/api/connectors` | Create connector |
| PUT | `/api/connectors/:id` | Update connector |
| DELETE | `/api/connectors/:id` | Delete connector |
| POST | `/api/connectors/:id/test` | Test connector |
| POST | `/api/connectors/:id/enable` | Enable connector |
| POST | `/api/connectors/:id/disable` | Disable connector |

## Cron Scheduler

Built-in scheduler that triggers agents on a schedule. Cron jobs always route to the **root agent** for orchestration.

### Cron Configuration

```json
{
  "id": "daily-report",
  "name": "Daily Report Generator",
  "schedule": "0 0 9 * * *",
  "message": "Generate the daily sales report",
  "respond_to": ["slack-notifier"],
  "enabled": true
}
```

**Note**: Schedule uses 6-field cron format: `sec min hour day month weekday`

### Cron API

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/cron` | List all cron jobs |
| GET | `/api/cron/:id` | Get cron job by ID |
| POST | `/api/cron` | Create cron job |
| PUT | `/api/cron/:id` | Update cron job |
| DELETE | `/api/cron/:id` | Delete cron job |
| POST | `/api/cron/:id/trigger` | Manually trigger job |
| POST | `/api/cron/:id/enable` | Enable job |
| POST | `/api/cron/:id/disable` | Disable job |

## Response Routing

The `respond_to` field controls where agent responses are delivered:

```json
{
  "agent_id": "root",
  "message": "Generate a report",
  "respond_to": ["slack-notifier", "email-bridge"]
}
```

- **Empty/null**: Response goes to web UI only (default)
- **Specified**: Response dispatched to listed connectors
- **Original source NOT automatically included** (explicit routing)

## Data Layer Additions

The following JSON files are added to the data directory:

```
~/Documents/agentzero/
├── connectors.json      # Connector configurations
└── cron_jobs.json       # Scheduled job configurations
```
