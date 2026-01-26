# Agent Zero

A desktop application for creating and managing AI agents with visual workflow orchestration, multi-provider support, and extensible capabilities.

## Features

- **Agent Management** - Create AI agents with custom instructions, provider/model selection, and capability configuration
- **Visual Workflow Builder** - BPMN-inspired workflow editor for multi-agent orchestration with real-time execution visualization
- **Agent Channels** - Discord-like interface for daily conversations with knowledge graph memory
- **Multi-Provider** - Support for OpenAI, Anthropic, DeepSeek, Z.AI, and any OpenAI-compatible API
- **MCP Integration** - Connect to external tools via Model Context Protocol servers
- **Skill System** - Create reusable skills with frontmatter metadata and markdown instructions
- **Multi-Vault** - Organize data across isolated vaults with full portability

## Quick Start

### Prerequisites

**Windows:**
- [WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)
- [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)

**macOS:** No additional dependencies.

**Linux (Ubuntu/Debian):**
```bash
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
                 libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

See [Tauri Prerequisites](https://tauri.app/guides/prerequisites/) for complete details.

### Installation

```bash
# Install dependencies
npm install

# Run in development mode
npm run tauri dev
```

### Build

```bash
npm run tauri build
```

Output: `src-tauri/target/release/bundle/`

## Technology Stack

| Layer | Technology |
|-------|------------|
| Desktop | Tauri 2.x |
| Frontend | React 19 + TypeScript + Vite |
| UI | Radix UI + Tailwind CSS v4 |
| Workflow | XY Flow (React Flow v12+) |
| State | Zustand |
| Backend | Rust (Cargo workspace) |
| Database | SQLite |

## Project Structure

```
agentzero/
├── src/                       # Frontend (React + TypeScript)
│   ├── features/              # Feature modules (workflow-ide, agent-channels, etc.)
│   ├── shared/                # UI components, types
│   └── services/              # Tauri IPC wrappers
├── crates/                    # Zero Framework (reusable Rust crates)
│   ├── zero-core/             # Core traits: Agent, Tool, Session
│   ├── zero-llm/              # LLM abstractions
│   ├── zero-agent/            # Agent implementations
│   └── zero-mcp/              # MCP protocol
├── application/               # Application-specific crates
│   ├── agent-runtime/         # Agent executor
│   ├── agent-tools/           # Built-in tools
│   └── workflow-executor/     # Workflow execution
├── src-tauri/                 # Tauri application
└── memory-bank/               # Architecture documentation
```

## Documentation

| Document | Description |
|----------|-------------|
| `memory-bank/product.md` | Product definition and features |
| `memory-bank/architecture.md` | Technical architecture |
| `memory-bank/technical_map.md` | Key modules, decisions, fixes |

## Development

```bash
# Type check frontend
npx tsc --noEmit

# Check Rust code
cd src-tauri && cargo check

# Run tests
cargo test --workspace
```

## License

MIT
