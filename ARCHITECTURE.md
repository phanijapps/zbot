# AgentZero Architecture

## Overview

AgentZero is a Tauri-based desktop application for managing AI agents with MCP (Model Context Protocol) server integration, skills, and modular middleware support.

## Technology Stack

### Frontend
- **Framework**: React (via Vite)
- **UI Components**: Radix UI primitives with custom styling
- **Editor**: `@uiw/react-md-editor` for markdown editing
- **State Management**: React hooks (useState, useEffect)
- **Build Tool**: Vite

### Backend
- **Framework**: Tauri 2.x
- **Language**: Rust
- **Async Runtime**: tokio
- **Serialization**: serde (JSON, YAML)

### Key Dependencies
- `tokio` - Async runtime
- `serde` / `serde_yaml` - Serialization
- `tauri` - Desktop framework
- `async-trait` - Async trait support

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                         Frontend (React)                         │
│                                                                   │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐         │
│  │   Agent      │  │  Provider    │  │     MCP      │         │
│  │  Management  │  │  Management  │  │  Management  │         │
│  └──────────────┘  └──────────────┘  └──────────────┘         │
│                                                                   │
│  ┌────────────────────────────────────────────────────────┐    │
│  │                    Agent IDE Page                       │    │
│  │  ┌────────────┐  ┌────────────┐  ┌────────────┐       │    │
│  │  │    File    │  │   Config   │  │  Markdown  │       │    │
│  │  │  Explorer  │  │  Editor    │  │   Editor   │       │    │
│  │  └────────────┘  └────────────┘  └────────────┘       │    │
│  └────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                       Tauri IPC Layer                           │
│                   (Commands & Events)                            │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                          Backend (Rust)                          │
│                                                                   │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                    Commands Layer                        │   │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐   │   │
│  │  │  Agents  │ │Provider  │ │   MCP    │ │ Skills   │   │   │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘   │   │
│  └─────────────────────────────────────────────────────────┘   │
│                              │                                  │
│                              ▼                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                  Domain Layer                            │   │
│  │  ┌─────────────────────────────────────────────────┐    │   │
│  │  │              Agent Runtime                        │    │   │
│  │  │  ┌────────────┐  ┌────────────┐  ┌───────────┐ │    │   │
│  │  │  │   LLM      │  │    Tools   │  │ Middleware│ │    │   │
│  │  │  │  Client    │  │  Registry  │  │  Pipeline  │ │    │   │
│  │  │  └────────────┘  └────────────┘  └───────────┘ │    │   │
│  │  │                                                       │    │   │
│  │  │  ┌────────────┐  ┌────────────┐  ┌───────────┐ │    │   │
│  │  │  │   MCP      │  │  Executor  │  │  Token    │ │    │   │
│  │  │  │  Manager   │  │            │  │  Counter  │ │    │   │
│  │  │  └────────────┘  └────────────┘  └───────────┘ │    │   │
│  │  └─────────────────────────────────────────────────┘    │   │
│  │                                                           │   │
│  │  ┌─────────────────────────────────────────────────┐    │   │
│  │  │           Conversation Runtime                    │    │   │
│  │  │  ┌────────────┐  ┌────────────┐  ┌───────────┐ │    │   │
│  │  │  │  Database  │  │ Repository │  │   Memory   │ │    │   │
│  │  │  └────────────┘  └────────────┘  └───────────┘ │    │   │
│  │  └─────────────────────────────────────────────────┘    │   │
│  └─────────────────────────────────────────────────────────┘   │
│                              │                                  │
│                              ▼                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                    Storage Layer                         │   │
│  │  ~/.config/zeroagent/                                   │   │
│  │    ├── agents/              # Agent folders              │   │
│  │    ├── providers/           # Provider configs          │   │
│  │    ├── mcp_servers/         # MCP server configs        │   │
│  │    ├── skills/              # Skill folders             │   │
│  │    ├── conversations.db     # SQLite database           │   │
│  │    └── staging/             # Temp files for creation   │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                   │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                   External APIs                          │   │
│  │  ┌────────────┐  ┌────────────┐  ┌────────────┐       │   │
│  │  │  OpenAI    │  │  Anthropic │  │   Custom   │       │   │
│  │  │  Compatible│  │   Claude   │  │    APIs    │       │   │
│  │  └────────────┘  └────────────┘  └────────────┘       │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

## Module Structure

### Commands Layer (`src/commands/`)

Tauri commands that expose functionality to the frontend via IPC.

| Module | File | Purpose |
|--------|------|---------|
| Agents | `agents.rs` | Agent CRUD, file management |
| Providers | `providers.rs` | Provider CRUD operations |
| MCP | `mcp_servers.rs` | MCP server management |
| Skills | `skills/` | Skill management with frontmatter |
| Conversations | `conversations.rs` | Chat history management |
| Agent Runtime | `agents_runtime.rs` | Agent execution with streaming |

### Domain Layer (`src/domains/`)

Business logic and domain models.

#### Agent Runtime (`src/domains/agent_runtime/`)

Core agent execution engine with modular middleware support.

```
agent_runtime/
├── mod.rs              # Module exports
├── executor.rs         # Main executor (orchestrates LLM + tools + MCP)
├── llm.rs             # LLM client (OpenAI-compatible API)
├── tools.rs           # Tool registry and execution
├── mcp_manager.rs     # MCP server client (HTTP, stdio, SSE)
└── middleware/        # Middleware system
    ├── mod.rs
    ├── traits.rs      # Middleware trait definitions
    ├── pipeline.rs    # Middleware orchestration
    ├── config.rs      # Middleware configuration
    ├── summarization.rs   # Summarization middleware
    ├── context_editing.rs # Context editing middleware
    └── token_counter.rs    # Token estimation
```

#### Conversation Runtime (`src/domains/conversation_runtime/`)

Chat history and database management.

```
conversation_runtime/
├── mod.rs              # Module exports
├── database/
│   ├── connection.rs   # SQLite connection management
│   └── schema.rs       # Database schema and migrations
├── repository/
│   ├── conversations.rs # Conversation CRUD
│   └── messages.rs     # Message CRUD
└── memory/
    └── mod.rs          # Context window management
```

## Data Flow

### Agent Execution Flow

```
User Input (Frontend)
       │
       ▼
┌─────────────────────────────────────────────────────────┐
│  execute_agent_stream(id, messages)                     │
│       │                                                  │
│       ▼                                                  │
│  1. Load Agent (config.yaml + AGENTS.md)                │
│       │                                                  │
│       ▼                                                  │
│  2. Initialize Middleware Pipeline                      │
│       │                                                  │
│       ▼                                                  │
│  3. Process Messages through Middleware                 │
│       │  - Summarization (compress if needed)            │
│       │  - Context Editing (clear old tool results)      │
│       │                                                  │
│       ▼                                                  │
│  4. Call LLM with processed messages                    │
│       │                                                  │
│       ▼                                                  │
│  5. Stream Response Events                               │
│       │  - Token events (content chunks)                │
│       │  - Tool call events                              │
│       │  - Middleware events                             │
│       │                                                  │
│       ▼                                                  │
│  6. Execute Tool Calls (if any)                          │
│       │  - Built-in tools (python, browser)             │
│       │  - MCP tools (external services)                 │
│       │                                                  │
│       ▼                                                  │
│  7. Return Final Response                               │
└─────────────────────────────────────────────────────────┘
       │
       ▼
UI Updates (Real-time)
```

### Middleware Processing Flow

```
Messages (Vec<ChatMessage>)
       │
       ▼
┌─────────────────────────────────────────────────────────┐
│  MiddlewarePipeline::process_messages()                 │
│       │                                                  │
│       ▼                                                  │
│  For each middleware in order:                          │
│       │                                                  │
│       ├─▶ SummarizationMiddleware                        │
│       │   ├─ Check trigger (tokens/messages/fraction)    │
│       │   ├─ If triggered:                               │
│       │   │   ├─ Split messages (keep vs summarize)     │
│       │   │   ├─ Call LLM to summarize                  │
│       │   │   └─ Return EmitAndModify with summary      │
│       │   └─ Else: Return Proceed                       │
│       │                                                  │
│       ├─▶ ContextEditingMiddleware                      │
│       │   ├─ Check trigger (token count)                 │
│       │   ├─ If triggered:                               │
│       │   │   ├─ Find tool results to clear              │
│       │   │   ├─ Replace with placeholder               │
│       │   │   └─ Return EmitAndModify with cleared msg  │
│       │   └─ Else: Return Proceed                       │
│       │                                                  │
│       └─▶ Custom Middleware (future)                     │
│                                                          │
│  Collect all events                                      │
│  Return processed messages                               │
└─────────────────────────────────────────────────────────┘
       │
       ▼
Processed Messages (with events emitted)
```

### Tool Execution Flow

```
LLM returns tool calls
       │
       ▼
┌─────────────────────────────────────────────────────────┐
│  Executor::execute_tool_calls()                         │
│       │                                                  │
│       ▼                                                  │
│  For each tool call:                                    │
│       │                                                  │
│       ├─ Is it an MCP tool?                             │
│       │   ├─ Yes → MCP Manager → External MCP Server    │
│       │   └─ No → Tool Registry → Built-in Tool         │
│       │                                                  │
│       ├─ Execute Tool                                   │
│       │   ├─ Python execution (isolated environment)    │
│       │   ├─ Browser automation (playwright)           │
│       │   ├─ Custom tools (user-defined)               │
│       │   └─ MCP tools (external)                      │
│       │                                                  │
│       ├─ Stream Tool Events                             │
│       │   ├─ ToolStart (tool started)                  │
│       │   ├─ ToolOutput (progress output)              │
│       │   └─ ToolEnd (tool completed)                  │
│       │                                                  │
│       └─ Collect Results                                │
│                                                          │
│  Return tool results to LLM                             │
└─────────────────────────────────────────────────────────┘
```

## Storage Schema

### Agent Folder Structure

```
~/.config/zeroagent/agents/{agent-name}/
├── config.yaml           # Agent metadata (YAML)
│   - name
│   - displayName
│   - description
│   - providerId
│   - model
│   - temperature
│   - maxTokens
│   - thinkingEnabled
│   - skills[]
│   - mcps[]
│   - middleware (optional YAML string)
│
├── AGENTS.md             # Agent instructions (markdown)
│   - Free-form instructions
│   - No frontmatter (unlike skills)
│
└── [user files]          # Additional files/folders
    ├── assets/
    │   └── image.png
    └── knowledge/
        └── docs.md
```

### Skill Folder Structure

```
~/.config/zeroagent/skills/{skill-name}/
├── SKILL.md             # Skill definition (markdown with frontmatter)
│   ---
│   name: Search
│   description: Search the web
│   parameters:
│     - name: query
│       type: string
│       required: true
│   ---
│
│   # Search the web for information...
│
└── [additional files]   # Supporting files
```

### MCP Server Config

```
~/.config/zeroagent/mcp_servers/{server-id}.json
{
  "id": "filesystem",
  "name": "Filesystem",
  "transport": "stdio",
  "command": "npx",
  "args": ["-y", "@modelcontextprotocol/server-filesystem", "/allowed/path"],
  "env": {}
}
```

### Conversation Database

```
~/.config/zeroagent/conversations.db (SQLite)

conversations:
  - id (TEXT PRIMARY KEY)
  - agent_id (TEXT)
  - title (TEXT)
  - created_at (TEXT)
  - updated_at (TEXT)

messages:
  - id (TEXT PRIMARY KEY)
  - conversation_id (TEXT)
  - role (TEXT)  -- "user" | "assistant" | "tool"
  - content (TEXT)
  - tool_calls (TEXT - JSON array)
  - tool_call_id (TEXT)
  - created_at (TEXT)
```

## Frontend Component Structure

### Page Components (`src/features/`)

```
src/features/
├── agents/
│   ├── AgentIDEPage.tsx       # Main agent editor (full-page)
│   ├── AgentIDEDialog.tsx     # Agent editor (dialog)
│   ├── AddAgentDialog.tsx     # Quick agent creation
│   └── ConfigYamlForm.tsx     # Config editor with tabs
│
├── providers/
│   └── ProviderManagement.tsx # Provider CRUD
│
├── mcp/
│   ├── McpPage.tsx            # MCP server management
│   └── types.ts               # MCP types
│
└── skills/
    └── SkillsPage.tsx         # Skill management
```

### Shared UI (`src/shared/ui/`)

```
src/shared/ui/
├── button.tsx
├── input.tsx
├── dialog.tsx
├── tabs.tsx
├── select.tsx
├── switch.tsx
├── textarea.tsx
├── label.tsx
└── ...
```

## Configuration Files

### Tauri Config (`src-tauri/tauri.conf.json`)

- Application metadata
- Window configuration
- Security settings
- Build options

### Cargo.toml Dependencies

Key dependencies:
- `tauri` - Desktop framework
- `tokio` - Async runtime
- `serde` / `serde_yaml` - Serialization
- `async-trait` - Async trait support
- `sqlx` - Database (SQLite)
- `reqwest` - HTTP client
- `tokio-process` - Process spawning

## Security Considerations

### IPC Communication

- All Tauri commands validate inputs
- File operations restricted to agent directories
- Protected files cannot be deleted

### Tool Execution

- Python execution in isolated environment
- MCP tools with configurable transport (stdio, HTTP, SSE)
- No arbitrary code execution

### File System

- Agents stored in user config directory
- Staging directory for new agents (cleaned on cancel)
- No access to system files outside config dir

## Performance Optimizations

### Middleware

- Token estimation (no API calls)
- Lazy evaluation (only run when needed)
- Minimal memory allocations

### Streaming

- Real-time token streaming from LLM
- Tool execution progress events
- Efficient event propagation

### Database

- SQLite for fast local storage
- Indexed queries for conversation history
- Connection pooling (single connection)

## Error Handling

### Backend

- All commands return `Result<T, String>`
- Descriptive error messages
- Graceful degradation (optional features fail silently)

### Frontend

- Try-catch around all async operations
- User-friendly error toasts
- Retry logic for network operations

## Future Architecture Plans

### Planned Features

1. **Plugin System**: Dynamic loading of custom middleware
2. **Distributed Agents**: Multi-agent collaboration
3. **Agent Marketplace**: Share/sell agents
4. **Version Control**: Track agent changes over time
5. **Testing Framework**: Test agents with scenarios

### Technical Debt

1. **Remove Unused Code**: Many warnings about unused imports
2. **Add Integration Tests**: End-to-end testing
3. **Improve Logging**: Structured logging with levels
4. **Type Safety**: More Rust-like error types (not String)

## Related Documentation

| File | Description |
|------|-------------|
| `src/commands/AGENTS.md` | Agent commands implementation |
| `src/domains/agent_runtime/middleware/AGENTS.md` | Middleware system |
| `src/commands/skills/AGENTS.md` | Skills implementation |
| `WORKING_SCENARIOS.md` | User-facing scenarios |
| `LOGGING.md` | Logging guidelines |
