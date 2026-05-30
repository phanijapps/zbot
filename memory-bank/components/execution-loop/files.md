# Execution Loop — File Reference

Every file involved in the execution loop with key functions. Line numbers are intentionally omitted because these modules move frequently.

## UI Layer

| File | Function / Export | Purpose |
|------|-------------------|---------|
| `apps/ui/src/features/research-v2/useResearchSession.ts` | `useResearchSession()` | Research hook: snapshot hydrate + WS subscribe + dispatch |
| `apps/ui/src/features/research-v2/event-map.ts` | `mapGatewayEventToResearchAction()` | WS event → `ResearchAction` mapper |
| `apps/ui/src/features/research-v2/reducer.ts` | `reduceResearchSession()` | Reduces actions over per-turn state (turns, subagents, intent) |
| `apps/ui/src/features/research-v2/SessionTurnBlock.tsx` | `SessionTurnBlock` | Renders one user-prompt + assistant-response turn |
| `apps/ui/src/features/chat-v2/useQuickChat.ts` | `useQuickChat()` | Chat hook: WS subscribe + dispatch (no intent analysis UI) |
| `apps/ui/src/features/chat/mission-hooks.ts` | `useRecentSessions()`, `switchToSession()`, `timeAgo()` | Recent-sessions list + session switching helpers |
| `apps/ui/src/services/transport/http.ts` | `executeAgent()` | Sends invoke command via WebSocket |

## Gateway — WebSocket & Routing

| File | Function | Purpose |
|------|----------|---------|
| `gateway/src/websocket/handler.rs` | `handle_client_message()` | Routes ClientMessage::Invoke |
| | `gateway_event_to_server_message()` | Converts GatewayEvent → ServerMessage |
| `gateway/src/services/runtime.rs` | `invoke_with_hook_and_callback()` | Entry point from WS to runner |
| | `invoke()` | Simplified invoke (no hooks) |
| `gateway/gateway-ws-protocol/src/messages.rs` | `ServerMessage` enum | All WS messages (AgentStarted, Token, ToolCall, IntentAnalysis*, etc.) |
| | `ClientMessage` enum | Invoke, Stop, Pause, Resume, EndSession |

## Gateway — Execution Runner

| File | Function | Purpose |
|------|----------|---------|
| `gateway/gateway-execution/src/runner/core.rs` | `invoke_with_callback()` | Main orchestration entry; wires bootstrap, state, and runner services |
| `gateway/gateway-execution/src/runner/invoke_bootstrap.rs` | `create_executor()` | Memory recall, intent analysis, prompt injection, and executor build |
| `gateway/gateway-execution/src/runner/execution_stream.rs` | `ExecutionStream` | Root async LLM loop + event processing |
| `gateway/gateway-execution/src/runner/delegation_dispatcher.rs` | `DelegationDispatcher` | Background per-session delegation queue |
| `gateway/gateway-execution/src/runner/continuation_watcher.rs` | `ContinuationWatcher` | Resumes root after delegations |
| `gateway/gateway-execution/src/lifecycle.rs` | `get_or_create_session()` | Session reuse or creation |
| | `start_execution()` | QUEUED → RUNNING transition |
| | `complete_execution()` | RUNNING → COMPLETED transition |
| | `emit_agent_started()` | Publishes AgentStarted event |

## Gateway — Intent Analysis

| File | Function | Purpose |
|------|----------|---------|
| `gateway/gateway-execution/src/middleware/intent_analysis.rs` | `analyze_intent()` | LLM call for intent analysis |
| | `format_intent_injection()` | Formats analysis as prompt injection |
| | `index_resources()` | Upserts skills/agents/wards to memory_facts |

## Gateway — Delegation

| File | Function | Purpose |
|------|----------|---------|
| `gateway/gateway-execution/src/delegation/spawn.rs` | `spawn_delegated_agent()` | Creates child session + execution, runs subagent |
| `gateway/gateway-execution/src/delegation/callback.rs` | `handle_delegation_success()` | Completes delegation, sends result to parent |

## Gateway — Executor Build

| File | Function | Purpose |
|------|----------|---------|
| `gateway/gateway-execution/src/invoke/executor.rs` | `ExecutorBuilder.build()` | Assembles LLM client + tools + middleware |
| `gateway/gateway-execution/src/invoke/setup.rs` | `AgentLoader.load_or_create_root()` | Loads agent YAML + provider |

## Gateway — Prompt Templates

| File | Function | Purpose |
|------|----------|---------|
| `gateway/gateway-templates/src/lib.rs` | `assemble_prompt()` | Full prompt: SOUL + INSTRUCTIONS + OS + shards + runtime |
| | `assemble_chat_prompt()` | Chat mode: SOUL + chat_instructions + OS + 2 shards |
| | `load_shards()` | Loads required + user shards |

## Gateway — Events

| File | Type | Purpose |
|------|------|---------|
| `gateway/gateway-events/src/lib.rs` | `GatewayEvent` enum | All events: AgentStarted, Token, ToolCall, IntentAnalysis*, Delegation*, etc. |
| `gateway/gateway-events/src/broadcast.rs` | `EventBus` | Pub/sub event routing |

## Gateway — Distillation

| File | Function | Purpose |
|------|----------|---------|
| `gateway/gateway-execution/src/distillation.rs` | `SessionDistiller.distill()` | Post-session fact/entity/episode extraction |

## Runtime — Executor

| File | Function | Purpose |
|------|----------|---------|
| `runtime/agent-runtime/src/executor.rs` | `execute_stream()` | Streaming entry point |
| | `execute_with_tools_loop()` | Main LLM ↔ tool iteration loop |
| | Max iterations check | Hard stop at max_turns |
| | Tool execution | Sequential/parallel tool dispatch |
| | Stop signals | respond/delegation exit conditions |

## Runtime — Tools

| File | Tool Name | Purpose |
|------|-----------|---------|
| `runtime/agent-runtime/src/tools/respond.rs` | `respond` | Send final response to user |
| `runtime/agent-runtime/src/tools/delegate.rs` | `delegate_to_agent` | Spawn subagent with task |
| `runtime/agent-tools/src/tools/memory.rs` | `memory` | Save/recall facts |
| `runtime/agent-tools/src/tools/execution/skills.rs` | `load_skill` | Load skill into context |
| `runtime/agent-tools/src/tools/ward.rs` | `ward` | Ward management |
| `runtime/agent-tools/src/tools/execution/shell.rs` | `shell` | Command execution |

## Services — State & Logging

| File | Service | Purpose |
|------|---------|---------|
| `services/execution-state/src/service.rs` | `StateService` | Session/execution CRUD, reactivation, continuation |
| `services/api-logs/src/service.rs` | `LogService` | Execution logs, `has_intent_log()` |
| `services/api-logs/src/repository.rs` | `has_category_log()` | SQL check for intent log existence |

## Services — Memory

| File | Service | Purpose |
|------|---------|---------|
| `gateway/gateway-memory/src/recall/mod.rs` | `MemoryRecall` | Hybrid search: vector + FTS5 + graph |
| `stores/zero-stores-sqlite/src/memory_fact_store.rs` | `SqliteMemoryFactStore` | CRUD for memory_facts |
| `stores/zero-stores-sqlite/src/kg/storage.rs` | `GraphStorage` | Entity/relationship persistence |
