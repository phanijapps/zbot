# Pending Tasks & Nice-to-Haves

This file tracks tasks that are out of scope or blocked, with notes on previous attempts.

---

## Nice-to-Have: Real-Time Streaming from LLM API

**Status:** Blocked - Technical limitations with current architecture

### Current Implementation
- Uses non-streaming `chat()` API
- Waits for complete response from LLM
- Simulates streaming by emitting tokens character-by-character with 5ms delay
- Works correctly but has fake streaming feel

### Desired Implementation
- Use `chat_stream()` API to emit tokens as they arrive from the LLM
- True real-time streaming effect
- Lower perceived latency

### Why It Failed

#### Attempt 1: Direct `chat_stream()` with callback
```rust
let response = self.llm_client.chat_stream(
    current_messages.clone(),
    tools_schema.clone(),
    |token| { /* emit token */ }
).await?;
```

**Error:** The `chat_stream()` callback requires `Fn + Send + Sync` but the executor's `on_event` callback is `FnMut`, which is not `Send`.

#### Attempt 2: Arc<Mutex<dyn FnMut + Send + Sync>> wrapper
```rust
let callback = Arc::new(Mutex::new(|event| {
    on_event(event);
}));
```

**Error:** `FnMut` cannot be made thread-safe (`Send + Sync`) because it captures mutable state.

#### Attempt 3: mpsc channel for token forwarding
```rust
let (token_tx, mut token_rx) = mpsc::unbounded_channel();
// Spawn task to receive tokens and emit events
tokio::spawn(async move {
    while let Some(token) = token_rx.recv().await {
        on_event(StreamEvent::Token { ... });
    }
});
```

**Error:** `use of moved value: token_tx` - the channel sender was moved into the spawn task but still needed for the stream callback.

#### Attempt 4: TokenCollector struct with Arc<Mutex<Vec<Token>>>
```rust
struct TokenCollector {
    tokens: Arc<Mutex<Vec<String>>>,
}
```

**Error:** Trait bound conflicts - `FnMut + Send + Sync` cannot be satisfied simultaneously for callbacks that capture mutable state.

### Root Cause

The core issue is the **tool calling loop** in `execute_with_tools_loop()`:

1. LLM is called in a loop (max 10 iterations)
2. Each iteration may produce tool calls
3. Tool results are added to conversation
4. LLM is called again with updated conversation

This requires the callback (`on_event`) to be:
- `FnMut` - to emit multiple events during execution
- Passed by mutable reference across loop iterations
- Used after each LLM call and each tool execution

But `chat_stream()` requires a callback that is:
- `Fn + Send + Sync` - thread-safe for concurrent token processing
- Cannot capture mutable state

### Potential Solutions

#### Option 1: Refactor to streaming-first architecture
- Change `execute_with_tools_loop` to use streaming throughout
- Accumulate stream events in a channel
- Process events in a separate task
- **Complexity:** High - requires redesigning the entire execution flow

#### Option 2: Hybrid approach
- Use `chat_stream()` for final response (no tool calls)
- Fall back to `chat()` when tool calls are expected
- **Complexity:** Medium - need to predict if tools will be called

#### Option 3: Keep simulated streaming
- Current approach: 5ms delay between characters
- Works reliably, no complexity
- **Trade-off:** Not true real-time, but visually acceptable

### Recommendation

Stick with **simulated streaming** for now. The current implementation:
- Works correctly with tool calling
- Provides visual streaming effect
- Has no technical debt or complexity
- Can be revisited if/when OpenAI SDK provides better streaming support

### References
- `src-tauri/src/domains/agent_runtime/executor.rs:212-337` - execute_with_tools_loop
- `src-tauri/src/domains/agent_runtime/llm.rs:118-127` - chat_stream trait definition
- Conversation history: Summary from 2025-01-15 session

---

## Other Pending Tasks

*Add new tasks here as they are identified.*
