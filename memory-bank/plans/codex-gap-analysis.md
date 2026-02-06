# Codex vs AgentZero: Critical Gap Analysis

## Purpose

This is NOT a "copy Codex" document. Codex is a CLI-first, single-user, single-agent tool backed by OpenAI's proprietary Responses API. AgentZero is a multi-agent, multi-provider, web-first platform with delegation, skills, and shared memory. Many Codex patterns don't apply. This analysis identifies **specific patterns that close real gaps** in AgentZero's architecture.

---

## Architecture Comparison at a Glance

| Dimension | Codex | AgentZero | Verdict |
|-----------|-------|-----------|---------|
| **Crate count** | 54 | ~8 | Codex is hyper-modular; AZ could benefit from splitting `gateway` |
| **LLM protocol** | OpenAI Responses API (WebSocket) | Chat Completions API (HTTP) | Different. AZ's is more portable (multi-provider) |
| **Streaming** | Real SSE/WebSocket from LLM | Simulated 5ms/char | **GAP** — AZ must switch to real streaming |
| **Tool calls** | Complete JSON per item (OutputItemDone) | Chunked SSE (tool_calls delta) | Different protocol, both valid |
| **Concurrency** | FuturesOrdered (model-driven parallel) | Sequential tool execution | **GAP** — AZ should support parallel tools |
| **Sandbox** | Platform-native (bubblewrap/seatbelt/restricted tokens) | None | **GAP** — but AZ runs in user's own env, different threat model |
| **Approval model** | 3-tier (rules→heuristics→sandbox) + amendments | None | **PARTIAL GAP** — AZ needs approval for dangerous ops |
| **State persistence** | sqlx async pool + in-memory history | rusqlite single Mutex | **GAP** — AZ needs async pool |
| **Config** | TOML, 7-layer hierarchy, hot-reload | JSON/YAML, single file per service | **Not a gap** — AZ's model is simpler and sufficient |
| **Context management** | Auto-compaction mid-turn, token tracking | No compaction, basic token tracking | **GAP** — AZ needs compaction for long sessions |
| **Retry/backoff** | Exponential + jitter, transport fallback | None | **GAP** — AZ needs retry logic |
| **Cancellation** | CancellationToken propagated everywhere | stop_flag on ExecutionHandle | **PARTIAL GAP** — AZ's is simpler but functional |
| **Observability** | OpenTelemetry (traces + metrics) | tracing crate only | **LOW PRIORITY** — tracing is sufficient for now |
| **Undo/rollback** | Git ghost commits | None | **NICE-TO-HAVE** — not critical path |
| **MCP** | rmcp client + mcp-server (exposes Codex as tool) | MCP client only | **MINOR GAP** — AZ could expose itself as MCP server |
| **File editing** | apply-patch (diff-based) | write tool (full replace) | **Not a gap** — different approach, both work |
| **Multi-agent** | N/A (single agent) | Delegation registry, subagent spawning | **AZ ADVANTAGE** |
| **Skills system** | N/A | Skill loading, SKILL.md format | **AZ ADVANTAGE** |
| **Shared memory** | N/A | Memory tool, workspace.json | **AZ ADVANTAGE** |
| **Web UI** | TUI (ratatui) | React web app + WebSocket | **Different domain** |

---

## CRITICAL GAPS (Must Fix)

### Gap 1: Real Streaming + Intermediate Text
**Codex pattern**: WebSocket → `OutputTextDelta` events stream in real-time, independently from tool processing. Text arrives as it's generated, tool calls arrive as complete items.

**AgentZero today**: `chat()` returns full response, then simulates streaming at 5ms/char. Text alongside tool calls is silently dropped.

**What to adopt**: Switch to `chat_stream()` (already exists in AZ's LLM client trait). Use mpsc channel bridge pattern from our Phase 1 plan. This is NOT copying Codex — it's using AZ's own existing infrastructure.

**What NOT to adopt**: Codex's WebSocket-to-LLM transport. AZ's HTTP SSE streaming is fine for multi-provider support.

**Priority**: P0 — This is the single biggest UX gap.

---

### Gap 2: Retry with Exponential Backoff
**Codex pattern** (`codex-client/src/retry.rs`):
- Configurable retry policy (max attempts, base delay)
- Per-error-class retry decisions (429, 5xx, transport)
- Exponential backoff with jitter: `2^(attempt-1) * base * random(0.9..1.1)`
- Saturating arithmetic prevents overflow

**AgentZero today**: Zero retry logic. LLM API failure = execution failure. One transient 500 from OpenAI kills the entire agent run.

**What to adopt**:
```rust
// Add to agent-runtime/src/llm/
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub retry_on_429: bool,
    pub retry_on_5xx: bool,
    pub retry_on_timeout: bool,
}
```
Wrap `chat()` and `chat_stream()` with retry logic. Use jitter to prevent thundering herd.

**What NOT to adopt**: Codex's transport fallback (WebSocket→HTTPS). AZ uses HTTP only.

**Priority**: P1 — Essential for production reliability.

---

### Gap 3: Async SQLite with Connection Pool
**Codex pattern** (`state/src/runtime.rs`):
- `sqlx::SqlitePool` — async, connection-pooled SQLite
- Compile-time checked queries via `sqlx::query!` macros
- Async migrations

**AgentZero today**: Single `Arc<Mutex<Connection>>` via rusqlite. All DB ops serialize. Hot-path writes (token updates, logs) block stream processing.

**What to adopt**: Two options:
1. **Minimal**: Keep rusqlite, switch to r2d2 pool (4-8 connections) + WAL mode. Least disruption.
2. **Full**: Migrate to sqlx. Better long-term but significant refactor.

**Recommendation**: Option 1 for now (r2d2 + WAL). The single Mutex is the bottleneck, not the sync API. Codex uses sqlx because they're async-first, but AZ's DB operations are simple enough that sync + pool is fine.

**What NOT to adopt**: sqlx compile-time query checking. Overkill for AZ's current schema complexity.

**Priority**: P1 — Directly impacts concurrent execution throughput.

---

### Gap 4: Parallel Tool Execution
**Codex pattern** (`core/src/tools/parallel.rs`):
- `FuturesOrdered` queue for tool calls
- Model-driven parallelism flag (`supports_parallel_tool_calls`)
- Tools spawn as tokio tasks, drain in order

**AgentZero today**: Tools execute sequentially in a `for tool_call in tool_calls` loop. If the LLM requests 3 tool calls, they run one after another.

**What to adopt**: When the LLM returns multiple tool calls:
```rust
let mut futures = FuturesOrdered::new();
for tool_call in tool_calls {
    futures.push_back(execute_tool(tool_call));
}
while let Some(result) = futures.next().await {
    // Record result in order
}
```

**What NOT to adopt**: Codex's read/write lock pattern for model-driven parallelism toggle. AZ can start simpler — always parallel when multiple tools returned.

**Priority**: P1 — Significant speedup for multi-tool turns (e.g., reading 3 files).

---

### Gap 5: Context Window Management / Auto-Compaction
**Codex pattern** (`core/src/compact.rs`):
- Token counting (heuristic word-based)
- Configurable auto-compact threshold (`model_auto_compact_token_limit`)
- Mid-turn compaction: when tokens exceed limit, pause, summarize history, retry
- LLM-based summarization prompt
- Truncation of overlong user messages (20K token cap)

**AgentZero today**: No context management. Long conversations eventually hit context window limits and crash with an API error.

**What to adopt**:
1. **Token counting**: Add approximate token counter (word-based heuristic, like Codex)
2. **Auto-compaction trigger**: When approaching context limit, summarize older messages
3. **Compaction strategy**: Use the same LLM to summarize conversation prefix

**What NOT to adopt**: Codex's remote cloud-based compaction. AZ should compact locally using the same LLM.

**Priority**: P2 — Important for multi-turn sessions, but existing sessions are short enough to survive without it initially.

---

## VALUABLE PATTERNS (Should Adopt)

### Pattern A: CancellationToken Propagation
**Codex**: `CancellationToken` from `tokio_util` passed to every async operation. `.or_cancel(token)` extension trait on futures.

**AgentZero**: `stop_flag: Arc<AtomicBool>` on ExecutionHandle, checked in stream callback.

**Gap**: AZ's stop flag only works in the stream callback. If the LLM API call takes 30 seconds and user cancels, nothing happens until the next callback invocation.

**Adopt**: Use `CancellationToken` (or `tokio::select!` with the stop flag) around the LLM API call itself:
```rust
tokio::select! {
    result = llm_client.chat_stream(...) => { /* process */ }
    _ = handle.cancelled() => { return Err("Cancelled") }
}
```

---

### Pattern B: Output Streaming with Chunking + Limits
**Codex**: Tool output capped at 1 MiB, streamed in 8 KiB chunks, with `HeadTailBuffer` for bounded memory.

**AgentZero**: Shell tool captures full output, no size limits, no chunking.

**Adopt**: Add output size limits and chunking to the shell tool. Cap at 1 MiB. Stream chunks to user while tool runs (improves perceived responsiveness).

---

### Pattern C: Batch DB Writes (Token Updates)
**Codex**: Token updates tracked in-memory, flushed periodically (not per-event).

**AgentZero**: `update_execution_tokens()` hits SQLite on every TokenUpdate event in the streaming callback.

**Adopt**: Already in our Phase 2C plan — BatchWriter with channel. Codex validates this pattern.

---

### Pattern D: Process Spawning Safety
**Codex** (`utils/pty/src/process_group.rs`):
- `PR_SET_PDEATHSIG` — child dies when parent dies (prevents orphans)
- `setpgid(0, 0)` — new process group (isolates from terminal signals)
- `setsid()` — new session (detach from TTY)
- Stdin set to `Stdio::null()` (prevents hanging on input)

**AgentZero**: Basic `tokio::process::Command` with no process group isolation.

**Adopt**: Add `PR_SET_PDEATHSIG` and process group isolation to shell tool execution. Prevents orphan processes on agent crash.

---

## PATTERNS TO SKIP (Not Applicable to AZ)

### Skip 1: Platform-Native Sandboxing
**Why**: Codex runs untrusted LLM-generated commands on user's machine (CLI tool). AZ is a web platform where the user explicitly configures agents and tools. Different threat model. AZ's agent can't run arbitrary commands unless the user gives it the shell tool.

**Alternative**: AZ should focus on per-tool permission controls (which tools each agent has access to — already implemented via tool tiers).

### Skip 2: Exec Policy / Rules Engine
**Why**: Codex needs interactive approval because it's a CLI where the user watches. AZ is a web platform where agents run in the background. Interactive approval would require WebSocket round-trips and UI modals — different UX pattern.

**Alternative**: AZ already has tool tiers (core/optional). Could add a simple allowlist/blocklist for shell commands per agent config.

### Skip 3: TOML Config Hierarchy
**Why**: Codex needs 7 layers because it's a CLI tool running in different environments (cloud, admin, user, repo). AZ has a single config directory per installation. AZ's JSON/YAML config files are fine.

### Skip 4: Git Ghost Commits / Undo
**Why**: Codex's undo is for interactive CLI users who can immediately see and revert changes. AZ agents work on tasks asynchronously — the undo UX would be different (and more complex to implement via web UI).

### Skip 5: TUI Architecture
**Why**: Completely different frontend stack. AZ uses React web app.

### Skip 6: Starlark Policy Language
**Why**: Over-engineered for AZ's current needs. Simple JSON allowlists are sufficient.

### Skip 7: OpenTelemetry
**Why**: AZ's `tracing` crate usage is sufficient. OTEL adds complexity and infrastructure requirements (collector, backend). Not needed until AZ is running at scale with multiple instances.

---

## IMPLEMENTATION ROADMAP (Updated)

### Phase 1: Streaming & Core UX (Week 1-2)
1. **Real streaming** via `chat_stream()` with mpsc bridge
2. **Intermediate text** streams alongside tool calls
3. **Retry with backoff** on LLM client (`RetryPolicy`)
4. **CancellationToken** on LLM API calls (not just callback)

### Phase 2: Database & Performance (Week 2-3)
1. **WAL mode + pragmas** on SQLite
2. **r2d2 connection pool** (replace single Mutex)
3. **BatchWriter** for hot-path DB writes
4. **Moka caching** for Provider, MCP, Settings

### Phase 3: Execution Quality (Week 3-4)
1. **Parallel tool execution** (FuturesOrdered)
2. **Output chunking + limits** for shell tool (1 MiB cap, 8 KiB chunks)
3. **Process group isolation** for spawned commands
4. **Token tracking fix** for web sessions

### Phase 4: Context Management (Week 4-5)
1. **Approximate token counting** (word-based heuristic)
2. **Auto-compaction** when approaching context limit
3. **LLM-based summarization** for conversation prefix
4. **Max message length enforcement**

### Phase 5: Polish (Week 5+)
1. **Rate limit awareness** (parse and display provider rate limit headers)
2. **Shell command safety heuristics** (warn on `rm -rf`, `DROP TABLE`, etc.)
3. **Output head+tail buffer** (show first/last N lines of large output)

### Phase 6: Gateway Decomposition (Week 6+)
Codex uses 54 hyper-modular crates. AZ's gateway is a 73-file monolith (~15,675 LOC). While AZ doesn't need 54 crates, breaking gateway into 8-10 focused crates improves compile times, testability, and maintainability. See `responsive-agent-architecture.md` Phase 5 for the full decomposition plan.

---

## KEY TAKEAWAYS

1. **Codex validates our Phase 1-2 plan** — real streaming, retry logic, and DB pooling are exactly what production agent systems need.

2. **Parallel tool execution is a significant miss** we hadn't identified before. Adding `FuturesOrdered` is straightforward and impactful.

3. **Context management/compaction is the next frontier** after streaming and DB fixes. Without it, long multi-turn sessions will fail.

4. **AZ has unique advantages** that Codex doesn't: multi-agent delegation, skills system, shared memory, web UI. These shouldn't be sacrificed for Codex-style patterns.

5. **Sandboxing and approval are different problem spaces** for AZ. Don't cargo-cult Codex's CLI-oriented security model into a web platform.

6. **The biggest architectural difference**: Codex is a single monolithic agent loop. AZ is a distributed execution platform with sessions, delegation trees, and event buses. This complexity is AZ's strength for agentic workflows, but it means some Codex patterns need adaptation rather than direct transplant.

7. **Gateway needs modularization** — Codex's 54-crate workspace is extreme, but AZ's 73-file gateway monolith is the other extreme. Breaking it into 8-10 focused crates (persistence, execution, transport, connectors) will improve compile times, testability, and make the performance fixes easier to implement in isolation.
