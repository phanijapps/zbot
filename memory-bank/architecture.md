# AgentZero Architecture

## Overview

AgentZero is a Tauri-based desktop application for managing AI agents with MCP (Model Context Protocol) server integration, skills, and modular middleware support.

The project is structured as a **Cargo workspace** with a modular framework design. The core framework is split into multiple reusable crates (`zero-*`), each with a specific responsibility, plus application-specific crates (`agent-runtime`, `agent-tools`) and the Tauri application.

## Technology Stack

### Frontend
- **Framework**: React 19 (via Vite)
- **Language**: TypeScript
- **UI Components**: Radix UI primitives with Tailwind CSS styling
- **Editor**: `@uiw/react-md-editor` for markdown editing
- **State Management**: React hooks (useState, useEffect)
- **Routing**: react-router-dom v7
- **Icons**: lucide-react
- **Validation**: zod
- **Build Tool**: Vite

### Backend
- **Framework**: Tauri 2.x
- **Language**: Rust (Cargo workspace)
- **Async Runtime**: tokio
- **Serialization**: serde (JSON, YAML)

### Key Dependencies
- `tokio` - Async runtime
- `serde` / `serde_yaml` - Serialization
- `tauri` - Desktop framework
- `async-trait` - Async trait support
- `thiserror` - Error handling
- `tracing` - Structured logging
- `reqwest` - HTTP client for LLM APIs

## Workspace Structure

```
agentzero/
├── Cargo.toml                 # Workspace root
├── src/                       # Frontend (React + TypeScript)
│   ├── core/                  # Core UI infrastructure
│   │   ├── layout/            # AppShell, Sidebar, StatusBar
│   │   └── utils/             # Utilities (cn classnames)
│   ├── shared/                # Shared code
│   │   ├── ui/                # Radix UI components (button, dialog, etc.)
│   │   ├── types/             # TypeScript types (agent, etc.)
│   │   └── constants/         # Routes, constants
│   ├── features/              # Feature-based modules
│   │   ├── agents/            # Agent management UI (IDE, panels)
│   │   ├── providers/         # LLM provider management
│   │   ├── mcp/               # MCP server management
│   │   ├── skills/            # Skill editor and management
│   │   ├── conversations/     # Chat conversations
│   │   └── settings/          # App settings
│   ├── domains/               # Domain-specific logic
│   │   └── agent-runtime/     # Agent execution components (ConversationView, etc.)
│   └── services/              # Tauri IPC wrappers (agent.ts, provider.ts, etc.)
├── crates/                    # Zero Framework crates
│   ├── zero-core/             # Core traits, types, errors
│   ├── zero-llm/              # LLM abstractions & OpenAI client
│   ├── zero-agent/            # Agent implementations (LlmAgent, workflows)
│   ├── zero-tool/             # Tool definitions & abstractions
│   ├── zero-session/          # Session management
│   ├── zero-mcp/              # MCP protocol integration
│   ├── zero-prompt/           # Prompt templates
│   ├── zero-middleware/       # Middleware system
│   └── zero-app/              # Meta-package (all zero-* crates)
├── application/               # Application-specific crates
│   ├── agent-runtime/         # Agent executor with config, MCP, skills
│   ├── agent-tools/           # Built-in tools (read, write, grep, python, etc.)
│   ├── daily-sessions/        # Daily session management for agent channels
│   ├── search-index/          # Tantivy-based full-text search
│   ├── session-archive/       # Parquet-based long-term message archival
│   └── knowledge-graph/       # Semantic memory (entities and relationships)
├── memory-bank/               # Project documentation
│   ├── architecture.md        # This file
│   ├── learnings.md           # Architecture learnings
│   ├── known_issues.md        # Known issues tracking
│   └── product.md             # Product definition
└── src-tauri/                 # Tauri application
    └── src/
        ├── commands/          # Tauri IPC commands
        └── domains/           # Domain layer (agent_runtime, conversation_runtime)
```

## Framework Crate Overview

### Zero Framework (`crates/`)

The **zero-* crates** form the reusable framework - independent of the Tauri application.

| Crate | Purpose |
|-------|---------|
| `zero-core` | Core traits: `Agent`, `Tool`, `Session`, `Event`, `Content`, errors |
| `zero-llm` | LLM trait, OpenAI client, request/response types |
| `zero-agent` | Agent implementations: `LlmAgent`, workflow agents |
| `zero-tool` | Tool trait and abstractions |
| `zero-session` | Session trait and in-memory implementation |
| `zero-mcp` | MCP client and tool bridging |
| `zero-prompt` | Prompt template system |
| `zero-middleware` | Middleware pipeline for request/response processing |
| `zero-app` | Convenience meta-package importing all zero-* crates |

### Application Crates (`application/`)

The **application crates** are tightly coupled to the Tauri app and its specific needs.

| Crate | Purpose |
|-------|---------|
| `agent-runtime` | YAML config, executor, MCP managers, skill loading |
| `agent-tools` | Built-in tools: Read, Write, Edit, Grep, Glob, Python, Knowledge Graph |
| `daily-sessions` | Daily session management with SQLite storage |
| `search-index` | Tantivy-based full-text search across messages |
| `session-archive` | Parquet-based long-term message archival |
| `knowledge-graph` | Semantic memory with entities and relationships |

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
│  │                    Domain Layer                          │   │
│  │                                                           │   │
│  │  ┌────────────────────────────────────────────────────┐  │   │
│  │  │              agent_runtime                          │  │   │
│  │  │  (YAML config, executor, MCP managers, skills)     │  │   │
│  │  └────────────────────────────────────────────────────┘  │   │
│  │                                                           │   │
│  │  ┌────────────────────────────────────────────────────┐  │   │
│  │  │           conversation_runtime                      │  │   │
│  │  │  (SQLite database, repositories)                    │  │   │
│  │  └────────────────────────────────────────────────────┘  │   │
│  └─────────────────────────────────────────────────────────┘   │
│                              │                                  │
│                              ▼                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                  Zero Framework Crates                   │   │
│  │                                                           │   │
│  │  ┌─────────────────────────────────────────────────────┐ │   │
│  │  │  zero-app (meta-package)                            │ │   │
│  │  │    ├── zero-core      (Agent, Tool, Session, Event) │ │   │
│  │  │    ├── zero-llm       (Llm trait, OpenAI client)    │ │   │
│  │  │    ├── zero-agent     (LlmAgent, workflows)         │ │   │
│  │  │    ├── zero-tool      (Tool trait)                  │ │   │
│  │  │    ├── zero-session   (InMemorySession)             │ │   │
│  │  │    ├── zero-mcp       (MCP client, bridge)          │ │   │
│  │  │    ├── zero-prompt    (Prompt templates)            │ │   │
│  │  │    └── zero-middleware (Middleware pipeline)        │ │   │
│  │  └─────────────────────────────────────────────────────┘ │   │
│  │                                                           │   │
│  │  ┌─────────────────────────────────────────────────────┐ │   │
│  │  │  agent-runtime                                      │ │   │
│  │  │    ├── YAML config parsing                          │ │   │
│  │  │    ├── MCP managers (stdio, HTTP/SSE)               │ │   │
│  │  │    ├── Skill file loading                           │ │   │
│  │  │    └── Executor orchestration                       │ │   │
│  │  └─────────────────────────────────────────────────────┘ │   │
│  │                                                           │   │
│  │  ┌─────────────────────────────────────────────────────┐ │   │
│  │  │  agent-tools                                        │ │   │
│  │  │    ├── File: Read, Write, Edit                      │ │   │
│  │  │    ├── Search: Grep, Glob                           │ │   │
│  │  │    ├── Exec: Python, LoadSkill                      │ │   │
│  │  │    ├── UI: RequestInput, ShowContent                │ │   │
│  │  │    └── Knowledge Graph: list_entities,              │ │   │
│  │  │       search_entities, get_entity_relationships,    │ │   │
│  │  │       add_entity, add_relationship                 │ │   │
│  │  └─────────────────────────────────────────────────────┘ │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

## Module Structure

### Commands Layer (`src-tauri/src/commands/`)

Tauri commands that expose functionality to the frontend via IPC.

| Module | Purpose |
|--------|---------|
| `agents.rs` | Agent CRUD, file management |
| `agents_runtime.rs` | Agent execution with streaming |
| `providers.rs` | Provider CRUD operations |
| `mcp.rs` | MCP server management |
| `skills.rs` | Skill management with frontmatter |
| `conversations.rs` | Chat history management |
| `tools.rs` | Tool management |
| `settings.rs` | Application settings |

### Domain Layer (`src-tauri/src/domains/`)

#### agent_runtime

Core agent execution engine.

```
agent_runtime/
├── mod.rs                  # Module exports
├── executor.rs             # Main executor orchestration
├── executor_v2.rs          # V2 executor with zero-framework
├── config_adapter.rs       # Convert agent config to LlmAgent
├── filesystem.rs           # FileSystemContext implementation
├── middleware_integration.rs # Middleware integration
└── types.rs                # Additional types
```

#### conversation_runtime

Chat history and database management.

```
conversation_runtime/
├── mod.rs                  # Module exports
├── database/
│   ├── connection.rs       # SQLite connection
│   └── schema.rs           # Database schema
└── repository/
    ├── conversations.rs    # Conversation CRUD
    └── messages.rs         # Message CRUD
```

### Frontend (`src/`)

React + TypeScript frontend organized by feature and domain.

```
src/
├── core/                    # Core UI infrastructure
│   ├── layout/
│   │   ├── AppShell.tsx    # Main app layout with sidebar
│   │   ├── Sidebar.tsx     # Navigation sidebar
│   │   └── StatusBar.tsx   # Status bar
│   └── utils/
│       └── cn.ts           # Classname utility (clsx + tailwind-merge)
│
├── shared/                  # Shared code across features
│   ├── ui/                 # Reusable UI components (Radix UI wrappers)
│   │   ├── button.tsx
│   │   ├── dialog.tsx
│   │   ├── tabs.tsx
│   │   ├── select.tsx
│   │   ├── input.tsx
│   │   ├── textarea.tsx
│   │   ├── switch.tsx
│   │   ├── separator.tsx
│   │   ├── scroll-area.tsx
│   │   ├── dropdown-menu.tsx
│   │   ├── tooltip.tsx
│   │   ├── label.tsx
│   │   ├── badge.tsx
│   │   ├── card.tsx
│   │   └── utils.ts
│   ├── types/              # TypeScript type definitions
│   │   ├── agent.ts
│   │   └── index.ts
│   └── constants/          # Constants
│       └── routes.ts       # Route definitions
│
├── features/               # Feature-based modules (pages & panels)
│   ├── agents/             # Agent management
│   │   ├── AgentIDEPage.tsx       # Agent IDE page
│   │   ├── AgentIDEDialog.tsx     # Agent IDE dialog
│   │   ├── AgentsPanel.tsx        # Agents list panel
│   │   ├── AddAgentDialog.tsx     # Add agent dialog
│   │   ├── ConfigYamlForm.tsx     # YAML config form
│   │   └── AGENTS.md              # Agent management docs
│   ├── providers/          # LLM provider management
│   │   ├── ProvidersPanel.tsx
│   │   └── AddProviderDialog.tsx
│   ├── mcp/                # MCP server management
│   │   ├── MCPServersPanel.tsx
│   │   ├── AddMCPServerDialog.tsx
│   │   └── types.ts
│   ├── skills/             # Skill editor and management
│   │   ├── SkillIDEPage.tsx
│   │   ├── SkillsPanel.tsx
│   │   ├── SkillMdForm.tsx
│   │   └── types.ts
│   ├── conversations/      # Chat conversations
│   │   └── ConversationsPanel.tsx
│   └── settings/           # App settings
│       ├── SettingsPanel.tsx
│       └── types.ts
│
├── domains/                # Domain-specific logic (not feature-specific)
│   └── agent-runtime/      # Agent execution domain
│       ├── components/     # Agent execution UI components
│       │   ├── ConversationView.tsx    # Main conversation view
│       │   ├── ConversationList.tsx    # Conversation history list
│       │   ├── ThinkingPanel.tsx       # Thinking mode panel
│       │   ├── ThinkingTab.tsx         # Thinking mode tab
│       │   ├── ToolCallsSection.tsx    # Tool calls display
│       │   ├── PlanSection.tsx         # Plan section
│       │   ├── GenerativeCanvas.tsx    # Generative canvas
│       │   ├── types.ts                # Domain types
│       │   ├── useStreamEvents.ts      # Streaming events hook
│       │   └── index.ts
│       └── services/
│           └── ConversationService.ts  # Conversation business logic
│
├── services/               # Tauri IPC service wrappers
│   ├── agent.ts            # Agent commands wrapper
│   ├── provider.ts         # Provider commands wrapper
│   ├── mcp.ts              # MCP commands wrapper
│   ├── skills.ts           # Skill commands wrapper
│   ├── conversation.ts     # Conversation commands wrapper
│   └── settings.ts         # Settings commands wrapper
│
├── styles/                 # Global styles
│   └── index.css
│
├── App.tsx                 # Root app component
└── main.tsx                # Entry point
```

## Core Abstractions

### Agent (zero-core)

```rust
#[async_trait]
pub trait Agent: Send + Sync {
    async fn invoke(&self, context: InvocationContext) -> Result<EventStream>;
}
```

### Tool (zero-core)

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Option<Value>;
    async fn execute(&self, ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value>;
}
```

### Session (zero-session)

```rust
#[async_trait]
pub trait Session: Send + Sync {
    async fn append(&self, event: Event) -> Result<()>;
    async fn events(&self) -> Result<Vec<Event>>;
}
```

### Llm (zero-llm)

```rust
#[async_trait]
pub trait Llm: Send + Sync {
    async fn generate(&self, request: LlmRequest) -> Result<LlmResponse>;
    async fn generate_stream(&self, request: LlmRequest) -> Result<LlmResponseStream>;
}
```

## Data Flow: Agent Execution

```
User Message (Frontend)
       │
       ▼
┌─────────────────────────────────────────────────────────┐
│  Tauri Command: execute_agent_stream                    │
│       │                                                  │
│       ▼                                                  │
│  1. Load Agent Configuration                            │
│     - Read config.yaml from ~/.config/zeroagent/agents/ │
│     - Parse YAML to AgentConfig                          │
│       │                                                  │
│       ▼                                                  │
│  2. Create LLM Client                                   │
│     - Use provider config for API key, base URL         │
│     - Create OpenAiLlm instance                         │
│       │                                                  │
│       ▼                                                  │
│  3. Initialize MCP Servers                              │
│     - For each MCP in agent config:                      │
│       - Start stdio or HTTP/SSE client                   │
│       - Discover tools                                   │
│       - Bridge to zero-core Tool trait                   │
│       │                                                  │
│       ▼                                                  │
│  4. Create Tools                                       │
│     - Built-in tools from application/agent-tools       │
│     - MCP tools from bridges                            │
│     - Wrap in Toolset                                   │
│       │                                                  │
│       ▼                                                  │
│  5. Create LlmAgent                                    │
│     - Using builder pattern                             │
│     - With LLM, session, tools, system instruction      │
│       │                                                  │
│       ▼                                                  │
│  6. Invoke Agent                                       │
│     - agent.invoke(context)                             │
│     - Stream events back to frontend                    │
└─────────────────────────────────────────────────────────┘
       │
       ▼
LlmAgent Execution Loop
┌─────────────────────────────────────────────────────────┐
│  1. Build Request                                      │
│     - Get events from session                          │
│     - Convert to Content messages                     │
│     - Add system instruction                           │
│       │                                                  │
│       ▼                                                  │
│  2. Call LLM                                           │
│     - llm.generate(request)                            │
│       │                                                  │
│       ▼                                                  │
│  3. Check for Tool Calls                               │
│     - If tool calls present:                           │
│       - For each tool call:                            │
│         - Execute tool via Toolset                     │
│         - Append tool call event to session            │
│         - Append tool response event to session        │
│       - Loop back to step 1                            │
│     - If no tool calls (turn_complete = true):         │
│       - Return final response                          │
└─────────────────────────────────────────────────────────┘
```

## Storage Schema

### Agent Folder Structure

```
~/.config/zeroagent/agents/{agent-name}/
├── config.yaml           # Agent metadata
│   - name, displayName, description
│   - providerId, model
│   - temperature, maxTokens
│   - thinkingEnabled
│   - skills[]
│   - mcps[]
│
├── AGENTS.md             # Agent instructions (markdown)
└── [user files]          # Additional files/folders
```

### Skill Folder Structure

```
~/.config/zeroagent/skills/{skill-name}/
├── SKILL.md             # Skill definition (markdown with frontmatter)
│   ---
│   name: Search
│   description: Search the web
│   parameters: [...]
│   ---
│   # Skill instructions...
│
└── [additional files]
```

### MCP Server Config

```
~/.config/zeroagent/mcp_servers/{server-id}.json
{
  "id": "filesystem",
  "name": "Filesystem",
  "transport": "stdio",
  "command": "npx",
  "args": ["-y", "@modelcontextprotocol/server-filesystem", "/path"],
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
  - tool_calls (TEXT - JSON)
  - tool_call_id (TEXT)
  - created_at (TEXT)
```

## Configuration Files

### Cargo Workspace (`Cargo.toml`)

Defines workspace members and shared dependencies.

### Tauri Config (`src-tauri/tauri.conf.json`)

Application metadata, window config, security settings.

## Known Issues

See `memory-bank/known_issues.md` for tracked issues, including:
- Write tool path resolution issue

## Related Documentation

| File | Description |
|------|-------------|
| `memory-bank/knowledge-graph.md` | Knowledge Graph feature guide with examples |
| `memory-bank/known_issues.md` | Known issues tracking |
| `memory-bank/learnings.md` | Architecture learnings |
| `memory-bank/product.md` | Product definition |
| `crates/*/AGENTS.md` | Framework crate documentation |
| `crates/AGENTS.md` | Framework crates overview |
| `application/*/AGENTS.md` | Application crate documentation |
| `src-tauri/src/commands/AGENTS.md` | Commands implementation |
| `LOGGING.md` | Logging guidelines |
