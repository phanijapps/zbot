# Agent Zero - Product Definition

## Product Vision

Agent Zero is a desktop application for creating and managing AI agents, similar to Claude Desktop. It enables users to build specialized AI assistants with custom instructions, integrate with multiple LLM providers, extend capabilities through skills, connect to external tools via MCP (Model Context Protocol) servers, and create multi-agent orchestrators with visual workflow editing.

## Target Users

- **Developers**: Building AI-powered workflows and automation
- **Power Users**: Creating specialized assistants for specific tasks
- **Teams**: Managing multiple AI agents with different capabilities

## Core Features

### 1. Agent Management
**Location:** `src/features/agents/`

Create AI agents with custom instructions, configure providers, models, temperature, and max tokens. Full IDE-style editor for agent development.

- **Agent IDE**: Configure agents with provider, model, temperature, max tokens
- **Instructions Editor**: Markdown-based AGENTS.md editor with live preview
- **File Explorer**: Create, edit, delete files within agent folders
- **Quick Actions**: Create, edit, duplicate, delete agents from panel

**Backend Commands:**
- `list_agents()` - List all agents
- `get_agent(id)` - Get single agent
- `create_agent(agent)` - Create new agent
- `update_agent(id, agent)` - Update agent
- `delete_agent(id)` - Delete agent

### 2. Zero IDE (Workflow IDE)
**Location:** `src/features/workflow-ide/`

Visual workflow builder for creating orchestrator agents with BPMN-inspired design. Build multi-agent workflows by dragging nodes onto an infinite canvas.

**Key Features:**
- **BPMN-style nodes**: Start (thin circle), End (thick circle), Subagent (card)
- **Top-down flow**: Handles positioned for vertical connections
- **Orchestrator config**: Flow-level LLM configuration (provider, model, tools, MCPs, skills)
- **Properties panel**: Dynamic configuration based on selected node/edge
- **Template system**: Pre-built workflow patterns (Pipeline, Swarm, Router, Map-Reduce, Hierarchical)
- **Undo/Redo**: Full history management with 50-state limit
- **New agent dialog**: Create agents with template selection
- **Migration**: Automatic migration of legacy orchestrator nodes to flow-level config

**Node Types:**

| Node | Visual | Purpose | Status |
|------|--------|---------|--------|
| **Start** | Green thin circle (BPMN event) | Workflow entry point | ✅ Complete |
| **End** | Red thick circle (BPMN event) | Workflow exit point | ✅ Complete |
| **Subagent** | Purple rounded rectangle | Worker agent | ✅ Complete |
| **Conditional** | Amber diamond | Branching logic | 🚧 Draft |

**Templates:**
- **Pipeline**: Sequential execution (A → B → C)
- **Swarm**: Parallel execution across specialized subagents
- **Router**: Conditional routing based on input
- **Map-Reduce**: Parallel processing with aggregation
- **Hierarchical**: Multi-level delegation with team structure

**Access:** Click the Git branch icon in Agent Channel header or navigate to `/workflow/:agentId`

**Backend Integration:**
- `get_orchestrator_structure(agentId)` - Load workflow graph
- `save_orchestrator_structure(agentId, graph)` - Save workflow graph
- `validate_workflow(graph)` - Validate workflow structure

### 3. Agent Channels
**Location:** `src/features/agent-channels/`

Discord-like interface for daily agent conversations with knowledge graph integration.

**Key Features:**
- **Daily Sessions**: Conversations organized by date with expandable day separators
- **Knowledge Graph**: Semantic memory for entities and relationships
- **Voice Recording**: Record voice inputs with automatic transcription
- **Attachments Panel**: Manage transcript attachments
- **History Management**: Browse past sessions with full context
- **Vault Switching**: Switch vaults with proper state reset

**Storage:** SQLite database at `{vault_path}/db/agent_channels.db`

**Backend Commands:**
- `get_or_create_today_session(agentId)` - Get or create today's session
- `list_previous_days(agentId, limit)` - List previous days with sessions
- `load_session_messages(sessionId)` - Load messages for a session
- `record_session_message(...)` - Record a message to session

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

**Backend Commands:**
- `list_mcp_servers()` - List all MCP servers
- `create_mcp_server(server)` - Create MCP server
- `update_mcp_server(id, server)` - Update MCP server
- `delete_mcp_server(id)` - Delete MCP server
- `test_mcp_server(server)` - Test MCP server connection

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

**Storage:** `~/.config/zeroagent/skills/{skill-name}/`

### 7. Vault Management
**Location:** `src/features/vaults/`

Manage multiple vaults for data organization.

**Backend Commands:**
- `list_vaults()` - List all vaults
- `create_vault(vault)` - Create vault
- `update_vault(id, vault)` - Update vault
- `delete_vault(id)` - Delete vault
- `set_default_vault(id)` - Set default vault

## Technical Stack

| Layer | Technology |
|-------|-----------|
| Desktop Framework | Tauri 2.x |
| Frontend | React 19 + TypeScript |
| Workflow Canvas | XY Flow (React Flow v12+) |
| State Management | Zustand |
| Styling | Tailwind CSS v4 + Radix UI |
| Backend | Rust (Cargo workspace) |
| Build | Vite |
| Database | SQLite |

## Storage Locations

| Data | Location |
|------|----------|
| Agents | `~/.config/zeroagent/agents/` |
| Skills | `~/.config/zeroagent/skills/` |
| Providers | `~/.config/zeroagent/providers.json` |
| MCP Servers | `~/.config/zeroagent/mcp_servers/` |
| Agent Channels | `{vault_path}/db/agent_channels.db` |
| Workflow Layout | `~/.config/zeroagent/agents/{agent-name}/.workflow-layout.json` |

## Key Differentiators

1. **Local-first**: All data stored locally, full control
2. **Extensible**: Skills and MCP servers for customization
3. **IDE-like**: Full file management for agents and skills
4. **Multi-provider**: Not locked into single LLM provider
5. **Open Standards**: Uses Agent Skills and MCP specifications
6. **Visual Workflow**: Zero IDE for orchestrator agents with BPMN-inspired design
7. **Multi-Agent**: Dynamic subagent system for agent orchestration
8. **Long-term Memory**: Knowledge graph for semantic memory across sessions
9. **Template System**: Pre-built workflow patterns for common use cases

## User Journey

1. **Setup Providers**: Add LLM providers (OpenAI, Anthropic, DeepSeek, Z.AI, etc.)
2. **Create Skills** (optional): Import or create skills for specific capabilities
3. **Configure MCP Servers** (optional): Add MCP servers for external tool access
4. **Create Agents**:
   - **Simple agents**: Configure with instructions, skills, MCPs
   - **Orchestrator agents**: Use Zero IDE to design workflows with subagents
5. **Run Conversations**: Chat with agents via Agent Channels

## Orchestrator Agent Pattern

Agents can be designed as **orchestrators** that coordinate subagents:

### Architecture

Each workflow flow represents a **single Orchestrator Agent** with:
- Flow-level LLM configuration (provider, model, tools, MCPs, skills, middleware, system prompt)
- Subagents available as tools that the orchestrator can delegate to
- Visual workflow defining the conversation flow

### Storage

```
~/.config/zeroagent/agents/my-orchestrator/
├── config.yaml              # Agent metadata
├── AGENTS.md                # Orchestrator instructions
├── .workflow-layout.json    # Visual workflow layout (XY Flow positions)
└── .subagents/              # Subagent definitions (generated from workflow)
    ├── subagent-1/
    │   ├── config.yaml
    │   └── AGENTS.md
    └── subagent-2/
        ├── config.yaml
        └── AGENTS.md
```

### How It Works

1. User designs workflow in Zero IDE with Start, End, and Subagent nodes
2. Orchestrator config is set at flow level (provider, model, system prompt, etc.)
3. Subagents are generated as standalone agents in `.subagents/` folder
4. Orchestrator's LLM can call subagents with context/task/goal parameters
5. Bidirectional isolation: Orchestrator gets final results, subagents get injected context

### Example: Chef Bot

- **Orchestrator**: chef-bot (z.ai/glm-4.6)
  - Coordinates the cooking workflow
  - Routes to appropriate subagent based on user request
- **Subagents**:
  - inventory-checker (deepseek/deepseek-chat) - Validates ingredients
  - recipe-finder (deepseek/deepseek-chat) - Finds matching recipes
  - substituter (z.ai/glm-4.6) - Suggests ingredient substitutions
  - instruction-formatter (z.ai/glm-4.7) - Formats cooking instructions

## Related Documentation

| File | Description |
|------|-------------|
| `AGENTS.md` | Project overview and quick start |
| `memory-bank/architecture.md` | Technical architecture |
| `memory-bank/known_issues.md` | Known issues and TODOs |
| `memory-bank/learnings.md` | Architecture learnings |
