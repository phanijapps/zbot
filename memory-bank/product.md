# Agent Zero - Product Definition

## Vision

Agent Zero is a desktop application for creating and managing AI agents. Build specialized AI assistants with custom instructions, integrate with multiple LLM providers, extend capabilities through skills and MCP servers, and create multi-agent workflows with visual editing.

## Target Users

- **Developers**: Building AI-powered workflows and automation
- **Power Users**: Creating specialized assistants for specific tasks
- **Teams**: Managing multiple AI agents with different capabilities

## Core Features

### 1. Agent Management
Create AI agents with custom instructions, provider/model selection, and capability configuration.

### 2. Zero IDE (Workflow Builder)
Visual workflow builder for multi-agent orchestration with BPMN-inspired design:
- **Nodes**: Start, End, Subagent, Conditional (draft)
- **Templates**: Pipeline, Swarm, Router, Map-Reduce, Hierarchical
- **Execution**: Real-time streaming with visual node status

### 3. Agent Channels
Discord-like interface for daily conversations with knowledge graph memory.

### 4. Provider Management
Multi-provider support: OpenAI, Anthropic, DeepSeek, Z.AI, any OpenAI-compatible API.

### 5. MCP Server Integration
Model Context Protocol servers for external tool access.

### 6. Skill System
Reusable skills with frontmatter metadata and markdown instructions.

### 7. Vault Management
Multi-vault architecture for data organization.

## Technology Stack

| Layer | Technology |
|-------|-----------|
| Desktop | Tauri 2.x |
| Frontend | React 19 + TypeScript |
| Workflow | XY Flow (React Flow v12+) |
| State | Zustand |
| Styling | Tailwind CSS v4 + Radix UI |
| Backend | Rust (Cargo workspace) |
| Database | SQLite |

## Storage

**Global Config**: `~/.config/agentzero/` (vaults registry, shared utils)

**Vault Structure**:
```
{vault}/
├── agents/{name}/          # Agent configs
│   ├── config.yaml
│   ├── AGENTS.md
│   ├── .workflow-layout.json
│   └── .subagents/
├── skills/                 # Skill definitions
├── agent_data/             # Runtime data
├── db/                     # SQLite databases
├── providers.json
└── mcps.json
```

## Key Differentiators

1. **Local-first**: Full data control
2. **Multi-provider**: Not locked to single LLM
3. **Visual Workflow**: BPMN-inspired orchestration
4. **Extensible**: Skills + MCP servers
5. **Open Standards**: Agent Skills and MCP specifications
