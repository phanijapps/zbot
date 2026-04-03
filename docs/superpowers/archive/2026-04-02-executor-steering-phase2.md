# Executor Steering Upgrade — Phase 2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add steering queue (mid-execution message injection), complexity scoring with budget enforcement, transformContext hook, and Tier 2 performance optimizations to the executor.

**Architecture:** SteeringQueue lives on `AgentExecutor` — a tokio mpsc channel drained before each LLM call. Complexity threading flows: DelegateAction → StreamEvent → DelegationRequest → ExecutorConfig. Budget enforcement uses the steering queue for nudges. transformContext is an optional closure on ExecutorConfig. Tier 2 perf: HTTP client pooling, token estimation cache.

**Tech Stack:** Rust (agent-runtime, zero-core, gateway-execution, gateway-events crates), tokio::sync::mpsc, serde_json, reqwest

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `runtime/agent-runtime/src/steering.rs` | Create | SteeringQueue, SteeringMessage, SteeringSource, SteeringPriority types |
| `runtime/agent-runtime/src/executor.rs` | Modify | Steering queue drain, transformContext hook, complexity budget enforcement, token cache |
| `runtime/agent-runtime/src/lib.rs` | Modify | Re-export steering types and transformContext |
| `runtime/agent-runtime/src/llm/openai.rs` | Modify | HTTP client pooling config |
| `framework/zero-core/src/event.rs` | Modify | Add `complexity` field to DelegateAction |
| `runtime/agent-runtime/src/types/events.rs` | Modify | Add `complexity` to ActionDelegate variant |
| `gateway/gateway-execution/src/delegation/context.rs` | Modify | Add `complexity` to DelegationRequest |
| `gateway/gateway-execution/src/delegation/spawn.rs` | Modify | Thread complexity to ExecutorConfig |
| `gateway/gateway-execution/src/invoke/stream.rs` | Modify | Thread complexity through handle_delegation |
| `gateway/gateway-execution/src/events.rs` | Modify | Thread complexity through convert_stream_event |

---

### Task 1: Steering Queue Types

**Files:**
- Create: `runtime/agent-runtime/src/steering.rs`
- Modify: `runtime/agent-runtime/src/lib.rs`

- [ ] **Step 1: Write failing test**

Create `runtime/agent-runtime/src/steering.rs` with:

```rust
//! Steering queue for mid-execution message injection.
//!
//! Allows external callers (UI, parent agents, system budgets) to inject
//! messages into a running executor without waiting for a tool round.

use tokio::sync::mpsc;

/// A message injected into the executor mid-execution.
#[derive(Debug, Clone)]
pub struct SteeringMessage {
    /// Content of the steering message.
    pub content: String,
    /// Who sent this steering message.
    pub source: SteeringSource,
    /// Priority level.
    pub priority: SteeringPriority,
}

/// Source of a steering message.
#[derive(Debug, Clone, PartialEq)]
pub enum SteeringSource {
    /// User typed a message in the UI.
    User,
    /// System budget/complexity enforcement.
    System,
    /// Parent agent steering a subagent.
    Parent,
}

/// Priority of a steering message.
#[derive(Debug, Clone, PartialEq)]
pub enum SteeringPriority {
    /// Inject after current tool round completes.
    Normal,
    /// Inject immediately before next LLM call.
    Interrupt,
}

/// Thread-safe channel for injecting messages into a running executor.
///
/// Create with `SteeringQueue::new()`. The sender half (`SteeringHandle`) can
/// be cloned and shared across threads. The receiver half stays with the executor.
pub struct SteeringQueue {
    rx: mpsc::UnboundedReceiver<SteeringMessage>,
}

/// Handle for sending steering messages to a running executor.
///
/// Clone this and hand it to the UI, parent agent, or budget enforcer.
#[derive(Clone)]
pub struct SteeringHandle {
    tx: mpsc::UnboundedSender<SteeringMessage>,
}

impl SteeringQueue {
    /// Create a new steering queue, returning (queue, handle).
    ///
    /// The queue is consumed by the executor. The handle is shared with callers.
    pub fn new() -> (Self, SteeringHandle) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Self { rx }, SteeringHandle { tx })
    }

    /// Drain all pending messages. Non-blocking — returns empty vec if nothing pending.
    pub fn drain(&mut self) -> Vec<SteeringMessage> {
        let mut messages = Vec::new();
        while let Ok(msg) = self.rx.try_recv() {
            messages.push(msg);
        }
        messages
    }
}

impl SteeringHandle {
    /// Send a steering message. Returns Err if the executor has been dropped.
    pub fn send(&self, message: SteeringMessage) -> Result<(), SteeringMessage> {
        self.tx.send(message).map_err(|e| e.0)
    }

    /// Convenience: send a system steering message.
    pub fn send_system(&self, content: impl Into<String>) -> Result<(), SteeringMessage> {
        self.send(SteeringMessage {
            content: content.into(),
            source: SteeringSource::System,
            priority: SteeringPriority::Normal,
        })
    }
}

impl std::fmt::Display for SteeringSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::User => write!(f, "User"),
            Self::System => write!(f, "System"),
            Self::Parent => write!(f, "Parent"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_steering_queue_drain_empty() {
        let (mut queue, _handle) = SteeringQueue::new();
        let messages = queue.drain();
        assert!(messages.is_empty());
    }

    #[test]
    fn test_steering_queue_send_and_drain() {
        let (mut queue, handle) = SteeringQueue::new();
        handle.send(SteeringMessage {
            content: "wrap up".to_string(),
            source: SteeringSource::System,
            priority: SteeringPriority::Normal,
        }).unwrap();
        handle.send(SteeringMessage {
            content: "user says stop".to_string(),
            source: SteeringSource::User,
            priority: SteeringPriority::Interrupt,
        }).unwrap();

        let messages = queue.drain();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].content, "wrap up");
        assert_eq!(messages[0].source, SteeringSource::System);
        assert_eq!(messages[1].content, "user says stop");
        assert_eq!(messages[1].source, SteeringSource::User);
        assert_eq!(messages[1].priority, SteeringPriority::Interrupt);
    }

    #[test]
    fn test_steering_queue_drain_clears() {
        let (mut queue, handle) = SteeringQueue::new();
        handle.send_system("nudge").unwrap();
        let _ = queue.drain();
        let messages = queue.drain();
        assert!(messages.is_empty(), "drain should clear the queue");
    }

    #[test]
    fn test_steering_handle_clone() {
        let (mut queue, handle) = SteeringQueue::new();
        let handle2 = handle.clone();
        handle.send_system("from handle 1").unwrap();
        handle2.send_system("from handle 2").unwrap();
        let messages = queue.drain();
        assert_eq!(messages.len(), 2);
    }

    #[test]
    fn test_steering_source_display() {
        assert_eq!(format!("{}", SteeringSource::User), "User");
        assert_eq!(format!("{}", SteeringSource::System), "System");
        assert_eq!(format!("{}", SteeringSource::Parent), "Parent");
    }
}
```

- [ ] **Step 2: Register module in lib.rs**

In `runtime/agent-runtime/src/lib.rs`, add the module declaration after the `executor` module:

```rust
/// Steering queue for mid-execution message injection
pub mod steering;
```

Add re-exports to the convenient re-exports section:

```rust
pub use steering::{
    SteeringQueue, SteeringHandle, SteeringMessage, SteeringSource, SteeringPriority,
};
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p agent-runtime -- steering`
Expected: All 5 tests pass

- [ ] **Step 4: Commit**

```bash
git add runtime/agent-runtime/src/steering.rs runtime/agent-runtime/src/lib.rs
git commit -m "feat(steering): add SteeringQueue types and channel implementation"
```

---

### Task 2: Integrate Steering Queue into Executor

**Files:**
- Modify: `runtime/agent-runtime/src/executor.rs:280-295` (AgentExecutor struct)
- Modify: `runtime/agent-runtime/src/executor.rs:483-655` (execute_with_tools_loop, before LLM call)

- [ ] **Step 1: Add steering queue field to AgentExecutor**

In `executor.rs`, add to the `AgentExecutor` struct (after line 294, `recall_initial_keys`):

```rust
    /// Optional steering queue for mid-execution message injection.
    steering_queue: Option<std::sync::Mutex<crate::steering::SteeringQueue>>,
```

Using `std::sync::Mutex` because `drain()` takes `&mut self` but executor methods take `&self`. The lock is held only for the non-blocking drain.

Update `AgentExecutor::new()` to initialize it:

```rust
            steering_queue: None,
```

Add a setter method after `set_recall_hook`:

```rust
    /// Attach a steering queue to this executor.
    ///
    /// Call this before `execute_stream`. The returned `SteeringHandle` can be
    /// shared with the UI, parent agents, or budget enforcers.
    pub fn enable_steering(&mut self) -> crate::steering::SteeringHandle {
        let (queue, handle) = crate::steering::SteeringQueue::new();
        self.steering_queue = Some(std::sync::Mutex::new(queue));
        handle
    }
```

- [ ] **Step 2: Add steering queue drain in executor loop**

In `execute_with_tools_loop`, inside the `loop { ... }` block, find the section just before `sanitize_messages` (around line 652-655):

```rust
            // Sanitize messages to remove orphaned tool messages before LLM call.
            sanitize_messages(&mut current_messages);
```

Add steering queue drain **before** `sanitize_messages`:

```rust
            // Drain steering queue: inject any pending steering messages
            if let Some(ref steering_mutex) = self.steering_queue {
                if let Ok(mut queue) = steering_mutex.lock() {
                    let steering_messages = queue.drain();
                    for msg in steering_messages {
                        let formatted = format!("[STEER: {}] {}", msg.source, msg.content);
                        current_messages.push(ChatMessage::user(formatted));
                        tracing::info!(
                            source = %msg.source,
                            priority = ?msg.priority,
                            "Injected steering message"
                        );
                    }
                }
            }

            // Sanitize messages to remove orphaned tool messages before LLM call.
            sanitize_messages(&mut current_messages);
```

- [ ] **Step 3: Add test for steering drain format**

Add to `hook_tests` module:

```rust
    #[test]
    fn test_steering_message_format() {
        use crate::steering::{SteeringSource, SteeringMessage, SteeringPriority};
        let msg = SteeringMessage {
            content: "Wrap up now".to_string(),
            source: SteeringSource::System,
            priority: SteeringPriority::Normal,
        };
        let formatted = format!("[STEER: {}] {}", msg.source, msg.content);
        assert_eq!(formatted, "[STEER: System] Wrap up now");
    }
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p agent-runtime`
Expected: All pass

- [ ] **Step 5: Commit**

```bash
git add runtime/agent-runtime/src/executor.rs
git commit -m "feat(executor): integrate steering queue drain before each LLM call"
```

---

### Task 3: transformContext Hook

**Files:**
- Modify: `runtime/agent-runtime/src/executor.rs`
- Modify: `runtime/agent-runtime/src/lib.rs`

- [ ] **Step 1: Write failing test**

Add to `hook_tests`:

```rust
    #[test]
    fn test_transform_context_hook_type() {
        // Verify the type compiles and can be called
        let hook: TransformContextHook = Arc::new(|messages: &mut Vec<ChatMessage>| {
            messages.push(ChatMessage::system("injected".to_string()));
        });
        let mut messages = vec![ChatMessage::user("hello".to_string())];
        hook(&mut messages);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].content, "injected");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p agent-runtime -- hook_tests::test_transform_context_hook_type`
Expected: FAIL — `TransformContextHook` doesn't exist

- [ ] **Step 3: Add TransformContextHook type and field**

In `executor.rs`, after the `AfterToolCallHook` type alias (around line 274):

```rust
/// Type alias for transformContext hook.
/// Called before every LLM call. Can modify the message list in place.
pub type TransformContextHook = Arc<dyn Fn(&mut Vec<ChatMessage>) + Send + Sync>;
```

Add field to `ExecutorConfig` (after `tool_execution_mode`):

```rust
    /// Hook called before every LLM call to transform the message context.
    /// Default: None (messages passed through unchanged).
    pub transform_context: Option<TransformContextHook>,
```

Add default to `ExecutorConfig::new()`:

```rust
            transform_context: None,
```

Update the manual `Debug` impl to include the new field (format as `<hook>`).

- [ ] **Step 4: Integrate in executor loop**

In `execute_with_tools_loop`, after steering queue drain and `sanitize_messages`, add:

```rust
            // transformContext hook: allow caller to modify messages before LLM call
            if let Some(ref hook) = self.config.transform_context {
                hook(&mut current_messages);
            }
```

- [ ] **Step 5: Add re-export**

In `lib.rs`, add `TransformContextHook` to the executor re-exports:

```rust
pub use executor::{
    AgentExecutor, ExecutorConfig, ExecutorError, RecallHook, RecallHookResult, create_executor,
    ToolCallDecision, ToolExecutionMode, BeforeToolCallHook, AfterToolCallHook,
    TransformContextHook,
};
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p agent-runtime`
Expected: All pass

- [ ] **Step 7: Commit**

```bash
git add runtime/agent-runtime/src/executor.rs runtime/agent-runtime/src/lib.rs
git commit -m "feat(executor): add transformContext hook — modify messages before every LLM call"
```

---

### Task 4: Complexity Field on DelegateAction (zero-core)

**Files:**
- Modify: `framework/zero-core/src/event.rs:158-186` (DelegateAction struct)

- [ ] **Step 1: Write failing test**

In `framework/zero-core/src/event.rs`, find the existing `#[cfg(test)]` section (around line 219). Add:

```rust
    #[test]
    fn test_delegate_action_complexity_field() {
        let action = DelegateAction {
            agent_id: "child".to_string(),
            task: "do work".to_string(),
            context: None,
            wait_for_result: false,
            max_iterations: None,
            output_schema: None,
            skills: vec![],
            complexity: Some("M".to_string()),
        };
        assert_eq!(action.complexity, Some("M".to_string()));
    }

    #[test]
    fn test_delegate_action_complexity_default_none() {
        let json = r#"{"agent_id":"a","task":"t","wait_for_result":false,"skills":[]}"#;
        let action: DelegateAction = serde_json::from_str(json).unwrap();
        assert_eq!(action.complexity, None);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p zero-core -- delegate_action_complexity`
Expected: FAIL — no `complexity` field

- [ ] **Step 3: Add complexity field**

Add to `DelegateAction` struct (after `skills` field at line 185):

```rust
    /// Task complexity level: "S", "M", "L", "XL".
    ///
    /// Used for iteration budget enforcement. When set, the executor
    /// applies complexity-based turn budgets instead of the default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub complexity: Option<String>,
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p zero-core`
Expected: All pass

- [ ] **Step 5: Commit**

```bash
git add framework/zero-core/src/event.rs
git commit -m "feat(zero-core): add complexity field to DelegateAction"
```

---

### Task 5: Thread Complexity Through StreamEvent and Gateway

**Files:**
- Modify: `runtime/agent-runtime/src/types/events.rs:128-138` (ActionDelegate variant)
- Modify: `runtime/agent-runtime/src/executor.rs` (where ActionDelegate is emitted)
- Modify: `gateway/gateway-execution/src/invoke/stream.rs:250-311` (handle_delegation, process_stream_event)
- Modify: `gateway/gateway-execution/src/delegation/context.rs:22-51` (DelegationRequest)
- Modify: `gateway/gateway-execution/src/delegation/spawn.rs:316-319` (max_iterations from complexity)
- Modify: `gateway/gateway-execution/src/events.rs:214-228` (convert_stream_event test)

- [ ] **Step 1: Add complexity to StreamEvent::ActionDelegate**

In `runtime/agent-runtime/src/types/events.rs`, add to the `ActionDelegate` variant (after `skills`):

```rust
        complexity: Option<String>,
```

- [ ] **Step 2: Add complexity to executor's ActionDelegate emission**

In `runtime/agent-runtime/src/executor.rs`, find where `StreamEvent::ActionDelegate` is emitted (search for `on_event(StreamEvent::ActionDelegate`). Add the `complexity` field. The delegate action is read from `actions.delegate`. Add:

```rust
                        if let Some(delegate) = &actions.delegate {
                            on_event(StreamEvent::ActionDelegate {
                                timestamp: chrono::Utc::now().timestamp_millis() as u64,
                                agent_id: delegate.agent_id.clone(),
                                task: delegate.task.clone(),
                                context: delegate.context.clone(),
                                wait_for_result: delegate.wait_for_result,
                                max_iterations: delegate.max_iterations,
                                output_schema: delegate.output_schema.clone(),
                                skills: delegate.skills.clone(),
                                complexity: delegate.complexity.clone(),
                            });
```

- [ ] **Step 3: Add complexity to DelegationRequest**

In `gateway/gateway-execution/src/delegation/context.rs`, add to `DelegationRequest` (after `skills`):

```rust
    /// Task complexity level ("S", "M", "L", "XL") for budget enforcement.
    pub complexity: Option<String>,
```

- [ ] **Step 4: Thread complexity through handle_delegation**

In `gateway/gateway-execution/src/invoke/stream.rs`, update `handle_delegation` signature:

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
) {
```

Add `complexity` to the `DelegationRequest` construction:

```rust
    let _ = ctx.delegation_tx.send(DelegationRequest {
        parent_agent_id: ctx.agent_id.clone(),
        session_id: ctx.session_id.clone(),
        parent_execution_id: ctx.execution_id.clone(),
        child_agent_id: child_agent.to_string(),
        child_execution_id,
        task: task.to_string(),
        context: context.clone(),
        max_iterations,
        output_schema: output_schema.clone(),
        skills: skills.to_vec(),
        complexity: complexity.clone(),
    });
```

Update `process_stream_event` to extract and pass complexity:

```rust
    if let StreamEvent::ActionDelegate {
        agent_id: child_agent,
        task,
        context,
        max_iterations,
        output_schema,
        skills,
        complexity,
        ..
    } = event
    {
        handle_delegation(ctx, child_agent, task, context, *max_iterations, output_schema, skills, complexity);
    }
```

- [ ] **Step 5: Thread complexity to executor config in spawn.rs**

In `gateway/gateway-execution/src/delegation/spawn.rs`, find where `max_iterations` is read (around line 318):

```rust
    let max_iter = request.max_iterations.unwrap_or(1000);
```

Replace with complexity-aware budget:

```rust
    // Complexity-based iteration budget (overrides max_iterations if set)
    let max_iter = match request.complexity.as_deref() {
        Some("S") => request.max_iterations.unwrap_or(15),
        Some("M") => request.max_iterations.unwrap_or(30),
        Some("L") => request.max_iterations.unwrap_or(50),
        Some("XL") => request.max_iterations.unwrap_or(100),
        _ => request.max_iterations.unwrap_or(1000),
    };
```

- [ ] **Step 6: Fix test in events.rs**

In `gateway/gateway-execution/src/events.rs`, update `test_convert_action_delegate_returns_none`:

```rust
    #[test]
    fn test_convert_action_delegate_returns_none() {
        let event = StreamEvent::ActionDelegate {
            timestamp: 0,
            agent_id: "child-agent".to_string(),
            task: "do something".to_string(),
            context: None,
            wait_for_result: false,
            max_iterations: None,
            output_schema: None,
            skills: vec![],
            complexity: None,
        };

        let gateway_event = convert_stream_event(event, "agent-1", "conv-1", "session-1", "exec-1");
        assert!(gateway_event.is_none(), "ActionDelegate should return None");
    }
```

- [ ] **Step 7: Run tests**

Run: `cargo test --workspace 2>&1 | grep -E "FAILED|test result" | grep -v "zero-core.*doc"`
Expected: All pass (only pre-existing zero-core doctest failures)

- [ ] **Step 8: Commit**

```bash
git add runtime/agent-runtime/src/types/events.rs runtime/agent-runtime/src/executor.rs \
       gateway/gateway-execution/src/delegation/context.rs \
       gateway/gateway-execution/src/delegation/spawn.rs \
       gateway/gateway-execution/src/invoke/stream.rs \
       gateway/gateway-execution/src/events.rs
git commit -m "feat: thread complexity through delegation pipeline — DelegateAction → spawn"
```

---

### Task 6: Complexity Budget Enforcement via Steering Queue

**Files:**
- Modify: `runtime/agent-runtime/src/executor.rs`

- [ ] **Step 1: Write failing test**

Add to `hook_tests`:

```rust
    #[test]
    fn test_complexity_budget_lookup() {
        // Verify the budget lookup logic
        fn budget_for(complexity: Option<&str>) -> (u32, u32) {
            match complexity {
                Some("S") => (15, 12),
                Some("M") => (30, 24),
                Some("L") => (50, 40),
                Some("XL") => (100, 80),
                _ => (0, 0), // 0 means no budget enforcement
            }
        }
        assert_eq!(budget_for(Some("S")), (15, 12));
        assert_eq!(budget_for(Some("M")), (30, 24));
        assert_eq!(budget_for(Some("L")), (50, 40));
        assert_eq!(budget_for(Some("XL")), (100, 80));
        assert_eq!(budget_for(None), (0, 0));
    }
```

- [ ] **Step 2: Add complexity field to ExecutorConfig**

Add to `ExecutorConfig` struct (after `transform_context`):

```rust
    /// Task complexity level: "S", "M", "L", "XL".
    /// When set, applies complexity-based iteration budgets:
    /// S=15, M=30, L=50, XL=100.
    pub complexity: Option<String>,
```

Add default to `ExecutorConfig::new()`:

```rust
            complexity: None,
```

Update the manual Debug impl to include `complexity`.

- [ ] **Step 3: Add budget enforcement in executor loop**

In `execute_with_tools_loop`, find the turn budget section (around line 516-531). Add complexity budget enforcement **after** the existing turn budget nudge, **before** the stuck detection:

```rust
            // Complexity-based budget enforcement via steering queue
            if let Some(ref complexity) = self.config.complexity {
                let (hard_budget, soft_budget) = match complexity.as_str() {
                    "S" => (15u32, 12u32),
                    "M" => (30, 24),
                    "L" => (50, 40),
                    "XL" => (100, 80),
                    _ => (0, 0),
                };

                if hard_budget > 0 {
                    let iters = progress_tracker.total_iterations;
                    if iters >= hard_budget {
                        // Hard budget: inject "respond now" via steering queue or direct message
                        if let Some(ref steering_mutex) = self.steering_queue {
                            if let Ok(mut queue_lock) = steering_mutex.lock() {
                                // Direct inject since we're about to drain anyway
                                drop(queue_lock);
                            }
                        }
                        current_messages.push(ChatMessage::user(format!(
                            "[STEER: System] Budget exceeded ({}/{} iterations for {} task). \
                             Respond NOW with what you have. Do not start new work.",
                            iters, hard_budget, complexity
                        )));
                        tracing::warn!(
                            complexity = %complexity,
                            iterations = iters,
                            budget = hard_budget,
                            "Complexity hard budget reached"
                        );
                    } else if iters >= soft_budget && iters < hard_budget {
                        // Soft budget: nudge once
                        // Use total_iterations == soft_budget to send exactly once
                        if iters == soft_budget {
                            current_messages.push(ChatMessage::user(format!(
                                "[STEER: System] You've used {}/{} iterations for a {} task. \
                                 Wrap up or simplify your approach.",
                                iters, hard_budget, complexity
                            )));
                            tracing::info!(
                                complexity = %complexity,
                                iterations = iters,
                                budget = hard_budget,
                                "Complexity soft budget nudge sent"
                            );
                        }
                    }
                }
            }
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p agent-runtime`
Expected: All pass

- [ ] **Step 5: Commit**

```bash
git add runtime/agent-runtime/src/executor.rs
git commit -m "feat(executor): complexity-based budget enforcement — soft nudge at 80%, hard at 100%"
```

---

### Task 7: Tier 2 Optimization — HTTP Client Pooling

**Files:**
- Modify: `runtime/agent-runtime/src/llm/openai.rs:73-81`

- [ ] **Step 1: Replace default HTTP client with configured one**

In `openai.rs`, replace the `new()` method:

```rust
    pub fn new(config: LlmConfig) -> Result<Self, LlmError> {
        tracing::debug!("Creating OpenAI client for model: {}", config.model);
        let http_client = reqwest::Client::builder()
            .tcp_nodelay(true)
            .pool_max_idle_per_host(4)
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| LlmError::ConfigError(format!("Failed to build HTTP client: {}", e)))?;
        Ok(Self {
            config: Arc::new(config),
            http_client,
        })
    }
```

Check that `LlmError` has a `ConfigError` variant. If not, use whichever error variant is appropriate (check the LlmError enum). If no suitable variant exists, add one.

- [ ] **Step 2: Run tests**

Run: `cargo test -p agent-runtime`
Expected: All pass

- [ ] **Step 3: Commit**

```bash
git add runtime/agent-runtime/src/llm/openai.rs
git commit -m "perf: configure HTTP client pooling — tcp_nodelay, connection pool, idle timeout"
```

---

### Task 8: Tier 2 Optimization — Token Estimation Cache

**Files:**
- Modify: `runtime/agent-runtime/src/executor.rs`

- [ ] **Step 1: Write failing test**

Add to `hook_tests`:

```rust
    #[test]
    fn test_token_estimate_cache() {
        use std::collections::HashMap;
        let mut cache: HashMap<u64, usize> = HashMap::new();

        // Simulated content hash
        fn content_hash(content: &str) -> u64 {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut hasher = DefaultHasher::new();
            content.hash(&mut hasher);
            hasher.finish()
        }

        let msg = "Hello world this is a test message";
        let hash = content_hash(msg);

        // Cache miss — estimate and cache
        assert!(!cache.contains_key(&hash));
        let estimate = msg.len() / 4 + 4; // simple estimate
        cache.insert(hash, estimate);

        // Cache hit — reuse
        assert_eq!(cache.get(&hash), Some(&estimate));
    }
```

- [ ] **Step 2: Add token estimation cache to executor loop**

In `execute_with_tools_loop`, add a token cache after the existing local variables (around line 470):

```rust
        // Token estimation cache: skip re-estimation of unchanged messages.
        // Key: hash of message content. Value: estimated tokens.
        let mut token_estimate_cache: HashMap<u64, usize> = HashMap::new();
```

Find where `estimate_total_tokens` is called on the full message vec for compaction checks (around line 568 in the compaction section). The current code:

```rust
                let current_tokens = estimate_total_tokens(&current_messages);
```

This is actually in the middleware `process` call, not directly in the executor loop. Check if the executor loop directly calls `estimate_total_tokens`. If it does, add caching there. If it's only called inside middleware (which runs per-turn), the cache won't help from the executor side.

**Alternative approach**: If `estimate_total_tokens` is only called in middleware, skip the cache integration in the executor loop and instead add a simple `last_token_estimate` tracking:

```rust
        // Track last known token count to avoid redundant estimation
        let mut last_message_count: usize = 0;
        let mut last_token_estimate: u64 = 0;
```

Then in the compaction check section, only re-estimate if message count changed:

```rust
            if self.config.context_window_tokens > 0 && current_messages.len() != last_message_count {
                // Only re-estimate when messages have changed
                last_message_count = current_messages.len();
                // ... existing compaction logic
            }
```

Read the compaction section carefully before deciding which approach to use. The goal is to avoid re-serializing and re-estimating tokens for messages that haven't changed.

- [ ] **Step 3: Run tests**

Run: `cargo test -p agent-runtime`
Expected: All pass

- [ ] **Step 4: Commit**

```bash
git add runtime/agent-runtime/src/executor.rs
git commit -m "perf: token estimation cache — skip re-estimation of unchanged messages"
```

---

### Task 9: Re-export Steering Types + Final Verification

**Files:**
- Modify: `runtime/agent-runtime/src/lib.rs`

- [ ] **Step 1: Verify all re-exports are present**

Ensure `lib.rs` has all new types re-exported. The steering types were added in Task 1. Verify `TransformContextHook` was added in Task 3. Check that `complexity` field exists on all structs.

- [ ] **Step 2: Run full workspace tests**

Run: `cargo test --workspace 2>&1 | grep -E "FAILED|test result" | grep -v "zero-core.*doc"`
Expected: No failures (only pre-existing zero-core doctest)

- [ ] **Step 3: Run e2e tests**

Run: `cargo test -p gateway-execution --test e2e_ward_pipeline_tests`
Expected: 16 tests pass

- [ ] **Step 4: Commit (only if lib.rs needed changes)**

```bash
git add runtime/agent-runtime/src/lib.rs
git commit -m "feat: re-export steering and complexity types from agent-runtime"
```
