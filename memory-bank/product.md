# Agent Zero - Product Definition

## Product Vision

Agent Zero is a desktop application for creating and managing AI agents, similar to Claude Desktop. It enables users to build specialized AI assistants with custom instructions, integrate with multiple LLM providers, extend capabilities through skills, connect to external tools via MCP (Model Context Protocol) servers, and create multi-agent orchestrators with visual workflow editing.

## Target Users

- **Developers**: Building AI-powered workflows and automation
- **Power Users**: Creating specialized assistants for specific tasks
- **Teams**: Managing multiple AI agents with different capabilities

## Core Features

### 1. Agent Management
- Create AI agents with custom instructions (AGENTS.md)
- Configure provider, model, temperature, max tokens
- Associate MCP servers and skills with agents
- Full IDE-style editor for agent development
- **Zero IDE**: Visual workflow editor for orchestrator agents
- **Subagent System**: Create subagents that orchestrators can delegate to

### 2. Skill System
- Create reusable skills following Agent Skills specification
- Skills have frontmatter metadata + markdown instructions
- Support for asset folders (assets/, resources/, scripts/)
- Categories: utility, coding, writing, analysis, communication, productivity, research, creative, automation

### 3. Provider Management
- Configure OpenAI-compatible API providers
- Support for multiple providers (OpenAI, Anthropic, local models, DeepSeek, Z.AI)
- Per-agent provider and model selection

### 4. MCP Server Integration
- Add Model Context Protocol servers for external tools
- Servers run as stdio processes or HTTP/SSE
- Test server connectivity
- Associate multiple MCP servers with agents

### 5. Agent Channels (New)
- **Daily Sessions**: Conversations organized by date with automatic summaries
- **Knowledge Graph**: Semantic memory for entities and relationships
- **Expandable History**: Browse past sessions with full context
- **Voice Recording**: Record voice inputs for agents

### 6. File Explorer (IDE Features)
- Hierarchical file tree for agents/skills
- Create, edit, delete files and folders
- Import files into agent/skill folders
- Markdown editor for .md files with live preview
- Auto-save with debouncing

### 7. Middleware System
- **Summarization**: Automatically compress long conversations to fit context window
- **Context Editing**: Clear old tool results to free up tokens
- Configurable triggers and keep parameters
- Custom summary prompts for domain-specific needs

## Technical Stack

| Layer | Technology |
|-------|-----------|
| Desktop Framework | Tauri 2.x |
| Frontend | React 19 + TypeScript |
| Styling | Tailwind CSS v4 + Radix UI |
| Backend | Rust (Cargo workspace) |
| Build | Vite |

## Storage Locations

- **Agents**: `~/.config/zeroagent/agents/`
- **Skills**: `~/.config/zeroagent/skills/`
- **Providers**: `~/.config/zeroagent/providers.json`
- **MCP Servers**: `~/.config/zeroagent/mcps.json`
- **Conversations**: `~/.config/zeroagent/conversations.db`
- **Agent Channels**: `{vault_path}/db/agent_channels.db`

## Key Differentiators

1. **Local-first**: All data stored locally, full control
2. **Extensible**: Skills and MCP servers for customization
3. **IDE-like**: Full file management for agents and skills
4. **Multi-provider**: Not locked into single LLM provider
5. **Open Standards**: Uses Agent Skills and MCP specifications
6. **Visual Workflow**: Zero IDE for orchestrator agents with BPMN-inspired design
7. **Multi-Agent**: Dynamic subagent tool system for agent orchestration
8. **Long-term Memory**: Knowledge graph for semantic memory across sessions

## User Journey

1. Add providers (OpenAI, Anthropic, DeepSeek, Z.AI, etc.)
2. Create or import skills for specific capabilities
3. Configure MCP servers for external tool access
4. Create agents:
   - Simple agents: Configure with instructions, skills, MCPs
   - Orchestrator agents: Use Zero IDE to design workflows with subagents
5. Run conversations with agents

## Orchestrator Agent Pattern

Agents can be designed as **orchestrators** that coordinate subagents:

### File Structure
```
~/.config/zeroagent/agents/my-orchestrator/
├── config.yaml       # Orchestrator LLM config
├── AGENTS.md         # Orchestrator instructions
├── flow.json         # Visual workflow definition (optional)
└── .subagents/       # Subagent definitions
    ├── subagent-1/
    │   ├── config.yaml
    │   └── AGENTS.md
    └── subagent-2/
        ├── config.yaml
        └── AGENTS.md
```

### How It Works
1. Orchestrator agent is created with subagents in `.subagents/` folder
2. Each subagent is automatically registered as a callable tool
3. Orchestrator's LLM can call subagents with context/task/goal parameters
4. Bidirectional isolation: Orchestrator only gets final results, subagents only get injected context

### Example: Chef Bot
- **Orchestrator**: chef-bot (z.ai/glm-4.6)
- **Subagents**:
  - inventory-checker (deepseek/deepseek-chat) - Validates ingredients
  - recipe-finder (deepseek/deepseek-chat) - Finds matching recipes
  - substituter (z.ai/glm-4.6) - Suggests ingredient substitutions
  - instruction-formatter (z.ai/glm-4.7) - Formats cooking instructions

## Future Roadmap

### High Priority
- React Flow integration for Zero IDE (professional workflow editor)
- Automatic entity extraction for Knowledge Graph
- Graph visualization UI for knowledge graph browsing

### Medium Priority
- Agent composition (agents using other agents)
- Import/Export via zip files
- Skill marketplace
- Enhanced episodic memory with vector search

### Low Priority
- Session persistence across app restarts
- Voice output for agents
- Multi-language support
