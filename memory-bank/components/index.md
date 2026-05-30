# Components Index

Quick reference for all system components. Check this before planning changes.

## Agent Execution

| Component | Location | Description |
|-----------|----------|-------------|
| Execution Loop | [execution-loop/overview.md](execution-loop/overview.md) | End-to-end pipeline: UI message → session → recall → intent → prompt → LLM loop → delegation → continuation → response → distillation. Covers new sessions and continuations. |
| — Files | [execution-loop/files.md](execution-loop/files.md) | Every file across UI, gateway, runtime, services with function references |
| Tool System | [tools/overview.md](tools/overview.md) | Current runtime tool setup: `Tool` trait, live root vs delegated registry, MCP tools, settings, result offload, safety hooks, and add-a-tool checklist. |
| Intent Analysis | [intent-analysis/overview.md](intent-analysis/overview.md) | Pre-execution middleware: indexes resources, semantic search, LLM analysis, emits events. Root agent only. |
| Ward Scaffolding | [ward-scaffolding/overview.md](ward-scaffolding/overview.md) | Post-execution: skill-driven directory scaffolding, AGENTS.md generation, core module indexing via language configs. |
| — Data Flow | [intent-analysis/data-flow.md](intent-analysis/data-flow.md) | Live execution pipeline, session replay, WS event routing |
| — Types | [intent-analysis/types.md](intent-analysis/types.md) | Rust + TypeScript types, field mapping, log format |
| — Error Handling | [intent-analysis/error-handling.md](intent-analysis/error-handling.md) | Truncation repair, fallback events, degradation hierarchy |
| — Files | [intent-analysis/files.md](intent-analysis/files.md) | Every file with line numbers |

## Chat Experience

| Component | Location | Description |
|-----------|----------|-------------|
| Chat v2 | [chat-v2/overview.md](chat-v2/overview.md) | `/chat` page (live route; `/chat-v2` is a legacy redirect). Reserved session via `settings.chat` slot, `mode=fast`. Two-row status pill, artifact strip + slide-out, Clear button. |
| — Learnings | [chat-v2/learnings.md](chat-v2/learnings.md) | Hard-earned rules from the build — **apply these to any new UI plan** (server-owned identity, deterministic pill, wire-format field drift, StrictMode-safe bootstrap, etc.). |
| — Backlog | [chat-v2/backlog.md](chat-v2/backlog.md) | Pending: artifact auto-registration, context compaction, silent-crash surfacing, multi-tab sync, history pagination, E2E mock-LLM harness. |
| Research v2 | [research-v2/overview.md](research-v2/overview.md) | `/research` page (live route; `/research-v2` is a legacy redirect). Multi-turn research surface — replaced the old `/` MissionControl page; legacy MissionControl still reachable via `/mission-control`. One session per user prompt, drawer of past sessions, nested subagent cards with Request/Response + inline LiveTicker, live artifacts strip, clickable ward chip. Dual WS subscription (conv_id + session_id) with reconnect recovery and event-driven reconcile. |
| — Learnings | [research-v2/learnings.md](research-v2/learnings.md) | Hard rules learned during R14a–R14j (respond lives in toolCalls, delegation events carry no conv_id, session-scope filters child events, subscription ack races early events, ping-timeout reconnect loses invoke_accepted, LogSession.conversation_id is the sess-*, etc.). |
| — Backlog | [research-v2/backlog.md](research-v2/backlog.md) | Pending: LiveTicker UX (too brief in fast sessions), memory_facts_index defect, E2E harness, title fallback, delete cascade on child rows, retire old `/` page. |

## LLM Client

| Component | Location | Description |
|-----------|----------|-------------|
| LLM Client | [llm-client/overview.md](llm-client/overview.md) | Text & multimodal content pipeline: Part types (Text, Image, File), ProviderEncoder trait, base64 flush persistence, `multimodal_analyze` tool, eagle-eye skill. Capability-aware encoding for OpenAI-compatible providers. |
| — Data Flow | [llm-client/data-flow.md](llm-client/data-flow.md) | Message lifecycle (text & multimodal), base64 flush, rehydration, tool flow, config injection |
| — Types | [llm-client/types.md](llm-client/types.md) | Rust + TypeScript types, Part/ContentSource/ImageDetail, ChatMessage serde, wire format mapping |
| — Error Handling | [llm-client/error-handling.md](llm-client/error-handling.md) | 8 error points: capability rejection, missing config, file not found, decode failures, API errors, backward compat |
| — Files | [llm-client/files.md](llm-client/files.md) | Every file across zero-core, zero-llm, agent-runtime, agent-tools, gateway, UI |

## Memory & Intelligence

| Component | Location | Description |
|-----------|----------|-------------|
| Memory Layer | [memory-layer/overview.md](memory-layer/overview.md) | The brain: facts, embeddings, knowledge graph, recall, distillation, ward knowledge sync. Six cooperating layers after Phases 1–6. |
| — Knowledge Graph | [memory-layer/knowledge-graph.md](memory-layer/knowledge-graph.md) | Phase 6 architecture: episodes, ward artifact indexer, expanded ontology (13 entity types / 27 relationship types), entity resolver, epistemic classes, MAGMA multi-view queries, real-time tool extraction. |
| — Data Model | [memory-layer/data-model.md](memory-layer/data-model.md) | Every table, every column, schema version history (v1 → v21), lifecycle events, query patterns. |
| — Explainer | [memory-layer/explainer.md](memory-layer/explainer.md) | Reader-friendly explanation of how z-Bot remembers, what enters prompts, and how recall works. |
| — Self-Improvement | [memory-layer/self-improvement.md](memory-layer/self-improvement.md) | Operator guide for belief network, query gate, default loops, and opt-in memory features. |
| — Performance | [memory-layer/performance.md](memory-layer/performance.md) | Historical memory-v2 baseline numbers and measurement notes. |
| — Architecture Deck | [memory-layer/architecture-deck.html](memory-layer/architecture-deck.html) | Visual architecture deck for the memory subsystem. |
| — Implementation Plans | [memory-layer/implementation-plans/README.md](memory-layer/implementation-plans/README.md) | Historical memory implementation plans consolidated from the old docs tree. |

## External Comparisons

| Component | Location | Description |
|-----------|----------|-------------|
| Hermes Comparison | [hermes-comparison/deltas.md](hermes-comparison/deltas.md) | Historical Hermes-agent gap analysis consolidated from `docs/`; paired with [deltas-rebuttal.md](hermes-comparison/deltas-rebuttal.md) and the current [2026-05-30 impact analysis](hermes-comparison/impact-analysis-2026-05-30.md). |

## Adding New Components

When adding a new component to `memory-bank/components/`:
1. Create a subdirectory: `components/{component-name}/`
2. Add an `overview.md` with purpose, when it runs, design principles
3. Add additional docs as needed (data-flow, types, error-handling, files)
4. Add a row to the table above
