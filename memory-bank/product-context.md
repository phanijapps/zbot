# Agent Zero — Product Context

## Why Agent Zero Exists

Agent Zero is a local-first AI agent platform that puts you in control. It's a creative hub where agents self-improve through persistent code, shared environments, and cross-session learning.

Most AI platforms lock you into a cloud provider, take your data off-machine, and give you no way to extend the system. Agent Zero exists to solve this: run sophisticated AI assistants on your own machine with full data ownership, multi-provider flexibility, and unlimited extensibility.

## Problems It Solves

1. **Data sovereignty** — Cloud AI platforms store your conversations, code, and context on their servers. Agent Zero keeps everything on your machine in plain files and SQLite.

2. **No unified local platform** — Running local AI today means cobbling together CLI tools, model servers, and shell scripts. Agent Zero provides a single daemon with web UI, CLI, sessions, tools, skills, and multi-agent delegation.

3. **No persistent project context** — Session-based AI assistants start from scratch every time. Agent Zero's Code Wards give agents persistent project directories they create, name, and navigate autonomously. Code survives across sessions.

4. **No cross-session learning** — Most AI assistants forget everything between sessions. Agent Zero automatically distills session transcripts into structured facts, then recalls relevant facts at the start of new sessions via hybrid semantic + keyword search. The agent learns without manual effort.

5. **Provider lock-in** — Most platforms work with one LLM provider. Agent Zero is provider-agnostic: OpenAI, Anthropic, DeepSeek, Groq, Ollama, or any OpenAI-compatible API.

6. **Limited extensibility** — Agent Zero supports Skills (reusable instruction packages) and MCP servers (external tool integration) as first-class concepts.

## Core Principles

1. **Local-First** — Your data stays on your machine. No cloud lock-in.
2. **Provider-Agnostic** — Use any LLM: OpenAI, Anthropic, DeepSeek, Ollama, self-hosted.
3. **Extensible** — Skills and MCP servers let you add any capability.
4. **Open Standards** — Built on Agent Skills and Model Context Protocol specifications.
5. **Simple Deployment** — Single daemon binary + static web files.

## Target Users

| User | Use Case |
|------|----------|
| **Developers** | Building AI-powered workflows, automation, code assistants |
| **Power Users** | Creating specialized assistants for research, writing, analysis |
| **Teams** | Managing multiple agents with different capabilities and contexts |
| **Privacy-Conscious** | Running AI locally without sending data to third parties |

## How It Should Work

1. **Start the daemon** — Single binary (`zerod`) serves HTTP API, WebSocket, and static web files
2. **Configure providers** — Add API keys for your preferred LLM providers (or point to local Ollama)
3. **Create agents** — Define agents with custom instructions (AGENTS.md), model selection, and tool access
4. **Agents create wards** — When given a coding task, the agent autonomously creates or selects a Code Ward (persistent project directory)
5. **Persistent code** — Code persists across sessions. Ward memory stores project context (tech stack, build commands)
6. **Delegation** — Root agent orchestrates by delegating tasks to specialized subagents, who execute in parallel and report back
7. **Skills & MCP** — Load instruction packages on demand; connect to external tool servers for filesystem, databases, APIs

## Differentiators

| Feature | Agent Zero | Cloud AI Platforms |
|---------|------------|-------------------|
| Data Location | Local machine | Cloud servers |
| Provider Lock-in | None | Usually locked |
| Offline Capable | Yes (with Ollama) | No |
| Cost | API costs only | Subscription + API |
| Customization | Unlimited | Limited |
| Privacy | Full control | Varies |
| Multi-Agent | Built-in delegation | Rare |
| Persistent Code | Code Wards | Session-scoped |
| Cross-Session Learning | Auto-distillation + hybrid recall | None |
| Embedding Cost | Local ONNX (zero cost) default | API-only |
