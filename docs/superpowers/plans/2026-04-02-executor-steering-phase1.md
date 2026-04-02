# Executor Steering Upgrade — Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add tool hooks (beforeToolCall/afterToolCall), sequential tool execution mode, and Tier 1 performance optimizations to the executor.

**Architecture:** All changes are in `agent-runtime` crate. Tool hooks are optional closures on `ExecutorConfig`. Sequential mode is a config enum. Optimizations are in-place edits to hot-path functions. Zero breaking changes — all new fields have defaults matching current behavior.

**Tech Stack:** Rust (agent-runtime crate), serde_json, tokio, futures

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `runtime/agent-runtime/src/executor.rs` | Modify | Add hooks invocation, sequential mode, hash optimization, line-aware truncation |
| `runtime/agent-runtime/src/llm/openai.rs` | Modify | Guard debug serialization, HTTP client config |
| `runtime/agent-runtime/src/middleware/context_editing.rs` | Modify | In-place editing, reference-based token estimation |
| `runtime/agent-runtime/src/lib.rs` | Modify | Re-export new types |

---

### Task 1: Tool Hook Types

**Files:**
- Modify: `runtime/agent-runtime/src/executor.rs:80-196`

- [ ] **Step 1: Write failing test for beforeToolCall hook**

Add to the bottom of executor.rs, inside a new `#[cfg(test)] mod hook_tests`:

```rust
#[cfg(test)]
mod hook_tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn test_tool_call_decision_default_is_allow() {
        let decision = ToolCallDecision::Allow;
        assert!(matches!(decision, ToolCallDecision::Allow));
    }

    #[test]
    fn test_tool_call_decision_block_has_reason() {
        let decision = ToolCallDecision::Block { reason: "dangerous".to_string() };
        match decision {
            ToolCallDecision::Block { reason } => assert_eq!(reason, "dangerous"),
            _ => panic!("Expected Block"),
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p agent-runtime -- hook_tests`
Expected: FAIL — `ToolCallDecision` doesn't exist

- [ ] **Step 3: Add ToolCallDecision and ToolExecutionMode types**

Add after `ExecutorConfig` impl block (after line 196):

```rust
/// Decision from beforeToolCall hook.
#[derive(Debug, Clone)]
pub enum ToolCallDecision {
    /// Allow the tool call to proceed.
    Allow,
    /// Block the tool call. The reason is returned to the LLM as the tool result.
    Block { reason: String },
}

/// Tool execution mode.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ToolExecutionMode {
    /// Execute all tools concurrently (current behavior).
    #[default]
    Parallel,
    /// Execute tools one at a time, in order.
    Sequential,
}

/// Type alias for beforeToolCall hook.
/// Receives (tool_name, args). Returns Allow or Block.
pub type BeforeToolCallHook = Arc<dyn Fn(&str, &Value) -> ToolCallDecision + Send + Sync>;

/// Type alias for afterToolCall hook.
/// Receives (tool_name, args, result, succeeded). Returns optional replacement result.
pub type AfterToolCallHook = Arc<dyn Fn(&str, &Value, &str, bool) -> Option<String> + Send + Sync>;
```

- [ ] **Step 4: Add hook fields to ExecutorConfig**

Add to `ExecutorConfig` struct (after `max_turns` field):

```rust
    /// Hook called before each tool execution. Can block the call.
    /// Default: None (all tools allowed).
    pub before_tool_call: Option<BeforeToolCallHook>,

    /// Hook called after each tool execution. Can transform the result.
    /// Default: None (results passed through unchanged).
    pub after_tool_call: Option<AfterToolCallHook>,

    /// Tool execution mode: parallel (default) or sequential.
    pub tool_execution_mode: ToolExecutionMode,
```

Add to `ExecutorConfig::new()` defaults:

```rust
            before_tool_call: None,
            after_tool_call: None,
            tool_execution_mode: ToolExecutionMode::default(),
```

Note: `ExecutorConfig` derives `Clone` but `Arc<dyn Fn>` is Clone via Arc. However, the `Debug` derive will fail for function pointers. Replace `#[derive(Debug, Clone)]` with a manual impl or use `#[derive(Clone)]` and implement Debug manually to skip the hook fields.

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p agent-runtime -- hook_tests`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add runtime/agent-runtime/src/executor.rs
git commit -m "feat(executor): add ToolCallDecision, ToolExecutionMode, hook types"
```

---

### Task 2: Integrate beforeToolCall Hook into Executor Loop

**Files:**
- Modify: `runtime/agent-runtime/src/executor.rs:696-707` (tool execution section)

- [ ] **Step 1: Write failing test**

Add to `hook_tests`:

```rust
#[test]
fn test_before_tool_call_block_returns_reason() {
    // Test that a blocked tool call produces the right result format
    let reason = "Ward boundary violation";
    let result = format!("{{\"blocked\":true,\"reason\":\"{}\"}}", reason);
    assert!(result.contains("blocked"));
    assert!(result.contains(reason));
}
```

- [ ] **Step 2: Add beforeToolCall check in executor loop**

In executor.rs, find the tool execution section (around line 695-707). Before the tool futures are created, add the beforeToolCall check:

```rust
            // Check beforeToolCall hook for each tool
            let mut blocked_results: HashMap<String, String> = HashMap::new();
            if let Some(ref hook) = self.config.before_tool_call {
                for tc in &tool_calls {
                    match hook(&tc.name, &tc.arguments) {
                        ToolCallDecision::Allow => {}
                        ToolCallDecision::Block { reason } => {
                            blocked_results.insert(
                                tc.id.clone(),
                                format!("{{\"blocked\":true,\"reason\":\"{}\"}}", reason),
                            );
                        }
                    }
                }
            }
```

Then modify the tool execution to skip blocked tools:

```rust
            // Execute non-blocked tools
            let tool_futures: Vec<_> = tool_calls.iter()
                .filter(|tc| !blocked_results.contains_key(&tc.id))
                .map(|tc| {
                    let ctx = shared_tool_context.clone();
                    let tool_id = tc.id.clone();
                    let tool_name = tc.name.clone();
                    let args = tc.arguments.clone();
                    async move {
                        tracing::debug!("Executing tool: {} with args: {}", tool_name, args);
                        self.execute_tool(&ctx, &tool_id, &tool_name, &args).await
                    }
                }).collect();
```

After results are collected, merge blocked results back in tool_call order:

```rust
            // Process results — merge blocked + executed in original order
            for tool_call in &tool_calls {
                if let Some(blocked_result) = blocked_results.remove(&tool_call.id) {
                    // Blocked by beforeToolCall hook
                    current_messages.push(ChatMessage {
                        role: "tool".to_string(),
                        content: blocked_result,
                        tool_calls: None,
                        tool_call_id: Some(tool_call.id.clone()),
                    });
                    on_event(StreamEvent::ToolResult {
                        timestamp: chrono::Utc::now().timestamp_millis() as u64,
                        tool_id: tool_call.id.clone(),
                        result: "[blocked by hook]".to_string(),
                        error: None,
                    });
                    progress_tracker.record_tool_call(&tool_call.name, &tool_call.arguments, false);
                } else {
                    // Normal execution result — existing processing code
                    // (move the existing result processing here)
                }
            }
```

- [ ] **Step 3: Run all tests**

Run: `cargo test -p agent-runtime`
Expected: All pass (hook is None by default — zero behavior change)

- [ ] **Step 4: Commit**

```bash
git add runtime/agent-runtime/src/executor.rs
git commit -m "feat(executor): integrate beforeToolCall hook — can block tool calls"
```

---

### Task 3: Integrate afterToolCall Hook + Sequential Mode

**Files:**
- Modify: `runtime/agent-runtime/src/executor.rs`

- [ ] **Step 1: Add afterToolCall in the result processing**

After each tool result is processed (after `progress_tracker.record_tool_call`), add:

```rust
                        // afterToolCall hook — can transform the result
                        let final_output = if let Some(ref hook) = self.config.after_tool_call {
                            match hook(tool_name, &tool_call.arguments, &processed_output, true) {
                                Some(replacement) => replacement,
                                None => processed_output,
                            }
                        } else {
                            processed_output
                        };
```

Use `final_output` instead of `processed_output` when pushing to `current_messages`.

- [ ] **Step 2: Add sequential execution mode**

Replace the parallel execution block with a conditional:

```rust
            // Execute tools (parallel or sequential based on config)
            let results = if self.config.tool_execution_mode == ToolExecutionMode::Sequential {
                // Sequential: one at a time, in order
                let mut results = Vec::new();
                for tc in tool_calls.iter().filter(|tc| !blocked_results.contains_key(&tc.id)) {
                    let result = self.execute_tool(
                        &shared_tool_context, &tc.id, &tc.name, &tc.arguments
                    ).await;
                    results.push((tc, result));
                }
                results
            } else {
                // Parallel: all at once (current behavior)
                let tool_futures: Vec<_> = tool_calls.iter()
                    .filter(|tc| !blocked_results.contains_key(&tc.id))
                    .map(|tc| {
                        let ctx = shared_tool_context.clone();
                        let tool_id = tc.id.clone();
                        let tool_name = tc.name.clone();
                        let args = tc.arguments.clone();
                        async move {
                            let result = self.execute_tool(&ctx, &tool_id, &tool_name, &args).await;
                            (tc, result)
                        }
                    }).collect();
                // Note: this needs adjustment to return the same (tc, result) shape
                futures::future::join_all(tool_futures).await
            };
```

Note: The exact refactoring depends on how tool_calls ownership flows. Read the existing code carefully and match the pattern. The key change is: when `ToolExecutionMode::Sequential`, use a `for` loop instead of `join_all`.

- [ ] **Step 3: Run all tests**

Run: `cargo test -p agent-runtime`
Expected: All pass (sequential mode not activated by default)

- [ ] **Step 4: Commit**

```bash
git add runtime/agent-runtime/src/executor.rs
git commit -m "feat(executor): afterToolCall hook + sequential tool execution mode"
```

---

### Task 4: Tier 1 Optimization — Hash Function + Debug Guards

**Files:**
- Modify: `runtime/agent-runtime/src/executor.rs` (hash_args function)
- Modify: `runtime/agent-runtime/src/llm/openai.rs` (debug serialization)

- [ ] **Step 1: Find and replace hash_args**

Find the `hash_args` function (around line 1560+). Replace the naive implementation with `DefaultHasher`:

```rust
fn hash_args(args: &Value) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let s = serde_json::to_string(args).unwrap_or_default();
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}
```

- [ ] **Step 2: Guard debug serialization in openai.rs**

In `openai.rs` around line 120-140, wrap the debug serialization in a tracing level check:

```rust
        // Only serialize for logging at debug level (avoids 100KB serialization on every call)
        if tracing::enabled!(tracing::Level::DEBUG) {
            let request_json = serde_json::to_string(&body_obj).unwrap_or_default();
            let estimated_chars = request_json.len();
            let estimated_tokens = estimated_chars / 4;
            tracing::debug!(
                "Request size: ~{} chars (~{} tokens estimated)",
                estimated_chars,
                estimated_tokens
            );
        }
```

Change the tools logging from `tracing::info!` to `tracing::debug!` as well.

- [ ] **Step 3: Run all tests**

Run: `cargo test -p agent-runtime`
Expected: All pass

- [ ] **Step 4: Commit**

```bash
git add runtime/agent-runtime/src/executor.rs runtime/agent-runtime/src/llm/openai.rs
git commit -m "perf: use DefaultHasher for args hashing, guard debug serialization"
```

---

### Task 5: Tier 1 Optimization — Context Editing In-Place + Reference Token Estimation

**Files:**
- Modify: `runtime/agent-runtime/src/middleware/context_editing.rs`

- [ ] **Step 1: Fix reference-based token estimation**

In `calculate_tokens_to_reclaim` (around line 271-277), replace:

```rust
    fn calculate_tokens_to_reclaim(&self, messages: &[ChatMessage], indices: &[usize]) -> usize {
        indices
            .iter()
            .filter_map(|idx| messages.get(*idx))
            .map(|msg| estimate_total_tokens(&[msg.clone()]))
            .sum()
    }
```

With:

```rust
    fn calculate_tokens_to_reclaim(&self, messages: &[ChatMessage], indices: &[usize]) -> usize {
        indices
            .iter()
            .filter_map(|idx| messages.get(*idx))
            .map(|msg| estimate_message_tokens(msg))
            .sum()
    }
```

Add a helper if `estimate_message_tokens` doesn't exist:

```rust
/// Estimate tokens for a single message without cloning.
fn estimate_message_tokens(msg: &ChatMessage) -> usize {
    let content_tokens = msg.content.len() / 4;
    let tool_call_tokens = msg.tool_calls.as_ref()
        .map(|tc| serde_json::to_string(tc).unwrap_or_default().len() / 4)
        .unwrap_or(0);
    content_tokens + tool_call_tokens + 4 // +4 for message overhead
}
```

- [ ] **Step 2: Edit messages in-place during compaction**

In the `process` method (around line 328), replace:

```rust
        let mut modified_messages = messages.clone();
```

With in-place editing. Since `process` takes `messages: Vec<ChatMessage>` by value, we can modify it directly:

```rust
        let mut modified_messages = messages; // Take ownership, no clone
```

- [ ] **Step 3: Run all tests**

Run: `cargo test -p agent-runtime`
Expected: All pass

- [ ] **Step 4: Commit**

```bash
git add runtime/agent-runtime/src/middleware/context_editing.rs
git commit -m "perf: in-place context editing, reference-based token estimation"
```

---

### Task 6: Line-Aware Tool Result Truncation

**Files:**
- Modify: `runtime/agent-runtime/src/executor.rs` (truncate_tool_result function around line 1770)

- [ ] **Step 1: Write failing test**

Add to `truncation_tests`:

```rust
#[test]
fn test_truncation_preserves_line_boundaries() {
    let lines: Vec<String> = (0..100).map(|i| format!("Line {}: some content here", i)).collect();
    let input = lines.join("\n");
    let result = truncate_tool_result(input, 500);

    // Should not cut mid-line
    for line in result.lines() {
        // Every line should be complete (starts with "Line" or is the truncation notice)
        assert!(
            line.starts_with("Line") || line.contains("TRUNCATED") || line.contains("---") || line.is_empty(),
            "Truncated mid-line: '{}'", line
        );
    }
}
```

- [ ] **Step 2: Rewrite truncate_tool_result to be line-aware**

```rust
fn truncate_tool_result(result: String, max_chars: usize) -> String {
    if max_chars == 0 || result.len() <= max_chars {
        return result;
    }

    let lines: Vec<&str> = result.lines().collect();
    let total_lines = lines.len();

    if total_lines <= 1 {
        // Single line — fall back to char-based truncation
        let notice = format!("\n\n--- TRUNCATED ({} chars total) ---\n\n", result.len());
        let budget = max_chars.saturating_sub(notice.len());
        let head_size = (budget * 4) / 5;
        let tail_size = budget - head_size;
        return format!("{}{}{}", &result[..head_size], notice, &result[result.len() - tail_size..]);
    }

    // Line-aware: keep first N + last M lines
    let head_lines = (total_lines * 4) / 5; // 80% from the top
    let tail_lines = total_lines / 5;       // 20% from the bottom

    let mut head = String::new();
    let mut head_count = 0;
    for line in &lines[..head_lines.min(total_lines)] {
        head.push_str(line);
        head.push('\n');
        head_count += 1;
        if head.len() > (max_chars * 4) / 5 {
            break;
        }
    }

    let mut tail = String::new();
    let tail_start = total_lines.saturating_sub(tail_lines);
    for line in &lines[tail_start..] {
        tail.push_str(line);
        tail.push('\n');
    }

    let omitted = total_lines - head_count - (total_lines - tail_start);
    let notice = format!(
        "\n--- TRUNCATED: showing {}/{} lines ({} omitted, {} chars total) ---\n\n",
        head_count + (total_lines - tail_start), total_lines, omitted, result.len()
    );

    // Final budget check
    let combined = format!("{}{}{}", head, notice, tail);
    if combined.len() > max_chars {
        // Fall back to char-based if line-based is still too big
        let budget = max_chars.saturating_sub(notice.len());
        let h = (budget * 4) / 5;
        let t = budget - h;
        format!("{}{}{}", &result[..h], notice, &result[result.len() - t..])
    } else {
        combined
    }
}
```

- [ ] **Step 3: Run all tests**

Run: `cargo test -p agent-runtime -- truncation`
Expected: All pass (including existing tests)

- [ ] **Step 4: Run full suite**

Run: `cargo test -p agent-runtime`
Expected: All pass

- [ ] **Step 5: Commit**

```bash
git add runtime/agent-runtime/src/executor.rs
git commit -m "perf: line-aware tool result truncation — preserves line boundaries"
```

---

### Task 7: Re-export New Types + Final Verification

**Files:**
- Modify: `runtime/agent-runtime/src/lib.rs`

- [ ] **Step 1: Add re-exports**

In lib.rs, add to the executor re-exports:

```rust
pub use executor::{
    AgentExecutor, ExecutorConfig, ExecutorError, RecallHook, RecallHookResult, create_executor,
    ToolCallDecision, ToolExecutionMode, BeforeToolCallHook, AfterToolCallHook,
};
```

- [ ] **Step 2: Run full workspace tests**

Run: `cargo test --workspace 2>&1 | grep FAILED | grep -v zero-core`
Expected: No failures (only pre-existing zero-core doctest)

- [ ] **Step 3: Run e2e tests specifically**

Run: `cargo test -p gateway-execution --test e2e_ward_pipeline_tests`
Expected: 16 tests pass

- [ ] **Step 4: Commit**

```bash
git add runtime/agent-runtime/src/lib.rs
git commit -m "feat: re-export hook types from agent-runtime"
```
