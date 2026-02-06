# Responsive Agent Architecture Plan

## Executive Summary

AgentZero's execution pipeline has **five systemic bottlenecks** that prevent it from feeling like Claude Code or Manus in "yolo mode". This plan addresses each with proven, production-grade strategies.

| Bottleneck | Impact | Fix | ETA |
|-----------|--------|-----|-----|
| Simulated streaming (5ms/char) | Massive latency on every response | Switch to real SSE streaming | Phase 1 |
| Text lost alongside tool calls | User sees nothing while agent works | Stream intermediate text | Phase 1 |
| Single SQLite Mutex | All DB ops serialize across all agents | WAL mode + connection pool | Phase 2 |
| Hot-path DB writes in callback | Token updates block stream processing | Batch writes with channel | Phase 2 |
| Config files read every execution | 4-6ms file I/O per invocation | Moka/RwLock caching | Phase 3 |

**Expected outcome**: First token to screen in <100ms (currently 500ms+). Smooth real-time streaming. No UI freezes during tool execution.

---

## Phase 1: Real Streaming & Intermediate Text (Critical Path)

### 1A. Switch executor from `chat()` to `chat_stream()`

**Problem**: `executor.rs` calls `self.llm_client.chat()` (non-streaming), gets full response, then simulates streaming at 5ms per character. A 500-char response takes 2.5 seconds of fake delay.

**Current flow** (executor.rs:227):
```
LLM API call (1-5s) → Full response → Simulate 5ms/char → User sees tokens
Total: API latency + (chars × 5ms)
```

**Target flow**:
```
LLM API SSE stream → Token arrives → Emit immediately → User sees token
Total: Time to first token (~200ms) + natural streaming
```

**Changes required**:

1. **Extend `chat_stream` trait** to accept tools parameter:
   - File: `runtime/agent-runtime/src/llm/client.rs:63-67`
   - Add `tools: Option<Value>` parameter to `chat_stream()` (matching `chat()` signature)

2. **Update OpenAI `chat_stream` implementation**:
   - File: `runtime/agent-runtime/src/llm/openai.rs:223-350`
   - Line 232: Change `self.build_request_body(messages, None)` → `self.build_request_body(messages, tools)`
   - The SSE parsing already handles tool_calls chunks (lines 314-346) — just needs tools in request

3. **Replace `chat()` with `chat_stream()` in executor loop**:
   - File: `runtime/agent-runtime/src/executor.rs` (the main execution loop, ~line 227)
   - Replace the non-streaming call with `chat_stream()`
   - Remove the 5ms-per-char simulation loop entirely
   - Accumulate content + tool_calls from StreamChunk callbacks
   - Emit Token events in real-time as StreamChunk::Token arrives

**Sketch of new executor loop body** (replaces lines ~227-260):
```rust
// Accumulate response from streaming
let stream_content = Arc::new(Mutex::new(String::new()));
let stream_tool_calls = Arc::new(Mutex::new(Vec::new()));
let stream_reasoning = Arc::new(Mutex::new(String::new()));

let content_ref = stream_content.clone();
let tools_ref = stream_tool_calls.clone();
let reasoning_ref = stream_reasoning.clone();
let on_event_ref = &on_event;

let response = self.llm_client.chat_stream(
    current_messages.clone(),
    Box::new(move |chunk| {
        match chunk {
            StreamChunk::Token(text) => {
                content_ref.lock().unwrap().push_str(&text);
                // Emit IMMEDIATELY — no 5ms delay
                on_event_ref(StreamEvent::Token {
                    content: text,
                    agent_id: agent_id.clone(),
                });
            }
            StreamChunk::Reasoning(text) => {
                reasoning_ref.lock().unwrap().push_str(&text);
                on_event_ref(StreamEvent::Reasoning {
                    content: text,
                    agent_id: agent_id.clone(),
                });
            }
            StreamChunk::ToolCall(tc) => {
                // Accumulate tool calls
                // (tool call streaming handled separately)
            }
        }
    }),
    tools_json.clone(), // NEW: pass tools
).await?;

// Now response.content, response.tool_calls are populated
// Token events already emitted — no simulation needed
```

**Key constraint**: The `on_event` callback in `execute_stream` is `FnMut`, not `Fn`. The `chat_stream` callback is `Fn + Send + Sync`. We need to bridge this — likely via an `mpsc` channel where the stream callback sends events and the executor loop receives and forwards them.

**Recommended bridge pattern**:
```rust
let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<StreamChunk>();

// Spawn the streaming call
let stream_handle = tokio::spawn(async move {
    llm_client.chat_stream(messages, Box::new(move |chunk| {
        let _ = tx.send(chunk);
    }), tools).await
});

// Process chunks as they arrive
while let Some(chunk) = rx.recv().await {
    match chunk {
        StreamChunk::Token(text) => {
            on_event(StreamEvent::Token { content: text, .. });
        }
        // ... handle other chunk types
    }
}

let response = stream_handle.await??;
```

### 1B. Stream intermediate text alongside tool calls

**Problem**: When LLM returns "Let me help you with that..." + tool_calls, the text is silently swallowed (executor.rs:245-260). User sees nothing until tools finish.

**Current code** (executor.rs ~line 245):
```rust
let tool_calls = response.tool_calls.clone().unwrap_or_default();
if tool_calls.is_empty() {
    // ONLY HERE: Token events fire
    for ch in response.content.chars() { ... }
    break;
} else {
    // Text stored but NEVER streamed
    current_messages.push(ChatMessage { content: response.content, ... });
}
```

**Fix**: With 1A implemented (real streaming), this is **automatically solved**. The `StreamChunk::Token` callback fires for ALL text content as it arrives from the LLM, regardless of whether tool calls follow. By the time we know tool calls are coming, the text is already streamed.

**If 1A is deferred** (fallback fix for `chat()` path):
```rust
// After getting response from chat(), ALWAYS stream content first
if !response.content.is_empty() {
    on_event(StreamEvent::Token {
        content: response.content.clone(),
        agent_id: agent_id.clone(),
    });
}

// Then handle tool calls
let tool_calls = response.tool_calls.clone().unwrap_or_default();
if tool_calls.is_empty() {
    break; // Final response, already streamed above
}
// Continue with tool execution...
```

### 1C. Stream tool execution progress

**Problem**: While tools execute (shell, write, read), user sees nothing. Claude Code shows tool names and progress indicators in real-time.

**Already partially solved**: `ToolCallStart` and `ToolResult` events already fire and propagate to WebSocket as `GatewayEvent::ToolCall` and `GatewayEvent::ToolResult`. The UI just needs to render them (frontend concern).

**Additional improvement**: Add a `StreamEvent::ToolProgress` variant for long-running tools (shell output streaming, file read progress). This is lower priority.

---

## Phase 2: SQLite Performance (High Impact)

### 2A. Enable WAL mode + pragmas

**Problem**: Default SQLite journal mode blocks readers during writes. Single connection means all operations serialize.

**File**: `gateway/src/database/connection.rs` (line 15-65)

**Changes**:
```rust
impl DatabaseManager {
    pub fn new(config_dir: PathBuf) -> Result<Self, String> {
        let db_path = config_dir.join("conversations.db");
        let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;

        // Performance pragmas
        conn.execute_batch("
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA cache_size = -8000;        -- 8MB cache
            PRAGMA busy_timeout = 5000;       -- 5s wait on lock
            PRAGMA wal_autocheckpoint = 1000;  -- checkpoint every 1000 pages
            PRAGMA temp_store = MEMORY;
        ").map_err(|e| e.to_string())?;

        // Run migrations...
        Ok(Self { db_path, conn: Arc::new(Mutex::new(conn)) })
    }
}
```

**Impact**: WAL mode allows concurrent reads during writes. `synchronous = NORMAL` is safe with WAL and significantly faster. Cache size increase reduces disk I/O for repeated queries.

**Risk**: Minimal. WAL is the recommended mode for concurrent SQLite access.

### 2B. Move from single Mutex to connection pool

**Problem**: `Arc<Mutex<Connection>>` serializes ALL database access. A token update in one agent blocks message history load in another.

**Option A: r2d2-sqlite (synchronous pool)**
```toml
[dependencies]
r2d2 = "0.8"
r2d2_sqlite = "0.24"
```

```rust
pub struct DatabaseManager {
    pool: r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>,
}

impl DatabaseManager {
    pub fn new(config_dir: PathBuf) -> Result<Self, String> {
        let db_path = config_dir.join("conversations.db");
        let manager = SqliteConnectionManager::file(&db_path);
        let pool = r2d2::Pool::builder()
            .max_size(8)               // 8 connections
            .min_idle(Some(2))         // Keep 2 warm
            .connection_timeout(Duration::from_secs(5))
            .build(manager)
            .map_err(|e| e.to_string())?;

        // Apply pragmas to each connection via customizer
        // ...
        Ok(Self { pool })
    }

    pub fn with_connection<F, T>(&self, f: F) -> Result<T, String>
    where F: FnOnce(&Connection) -> Result<T, rusqlite::Error>
    {
        let conn = self.pool.get().map_err(|e| e.to_string())?;
        f(&conn).map_err(|e| e.to_string())
    }
}
```

**Option B: deadpool-sqlite (async pool)** — preferred for tokio
```toml
[dependencies]
deadpool-sqlite = "0.8"
```

**Recommendation**: Option A (r2d2) — simpler, proven, and since SQLite itself is synchronous, the async wrapper in deadpool adds complexity without real benefit. The key win is having multiple connections so reads don't block on writes.

### 2C. Batch hot-path writes via channel

**Problem**: Inside the streaming callback, DB writes for token updates and log entries block event processing. With simulated streaming this was masked; with real streaming it becomes critical.

**Current hot path** (stream.rs:169-176):
```rust
// Called per token event — INSIDE the stream callback
state_service.update_execution_tokens(execution_id, tokens_in, tokens_out);
log_service.log(session_id, "token_update", ...);
```

**Solution**: Decouple writes via a bounded channel + background writer.

```rust
pub struct BatchWriter {
    tx: mpsc::UnboundedSender<DbWrite>,
}

enum DbWrite {
    TokenUpdate { execution_id: String, tokens_in: u32, tokens_out: u32 },
    LogEntry { session_id: String, event_type: String, data: Value },
    Message { execution_id: String, role: String, content: String },
}

impl BatchWriter {
    pub fn new(state_service: Arc<StateService>, log_service: Arc<LogService>) -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel();

        // Background task: batch writes every 100ms or 10 items
        tokio::spawn(async move {
            let mut batch = Vec::new();
            let mut interval = tokio::time::interval(Duration::from_millis(100));

            loop {
                tokio::select! {
                    Some(write) = rx.recv() => {
                        batch.push(write);
                        if batch.len() >= 10 {
                            flush_batch(&state_service, &log_service, &mut batch);
                        }
                    }
                    _ = interval.tick() => {
                        if !batch.is_empty() {
                            flush_batch(&state_service, &log_service, &mut batch);
                        }
                    }
                }
            }
        });

        Self { tx }
    }
}

fn flush_batch(state: &StateService, logs: &LogService, batch: &mut Vec<DbWrite>) {
    // Group by type, execute in single transaction
    // Token updates: keep only the latest per execution_id
    // Log entries: batch INSERT
    batch.clear();
}
```

**Impact**: Reduces DB writes from per-event to batched (10x fewer lock acquisitions). Token updates coalesce (only latest value persisted). Log entries batch-inserted in single transaction.

**Integration point**: Replace direct `state_service.update_execution_tokens()` calls in `stream.rs` with `batch_writer.send(DbWrite::TokenUpdate { ... })`.

---

## Phase 3: Smart Caching with Moka

### 3A. Provider configuration cache

**Problem**: `ProviderService` reads `providers.json` from disk on every agent invocation. This file changes rarely (only when user edits provider settings).

**File**: `gateway/src/services/providers.rs`

**Implementation**: Use existing `RwLock<Option<Vec<T>>>` pattern (same as AgentService) for consistency.

```rust
pub struct ProviderService {
    config_path: PathBuf,
    cache: tokio::sync::RwLock<Option<Vec<Provider>>>,
}

impl ProviderService {
    pub async fn list(&self) -> Result<Vec<Provider>, String> {
        {
            let cache = self.cache.read().await;
            if let Some(providers) = cache.as_ref() {
                return Ok(providers.clone());
            }
        }
        // Cache miss: read from disk
        let providers = self.load_from_disk()?;
        *self.cache.write().await = Some(providers.clone());
        Ok(providers)
    }

    pub async fn invalidate_cache(&self) {
        *self.cache.write().await = None;
    }

    // Call invalidate_cache() in create(), update(), delete(), set_default()
}
```

**Effort**: 2 hours. Copy pattern from `agents.rs`.

### 3B. MCP server configuration cache

**Problem**: `McpService` reads `mcps.json` on every agent execution.

**File**: `gateway/src/services/mcp.rs`

**Implementation**: Identical RwLock pattern. Invalidate on add/update/delete.

**Effort**: 1.5 hours.

### 3C. Settings cache

**Problem**: `SettingsService` reads `settings.json` every executor setup.

**File**: `gateway/src/services/settings.rs`

**Implementation**: Identical RwLock pattern. Invalidate on save.

**Effort**: 1 hour.

### 3D. Session state cache with Moka (for token aggregation fix)

**Problem**: Web session tokens always show 0. Need real-time aggregation without hammering SQLite.

**File**: `services/execution-state/src/service.rs`

**Two-part fix**:

**Part 1**: Add `aggregate_session_tokens_now()` that runs the SUM query without requiring session completion:
```rust
pub fn aggregate_session_tokens_now(&self, session_id: &str) -> Result<(i64, i64), String> {
    self.repo.get_execution_tokens_sum(session_id)
}
```

**Part 2**: Add Moka cache for session token lookups:
```rust
use moka::future::Cache;

pub struct StateService<D> {
    repo: StateRepository<D>,
    db: Arc<D>,
    token_cache: Cache<String, (i64, i64)>,  // session_id → (tokens_in, tokens_out)
}

impl<D: StateDbProvider> StateService<D> {
    pub fn new(db: Arc<D>) -> Self {
        Self {
            repo: StateRepository::new(),
            db,
            token_cache: Cache::builder()
                .max_capacity(1_000)
                .time_to_live(Duration::from_secs(5))  // 5s TTL
                .build(),
        }
    }

    pub async fn get_session_tokens(&self, session_id: &str) -> Result<(i64, i64), String> {
        if let Some(cached) = self.token_cache.get(session_id).await {
            return Ok(cached);
        }
        let tokens = self.aggregate_session_tokens_now(session_id)?;
        self.token_cache.insert(session_id.to_string(), tokens).await;
        Ok(tokens)
    }

    // Invalidate on token update
    pub fn update_execution_tokens(&self, ...) {
        // ... existing DB write ...
        self.token_cache.invalidate(session_id);
    }
}
```

**Part 3**: Call `update_session_tokens()` eagerly on every execution completion, not just session completion:
```rust
// In complete_execution() — gateway/src/execution/lifecycle.rs
pub async fn complete_execution(...) {
    // ... existing completion logic ...

    // Eagerly aggregate session tokens (even for web sessions)
    if let Err(e) = state_service.update_session_tokens(session_id) {
        tracing::warn!("Failed to aggregate session tokens: {}", e);
    }
}
```

**Effort**: 4 hours. Requires adding moka to execution-state Cargo.toml.

### 3E. Message history cache

**Problem**: `ConversationRepository.get_messages_for_conversation()` runs an expensive JOIN query at every execution start. For multi-turn conversations, this grows linearly.

**Strategy**: Cache recent conversation history in memory with moka:
```rust
pub struct ConversationRepository {
    db: Arc<DatabaseManager>,
    history_cache: Cache<String, Vec<Message>>,  // conversation_id → messages
}
```

**TTL**: 30 seconds (conversations change on every execution, but within a session burst, cache hits save repeated JOINs).

**Invalidation**: After `add_message()`.

**Effort**: 3 hours.

---

## Phase 4: Architecture Improvements (Medium-term)

### 4A. Decouple execution state from SQLite for active sessions

For sessions that are actively executing, keep state in memory and flush periodically:

```rust
pub struct ActiveSessionState {
    session: Session,
    executions: HashMap<String, Execution>,
    pending_writes: Vec<StateWrite>,
    last_flush: Instant,
}

pub struct InMemoryStateManager {
    active: moka::future::Cache<String, Arc<RwLock<ActiveSessionState>>>,
    db_service: Arc<StateService>,
}
```

**Benefit**: During an active execution, all state reads (session status, execution status, running check) hit memory, not SQLite. Writes are batched and flushed every 500ms.

### 4B. Event-driven token aggregation

Instead of aggregating tokens on session completion (which never happens for web sessions), use the EventBus:

```rust
// Listen for TokenUpdate events
event_bus.subscribe(|event| {
    if let GatewayEvent::TokenUpdate { session_id, tokens_in, tokens_out, .. } = event {
        // Update in-memory counter (atomic)
        // Periodically flush to SQLite
    }
});
```

### 4C. Streaming-first executor design

Long-term: Rewrite the executor loop to be truly streaming-first:
- Use `chat_stream()` everywhere (never `chat()`)
- Tool call arguments stream and parse incrementally
- Tool execution starts as soon as enough args are available (for simple tools)
- Multiple tool calls can potentially execute in parallel

---

## Phase 5: Gateway Crate Decomposition

The gateway crate is 73 files / ~15,675 lines with 10 major domains crammed into one crate. This creates:
- Slow compile times (any change recompiles everything)
- Tangled dependencies (execution logic depends on HTTP routing)
- Difficult testing (need full AppState for unit tests)
- Hard to reason about boundaries

### Current module breakdown

| Module | Lines | Responsibility |
|--------|-------|----------------|
| `websocket/` | 3,110 | WebSocket transport, session management |
| `http/` | 3,020 | HTTP API routes, request handlers |
| `execution/` | 2,073 | Executor lifecycle, delegation, streaming |
| `services/` | 1,913 | Runtime, providers, MCP, settings, agents, skills |
| `connectors/` | 1,498 | Discord, Telegram, Slack bridges |
| `events/` | 920 | EventBus, GatewayEvent types |
| `database/` | 860 | DatabaseManager, ConversationRepository |
| `hooks/` | 780 | HookRegistry, inbound triggers |
| `templates/` | 650 | System prompt assembly, shard injection |
| `cron/` | 480 | Scheduled agent triggers |
| `state.rs` | 490 | AppState (wires everything together) |

### Decomposition strategy (4 phases)

**Phase 5A: Extract foundation crates** (lowest risk, highest reuse)

| New Crate | Source | Why |
|-----------|--------|-----|
| `gateway-persistence` | `database/`, `events/` | DB manager, event bus, conversation repo — used by everything |
| `gateway-hooks` | `hooks/` | Self-contained, only depends on event bus |

These have the fewest outbound dependencies and are imported by other modules.

**Phase 5B: Extract execution engine** (highest value)

| New Crate | Source | Why |
|-----------|--------|-----|
| `gateway-execution` | `execution/` | Core agent lifecycle, delegation, streaming — the heart of the system |
| `gateway-templates` | `templates/` | Prompt assembly — only depends on agent/skill types |

The execution engine is the most critical module. Isolating it means:
- Can test executor without HTTP/WS stack
- Cleaner dependency on `agent-runtime` (no gateway coupling)
- Future: could run execution in a separate process

**Phase 5C: Extract transport layer** (largest by LOC)

| New Crate | Source | Why |
|-----------|--------|-----|
| `gateway-http` | `http/` | REST API handlers |
| `gateway-ws` | `websocket/` | WebSocket transport |

These are the largest modules but mostly leaf nodes — they consume execution results and serve them over different transports. Extracting them makes it possible to swap transports or add new ones (e.g., gRPC) without touching core logic.

**Phase 5D: Extract services and bridges**

| New Crate | Source | Why |
|-----------|--------|-----|
| `gateway-connectors` | `connectors/` | External platform bridges (Discord, Telegram, Slack) |
| `gateway-cron` | `cron/` | Scheduled triggers |
| `gateway-services` | `services/` | Config/service wrappers (or merge into relevant crates) |

### What remains in `gateway`

After decomposition, the gateway crate becomes a thin shell:
- `main.rs` / `server.rs` — starts the server, wires crates together
- `state.rs` — `AppState` struct that holds all service Arcs
- Re-exports for backward compatibility during migration

### Constraints

- **Incremental**: Each phase can be done independently. No big-bang refactor.
- **No API changes**: The HTTP/WS API stays identical. This is purely internal.
- **Test preservation**: Move tests with their code. `#[cfg(test)]` modules stay with their crate.
- **Dependency direction**: Foundation ← Execution ← Transport. Never reverse.

### When to do this

This is a **Week 6+** effort, after the performance fixes (Phases 1-4) stabilize. The gateway works today — it's just harder to maintain than it should be. Crate splitting improves developer velocity but doesn't directly impact user-facing performance.

**Exception**: If Phase 2 (SQLite changes) or Phase 1 (streaming) require touching many gateway files simultaneously, consider pulling Phase 5A (extract persistence) forward to reduce merge conflicts.

---

## Implementation Order

### Week 1: Streaming (Phase 1)
1. **1A**: Extend `chat_stream` trait with tools param (1 hour)
2. **1A**: Update OpenAI impl to pass tools (30 min)
3. **1A**: Refactor executor loop to use `chat_stream` with channel bridge (4-6 hours)
4. **1B**: Verify intermediate text streams automatically (testing)
5. Remove 5ms simulation code entirely

### Week 2: SQLite Foundation (Phase 2A-2B)
1. **2A**: Add WAL mode + pragmas to DatabaseManager (1 hour)
2. **2B**: Replace single Mutex with r2d2 connection pool (4-6 hours)
3. Update all `with_connection` call sites (mechanical)
4. Benchmark before/after with concurrent agent executions

### Week 3: Caching (Phase 3A-3C)
1. **3A**: Provider cache (2 hours)
2. **3B**: MCP cache (1.5 hours)
3. **3C**: Settings cache (1 hour)
4. **3D**: Session token aggregation fix with moka (4 hours)

### Week 4: Hot Path Optimization (Phase 2C)
1. **2C**: Implement BatchWriter with channel (4 hours)
2. Integrate into stream.rs (2 hours)
3. Verify no data loss on shutdown (graceful drain)

### Week 5: Architecture (Phase 4)
1. **4A**: In-memory state for active sessions
2. **4B**: Event-driven token aggregation
3. **4C**: Streaming-first executor redesign

### Week 6+: Gateway Decomposition (Phase 5)
1. **5A**: Extract `gateway-persistence` and `gateway-hooks` (2-3 days)
2. **5B**: Extract `gateway-execution` and `gateway-templates` (3-4 days)
3. **5C**: Extract `gateway-http` and `gateway-ws` (3-4 days)
4. **5D**: Extract `gateway-connectors`, `gateway-cron` (2 days)

---

## Dependencies

```
moka = { version = "0.12", features = ["future"] }  # Already in workspace
r2d2 = "0.8"                                         # New
r2d2_sqlite = "0.24"                                  # New
```

## Risk Assessment

| Change | Risk | Mitigation |
|--------|------|------------|
| Real streaming | Medium — changes core executor loop | Feature flag, keep `chat()` fallback |
| WAL mode | Low — standard SQLite best practice | Test with concurrent writes |
| Connection pool | Low-Medium — changes DB access pattern | `with_connection` API stays same |
| Batch writes | Medium — potential data loss on crash | Graceful shutdown + WAL durability |
| Moka caching | Low — read-through cache, DB is source of truth | TTL ensures freshness |

## Success Metrics

| Metric | Current | Target |
|--------|---------|--------|
| Time to first token | 500ms-2s+ | <200ms |
| Text visible during tool use | Never | Always |
| Concurrent agent throughput | 1 (serialized DB) | 4-8 |
| Config file reads per execution | 4-6 | 0 (cached) |
| Web session token accuracy | Always 0 | Real-time |
| P95 execution startup latency | ~50ms | <10ms |
