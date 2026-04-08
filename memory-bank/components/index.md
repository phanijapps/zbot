# Components Index

Quick reference for all system components. Check this before planning changes.

## Agent Execution

| Component | Location | Description |
|-----------|----------|-------------|
| Intent Analysis | [intent-analysis/overview.md](intent-analysis/overview.md) | Pre-execution middleware: indexes resources, semantic search, LLM analysis, emits events. Root agent only. |
| Ward Scaffolding | [ward-scaffolding/overview.md](ward-scaffolding/overview.md) | Post-execution: skill-driven directory scaffolding, AGENTS.md generation, core module indexing via language configs. |
| — Data Flow | [intent-analysis/data-flow.md](intent-analysis/data-flow.md) | Live execution pipeline, session replay, WS event routing |
| — Types | [intent-analysis/types.md](intent-analysis/types.md) | Rust + TypeScript types, field mapping, log format |
| — Error Handling | [intent-analysis/error-handling.md](intent-analysis/error-handling.md) | Truncation repair, fallback events, degradation hierarchy |
| — Files | [intent-analysis/files.md](intent-analysis/files.md) | Every file with line numbers |

## Chat Experience

| Component | Location | Description |
|-----------|----------|-------------|
| Chat Experience | [chat-experience/overview.md](chat-experience/overview.md) | 3-panel chat UI: center (message → phases → response), sidebar (intent, ward, facts, subagents, plan). Session State API for reconnection. |

## LLM Client

| Component | Location | Description |
|-----------|----------|-------------|
| LLM Client | [llm-client/overview.md](llm-client/overview.md) | Text & multimodal content pipeline: Part types (Text, Image, File), ProviderEncoder trait, base64 flush persistence, `multimodal_analyze` tool, eagle-eye skill. Capability-aware encoding for OpenAI-compatible providers. |

## Memory & Intelligence

| Component | Location | Description |
|-----------|----------|-------------|
| Memory Layer | [memory-layer/overview.md](memory-layer/overview.md) | The brain: facts, embeddings, knowledge graph, recall, distillation, ward knowledge sync. Stores/retrieves/applies knowledge across sessions. |
| — Backlog | [memory-layer/backlog.md](memory-layer/backlog.md) | Planned: policies UI, graph query tool, pruning, cross-ward synthesis, dashboard |

## Adding New Components

When adding a new component to `memory-bank/components/`:
1. Create a subdirectory: `components/{component-name}/`
2. Add an `overview.md` with purpose, when it runs, design principles
3. Add additional docs as needed (data-flow, types, error-handling, files)
4. Add a row to the table above
