# Agent Zero - Product Definition

## Product Vision

Agent Zero is a desktop application for creating and managing AI agents, similar to Claude Desktop. It enables users to build specialized AI assistants with custom instructions, integrate with multiple LLM providers, extend capabilities through skills, and connect to external tools via MCP (Model Context Protocol) servers.

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

### 2. Skill System
- Create reusable skills following Agent Skills specification
- Skills have frontmatter metadata + markdown instructions
- Support for asset folders (assets/, resources/, scripts/)
- Categories: utility, coding, writing, analysis, communication, productivity, research, creative, automation

### 3. Provider Management
- Configure OpenAI-compatible API providers
- Support for multiple providers (OpenAI, Anthropic, local models)
- Per-agent provider and model selection

### 4. MCP Server Integration
- Add Model Context Protocol servers for external tools
- Servers run as stdio processes
- Test server connectivity
- Associate multiple MCP servers with agents

### 5. File Explorer (IDE Features)
- Hierarchical file tree for agents/skills
- Create, edit, delete files and folders
- Import files into agent/skill folders
- Markdown editor for .md files with live preview
- Auto-save with debouncing

## Technical Stack

| Layer | Technology |
|-------|-----------|
| Desktop Framework | Tauri 2.9 |
| Frontend | React 19 + TypeScript |
| Styling | Tailwind CSS v4 + Radix UI |
| Backend | Rust |
| Build | Vite |

## Storage Locations

- **Agents**: `~/.config/zeroagent/agents/`
- **Skills**: `~/.config/zeroagent/skills/`
- **Providers**: `~/.config/zeroagent/providers.json`
- **MCP Servers**: `~/.config/zeroagent/mcps.json`

## Key Differentiators

1. **Local-first**: All data stored locally, full control
2. **Extensible**: Skills and MCP servers for customization
3. **IDE-like**: Full file management for agents and skills
4. **Multi-provider**: Not locked into single LLM provider
5. **Open Standards**: Uses Agent Skills and MCP specifications

## User Journey

1. Add providers (OpenAI, Anthropic, etc.)
2. Create or import skills for specific capabilities
3. Configure MCP servers for external tool access
4. Create agents with instructions, associate skills/MCPs
5. Run conversations with agents

## Future Roadmap

### High Priority

**1. Modular Domain Architecture**
- Domain-driven design with clear separation
- Domain 1: Agent Runtime (LangChain.js, tools, MCP)
- Domain 2: Conversation Runtime (SQLite, memory, execution)
- UI layer unchanged - integrates via service layer

**2. LangChain.js Integration**
- LangChain 1.2.6 with `createAgent`
- LangGraph 1.0+ for complex workflows
- Integration as plug-and-play library in `src/domains/agent-runtime/langchain/`

**3. Basic Tools Implementation**
- Read Tool - Read file contents with offset/limit
- Write Tool - Write files with directory creation
- Grep Tool - Regex search with context
- Glob Tool - Find files by pattern
- Bash Tool - Cross-platform shell execution
  - Linux/macOS: bash
  - Windows: PowerShell
  - WSL fallback for Windows
- Python Tool - Execute Python code using venv in config location

**4. Conversation Management**
- SQLite at `~/.config/zeroagent/conversations.db`
- Conversation list per agent
- Message persistence with indexing
- CRUD operations for conversations

**5. Multi-turn Conversations**
- Memory management (buffer window, summarization)
- LangGraph StateGraph for agent execution
- Streaming responses
- Tool call handling

### Low Priority
- Agent composition (agents using other agents)
- Import/Export via zip files
- Skill marketplace
