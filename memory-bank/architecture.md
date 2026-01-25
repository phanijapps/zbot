# AgentZero Architecture

## Overview

AgentZero is a Tauri-based desktop application for managing AI agents with MCP (Model Context Protocol) server integration, skills, and modular middleware support.

The project is structured as a **Cargo workspace** with a modular framework design. The core framework is split into multiple reusable crates (`zero-*`), each with a specific responsibility, plus application-specific crates (`agent-runtime`, `agent-tools`) and the Tauri application.

## Technology Stack

### Frontend
- **Framework**: React 19 (via Vite)
- **Language**: TypeScript
- **UI Components**: Radix UI primitives with Tailwind CSS v4 styling
- **Workflow Canvas**: XY Flow (React Flow v12+) for visual workflow builder
- **State Management**: Zustand (workflowStore, workflowHistoryStore)
- **Routing**: react-router-dom v7
- **Icons**: lucide-react
- **Validation**: zod
- **Build Tool**: Vite

### Backend
- **Framework**: Tauri 2.x
- **Language**: Rust (Cargo workspace)
- **Async Runtime**: tokio
- **Serialization**: serde (JSON, YAML)
- **Database**: SQLite (via sqlx)

### Key Dependencies
- `tokio` - Async runtime
- `serde` / `serde_yaml` - Serialization
- `tauri` - Desktop framework
- `async-trait` - Async trait support
- `thiserror` - Error handling
- `tracing` - Structured logging
- `reqwest` - HTTP client for LLM APIs
- `sqlx` - Database toolkit
- `xyflow` - Workflow canvas library

## Workspace Structure

```
agentzero/
├── Cargo.toml                 # Workspace root
├── src/                       # Frontend (React + TypeScript)
│   ├── core/                  # Core UI infrastructure
│   │   ├── layout/            # AppShell, Sidebar, StatusBar
│   │   └── utils/             # Utilities (cn classnames)
│   ├── shared/                # Shared code
│   │   ├── ui/                # Radix UI components (button, dialog, select, etc.)
│   │   ├── types/             # TypeScript types (agent, vault, etc.)
│   │   └── constants/         # Routes, constants
│   ├── features/              # Feature-based modules
│   │   ├── agents/            # Agent management UI
│   │   ├── workflow-ide/      # Visual workflow builder (Zero IDE)
│   │   ├── agent-channels/    # Discord-like chat interface
│   │   ├── providers/         # LLM provider management
│   │   ├── mcp/               # MCP server management
│   │   ├── skills/            # Skill editor and management
│   │   ├── conversations/     # Chat conversations
│   │   └── settings/          # App settings
│   ├── domains/               # Domain-specific logic
│   │   └── agent-runtime/     # Agent execution components
│   └── services/              # Tauri IPC wrappers
├── crates/                    # Zero Framework crates
│   ├── zero-core/             # Core traits, types, errors
│   ├── zero-llm/              # LLM abstractions & OpenAI client
│   ├── zero-agent/            # Agent implementations
│   ├── zero-tool/             # Tool definitions & abstractions
│   ├── zero-session/          # Session management
│   ├── zero-mcp/              # MCP protocol integration
│   ├── zero-prompt/           # Prompt templates
│   └── zero-middleware/       # Middleware system
├── application/               # Application-specific crates
│   ├── agent-runtime/         # Agent executor with config, MCP, skills
│   ├── agent-tools/           # Built-in tools
│   ├── agent-channels/        # Agent channels backend
│   ├── daily-sessions/        # Daily session management
│   ├── search-index/          # Full-text search
│   ├── session-archive/       # Long-term message archival
│   └── knowledge-graph/       # Semantic memory
├── memory-bank/               # Project documentation
└── src-tauri/                 # Tauri application
    ├── templates/             # Default agents and skills
    └── src/
        ├── commands/          # Tauri IPC commands
        └── domains/           # Domain layer
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

| Crate | Purpose |
|-------|---------|
| `agent-runtime` | YAML config, executor, MCP managers, skill loading |
| `agent-tools` | Built-in tools: Read, Write, Edit, Grep, Glob, Python, KG |
| `agent-channels` | Agent channels UI and backend coordination |
| `daily-sessions` | Daily session management with SQLite storage |
| `search-index` | Tantivy-based full-text search across messages |
| `session-archive` | Parquet-based long-term message archival |
| `knowledge-graph` | Semantic memory with entities and relationships |

## Frontend Architecture

### Feature Modules (`src/features/`)

#### Workflow IDE (`workflow-ide/`)

**Purpose:** Visual workflow builder for creating orchestrator agents

**Component Hierarchy:**
```
WorkflowIDEPage
├── Header (navigation, save/undo/redo buttons)
├── WorkflowEditor (main canvas)
│   ├── NodePalette (left sidebar - drag & drop nodes)
│   ├── ReactFlow canvas (center - visual workflow builder)
│   └── PropertiesPanel (right sidebar - node/edge configuration)
├── NewAgentDialog (modal for creating new agents)
└── TemplateSelector (modal for applying workflow templates)
```

**State Management:**
- `workflowStore` (Zustand): Nodes, edges, selection, orchestrator config, execution state
- `workflowHistoryStore` (Zustand): Undo/redo with 50-state limit

**Node Types:**
- **Start Node** (`StartNode.tsx`): BPMN thin green circle, bottom handle
- **End Node** (`EndNode.tsx`): BPMN thick red circle, top handle
- **Subagent Node** (`SubagentNode.tsx`): Purple card, top+bottom handles
- **Conditional Node** (`ConditionalNode.tsx`): Amber diamond (DRAFT)
- **Orchestrator Node** (`OrchestratorNode.tsx`): Legacy (migrated to flow-level config)

**Key Files:**
- `WorkflowIDEPage.tsx` - Main page with migration logic
- `WorkflowEditor.tsx` - Canvas wrapper with MiniMap, Controls
- `components/nodes/` - Custom node components
- `components/panels/NodePalette.tsx` - Draggable node library
- `components/panels/PropertiesPanel.tsx` - Configuration panel
- `stores/workflowStore.ts` - Main state management
- `stores/workflowHistoryStore.ts` - Undo/redo history
- `types/workflow.ts` - Type definitions
- `types/templates.ts` - Workflow templates (Pipeline, Swarm, Router, Map-Reduce, Hierarchical)

**Backend Integration:**
- `get_orchestrator_structure(agentId)` - Load workflow graph
- `save_orchestrator_structure(agentId, graph)` - Save workflow graph
- `validate_workflow(graph)` - Validate workflow structure

#### Agent Channels (`agent-channels/`)

**Purpose:** Discord-like interface for daily agent conversations

**Key Features:**
- Daily sessions with expandable day separators
- Knowledge graph visualization (ReactFlow + Dagre)
- Voice recording with transcription
- Attachments panel for transcripts
- History management (Chrome-style clearing)
- Vault switching with state reset

**State Management:**
- Local component state (agents, selectedAgent, messages, etc.)
- Day-based message grouping (loadedDays array)
- Expanded/collapsed day tracking

**Key Files:**
- `AgentChannelPanel.tsx` - Main interface (1,446 lines)
- `DaySeparator.tsx` - Collapsible day headers
- `ClearHistoryDialog.tsx` - History clearing
- `VoiceRecordingDialog.tsx` - Audio recording
- `AttachmentsPanel.tsx` - Attachment management
- `KnowledgeGraphVisualizer.tsx` - Graph visualization
- `TranscriptCommentDialog.tsx` - Transcript comments

**Backend Integration:**
- `get_or_create_today_session(agentId)`
- `list_previous_days(agentId, limit)`
- `load_session_messages(sessionId)`
- `record_session_message(...)`

### Domain Layer (`src/domains/`)

#### Agent Runtime (`agent-runtime/`)

**Purpose:** Agent execution UI components and business logic

**Components:**
- `ConversationView.tsx` - Main conversation view
- `ConversationList.tsx` - Conversation history list
- `ThinkingPanel.tsx` - Thinking mode panel
- `ToolCallsSection.tsx` - Tool calls display
- `GenerativeCanvas.tsx` - Generative canvas for forms
- `useStreamEvents.ts` - Streaming events hook

### Services Layer (`src/services/`)

| Service | Purpose |
|---------|---------|
| `agent.ts` | Agent CRUD, file operations |
| `agentChannels.ts` | Session management, message recording |
| `workflow.ts` | Workflow graph operations, templates |
| `provider.ts` | Provider CRUD operations |
| `mcp.ts` | MCP server management |
| `skills.ts` | Skill management |
| `conversation.ts` | Chat history management |
| `vaults.ts` | Vault management |
| `search.ts` | Full-text search |
| `settings.ts` | Application settings |

## Backend Architecture

### Commands Layer (`src-tauri/src/commands/`)

Tauri commands that expose functionality to the frontend via IPC.

| Module | Purpose |
|--------|---------|
| `agents.rs` | Agent CRUD, file management |
| `agents_runtime.rs` | Agent execution with streaming |
| `providers.rs` | Provider CRUD operations |
| `mcp.rs` | MCP server management |
| `skills.rs` | Skill management |
| `conversations.rs` | Chat history management |
| `tools.rs` | Tool management |
| `settings.rs` | Application settings |
| `vaults.rs` | Vault management |
| `agent_channels.rs` | Daily session management |
| `workflow.rs` | Workflow graph operations |

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
├── .subagents/           # Subagent folder (for orchestrator agents)
│   ├── {subagent-name}/
│   │   ├── config.yaml
│   │   └── AGENTS.md
│   └── ...
└── [user files]          # Additional files/folders
```

### Workflow Storage

```
~/.config/zeroagent/agents/{agent-name}/
├── .workflow-layout.json  # Visual workflow layout (XY Flow positions)
└── .subagents/           # Subagent definitions generated from workflow
    ├── {subagent-name}/
    │   ├── config.yaml
    │   └── AGENTS.md
    └── ...
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

### Agent Channels Database

```
{vault_path}/db/agent_channels.db (SQLite)

daily_sessions:
  - id (TEXT PRIMARY KEY)
  - agent_id (TEXT)
  - session_date (TEXT)
  - message_count (INTEGER)
  - summary (TEXT)
  - previous_session_ids (TEXT - JSON array)
  - created_at (TEXT)
  - updated_at (TEXT)

messages:
  - id (TEXT PRIMARY KEY)
  - session_id (TEXT)
  - role (TEXT)
  - content (TEXT)
  - tool_calls (TEXT - JSON)
  - tool_results (TEXT - JSON)
  - created_at (TEXT)

kg_entities:
  - id (TEXT PRIMARY KEY)
  - agent_id (TEXT)
  - entity_type (TEXT)
  - name (TEXT)
  - properties (TEXT - JSON)
  - mention_count (INTEGER)
  - first_seen_at (TEXT)
  - last_seen_at (TEXT)

kg_relationships:
  - id (TEXT PRIMARY KEY)
  - source_entity_id (TEXT)
  - target_entity_id (TEXT)
  - relationship_type (TEXT)
  - properties (TEXT - JSON)
  - mention_count (INTEGER)
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

## Type System

### Shared Types (`src/shared/types/`)

**Vault:**
```typescript
interface Vault {
  id: string;
  name: string;
  path: string;
  isDefault: boolean;
  createdAt: string;
  lastAccessed: string;
}
```

**Agent:**
```typescript
interface Agent {
  id: string;
  name: string;
  displayName: string;
  description: string;
  providerId: string;
  model: string;
  temperature: number;
  maxTokens: number;
  thinkingEnabled?: boolean;
  voiceRecordingEnabled?: boolean;
  instructions: string;
  mcps: string[];
  skills: string[];
  middleware?: string;
  agentType?: "llm" | "sequential" | "parallel" | "loop" | "conditional" | "custom";
  subAgents?: Agent[];
  createdAt: string;
}
```

**Provider:**
```typescript
interface Provider {
  id: string;
  name: string;
  description: string;
  apiKey: string;
  baseUrl: string;
  models: string[];
  embeddingModels?: string[];
  verified?: boolean;
  createdAt: string;
}
```

**Workflow Graph:**
```typescript
interface WorkflowGraph {
  nodes: WorkflowNode[];
  edges: WorkflowEdge[];
  orchestrator?: OrchestratorConfig;
}

interface WorkflowNode {
  id: string;
  type: string;  // "start" | "end" | "subagent" | "conditional" | "orchestrator"
  position: { x: number; y: number };
  data: WorkflowNodeData;
}

interface OrchestratorConfig {
  displayName: string;
  description?: string;
  providerId: string;
  model: string;
  temperature: number;
  maxTokens: number;
  systemInstructions: string;
  mcps: string[];
  skills: string[];
  middleware?: string;
}
```

## Configuration Files

### Cargo Workspace (`Cargo.toml`)

Defines workspace members and shared dependencies.

### Tauri Config (`src-tauri/tauri.conf.json`)

Application metadata, window config, security settings.

## Related Documentation

| File | Description |
|------|-------------|
| `memory-bank/product.md` | Product definition |
| `memory-bank/known_issues.md` | Known issues tracking |
| `memory-bank/learnings.md` | Architecture learnings |
| `crates/*/AGENTS.md` | Framework crate documentation |
| `application/*/AGENTS.md` | Application crate documentation |
| `LOGGING.md` | Logging guidelines |
