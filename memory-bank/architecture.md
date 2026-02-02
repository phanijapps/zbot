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
│  ├── agents/{name}/            # Agent configurations                   │
│  │   ├── config.yaml           #   Model, provider, temperature         │
│  │   └── AGENTS.md             #   System instructions                  │
│  ├── agents_data/{id}/         # Per-agent runtime data                 │
│  │   └── memory.json           #   Persistent key-value storage         │
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
| Database | SQLite (rusqlite) | Conversation persistence |
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

Network layer:

```
gateway/
├── src/
│   ├── http/            # REST API routes
│   ├── websocket/       # WebSocket handler
│   ├── execution/       # Agent invocation + delegation
│   ├── database/        # SQLite persistence
│   └── services/        # Agent, Provider, Skill services
└── templates/           # System prompt templates
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

| Tool | Description | Permissions |
|------|-------------|-------------|
| `read_file` | Read file contents | Safe |
| `write_file` | Write content to file | Moderate |
| `list_dir` | List directory contents | Safe |
| `execute_command` | Run shell command | Dangerous |
| `memory` | Persistent key-value store | Safe |
| `list_skills` | List available skills | Safe |
| `list_tools` | List available tools | Safe |
| `list_mcps` | List MCP servers | Safe |
| `list_agents` | List available agents | Safe |
| `load_skill` | Load skill instructions | Safe |
| `delegate_to_agent` | Delegate task to subagent | Safe |
| `respond` | Send response to user | Safe |

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
