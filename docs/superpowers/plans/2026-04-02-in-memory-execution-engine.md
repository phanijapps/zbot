# In-Memory Execution Engine — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace DB-mediated delegation with inline channel-based delegation. Root executor stays alive, receives subagent results via oneshot channel, continues immediately. JSONL persistence replaces conversation DB.

**Architecture:** Add `result_tx: Option<oneshot::Sender<String>>` to DelegationRequest. When the subagent completes, send result through channel. Root executor awaits the channel instead of exiting. ConversationStore wraps an in-memory Vec + async JSONL writer, replacing ConversationRepository for message storage.

**Tech Stack:** Rust (agent-runtime, gateway-execution), tokio::sync::oneshot, serde_json, JSONL files

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `gateway/gateway-execution/src/conversation_store.rs` | Create | In-memory vec + async JSONL writer |
| `gateway/gateway-execution/src/delegation/context.rs` | Modify | Add result_tx to DelegationRequest |
| `gateway/gateway-execution/src/delegation/spawn.rs` | Modify | Send result through channel on completion |
| `runtime/agent-runtime/src/executor.rs` | Modify | Await delegation result inline instead of exiting |
| `runtime/agent-runtime/src/types/events.rs` | Modify | Add DelegationResultChannel to ActionDelegate |
| `gateway/gateway-execution/src/invoke/stream.rs` | Modify | Attach result_tx to DelegationRequest |
| `gateway/gateway-execution/src/runner.rs` | Modify | Use ConversationStore, simplify continuation path |
| `gateway/gateway-execution/src/lib.rs` | Modify | Register new module |

---

### Task 1: Add result_tx to DelegationRequest

**Files:**
- Modify: `gateway/gateway-execution/src/delegation/context.rs`

- [ ] **Step 1: Add result channel field**

Add to `DelegationRequest` struct (after `complexity`):

```rust
    /// Channel to send the delegation result back to the waiting executor.
    /// When set, the spawn handler sends the result through this channel
    /// instead of (in addition to) the event bus continuation path.
    #[allow(dead_code)]
    pub result_tx: Option<tokio::sync::oneshot::Sender<String>>,
```

- [ ] **Step 2: Fix all DelegationRequest constructions**

Search for all places that construct `DelegationRequest`. Each one needs `result_tx: None` added. These are in:
- `gateway/gateway-execution/src/invoke/stream.rs` — `handle_delegation` function
- Any test files that construct DelegationRequest

- [ ] **Step 3: Remove Clone derive from DelegationRequest**

`oneshot::Sender` is not `Clone`. Remove `Clone` from `#[derive(Debug, Clone)]` on DelegationRequest. Then fix any code that clones it (the spawn handler might clone it — change to move/take instead).

Search for `request.clone()` in runner.rs and spawn.rs. The delegation handler in runner.rs clones the request at line ~295 (`spawn_with_notification`). Change to take ownership.

- [ ] **Step 4: Run tests**

Run: `cargo test -p gateway-execution`
Expected: All pass (result_tx is None everywhere — no behavior change)

- [ ] **Step 5: Commit**

```bash
git add gateway/gateway-execution/src/delegation/context.rs gateway/gateway-execution/src/invoke/stream.rs
git commit -m "feat: add result_tx channel to DelegationRequest for inline delegation"
```

---

### Task 2: Send Result Through Channel on Subagent Completion

**Files:**
- Modify: `gateway/gateway-execution/src/delegation/spawn.rs:589-681` (handle_execution_success)

- [ ] **Step 1: Extract and send through result_tx**

In `handle_execution_success`, the function already has access to the `response` string and `delegation_registry`. After the callback is sent (line 667-677), add:

```rust
    // Send result through direct channel if available (inline delegation)
    // The result_tx is stored on the DelegationRequest, which was moved into the spawn task.
    // We need to thread it through. For now, store it on DelegationContext.
```

Actually, the `result_tx` is on `DelegationRequest`, which is consumed by `spawn_delegated_agent`. We need to store it somewhere the completion handler can access it. The cleanest place: `DelegationRegistry`.

Change `DelegationContext` to include `result_tx`:

```rust
// In delegation/context.rs, add to DelegationContext:
pub struct DelegationContext {
    // ... existing fields ...
    /// Channel to send result back to waiting executor
    pub result_tx: Option<tokio::sync::oneshot::Sender<String>>,
}
```

In `spawn_rs`, when registering in the delegation_registry (around line 122), pass the result_tx from the request:

```rust
let result_tx = request.result_tx.take(); // take ownership from request

delegation_registry.register(
    child_execution_id.clone(),
    DelegationContext {
        // ... existing fields ...
        result_tx,
    },
);
```

Then in `handle_execution_success`, after getting delegation_ctx:

```rust
    // Send result through direct channel (inline delegation path)
    if let Some(ctx) = &delegation_ctx {
        if let Some(tx) = ctx.result_tx.take() {
            let _ = tx.send(response.to_string());
            tracing::info!(
                execution_id = %execution_id,
                "Sent delegation result through direct channel"
            );
        }
    }
```

Wait — `delegation_ctx` is obtained via `delegation_registry.get(execution_id)` which returns an owned `Option<DelegationContext>`. The `result_tx` needs to be taken out, not just referenced. Check if `DelegationRegistry::get` returns ownership or a reference.

Read the DelegationRegistry implementation to understand the API, then implement accordingly.

- [ ] **Step 2: Run tests**

Run: `cargo test -p gateway-execution`
Expected: All pass

- [ ] **Step 3: Commit**

```bash
git add gateway/gateway-execution/src/delegation/spawn.rs gateway/gateway-execution/src/delegation/context.rs
git commit -m "feat: send delegation result through oneshot channel on subagent completion"
```

---

### Task 3: Executor Awaits Delegation Result Inline

**Files:**
- Modify: `runtime/agent-runtime/src/executor.rs`

This is the core change. When the executor detects a delegation action, instead of setting `stopped_for_delegation = true` and breaking the loop, it **pauses** and waits for the result on a channel.

- [ ] **Step 1: Add delegation channel infrastructure to executor**

The executor needs a way to create a oneshot channel and make the sender available to the `on_event` callback. The callback is `FnMut(StreamEvent)` — it can't return values. But it can write to shared state.

Add to `ExecutorConfig`:

```rust
    /// Sender for delegation results. Set by the executor before each delegation.
    /// The on_event callback reads this and attaches it to the DelegationRequest.
    /// Using Arc<Mutex<Option<>>> so the callback can take it.
    pub delegation_result_sender: Arc<std::sync::Mutex<Option<tokio::sync::oneshot::Sender<String>>>>,
```

Wait — this mixes runtime concerns with config. Better: add it to the executor struct directly, or use the shared tool context.

**Best approach:** Use the existing `shared_tool_context` state mechanism. The executor stores the sender in the tool context state (as a special non-serializable value). The `on_event` callback (which has access to the StreamContext, which can reach the tool context) extracts it.

Actually, the `on_event` callback is a closure in the runner/spawn code — it doesn't have direct access to the tool context. The simplest approach:

**Use an Arc<Mutex<Option<oneshot::Sender>>> passed to the executor and shared with the on_event callback via closure capture.**

In `execute_with_tools_loop`, before the tool processing:

```rust
// Create delegation result channel holder — shared between executor and on_event callback
let delegation_result_tx: Arc<std::sync::Mutex<Option<tokio::sync::oneshot::Sender<String>>>> =
    Arc::new(std::sync::Mutex::new(None));
```

But the `on_event` callback is defined OUTSIDE the executor (in runner.rs). The executor calls `on_event(StreamEvent::ActionDelegate { ... })` and the callback is supposed to read the sender from... somewhere.

**The cleanest solution:** Make the executor emit a NEW event type that includes the sender. Or, add the sender to the StreamEvent::ActionDelegate variant.

Add to `StreamEvent::ActionDelegate`:
```rust
    /// Channel for inline delegation — executor awaits this for the result.
    /// Set by the executor, consumed by the gateway's delegation handler.
    #[serde(skip)]
    delegation_result_tx: Option<Arc<std::sync::Mutex<Option<tokio::sync::oneshot::Sender<String>>>>>,
```

No — StreamEvent derives Serialize/Deserialize. Can't put channels in there.

**Final approach: Use a separate side-channel.**

The executor creates a `oneshot::channel()` per delegation. It stores the `Sender` in an `Arc<Mutex<Option<Sender>>>` that's shared with the runner via `ExecutorConfig` or a new field on `AgentExecutor`.

Add to `AgentExecutor`:
```rust
    /// Holder for inline delegation result channel.
    /// The executor sets the Sender before emitting ActionDelegate.
    /// The gateway's on_event callback takes the Sender and attaches it to DelegationRequest.
    /// The executor then awaits the Receiver.
    delegation_tx_holder: Arc<std::sync::Mutex<Option<tokio::sync::oneshot::Sender<String>>>>,
```

Initialize in `AgentExecutor::new()`:
```rust
    delegation_tx_holder: Arc::new(std::sync::Mutex::new(None)),
```

Add a public getter:
```rust
    pub fn delegation_tx_holder(&self) -> Arc<std::sync::Mutex<Option<tokio::sync::oneshot::Sender<String>>>> {
        self.delegation_tx_holder.clone()
    }
```

In `execute_with_tools_loop`, when delegation is detected:

```rust
    if let Some(delegate) = &actions.delegate {
        // Create inline delegation channel
        let (result_tx, result_rx) = tokio::sync::oneshot::channel::<String>();

        // Store sender where the on_event callback can find it
        {
            let mut holder = self.delegation_tx_holder.lock().unwrap();
            *holder = Some(result_tx);
        }

        // Emit ActionDelegate — the callback will take the sender from holder
        on_event(StreamEvent::ActionDelegate { ... });

        // Wait for subagent result (with timeout)
        tracing::info!("Executor pausing for inline delegation to {}", delegate.agent_id);
        match tokio::time::timeout(
            std::time::Duration::from_secs(600), // 10 min timeout
            result_rx,
        ).await {
            Ok(Ok(result)) => {
                // Subagent completed — inject result as user message
                current_messages.push(ChatMessage::user(format!(
                    "[Delegation result from {}]\n{}", delegate.agent_id, result
                )));
                tracing::info!("Delegation result received from {}", delegate.agent_id);
                // Continue the loop — DO NOT break
            }
            Ok(Err(_)) => {
                current_messages.push(ChatMessage::user(
                    "[Delegation failed — subagent channel closed]".to_string()
                ));
            }
            Err(_) => {
                current_messages.push(ChatMessage::user(
                    "[Delegation timed out after 10 minutes]".to_string()
                ));
            }
        }
        // Remove the stopped_for_delegation = true — we no longer break
    }
```

- [ ] **Step 2: Update on_event callback in runner.rs to extract sender**

In `runner.rs`, inside `spawn_execution_task` (the function that sets up root's execution), the `on_event` closure needs to take the sender from the holder and attach it to the DelegationRequest.

The closure already processes `StreamEvent::ActionDelegate` via `process_stream_event` which calls `handle_delegation`. We need `handle_delegation` to receive the sender.

In `handle_delegation` (stream.rs), add a parameter:
```rust
pub fn handle_delegation(
    ctx: &StreamContext,
    child_agent: &str,
    task: &str,
    context: &Option<serde_json::Value>,
    max_iterations: Option<u32>,
    output_schema: &Option<serde_json::Value>,
    skills: &[String],
    complexity: &Option<String>,
    result_tx: Option<tokio::sync::oneshot::Sender<String>>,  // NEW
) {
    // ... existing code ...
    let _ = ctx.delegation_tx.send(DelegationRequest {
        // ... existing fields ...
        result_tx,  // Attach the channel
    });
}
```

In `process_stream_event`, pass the sender from the holder:
```rust
if let StreamEvent::ActionDelegate { ... } = event {
    // Take the sender from the executor's holder
    let result_tx = delegation_tx_holder.lock().unwrap().take();
    handle_delegation(ctx, ..., result_tx);
}
```

The `delegation_tx_holder` needs to be accessible in the callback closure. Pass it when constructing the closure in runner.rs.

- [ ] **Step 3: Remove stopped_for_delegation and continuation for inline delegations**

When result_tx is set (inline delegation), the executor doesn't need to break. Remove `stopped_for_delegation = true` from the delegation path. The executor continues the loop after receiving the result.

The continuation handler still exists as a FALLBACK for cases where result_tx is None (e.g., old code paths, crash recovery). But the primary path is now inline.

- [ ] **Step 4: Run tests**

Run: `cargo test -p agent-runtime && cargo test -p gateway-execution`
Expected: All pass

- [ ] **Step 5: Commit**

```bash
git add runtime/agent-runtime/src/executor.rs gateway/gateway-execution/src/invoke/stream.rs gateway/gateway-execution/src/runner.rs
git commit -m "feat: inline delegation — executor awaits subagent result via oneshot channel, no exit/restart"
```

---

### Task 4: ConversationStore — In-Memory Vec + Async JSONL

**Files:**
- Create: `gateway/gateway-execution/src/conversation_store.rs`
- Modify: `gateway/gateway-execution/src/lib.rs`

- [ ] **Step 1: Create ConversationStore**

```rust
//! In-memory conversation store with async JSONL persistence.
//!
//! Messages live in a Vec (instant read/write). JSONL file is the durability layer.
//! The DB is never used for message content — only session metadata.

use agent_runtime::ChatMessage;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;
use tokio::io::AsyncWriteExt;

/// In-memory conversation store with async JSONL persistence.
pub struct ConversationStore {
    messages: Vec<ChatMessage>,
    jsonl_path: PathBuf,
    writer_tx: mpsc::UnboundedSender<String>,
}

impl ConversationStore {
    /// Create a new store for a session.
    pub fn new(session_id: &str, data_dir: &Path) -> Self {
        let conversations_dir = data_dir.join("data").join("conversations");
        let _ = std::fs::create_dir_all(&conversations_dir);
        let jsonl_path = conversations_dir.join(format!("{}.jsonl", session_id));

        let (writer_tx, writer_rx) = mpsc::unbounded_channel::<String>();
        let path = jsonl_path.clone();
        tokio::spawn(async move {
            Self::background_writer(path, writer_rx).await;
        });

        Self {
            messages: Vec::new(),
            jsonl_path,
            writer_tx,
        }
    }

    /// Load from existing JSONL (crash recovery / session resume).
    pub fn load(session_id: &str, data_dir: &Path) -> Result<Self, String> {
        let conversations_dir = data_dir.join("data").join("conversations");
        let jsonl_path = conversations_dir.join(format!("{}.jsonl", session_id));

        let mut messages = Vec::new();
        if jsonl_path.exists() {
            let content = std::fs::read_to_string(&jsonl_path)
                .map_err(|e| format!("Failed to read JSONL: {}", e))?;
            for line in content.lines() {
                if let Ok(msg) = serde_json::from_str::<ChatMessage>(line) {
                    messages.push(msg);
                }
            }
        }

        let (writer_tx, writer_rx) = mpsc::unbounded_channel::<String>();
        let path = jsonl_path.clone();
        tokio::spawn(async move {
            Self::background_writer(path, writer_rx).await;
        });

        Ok(Self {
            messages,
            jsonl_path,
            writer_tx,
        })
    }

    /// Append a message — instant in-memory, async file write.
    pub fn append(&mut self, message: ChatMessage) {
        if let Ok(json) = serde_json::to_string(&message) {
            let _ = self.writer_tx.send(json);
        }
        self.messages.push(message);
    }

    /// Get all messages.
    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    /// Get message count.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Background JSONL writer task.
    async fn background_writer(path: PathBuf, mut rx: mpsc::UnboundedReceiver<String>) {
        let mut file = match tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await
        {
            Ok(f) => f,
            Err(e) => {
                tracing::error!("Failed to open JSONL file {:?}: {}", path, e);
                return;
            }
        };

        while let Some(json_line) = rx.recv().await {
            let line = format!("{}\n", json_line);
            if let Err(e) = file.write_all(line.as_bytes()).await {
                tracing::warn!("Failed to write to JSONL: {}", e);
            }
            // Flush periodically (every message for now — optimize later)
            let _ = file.flush().await;
        }
    }
}
```

- [ ] **Step 2: Register module**

In `gateway/gateway-execution/src/lib.rs`, add:
```rust
pub mod conversation_store;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p gateway-execution`
Expected: All pass

- [ ] **Step 4: Commit**

```bash
git add gateway/gateway-execution/src/conversation_store.rs gateway/gateway-execution/src/lib.rs
git commit -m "feat: ConversationStore — in-memory vec with async JSONL persistence"
```

---

### Task 5: Wire ConversationStore into Runner (dual-write)

**Files:**
- Modify: `gateway/gateway-execution/src/runner.rs`

- [ ] **Step 1: Add ConversationStore to runner alongside existing ConversationRepository**

This is the dual-write phase. Both the old DB path and new JSONL path are written to. Reading switches to in-memory for active sessions.

Add `ConversationStore` as a per-session cache in the runner. Use a `HashMap<String, ConversationStore>` wrapped in `Arc<RwLock<>>`:

```rust
// In AgentRunner struct, add:
    conversation_stores: Arc<tokio::sync::RwLock<HashMap<String, ConversationStore>>>,
```

Initialize in constructor:
```rust
    conversation_stores: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
```

When a new session starts (in `invoke_with_callback`), create a ConversationStore:
```rust
    let store = ConversationStore::new(&session_id, &self.paths.vault_dir());
    self.conversation_stores.write().await.insert(session_id.clone(), store);
```

For continuations, reuse the existing store (it's already in memory with all messages).

- [ ] **Step 2: Dual-write messages to ConversationStore**

In `spawn_execution_task` (runner.rs), alongside `batch_writer.session_message(...)`, also append to the ConversationStore. This requires threading the stores HashMap into the spawn closure.

This is complex plumbing — the exact changes depend on how many call sites there are. Focus on the root's execution path first, not subagents.

- [ ] **Step 3: Run tests**

Run: `cargo test -p gateway-execution`
Expected: All pass

- [ ] **Step 4: Commit**

```bash
git add gateway/gateway-execution/src/runner.rs
git commit -m "feat: dual-write conversation messages to ConversationStore + DB"
```

---

### Task 6: Final Verification

- [ ] **Step 1: Run full workspace tests**

Run: `cargo test --workspace 2>&1 | grep -E "FAILED|test result" | grep -v "zero-core.*doc"`

- [ ] **Step 2: Build and test with real session**

Run: `cargo build`

Test with: "Perform a comprehensive analysis of NVDA"

Verify:
- Root delegates to planner → gets result INLINE (no continuation)
- Root delegates Step 1 → gets result INLINE
- Each delegation is sub-second handoff (no 15-30s overhead)
- JSONL file created in `data/conversations/`
- Session completes successfully
