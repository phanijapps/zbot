# Executor Steering Upgrade — Design Spec

## Goal

Upgrade the AgentZero executor with pi-mono-inspired features: steering queues, tool hooks, complexity-aware budgets, and performance optimizations. All agents (root + subagents) benefit. No new crate — the existing executor absorbs everything.

## Section 1: Steering Queue

Every `AgentExecutor` gets a `SteeringQueue` — thread-safe channel for injecting messages mid-execution.

```rust
pub struct SteeringQueue {
    tx: mpsc::UnboundedSender<SteeringMessage>,
    rx: mpsc::UnboundedReceiver<SteeringMessage>,
}

pub struct SteeringMessage {
    pub content: String,
    pub source: SteeringSource,
    pub priority: SteeringPriority,
}

pub enum SteeringSource {
    User,    // UI typed a message
    System,  // Complexity budget exceeded
    Parent,  // Root steering a subagent
}

pub enum SteeringPriority {
    Normal,    // Inject after current tool round
    Interrupt, // Inject immediately
}
```

**In the executor loop:** Before each LLM call, drain all pending steering messages and inject as `[STEER: {source}] {content}` user messages.

**UI integration:** New WebSocket event `SteerAgent { session_id, message }`. Gateway looks up the running executor's steering queue and pushes.

**Backwards compatible:** If no one pushes to the queue, the check is a `try_recv()` returning `Err(Empty)` — zero overhead.

## Section 2: Tool Hooks

Added to `ExecutorConfig`:

```rust
pub before_tool_call: Option<Arc<dyn Fn(&str, &Value) -> ToolCallDecision + Send + Sync>>,
pub after_tool_call: Option<Arc<dyn Fn(&str, &Value, &str, bool) -> Option<String> + Send + Sync>>,

pub enum ToolCallDecision {
    Allow,
    Block { reason: String },
}
```

**beforeToolCall:** Receives (tool_name, args). Returns Allow or Block. Blocked tools return `{"blocked": true, "reason": "..."}` as tool result.

**afterToolCall:** Receives (tool_name, args, result, succeeded). Returns `Option<String>`. If `Some(new_result)`, replaces what the LLM sees.

**Default:** Both `None` — zero overhead when not configured.

## Section 3: Complexity Scoring & Budget Enforcement

### Task-level complexity

tasks.json gets a `complexity` field per task:
```json
{"id": 1, "complexity": "S", "action": "create", "file": "core/data_fetcher.py", ...}
```

### Graph-node-level complexity

Added to `DelegateAction`:
```rust
pub complexity: Option<String>,  // "S", "M", "L", "XL"
```

Threads through: DelegateAction → StreamEvent::ActionDelegate → DelegationRequest → spawn.

### Iteration budgets

| Complexity | Budget | Nudge at 80% | Hard nudge at 100% |
|---|---|---|---|
| S | 15 | 12 | 15 |
| M | 30 | 24 | 30 |
| L | 50 | 40 | 50 |
| XL | 100 | 80 | 100 |
| None | 1000 | — | — (current behavior) |

Nudges go through the steering queue:
- 80%: `[STEER: System] You've used {n}/{budget} iterations for a {complexity} task. Wrap up or simplify.`
- 100%: `[STEER: System] Budget exceeded. Respond now with what you have.`

The existing loop detector (stuck detection via score) stays. Complexity handles "productive but too slow." Loop detector handles "stuck in a loop."

## Section 4: Context Management

### transformContext hook

```rust
pub transform_context: Option<Arc<dyn Fn(&mut Vec<ChatMessage>) + Send + Sync>>,
```

Called before every LLM call. Can modify messages. Default `None`.

### Line-aware tool result truncation

Upgrade `truncate_tool_result()`:
- Count lines, not bytes
- Keep first N + last M lines (configurable per tool)
- Insert `[... {omitted} lines truncated ...]`

### Pattern-based old message compression

During compaction, old assistant messages get compressed to one-liners:
- `[Turn 3: created core/data_fetcher.py, core/indicators.py]`
- Extract file paths and tool names, discard reasoning

No LLM call — pure pattern matching. Fast.

### Sequential tool execution mode

```rust
pub enum ToolExecutionMode {
    Parallel,    // Current default
    Sequential,  // One at a time
}
```

Added to `ExecutorConfig`. Default `Parallel`.

## Section 5: Root Re-Planning After Crashes

Prompt engineering on the SDLC pattern — no executor changes needed.

After a crash callback, root follows:
1. Check TASK RUNNER STATUS — complete/failed/pending counts
2. If >50% complete: re-delegate remaining only (smaller batch)
3. If <50% complete: split delegation into 2-3 smaller ones
4. If same task fails twice: break task into sub-tasks or change approach
5. NEVER code remaining tasks yourself

Root can modify the execution graph at runtime using the `execution_graph` tool.

## Section 6: Performance Optimization

### Tier 1: Quick Wins (with Phase 1)

- Use `DefaultHasher` for args hashing in progress tracker
- Guard debug serialization with `tracing::enabled!` in openai.rs
- Pass reference (not clone) in context_editing token estimation
- Edit messages in-place during compaction (no full vec clone)

### Tier 2: Medium (with Phase 2)

- Cache token estimates per message (skip re-estimation of unchanged messages)
- Configure HTTP client: connection pooling, tcp_nodelay, pool_max_idle
- Arc-wrap messages for streaming (avoid cloning entire vec per LLM call)

### Tier 3: Larger (Phase 3)

- Pattern-based old message summarization in compaction
- Pre-build tool_call_id → tool_name index for O(1) lookups

**Principle:** No optimization changes observable behavior. All tested against existing suite + 16 e2e tests.

## Files Changed

| File | Changes |
|------|---------|
| `runtime/agent-runtime/src/executor.rs` | Steering queue drain, tool hooks invocation, complexity budget, transformContext, sequential mode, Tier 1-2 optimizations |
| `runtime/agent-runtime/src/types/mod.rs` | SteeringQueue, SteeringMessage, ToolCallDecision, ToolExecutionMode types |
| `runtime/agent-runtime/src/llm/openai.rs` | HTTP client pooling config, debug guard on serialization |
| `runtime/agent-runtime/src/middleware/context_editing.rs` | In-place editing, reference-based token estimation, pattern-based compression |
| `framework/zero-core/src/event.rs` | `complexity` field on DelegateAction |
| `runtime/agent-runtime/src/types/events.rs` | `complexity` on ActionDelegate |
| `gateway/gateway-execution/src/delegation/context.rs` | `complexity` on DelegationRequest |
| `gateway/gateway-execution/src/delegation/spawn.rs` | Pass complexity to executor config |
| `gateway/gateway-execution/src/invoke/stream.rs` | Thread complexity, handle SteerAgent WS event |
| `gateway/gateway-execution/src/middleware/intent_analysis.rs` | SDLC re-planning guidance |
| `gateway/gateway-events/src/lib.rs` | SteerAgent WS event type |

## Implementation Order

1. **Phase 1:** Tool hooks + sequential mode + Tier 1 optimizations
2. **Phase 2:** Steering queue + complexity scoring + transformContext + Tier 2 optimizations
3. **Phase 3:** Pattern-based compaction + Tier 3 optimizations

Each phase is independently shippable and testable.
