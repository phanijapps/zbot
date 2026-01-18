# Agent Zero - Architecture Learnings

This document captures architectural decisions, patterns, and learnings as the project evolves.

## Project Overview

Agent Zero is a desktop application (similar to Claude Desktop) built with:
- **Tauri 2.9** - Cross-platform desktop framework with Rust backend
- **React 19** - Frontend UI framework
- **TypeScript** - Type safety across the stack
- **React Router** - Client-side routing
- **Tailwind CSS v4** - Utility-first CSS framework with modern engine
- **Radix UI** - Unstyled, accessible component primitives

## Backend Architecture

### Zero Framework: Modular Crate Design

The backend is structured as a **Cargo workspace** with clear separation between the reusable framework (`crates/`) and application-specific code (`application/`):

#### Zero Framework (`crates/`)

Reusable framework crates - independent of the Tauri application:

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

#### Application Crates (`application/`)

Application-specific crates - tightly coupled to Tauri:

| Crate | Purpose |
|-------|---------|
| `agent-runtime` | YAML config, executor, MCP managers, skill loading |
| `agent-tools` | Built-in tools: Read, Write, Edit, Grep, Glob, Python, etc. |

### Core Abstractions

**Agent** (`zero-core`):
```rust
#[async_trait]
pub trait Agent: Send + Sync {
    async fn invoke(&self, context: InvocationContext) -> Result<EventStream>;
}
```

**Tool** (`zero-core`):
```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Option<Value>;
    async fn execute(&self, ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value>;
}
```

**Session** (`zero-session`):
```rust
#[async_trait]
pub trait Session: Send + Sync {
    async fn append(&self, event: Event) -> Result<()>;
    async fn events(&self) -> Result<Vec<Event>>;
}
```

**Llm** (`zero-llm`):
```rust
#[async_trait]
pub trait Llm: Send + Sync {
    async fn generate(&self, request: LlmRequest) -> Result<LlmResponse>;
    async fn generate_stream(&self, request: LlmRequest) -> Result<LlmResponseStream>;
}
```

### Agent Execution Flow

```
User Message → Tauri Command
    │
    ▼
Load Agent (config.yaml + AGENTS.md)
    │
    ▼
Create LLM Client (OpenAiLlm with provider config)
    │
    ▼
Initialize MCP Servers (stdio/HTTP/SSE)
    │
    ▼
Create Tools (application/app-tools + MCP bridges)
    │
    ▼
Create LlmAgent (builder pattern)
    │
    ▼
Invoke Agent → Loop:
    1. Build request from session events
    2. Call LLM
    3. If tool calls: execute → add to session → repeat
    4. If no tool calls: return response
```

### Content and Event Types

**Content** represents messages with role and parts:
```rust
pub struct Content {
    pub role: String,
    pub parts: Vec<Part>,
}

pub enum Part {
    Text { text: String },
    FunctionCall { name, args, id },
    FunctionResponse { id, response },
    Binary { mime_type, data },
}
```

**Event** represents immutable conversation state changes:
- User messages
- Assistant messages
- Tool calls
- Tool responses
- Error events

### Tauri Commands Pattern

Commands are organized by domain in `src-tauri/src/commands/`:

```rust
#[tauri::command]
pub async fn list_agents() -> Result<Vec<Agent>, String> { }

#[tauri::command]
pub async fn create_agent(agent: Agent) -> Result<Agent, String> { }

#[tauri::command]
pub async fn execute_agent_stream(
    agent_id: String,
    messages: Vec<ChatMessage>,
    conversation_id: String,
) -> Result<(), String> { }
```

All commands are registered in `lib.rs`:
```rust
.invoke_handler(tauri::generate_handler![
    commands::list_agents,
    commands::create_agent,
    // ...
])
```

### Storage Schema

**Agent Folder** (`~/.config/zeroagent/agents/{agent-name}/`):
```
├── config.yaml    # Metadata: name, model, provider, skills[], mcps[]
├── AGENTS.md      # Agent instructions (markdown, no frontmatter)
└── [user files]   # Additional resources
```

**Skill Folder** (`~/.config/zeroagent/skills/{skill-name}/`):
```
├── SKILL.md       # Skill with YAML frontmatter (name, description, parameters)
└── [files]        # Supporting files
```

**Conversation Database** (`~/.config/zeroagent/conversations.db`):
```sql
conversations: id, agent_id, title, created_at, updated_at
messages: id, conversation_id, role, content, tool_calls, tool_call_id, created_at
```

### Conversation-Scoped File Operations

Files written by agents are scoped to `~/.config/zeroagent/logs/<conv-id>/`:
- `scratchpad/` - Temporary working files
- `attachments/` - Generated reports, images
- `memory.md` - Summarized context

**Implementation**: `ToolContext` carries `conversation_id` for path resolution in Write/Edit tools.

### Model Configuration Impact

**Critical Discovery**: High temperature causes models to ignore tool-calling instructions.

| Setting | Broken | Working |
|---------|--------|---------|
| temperature | 1.4 | 0.7 |
| maxTokens | 150 | 2000 |

### AGENTS.md Best Practices

```markdown
# AGENTS.md
You are a [description] agent.

## IMPORTANT - Tool Calling Rules
- When asked to write/create/save something, you MUST call the `write` tool
- ALWAYS use tools for actions - never just describe what you would do
- Use paths like `attachments/report.md` for generated files

## Available Tools
- `write` - Write content to a file
- `read` - Read file contents
- ...
```

### MCP Tool Naming Convention

Pattern: `{normalized_server_id}__{tool_name}`
- `time-server` → `time_server__get_current_time`
- Parse with `splitn(2, "__")` to extract server and tool name

### Known Issues

See `memory-bank/known_issues.md` for tracked issues.

## Frontend Architecture

### Modular by Domain

```
src/
├── core/           # Core shell, routing, layout (cross-cutting)
├── features/       # Feature modules (conversations, agents, providers, etc.)
├── shared/         # Shared types, constants, utilities, UI components
└── services/       # API services, storage abstraction
```

### Frontend Service Pattern

Services abstract Tauri command calls:

```typescript
// src/services/agents.ts
import { invoke } from "@tauri-apps/api/core";

export async function listAgents(): Promise<Agent[]> {
  return invoke("list_agents");
}
```

## Development Workflow

### Running the App

```bash
# Install dependencies
npm install

# Development mode (hot reload)
npm run tauri dev

# Build for production
npm run tauri build
```

## Design System

### Overview

Agent Zero uses a modern design system featuring:
- **Dark-first theme** with deep blacks (#0a0a0a)
- **Gradient accents** for visual hierarchy
- **Glassmorphism** with semi-transparent overlays
- **Icon-based navigation** with lucide-react icons

### Tech Stack

| Technology | Purpose |
|------------|---------|
| **Tailwind CSS v4.1.12** | Utility-first styling with new engine |
| **@tailwindcss/vite** | Official Vite plugin for Tailwind v4 |
| **Radix UI Primitives** | Unstyled, accessible components |
| **class-variance-authority** | Component variant management |
| **lucide-react** | Icon library |

### Design Tokens

**Background Colors**
- `bg-[#0a0a0a]` - Main background
- `bg-[#0f0f0f]` - Sidebar background
- `bg-white/5` - Card backgrounds
- `bg-white/10` - Hover states

**Gradients**
- `from-blue-500 to-purple-600` - Primary actions
- `from-orange-500 to-pink-600` - Accent

### Component Patterns

**Button with Variants (CVA)**
```typescript
const buttonVariants = cva(
  "inline-flex items-center justify-center rounded-md text-sm font-medium",
  {
    variants: {
      variant: {
        default: "bg-white text-black hover:bg-white/90",
        gradient: "bg-gradient-to-r from-blue-600 to-purple-600 text-white",
        outline: "border border-white/20 bg-transparent hover:bg-white/10",
      },
    },
  }
);
```

## Key Decisions

### Why Tauri over Electron?

- **Package size**: Tauri apps are ~10MB vs Electron's ~100MB+
- **Performance**: Rust backend is faster and more memory efficient
- **Security**: Smaller attack surface with Rust
- **System integration**: Better native OS integration

### Why React Router over Tauri router?

- **Client-side routing**: Faster navigation, no IPC overhead
- **Browser APIs**: Works with web standards
- **Development**: Easier testing and debugging

## Recent Session Learnings

### Agent Executor with Tool Calling Loop

**Tool Calling Loop Pattern**:
```rust
async fn execute_with_tools_loop(
    &self,
    messages: Vec<ChatMessage>,
    tools_schema: Option<Value>,
    on_event: &mut impl FnMut(StreamEvent),
) -> Result<(), String> {
    let mut current_messages = messages;
    let mut max_iterations = 10;

    loop {
        let response = self.llm_client.chat(current_messages.clone(), tools_schema.clone()).await?;

        if response.tool_calls.is_empty() {
            // Stream final response and break
            break;
        }

        // Add assistant message with tool calls
        current_messages.push(/* assistant message */);

        // Execute each tool and add results
        for tool_call in &response.tool_calls {
            let result = self.execute_tool(&tool_call.name(), &tool_call.arguments()).await?;
            current_messages.push(/* tool result message */);
        }
    }
}
```

**Learnings**:
- Max iterations prevents infinite loops
- Tool results added as messages enables multi-turn conversations
- Reasoning content parsed from `/choices/0/message/reasoning_content` for DeepSeek/GLM

### Streaming Event Architecture

**StreamEvent Types**:
```rust
pub enum StreamEvent {
    Metadata { timestamp, agent_id, model, provider },
    Token { timestamp, content },
    Reasoning { timestamp, content },
    ToolCallStart { timestamp, tool_id, tool_name, args },
    ToolCallEnd { timestamp, tool_id, tool_name, args },
    ToolResult { timestamp, tool_id, result, error },
    Done { timestamp, final_message, token_count },
    Error { timestamp, error, recoverable },
}
```

### File Explorer with Hierarchical Tree

**Recursive File Scanning** (Rust):
```rust
fn collect_files(dir: &PathBuf, base_path: &PathBuf, relative_path: &str, files: &mut Vec<AgentFile>) -> Result<(), String> {
    let entries = fs::read_dir(dir)?;
    for entry in entries.flatten() {
        // Process entry, then recurse for directories
        if !is_file {
            collect_files(&path, base_path, &new_relative_path, files)?;
        }
    }
}
```

**Frontend Tree Building**:
```typescript
const buildFileTree = (): FileNode[] => {
    const nodeMap = new Map<string, FileNode>();
    // Create nodes, then organize by path hierarchy
};
```

### Auto-Save Pattern with Debouncing

```typescript
useEffect(() => {
    if (!initialItem || !selectedFile || !fileContent) return;
    if (editingContent === fileContent.content) return;

    const timer = setTimeout(async () => {
        setIsAutoSaving(true);
        await service.writeFile(getItemId(), selectedFile.path, editingContent);
        setLastSaved(new Date());
        setIsAutoSaving(false);
    }, 1000); // 1 second debounce

    return () => clearTimeout(timer);
}, [editingContent, initialItem, selectedFile, fileContent]);
```

### Staging Mode Pattern

New items created in staging area before save:
```rust
fn is_staging_mode(agent_id: &str) -> bool {
    agent_id == "staging" || agent_id == "temp"
}
```

## References

- [Tauri Documentation](https://tauri.app/)
- [React Router Documentation](https://reactrouter.com/)
- See `crates/*/AGENTS.md` for framework crate documentation
