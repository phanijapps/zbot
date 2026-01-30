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

### Zero Framework (`crates/`)

Core abstractions that can be used independently of Agent Zero:

```
crates/
├── zero-core/           # Core traits: Agent, Tool, Session, Llm
├── zero-agent/          # Agent implementations (LLM, Sequential, etc.)
├── zero-session/        # Session management and persistence
├── zero-mcp/            # Model Context Protocol integration
└── zero-app/            # Meta-package for convenience
```

### Application (`application/`)

Agent Zero-specific crates:

```
application/
├── daemon/              # Main binary (zerod)
│   └── main.rs          #   CLI args, server startup
├── gateway/             # HTTP + WebSocket server
│   ├── http/            #   REST API routes
│   ├── websocket/       #   WebSocket handler
│   ├── execution/       #   Agent invocation
│   ├── database/        #   SQLite persistence
│   └── services/        #   Agent, Provider, Skill services
├── agent-runtime/       # Agent execution engine
│   ├── executor.rs      #   LLM loop, tool execution
│   ├── llm/             #   OpenAI-compatible client
│   └── tools/           #   Tool context, registry
├── agent-tools/         # Built-in tools
│   ├── file.rs          #   read_file, write_file, list_dir
│   ├── shell.rs         #   execute_command
│   ├── memory.rs        #   get, set, search memory
│   └── introspection.rs #   list_skills, list_tools
└── zero-cli/            # Command-line interface
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

## Execution Flow

```
User Message
     │
     ▼
┌─────────────────┐
│   WebSocket     │ ◄── { type: "invoke", message: "..." }
│   Handler       │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│   Execution     │
│   Runner        │
├─────────────────┤
│ 1. Load agent   │
│ 2. Load history │ ◄── SQLite
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

### WebSocket Protocol (port 18790)

**Client Commands:**
```typescript
// Invoke agent
{ type: "invoke", agent_id: string, conversation_id: string, message: string }

// Stop execution
{ type: "stop", conversation_id: string }

// Continue after max iterations
{ type: "continue", conversation_id: string }
```

**Server Events:**
```typescript
// Agent started processing
{ type: "agent_started", agent_id: string, conversation_id: string }

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
| `load_skill` | Load skill instructions | Safe |

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
