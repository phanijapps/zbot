# Pi-Mono Executor Upgrade — Design Spec

## Decision

Upgrade the existing `agent-runtime` executor with pi-mono features. Do NOT create a separate `zero-bhim` crate.

## Why

AgentZero's executor already has more infrastructure than pi-mono (progress tracking, middleware pipeline, delegation, wards, memory, MCP). Pi-mono's advantages are simpler features that can be added incrementally.

## Phases

### Phase 1: Tool Hooks + Sequential Execution (Trivial)

**beforeToolCall hook:**
- Receives: tool name, args, context
- Can return `block: true` to prevent execution
- Use case: block dangerous commands, enforce ward boundaries

**afterToolCall hook:**
- Receives: tool name, args, result, is_error, context
- Can transform the result before it goes to LLM
- Use case: truncate differently per tool, add metadata

**Sequential tool execution mode:**
- Config: `tool_execution_mode: "parallel" | "sequential"`
- Sequential: execute one tool at a time (safer for file operations)
- Parallel: current behavior (default)

**Files:**
- `runtime/agent-runtime/src/executor.rs` — add hook invocation around tool execution
- `runtime/agent-runtime/src/types/mod.rs` — hook type definitions

### Phase 2: Steering + Context Transform (Moderate)

**Steering queues:**
- `agent.steer(message)` — inject message after current tool round
- `agent.follow_up(message)` — inject after agent stops
- Drain modes: "all" (process all queued) vs "one-at-a-time"
- Use case: user redirects agent mid-execution

**transformContext hook:**
- Called before each LLM call
- Can modify/prune/reorder messages
- Use case: remove stale context, inject fresh data

**Files:**
- `runtime/agent-runtime/src/executor.rs` — steering queue in executor loop
- `runtime/agent-runtime/src/types/mod.rs` — SteeringQueue type
- `gateway/gateway-execution/src/invoke/stream.rs` — expose steering API via events

### Phase 3: LLM-Summarized Compaction + Session Persistence (Significant)

**LLM-summarized compaction:**
- At 80% context threshold, call LLM to summarize old turns
- Replace old messages with summary (not just truncate)
- Pi-mono uses `completeSimple()` for this

**Session persistence:**
- Save/restore conversation state between sessions
- Model switching mid-session
- Session branching (fork a conversation)

**Files:**
- `runtime/agent-runtime/src/middleware/context_editing.rs` — add summarization step
- New: session persistence layer (significant new code)

## What We Already Have (No Changes Needed)

- Progress tracking + stuck detection (better than pi-mono)
- Middleware pipeline (context editing, token counting)
- Delegation system (pi-mono doesn't have this)
- Ward system (pi-mono doesn't have this)
- Memory/knowledge graph (pi-mono doesn't have this)
- MCP integration (pi-mono doesn't have this)
- Tool result offloading to filesystem

## Reference

Pi-mono source: https://github.com/badlogic/pi-mono
Key files analyzed:
- `packages/agent/src/agent-loop.ts` — agent loop with steering
- `packages/agent/src/types.ts` — hooks, events, queues
- `packages/coding-agent/src/core/compaction/compaction.ts` — LLM summarization
- `packages/coding-agent/src/core/tools/truncate.ts` — line-aware truncation
