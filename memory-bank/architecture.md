# Agent Zero - Architecture

## Technology Stack

| Layer | Technology |
|-------|------------|
| Desktop | Tauri 2.x |
| Frontend | React 19 + TypeScript + Vite |
| UI | Radix UI + Tailwind CSS v4 |
| Workflow | XY Flow (React Flow v12+) |
| State | Zustand |
| Backend | Rust (Cargo workspace) |
| Database | SQLite (sqlx) |
| Async | tokio |

## Workspace Structure

```
agentzero/
├── src/                       # Frontend (React)
│   ├── core/                  # Layout (AppShell, Sidebar)
│   ├── shared/                # UI components, types, constants
│   ├── features/              # Feature modules
│   │   ├── workflow-ide/      # Visual workflow builder
│   │   ├── agent-channels/    # Chat interface
│   │   ├── agents/            # Agent management
│   │   ├── providers/         # LLM providers
│   │   ├── mcp/               # MCP servers
│   │   └── skills/            # Skill editor
│   └── services/              # Tauri IPC wrappers
├── crates/                    # Zero Framework
│   ├── zero-core/             # Core traits (Agent, Tool, Session)
│   ├── zero-llm/              # LLM abstractions
│   ├── zero-agent/            # Agent implementations
│   ├── zero-session/          # Session management
│   ├── zero-mcp/              # MCP integration
│   └── zero-app/              # Meta-package
├── application/               # Application crates
│   ├── agent-runtime/         # Agent executor
│   ├── agent-tools/           # Built-in tools
│   ├── workflow-executor/     # Workflow execution
│   ├── daily-sessions/        # Session storage
│   └── knowledge-graph/       # Semantic memory
└── src-tauri/                 # Tauri application
    ├── src/commands/          # IPC commands
    └── src/domains/           # Domain logic
```

## Core Abstractions

```rust
// Agent - invokable AI entity
trait Agent {
    async fn invoke(&self, context: InvocationContext) -> Result<EventStream>;
}

// Tool - callable function
trait Tool {
    fn name(&self) -> &str;
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

## Storage

**Global Config** (`~/.config/agentzero/`):
- `vaults_registry.json` - Vault registry
- `utils/` - Shared scripts
- `venv/` - Python environment

**Vault Directory** (`~/Documents/{vault}/`):
```
{vault}/
├── agents/{name}/
│   ├── config.yaml           # Metadata
│   ├── AGENTS.md             # Instructions
│   ├── .workflow-layout.json # Visual layout
│   └── .subagents/           # Subagent configs
├── skills/{name}/
│   └── SKILL.md              # Skill with frontmatter
├── db/
│   └── agent_channels.db     # Sessions, messages, KG
├── providers.json
└── mcps.json
```

## Agent Execution Flow

```
User Message → Tauri Command → Load Config → Create LLM →
Initialize MCPs → Create Tools → Build LlmAgent →
Agent Loop (LLM → Tool Calls → Responses) → Stream Events
```

## Workflow Execution

1. Frontend generates `invocationId`, sets up event listeners
2. Calls `execute_workflow` Tauri command
3. Backend loads workflow definition, builds executable graph
4. Orchestrator LLM receives subagents as tools
5. Events stream via `workflow-stream://{invocationId}`
6. Node status updates via `workflow-node://{workflowId}`

## Key Design Principles

1. **Instructions in AGENTS.md** - Not in config.yaml
2. **Flow-level orchestrator** - Not as a node
3. **Frontend-first invocation IDs** - Prevents event race conditions
4. **Multi-vault isolation** - Each vault is self-contained
