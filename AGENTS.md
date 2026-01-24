# Agent Zero
Agent Zero is an Agent UI similar to Claude Desktop. The difference being, it can be used to connect to any OpenAI based APIs and be used to build agents, skills and connect to tools for daily use.

## Recent Updates (January 2025)

### Visual Workflow Builder
- **Figma-Inspired Flow Editor**: Create multi-agent workflows visually by dragging nodes onto an infinite canvas
- **Chat Integration**: Click the Flow icon (git branch icon) in any agent chat to open the Visual Workflow Builder modal
- **Real-time Validation**: Get immediate feedback on workflow configuration errors and warnings
- **Flow Config Persistence**: Workflows are saved as `flow.json` in each agent's directory
- **Eight Node Types**: Agent, Trigger, Parallel, Sequential, Conditional, Loop, Aggregator, and Subtask nodes

### Agent Creator
- **Simplified Architecture**: Agent-creator now works as a regular agent using the standard workflow
- No special execution path - uses existing `execute_agent_stream` command
- Template files located in `src-tauri/templates/default-agents/agent-creator/`
- Users chat directly with agent-creator to create new agents conversationally

### System Prompt Improvements
- **Write vs Edit Guidance**: Agents now intelligently choose between write (small content) and edit (large content) tools
- **Error Handling**: Agents adapt their strategy when tools fail (e.g., switch to edit chunks on token limit errors)
- **Lazy Skill Loading**: Only skill name/description injected initially; full content loaded on-demand via `load_skill` tool
- **Conversation History Fix**: Day's conversation history now loads correctly when agent is selected

### Bug Fixes
- Resolved `__awaiting_input__` error by removing special AwaitingInput handling
- Fixed conversation loading bug by adding missing useEffect dependency

## Technology Stack
Read "Technology Stack" section in `memory-bank/architecture.md`

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

## Visual Workflow Builder

The Visual Workflow Builder is a Figma-inspired visual editor for creating multi-agent workflows. Design complex agent systems by dragging nodes onto an infinite canvas and connecting them to define sequential, parallel, conditional, and loop workflows.

### Accessing the Workflow Builder

1. From the Chat panel, select any agent
2. Click the Flow icon (git branch icon) in the header
3. The Visual Workflow Builder opens as a full-screen modal

### Layout

```
┌─────────────────────────────────────────────────────────────────┐
│ Agent Zero - Workflow Builder                      [Save] [●]      │
├─────────────────────────────────────────────────────────────────┤
│ ┌──────┐ ┌──────────────────────────────────────────────────┐ │
│ │Assets│ │  Visual Canvas (Infinite)                        │ │
│ │      │ │  ┌─────┐                                         │ │
│ │Nodes │ │  │Agent│ → connections → nodes                │ │
│ │      │ │  └─────┘                                         │ │
│ └──────┘ └──────────────────────────────────────────────────┘ │
│                    ┌──────────┐                                │
│                    │Properties│                                │
│                    └──────────┘                                │
└─────────────────────────────────────────────────────────────────┘
```

- **Left Sidebar (Assets Panel)**: Drag node types onto the canvas
- **Center (Canvas)**: Infinite pan/zoom canvas with grid background
- **Right Panel (Properties)**: Configure selected node, view YAML, see validation

### Node Types

| Node | Description | Use Case |
|------|-------------|----------|
| **Agent** | Main AI agent with LLM, tools, MCPs, skills | Core building block |
| **Trigger** | Entry point for workflow (manual/scheduled) | Define workflow start |
| **Parallel** | Execute multiple branches concurrently | Fan-out to multiple agents |
| **Sequential** | Chain agents in order | Pipeline processing |
| **Conditional** | Route based on conditions | Branching logic |
| **Loop** | Repeat until condition met | Iterative refinement |
| **Aggregator** | Merge multiple results | Combine parallel outputs |
| **Subtask** | Task definition for parallel execution | Delegate work |

### Keyboard Shortcuts

- **Space + Drag**: Pan canvas
- **Ctrl + Scroll**: Zoom in/out
- **Delete**: Remove selected node
- **Escape**: Deselect node

### Connection Rules

- Connect output ports (right side) to input ports (left side)
- Connections render as smooth bezier curves
- Right-click a connection to delete it
- Connections are validated for compatibility

### Auto-Save

- Changes are auto-saved 1 second after modification
- Save status shows in top-right: "All changes saved" / "Saving..." / "Unsaved changes..."
- Click the Save button for immediate save

### Backend Commands

The Visual Workflow Builder uses these Tauri commands:

- **`get_agent_flow_config(agent_id)`**: Returns the flow.json content or null
- **`save_agent_flow_config(agent_id, config)`**: Saves flow configuration as JSON

### File Storage

Workflows are stored in each agent's directory as `flow.json`:

```
~/.config/agentzero/
├── agents/
│   ├── my-agent/
│   │   ├── config.yaml      # Agent configuration
│   │   ├── AGENTS.md        # Agent instructions
│   │   └── flow.json        # Visual workflow (NEW)
```

### Properties Panel

The Properties Panel shows configuration for the selected node:

- **Properties Tab**: Configure node-specific settings
- **YAML Tab**: View/export as YAML
- **Validation Section**: Real-time error/warning display

### Validation

The builder validates in real-time and shows:
- **Errors** (red): Critical issues (missing display name, no provider)
- **Warnings** (yellow): Recommended actions (no tools selected)
- **Info** (blue): Helpful hints

## Project Structure

Read "Workspace Structure" section in `memory-bank/architecture.md`


## Agent Runtime Architecture
Read  `memory-bank/architecture.md` for clarity on architecture.

## Resources

- **Context7 Docs:** Use `mcp__context7__query-docs` for latest library documentation
- **Figma Design:** Use `mcp__figma-remote-mcp__*` tools for design work
-  `memory-bank/learnings.md` - This has information on how it resolved issues in the past. Use it when needed.
- `.ref/adk-rust` - is an opensource AI Agent Framework written in rust. crates in zero-* are partly inspired from it and langchain. Refer to them when needed but don't copy any code as that is an !!UNSTABLE!! project and cannot be used in our usecase.

## Contributing

When making changes:
1. Keep features modular and independent
2. Test with `npm run tauri dev` before building
3. Document new Tauri commands
4. Run `cargo check` in `src-tauri` to verify Rust code
