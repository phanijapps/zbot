# Executor Steering Upgrade — Phase 3 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add pattern-based old message compression during compaction and pre-built tool_call_id → tool_name index for O(1) lookups.

**Architecture:** Message compression is a new method on `ContextEditingMiddleware` that converts old assistant messages into one-line summaries (pure pattern matching, no LLM). The tool_call_id index is built once per `find_tool_results_to_clear_with_cascade` call and reused across all lookups instead of backwards-scanning per tool result.

**Tech Stack:** Rust (agent-runtime crate), regex, serde_json

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `runtime/agent-runtime/src/middleware/context_editing.rs` | Modify | Pattern-based message compression, tool_call_id index |
| `runtime/agent-runtime/Cargo.toml` | Modify | Add `regex` dependency (if not already present) |

---

### Task 1: Pre-build tool_call_id → tool_name Index

**Files:**
- Modify: `runtime/agent-runtime/src/middleware/context_editing.rs:46-149`

- [ ] **Step 1: Write failing test**

Add to the `tests` module at the bottom of `context_editing.rs`:

```rust
    #[test]
    fn test_tool_call_id_index_lookup() {
        let messages = create_test_messages_with_tool_calls();
        let index = build_tool_call_index(&messages);
        assert_eq!(index.get("call_1"), Some(&"search".to_string()));
        assert_eq!(index.get("call_2"), Some(&"calculator".to_string()));
        assert_eq!(index.get("nonexistent"), None);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p agent-runtime -- context_editing::tests::test_tool_call_id_index`
Expected: FAIL — `build_tool_call_index` doesn't exist

- [ ] **Step 3: Add build_tool_call_index function**

Add as a free function near the top of the file (after the `use` statements, before the struct):

```rust
/// Build an index mapping tool_call_id → tool_name from all assistant messages.
/// This is O(n) over messages, enabling O(1) lookups instead of O(n) backwards search.
fn build_tool_call_index(messages: &[ChatMessage]) -> std::collections::HashMap<String, String> {
    let mut index = std::collections::HashMap::new();
    for msg in messages {
        if msg.role == "assistant" {
            if let Some(ref tool_calls) = msg.tool_calls {
                for tc in tool_calls {
                    index.insert(tc.id.clone(), tc.name.clone());
                }
            }
        }
    }
    index
}
```

- [ ] **Step 4: Refactor find_tool_name_for_call to use the index**

The current `find_tool_name_for_call` method (lines 131-149) does a backwards scan per tool result. Change `find_tool_results_to_clear_with_cascade` to build the index once and pass it to a new version.

In `find_tool_results_to_clear_with_cascade`, add at the start (before the loop):

```rust
        // Build tool_call_id → tool_name index once for O(1) lookups
        let tool_call_index = build_tool_call_index(messages);
```

Then replace the call to `self.find_tool_name_for_call(messages, idx, tool_call_id)` (around line 60) with:

```rust
                    let tool_name = tool_call_index.get(tool_call_id).cloned();
```

Keep the old `find_tool_name_for_call` method but mark it with `#[allow(dead_code)]` as a fallback reference, or remove it entirely if no other code uses it.

- [ ] **Step 5: Run tests**

Run: `cargo test -p agent-runtime -- context_editing`
Expected: All pass (existing tests verify the same behavior)

- [ ] **Step 6: Commit**

```bash
git add runtime/agent-runtime/src/middleware/context_editing.rs
git commit -m "perf: pre-build tool_call_id → tool_name index for O(1) lookups in context editing"
```

---

### Task 2: Pattern-Based Old Message Compression

**Files:**
- Modify: `runtime/agent-runtime/src/middleware/context_editing.rs`

- [ ] **Step 1: Write failing tests**

Add to the `tests` module:

```rust
    #[test]
    fn test_compress_assistant_message_with_tool_calls() {
        let tool1 = ToolCall::new(
            "call_a".to_string(),
            "write_file".to_string(),
            json!({"path": "src/main.rs", "content": "fn main() {}"}),
        );
        let tool2 = ToolCall::new(
            "call_b".to_string(),
            "read_file".to_string(),
            json!({"path": "src/lib.rs"}),
        );
        let msg = ChatMessage {
            role: "assistant".to_string(),
            content: "Let me create the main file and read the lib file for context.".to_string(),
            tool_calls: Some(vec![tool1, tool2]),
            tool_call_id: None,
        };

        let compressed = compress_assistant_message(&msg, 3);
        assert!(compressed.starts_with("[Turn 3:"));
        assert!(compressed.contains("write_file"));
        assert!(compressed.contains("read_file"));
        assert!(compressed.contains("src/main.rs"));
        assert!(compressed.contains("src/lib.rs"));
        // Should be much shorter than the original
        assert!(compressed.len() < msg.content.len() + 200);
    }

    #[test]
    fn test_compress_assistant_message_no_tool_calls() {
        let msg = ChatMessage {
            role: "assistant".to_string(),
            content: "I'll help you with that. Let me think about the best approach for implementing this feature. We need to consider several factors including performance and maintainability.".to_string(),
            tool_calls: None,
            tool_call_id: None,
        };

        let compressed = compress_assistant_message(&msg, 5);
        assert!(compressed.starts_with("[Turn 5:"));
        // Should truncate long reasoning to a short summary
        assert!(compressed.len() < 100);
    }

    #[test]
    fn test_compress_extracts_file_paths() {
        let tool = ToolCall::new(
            "call_x".to_string(),
            "edit_file".to_string(),
            json!({"path": "core/data_fetcher.py", "old_text": "x", "new_text": "y"}),
        );
        let msg = ChatMessage {
            role: "assistant".to_string(),
            content: "".to_string(),
            tool_calls: Some(vec![tool]),
            tool_call_id: None,
        };

        let compressed = compress_assistant_message(&msg, 1);
        assert!(compressed.contains("core/data_fetcher.py"));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p agent-runtime -- context_editing::tests::test_compress`
Expected: FAIL — `compress_assistant_message` doesn't exist

- [ ] **Step 3: Implement compress_assistant_message**

Add as a free function:

```rust
/// Compress an assistant message into a one-line summary.
///
/// Extracts tool names and file paths from tool_calls arguments.
/// Pure pattern matching — no LLM call.
///
/// Format: `[Turn N: tool1(file1), tool2(file2)]` or `[Turn N: <truncated reasoning>]`
fn compress_assistant_message(msg: &ChatMessage, turn_number: usize) -> String {
    if let Some(ref tool_calls) = msg.tool_calls {
        // Extract tool name + file path pairs
        let summaries: Vec<String> = tool_calls.iter().map(|tc| {
            let path = extract_file_path(&tc.arguments);
            match path {
                Some(p) => format!("{}({})", tc.name, p),
                None => tc.name.clone(),
            }
        }).collect();

        format!("[Turn {}: {}]", turn_number, summaries.join(", "))
    } else if !msg.content.is_empty() {
        // No tool calls — truncate reasoning to first 60 chars
        let truncated: String = msg.content.chars().take(60).collect();
        let ellipsis = if msg.content.len() > 60 { "..." } else { "" };
        format!("[Turn {}: {}{}]", turn_number, truncated, ellipsis)
    } else {
        format!("[Turn {}]", turn_number)
    }
}

/// Extract a file path from tool call arguments.
///
/// Looks for common keys: "path", "file_path", "file", "filename".
fn extract_file_path(args: &serde_json::Value) -> Option<String> {
    let path_keys = ["path", "file_path", "file", "filename"];
    if let Some(obj) = args.as_object() {
        for key in &path_keys {
            if let Some(val) = obj.get(*key) {
                if let Some(s) = val.as_str() {
                    return Some(s.to_string());
                }
            }
        }
    }
    None
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p agent-runtime -- context_editing::tests::test_compress`
Expected: All 3 tests pass

- [ ] **Step 5: Commit**

```bash
git add runtime/agent-runtime/src/middleware/context_editing.rs
git commit -m "feat: pattern-based assistant message compression — extract tool names and file paths"
```

---

### Task 3: Integrate Compression into Compaction Pipeline

**Files:**
- Modify: `runtime/agent-runtime/src/middleware/context_editing.rs`

- [ ] **Step 1: Write integration test**

Add to the `tests` module:

```rust
    #[test]
    fn test_compress_old_assistant_messages() {
        let tool1 = ToolCall::new(
            "call_1".to_string(),
            "write_file".to_string(),
            json!({"path": "src/main.rs", "content": "lots of code"}),
        );
        let tool2 = ToolCall::new(
            "call_2".to_string(),
            "read_file".to_string(),
            json!({"path": "src/lib.rs"}),
        );

        let mut messages = vec![
            ChatMessage {
                role: "user".to_string(),
                content: "Create the files".to_string(),
                tool_calls: None,
                tool_call_id: None,
            },
            ChatMessage {
                role: "assistant".to_string(),
                content: "I'll create main.rs and read lib.rs for you. Let me start with the main file.".to_string(),
                tool_calls: Some(vec![tool1, tool2]),
                tool_call_id: None,
            },
            ChatMessage {
                role: "tool".to_string(),
                content: "[cleared]".to_string(),
                tool_calls: None,
                tool_call_id: Some("call_1".to_string()),
            },
            ChatMessage {
                role: "tool".to_string(),
                content: "[cleared]".to_string(),
                tool_calls: None,
                tool_call_id: Some("call_2".to_string()),
            },
            // More recent messages that should NOT be compressed
            ChatMessage {
                role: "user".to_string(),
                content: "Now add tests".to_string(),
                tool_calls: None,
                tool_call_id: None,
            },
            ChatMessage {
                role: "assistant".to_string(),
                content: "I'll add tests now.".to_string(),
                tool_calls: None,
                tool_call_id: None,
            },
        ];

        let original_assistant_content = messages[1].content.clone();
        compress_old_assistant_messages(&mut messages, 4); // keep last 4 messages

        // Old assistant message (index 1) should be compressed
        assert!(messages[1].content.starts_with("[Turn"));
        assert!(messages[1].content.contains("write_file"));
        assert!(messages[1].content != original_assistant_content);

        // Recent assistant message (index 5) should NOT be compressed
        assert_eq!(messages[5].content, "I'll add tests now.");
    }
```

- [ ] **Step 2: Add compress_old_assistant_messages function**

```rust
/// Compress old assistant messages in-place.
///
/// Messages in the "old" portion (before `keep_recent`) are compressed to
/// one-line summaries. Recent messages are left intact.
///
/// `keep_recent` is the number of messages from the end to preserve unchanged.
fn compress_old_assistant_messages(messages: &mut [ChatMessage], keep_recent: usize) {
    let total = messages.len();
    if total <= keep_recent {
        return; // Nothing old enough to compress
    }

    let compress_boundary = total.saturating_sub(keep_recent);
    let mut turn_counter = 0;

    for i in 0..compress_boundary {
        let msg = &messages[i];
        if msg.role == "assistant" {
            turn_counter += 1;

            // Only compress if the message has meaningful content to compress
            // (skip already-compressed messages)
            if msg.content.starts_with("[Turn") {
                continue;
            }

            let compressed = compress_assistant_message(msg, turn_counter);
            messages[i].content = compressed;
            // Keep tool_calls intact — the LLM API requires them for tool result pairing
        }
    }
}
```

- [ ] **Step 3: Integrate into the process method**

In the `process` method, after `clear_tool_results` and before the logging section, add compression of old assistant messages:

Find this section (around line 340-344):

```rust
        // Clear the tool results (skill-aware: uses meaningful placeholders for skill loads)
        let unloaded_skills = self.clear_tool_results(
            &mut modified_messages,
            &indices_to_clear,
            &context.execution_state,
        );
```

Add after it:

```rust
        // Compress old assistant messages to one-line summaries
        // Keep the most recent tool results + a buffer of messages uncompressed
        let keep_recent = (self.config.keep_tool_results as usize + 1) * 3; // ~3 msgs per tool round
        compress_old_assistant_messages(&mut modified_messages, keep_recent);
```

- [ ] **Step 4: Run all context_editing tests**

Run: `cargo test -p agent-runtime -- context_editing`
Expected: All pass

- [ ] **Step 5: Run full suite**

Run: `cargo test -p agent-runtime`
Expected: All pass

- [ ] **Step 6: Commit**

```bash
git add runtime/agent-runtime/src/middleware/context_editing.rs
git commit -m "feat: integrate pattern-based compression into compaction pipeline"
```

---

### Task 4: Final Verification

**Files:** None (verification only)

- [ ] **Step 1: Run full workspace tests**

Run: `cargo test --workspace 2>&1 | grep -E "FAILED|test result" | grep -v "zero-core.*doc"`
Expected: No failures (only pre-existing zero-core doctest)

- [ ] **Step 2: Run e2e tests**

Run: `cargo test -p gateway-execution --test e2e_ward_pipeline_tests`
Expected: 16 tests pass

- [ ] **Step 3: Verify no regressions in agent-runtime**

Run: `cargo test -p agent-runtime`
Expected: All pass (102+ tests)
