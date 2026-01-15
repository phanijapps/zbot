# Agent Zero

Agent Zero is an Agent UI similar to Claude Desktop. The difference being, it can be used to connect to any OpenAI based APIs and be used to build agents, skills and connect to tools for daily use.

## Technology Stack

| Library/Application | Version |
|--------------------|---------|
| Tauri               | 2.x     |
| React               | 19.x    |
| Rust                | 1.87+   |
| TypeScript          | 5.x     |
| reqwest             | 0.12    |
| async-trait         | 0.1     |

## Quick Start

### Prerequisites

Install system dependencies for your platform:

**Linux (Ubuntu/Debian):**
```bash
sudo apt install libwebkit2gtk-4.1-dev \
                 build-essential \
                 curl \
                 wget \
                 file \
                 libssl-dev \
                 libayatana-appindicator3-dev \
                 librsvg2-dev
```

**Linux (Fedora):**
```bash
sudo dnf install webkit2gtk4.1-devel \
                 openssl-devel \
                 curl \
                 wget \
                 file \
                 libappindicator-gtk3-devel \
                 librsvg2-devel
```

**macOS:** No additional dependencies needed.

**Windows:** Install [WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) and [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/).

See https://tauri.app/guides/prerequisites/ for full details.

### Installation

```bash
# Install dependencies
npm install

# Run in development mode
npm run tauri dev
```

### Building

```bash
# Build for production
npm run tauri build
```

The built application will be in `src-tauri/target/release/bundle/`.

## Project Structure

```
agentzero/
├── src/                          # React frontend
│   ├── core/                     # Core shell, routing, layout
│   ├── features/                 # Feature modules
│   │   ├── conversations/        # Chat conversations
│   │   ├── agents/               # Agent management
│   │   ├── providers/            # LLM provider config
│   │   ├── mcp/                  # MCP server management
│   │   ├── skills/               # Skills and plugins
│   │   └── settings/             # App settings
│   ├── shared/                   # Shared types, constants
│   └── services/                 # API services
├── src-tauri/                    # Rust backend
│   ├── src/
│   │   ├── commands/             # Tauri commands (by domain)
│   │   │   └── agents_runtime.rs # Agent execution commands
│   │   ├── domains/              # Business logic by domain
│   │   │   ├── agent_runtime/    # Agent execution engine
│   │   │   │   ├── executor.rs   # Main agent executor
│   │   │   │   ├── tools.rs      # Built-in tools
│   │   │   │   ├── llm.rs        # LLM client
│   │   │   │   └── mcp_manager.rs# MCP integration
│   │   │   └── conversation_runtime/ # Conversation persistence
│   │   └── settings/             # App configuration
│   └── Cargo.toml
└── AGENTS.md                     # This file
```

## Agent Runtime Architecture

The agent execution system is now fully implemented in Rust:

### Components

1. **Tool Registry** (`tools.rs`)
   - Custom `Tool` trait for async tool execution
   - Built-in tools: Read, Write, Edit, Grep, Glob, Python
   - Extensible design for adding new tools

2. **LLM Client** (`llm.rs`)
   - OpenAI-compatible API client
   - Streaming support via Server-Sent Events
   - Tool calling support

3. **Agent Executor** (`executor.rs`)
   - Conversation management
   - Tool calling loop with max iterations
   - Streaming event emission
   - Error handling and recovery

4. **MCP Manager** (`mcp_manager.rs`)
   - Model Context Protocol server integration
   - Dynamic tool discovery from MCP servers

### Built-in Tools

| Tool    | Description                              |
|---------|------------------------------------------|
| read    | Read file contents with offset/limit     |
| write   | Write content to file                    |
| edit    | Search and replace in files              |
| grep    | Regex search with context lines          |
| glob    | Pattern-based file finding               |
| python  | Execute Python code in virtual env       |

### Execution Flow

```
User Message → Tauri Command → AgentExecutor
                                      ↓
                               Load Conversation History
                                      ↓
                               Build LLM Request (with tools)
                                      ↓
                               Call LLM API
                                      ↓
                         ┌─────────────┴─────────────┐
                         ↓                           ↓
                    Tool Calls?                  No Tools
                         ↓                           ↓
                    Execute Tools              Return Response
                         ↓                           ↓
                    Get Results                  Save to DB
                         ↓                           ↓
                    Loop with Results            Emit Events
```

## Development Guidelines

### Adding a New Tool

1. Implement the `Tool` trait in `src-tauri/src/domains/agent_runtime/tools.rs`:

```rust
pub struct MyTool;

#[async_trait]
impl Tool for MyTool {
    fn name(&self) -> &str {
        "my_tool"
    }

    fn description(&self) -> &str {
        "Description of what my_tool does"
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "param1": {
                    "type": "string",
                    "description": "Parameter description"
                }
            },
            "required": ["param1"]
        }))
    }

    async fn execute(&self, _ctx: Arc<ToolContext>, args: Value) -> ToolResult<Value> {
        // Tool implementation
        Ok(json!({"result": "success"}))
    }
}
```

2. Register the tool in `builtin_tools()`:

```rust
pub fn builtin_tools() -> Vec<Arc<dyn Tool>> {
    vec![
        // ... existing tools
        Arc::new(MyTool::new()),
    ]
}
```

### Adding a New Feature

1. Create a new folder in `src/features/your-feature/`
2. Add types to `src/shared/types/index.ts`
3. Create Tauri commands in `src-tauri/src/commands/your-feature.rs`
4. Register commands in `src-tauri/src/lib.rs`
5. Create a service in `src/services/your-feature.ts`
6. Add route to `src/shared/constants/routes.ts`

### Code Style

- Use TypeScript strict mode
- Organize code by domain, not by layer
- Keep components small and focused
- Document complex logic with comments

## Resources

- **Context7 Docs:** Use `mcp__context7__query-docs` for latest library documentation
- **Figma Design:** Use `mcp__figma-remote-mcp__*` tools for design work

## Contributing

When making changes:
1. Keep features modular and independent
2. Test with `npm run tauri dev` before building
3. Document new Tauri commands
4. Run `cargo check` in `src-tauri` to verify Rust code
