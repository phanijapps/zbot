# Agent Zero

Agent Zero is a desktop application for creating and managing AI agents, similar to Claude Desktop. It enables users to build specialized AI assistants with custom instructions, integrate with multiple LLM providers, extend capabilities through skills, connect to external tools via MCP (Model Context Protocol) servers, and create multi-agent orchestrators with visual workflow editing.

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

## Technology Stack

Read "Technology Stack" section in `memory-bank/architecture.md`

## Project Structure

Read "Workspace Structure" section in `memory-bank/architecture.md`

## Architecture

Read `memory-bank/architecture.md` for detailed technical architecture.

## Core Features

### 1. Agent Management

**Location:** `src/features/agents/`

Create AI agents with custom instructions, configure providers, models, temperature, and max tokens. Full IDE-style editor for agent development.

**Key Files:**
- `AgentIDEPage.tsx` - Agent configuration interface
- `AgentIDEDialog.tsx` - YAML config editor
- `AgentsPanel.tsx` - Agent list panel

**Backend Commands:**
- `list_agents()` - List all agents
- `get_agent(id)` - Get single agent
- `create_agent(agent)` - Create new agent
- `update_agent(id, agent)` - Update agent
- `delete_agent(id)` - Delete agent

### 2. Workflow IDE (Zero IDE)

**Location:** `src/features/workflow-ide/`

Visual workflow builder for creating orchestrator agents with BPMN-inspired design. Build multi-agent workflows by dragging nodes onto an infinite canvas.

**Key Features:**
- **BPMN-style nodes**: Start (thin circle), End (thick circle), Subagent (card)
- **Top-down flow**: Handles positioned for vertical connections
- **Orchestrator config**: Flow-level LLM configuration (provider, model, tools, MCPs, skills)
- **Properties panel**: Dynamic configuration based on selected node/edge
- **Template system**: Pre-built workflow patterns (Pipeline, Swarm, Router, Map-Reduce, Hierarchical)
- **Undo/Redo**: Full history management with 50-state limit
- **Migration**: Automatic migration of legacy orchestrator nodes to flow-level config

**Node Types:**

| Node | Visual | Purpose | Status |
|------|--------|---------|--------|
| **Start** | Green thin circle (BPMN event) | Workflow entry point | ✅ Complete |
| **End** | Red thick circle (BPMN event) | Workflow exit point | ✅ Complete |
| **Subagent** | Purple rounded rectangle | Worker agent | ✅ Complete |
| **Conditional** | Amber diamond | Branching logic | 🚧 Draft |
| **Orchestrator** | Amber card | Legacy (migrated to flow-level) | ⚠️ Legacy only |

**Key Files:**
- `WorkflowIDEPage.tsx` - Main page with migration logic
- `WorkflowEditor.tsx` - Canvas with XY Flow
- `components/nodes/` - Custom node components
- `components/panels/NodePalette.tsx` - Draggable node library
- `components/panels/PropertiesPanel.tsx` - Configuration panel
- `stores/workflowStore.ts` - Zustand state management
- `stores/workflowHistoryStore.ts` - Undo/redo history
- `types/workflow.ts` - Type definitions
- `types/templates.ts` - Workflow templates

**Access:** Click the Git branch icon in Agent Channel header or navigate to `/workflow/:agentId`

### 3. Agent Channels

**Location:** `src/features/agent-channels/`

Discord-like interface for daily agent conversations with knowledge graph integration.

**Key Features:**
- **Daily Sessions**: Conversations organized by date with expandable day separators
- **Knowledge Graph**: Semantic memory for entities and relationships
- **Voice Recording**: Record voice inputs with automatic transcription
- **Attachments Panel**: Manage transcript attachments
- **History Management**: Browse past sessions with full context
- **Expandable Days**: Click to load historical sessions

**Key Files:**
- `AgentChannelPanel.tsx` - Main interface (1,446 lines)
- `DaySeparator.tsx` - Collapsible day headers
- `ClearHistoryDialog.tsx` - History clearing (Chrome-style)
- `VoiceRecordingDialog.tsx` - Audio recording modal
- `AttachmentsPanel.tsx` - Attachment management
- `KnowledgeGraphVisualizer.tsx` - Graph visualization

**Storage:** SQLite database at `{vault_path}/db/agent_channels.db`

### 4. Provider Management

**Location:** `src/features/providers/`

Configure OpenAI-compatible API providers. Support for multiple providers with per-agent selection.

**Supported Providers:**
- OpenAI
- Anthropic
- DeepSeek
- Z.AI
- Any OpenAI-compatible API

**Backend Commands:**
- `list_providers()` - List all providers
- `create_provider(provider)` - Create provider
- `update_provider(id, provider)` - Update provider
- `delete_provider(id)` - Delete provider
- `test_provider(provider)` - Test API connection

### 5. MCP Server Integration

**Location:** `src/features/mcp/`

Add Model Context Protocol servers for external tools. Servers run as stdio processes or HTTP/SSE.

**Configuration:** Stored in `~/.config/zeroagent/mcp_servers/{server-id}.json`

### 6. Skill System

**Location:** `src/features/skills/`

Create reusable skills following Agent Skills specification. Skills have frontmatter metadata + markdown instructions.

**Skill Categories:**
- utility
- coding
- writing
- analysis
- communication
- productivity
- research
- creative
- automation

**Skill Structure:**
```markdown
---
name: Search
description: Search the web
parameters: [...]
---
# Skill instructions...
```

### 7. File Explorer (IDE Features)

**Location:** `src/features/agents/` (IDE components)

Hierarchical file tree for agents/skills with create, edit, delete operations.

**Features:**
- File tree with folders and files
- Markdown editor with live preview
- Auto-save with debouncing
- Import files into agent/skill folders

## Resources

- **Context7 Docs:** Use `mcp__context7__query-docs` for latest library documentation
- **Figma Design:** Use `mcp__figma-remote-mcp__*` tools for design work
- `memory-bank/learnings.md` - Architecture learnings and past issue resolutions
- `memory-bank/architecture.md` - Technical architecture documentation
- `memory-bank/product.md` - Product definition and features
- `memory-bank/known_issues.md` - Known issues and TODOs

## Contributing

When making changes:
1. Keep features modular and independent
2. Test with `npm run tauri dev` before building
3. Document new Tauri commands
4. Run `cargo check` in `src-tauri` to verify Rust code
5. Run `npx tsc --noEmit` to verify TypeScript
