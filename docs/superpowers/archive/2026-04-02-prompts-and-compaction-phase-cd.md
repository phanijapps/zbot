# Prompts & Compaction (Phase C+D) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rewrite root prompts with structured XML-style sections for harder constraint enforcement. Add plan.md attention mechanism. Improve compaction to compress before dropping, preserving file paths and URLs.

**Architecture:** Phase C is template changes only (no Rust). Phase D modifies `compact_messages()` in executor.rs to call the existing `compress_old_assistant_messages` function before dropping messages, and adds restorable compression that preserves file paths.

**Tech Stack:** Rust (agent-runtime), Markdown templates

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `gateway/templates/shards/first_turn_protocol.md` | Rewrite | XML-tagged first actions |
| `gateway/templates/shards/planning_autonomy.md` | Rewrite | XML-tagged orchestration rules |
| `gateway/templates/instructions_starter.md` | Rewrite | XML-tagged execution instructions |
| `~/Documents/zbot/config/shards/first_turn_protocol.md` | Rewrite | User config copy |
| `~/Documents/zbot/config/shards/planning_autonomy.md` | Rewrite | User config copy |
| `~/Documents/zbot/config/INSTRUCTIONS.md` | Rewrite | User config copy |
| `runtime/agent-runtime/src/middleware/context_editing.rs` | Modify | Make compression functions pub |
| `runtime/agent-runtime/src/executor.rs` | Modify | Compress-first compaction strategy |

---

### Task 1: Structured Prompt Sections (Phase C)

**Files:**
- Rewrite: `gateway/templates/shards/first_turn_protocol.md`
- Rewrite: `gateway/templates/shards/planning_autonomy.md`
- Rewrite: `gateway/templates/instructions_starter.md`
- Copy to: user config equivalents

- [ ] **Step 1: Rewrite first_turn_protocol.md**

Replace content in BOTH `gateway/templates/shards/first_turn_protocol.md` AND `~/Documents/zbot/config/shards/first_turn_protocol.md`:

```
<agent_identity>
You are an autonomous orchestrator. You receive goals, delegate to specialist agents, review results, and synthesize deliverables. You never do specialized work yourself.
</agent_identity>

<agent_loop>
Each turn, perform exactly ONE action:
1. Read the latest result or observation
2. Decide the next action based on the execution plan
3. Call exactly one tool
4. The system returns the result — you are called again
Repeat until all plan steps are complete, then call respond.
</agent_loop>

<first_actions>
On a new task, execute these in order (one per turn):
1. memory(action="recall") — recall context for the user's request
2. set_session_title — concise title (2-8 words)
3. ward(action="use") — enter the ward from intent analysis
4. If approach=graph: delegate to planner-agent with the goal and ward name
5. After planner returns: read specs/plan.md, then delegate Step 1 to its assigned agent
6. After each delegation: read specs/plan.md to know your position, delegate next step
</first_actions>

<plan_attention>
After entering the ward, read specs/plan.md on EVERY continuation.
This file is your source of truth for what's done and what's next.
Update it after each delegation completes (mark step done, note key result).
If specs/plan.md doesn't exist, the planner didn't save it — ask planner to rerun.
</plan_attention>
```

- [ ] **Step 2: Rewrite planning_autonomy.md**

Replace content in BOTH `gateway/templates/shards/planning_autonomy.md` AND `~/Documents/zbot/config/shards/planning_autonomy.md`:

```
<available_agents>
| Agent | Use For |
|-------|---------|
| code-agent | Writing/running code, building pipelines, spec-driven development in wards |
| data-analyst | Interpreting existing data, statistical analysis, generating insights |
| research-agent | Web search, gathering news, analyst reports, external information |
| writing-agent | Creating formatted documents, HTML reports from existing data |

When a task needs code AND analysis, split it: code-agent builds, data-analyst interprets.
</available_agents>

<delegation_rules>
- Delegate with goals and acceptance criteria, not procedures
- One delegation at a time — system resumes you after each completes
- Include the ward name in every delegation message
- Review each result before proceeding to the next step
</delegation_rules>

<prohibited_actions>
You MUST NOT call these tools — they are not available to you:
- load_skill — subagents load their own skills
- list_skills — intent analysis provides recommendations
- list_agents — intent analysis provides recommendations
- apply_patch — you do not write files, delegate to code-agent
</prohibited_actions>

<failure_handling>
1. Read the crash report carefully
2. Retry once with a simpler, more focused task
3. If retry fails: mark step failed, continue with remaining steps
4. If >50% of steps failed: respond with partial results and explain gaps
</failure_handling>
```

- [ ] **Step 3: Rewrite instructions_starter.md**

Replace content in BOTH `gateway/templates/instructions_starter.md` AND `~/Documents/zbot/config/INSTRUCTIONS.md`:

```
<execution_mode>
- Simple tasks (greeting, quick question, 1-2 steps): handle directly. No delegation.
- Complex tasks (approach=graph from Intent Analysis): delegate to planner-agent first.
  The planner saves a spec-driven plan to specs/plan.md. Execute each step by delegating to the assigned agent.
</execution_mode>

<orchestration>
- Read specs/plan.md at the start of every continuation to know your position
- Delegate each step to the assigned agent with: goal, ward name, acceptance criteria
- Review results before moving to the next step
- Do NOT call respond until ALL plan steps are complete
</orchestration>

<completion>
When all steps are done:
1. Read the final outputs referenced in specs/plan.md
2. Synthesize into a clear response: what was accomplished, where artifacts are, key findings
3. Call respond with the synthesis
</completion>
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p gateway-execution`
Expected: All pass (templates aren't compiled code, but e2e tests may check content)

- [ ] **Step 5: Commit**

```bash
git add gateway/templates/shards/first_turn_protocol.md gateway/templates/shards/planning_autonomy.md gateway/templates/instructions_starter.md
git commit -m "refactor: structured XML-tagged prompts + plan.md attention mechanism"
```

---

### Task 2: Make Compression Functions Public (Phase D prerequisite)

**Files:**
- Modify: `runtime/agent-runtime/src/middleware/context_editing.rs:25,52,68`

- [ ] **Step 1: Make functions pub(crate)**

In `context_editing.rs`, change three functions from private to `pub(crate)`:

Line 25:
```rust
pub(crate) fn compress_assistant_message(msg: &ChatMessage, turn_number: usize) -> String {
```

Line 52 (extract_file_path):
```rust
pub(crate) fn extract_file_path(args: &serde_json::Value) -> Option<String> {
```

Line 68:
```rust
pub(crate) fn compress_old_assistant_messages(messages: &mut [ChatMessage], keep_recent: usize) {
```

- [ ] **Step 2: Re-export from middleware mod**

In `runtime/agent-runtime/src/middleware/mod.rs`, add:

```rust
pub use context_editing::{compress_old_assistant_messages, compress_assistant_message};
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p agent-runtime`
Expected: All pass

- [ ] **Step 4: Commit**

```bash
git add runtime/agent-runtime/src/middleware/context_editing.rs runtime/agent-runtime/src/middleware/mod.rs
git commit -m "refactor: make compression functions pub(crate) for use in compact_messages"
```

---

### Task 3: Compress-First Compaction Strategy (Phase D)

**Files:**
- Modify: `runtime/agent-runtime/src/executor.rs:1912-1969` (compact_messages function)

- [ ] **Step 1: Write test**

Add to a new test module at the bottom of executor.rs:

```rust
#[cfg(test)]
mod compaction_tests {
    use super::*;

    #[test]
    fn test_compact_messages_compresses_before_dropping() {
        // Create 30 messages — enough to trigger compaction (KEEP_RECENT = 20)
        let mut messages = vec![
            ChatMessage::system("system prompt".to_string()),
            ChatMessage::user("original request".to_string()),
        ];

        // Add 28 assistant+tool pairs
        for i in 0..14 {
            let tool = ToolCall::new(
                format!("call_{}", i),
                "write_file".to_string(),
                json!({"path": format!("src/file_{}.py", i)}),
            );
            messages.push(ChatMessage {
                role: "assistant".to_string(),
                content: format!("I'll create file_{}.py with a long detailed explanation that takes many tokens", i),
                tool_calls: Some(vec![tool]),
                tool_call_id: None,
            });
            messages.push(ChatMessage {
                role: "tool".to_string(),
                content: format!("File created successfully: src/file_{}.py with lots of content here that should be compressed", i),
                tool_calls: None,
                tool_call_id: Some(format!("call_{}", i)),
            });
        }

        let compacted = compact_messages(messages);

        // Should have compressed old messages, not just dropped them
        // Check that early assistant messages are compressed (start with [Turn)
        let has_compressed = compacted.iter().any(|m| m.content.starts_with("[Turn"));
        assert!(has_compressed, "Old assistant messages should be compressed to [Turn N: ...]");

        // Check that file paths are preserved in compressed form
        let compressed_content: String = compacted.iter()
            .filter(|m| m.content.starts_with("[Turn"))
            .map(|m| m.content.clone())
            .collect();
        assert!(
            compressed_content.contains("write_file") || compressed_content.contains("file_"),
            "Compressed messages should preserve tool names or file paths"
        );
    }

    #[test]
    fn test_compact_preserves_recent_messages() {
        let mut messages = vec![
            ChatMessage::system("system".to_string()),
            ChatMessage::user("request".to_string()),
        ];
        for i in 0..25 {
            messages.push(ChatMessage::user(format!("msg {}", i)));
        }

        let compacted = compact_messages(messages);

        // Last message should be preserved
        assert!(compacted.last().unwrap().content.contains("msg 24"));
    }
}
```

- [ ] **Step 2: Rewrite compact_messages with compress-first strategy**

Replace the entire `compact_messages` function:

```rust
/// Compact messages to reduce context size when approaching token limits.
///
/// Strategy:
/// 1. Compress old assistant messages to one-liners (preserving tool names and file paths)
/// 2. Clear old tool result content (replace with placeholder)
/// 3. Only drop messages if still over budget after compression
///
/// IMPORTANT: assistant+tool_call / tool_response pairs are treated as atomic
/// units. Split boundaries respect pair integrity.
fn compact_messages(messages: Vec<ChatMessage>) -> Vec<ChatMessage> {
    const KEEP_RECENT: usize = 20;

    if messages.len() <= KEEP_RECENT + 2 {
        return messages;
    }

    let mut messages = messages;

    // Phase 1: Compress old assistant messages to one-liners
    // This preserves tool names and file paths while reducing token count
    crate::middleware::context_editing::compress_old_assistant_messages(
        &mut messages, KEEP_RECENT
    );

    // Phase 2: Clear old tool result content (keep tool_call_id for pairing)
    let compress_boundary = messages.len().saturating_sub(KEEP_RECENT);
    for i in 0..compress_boundary {
        if messages[i].role == "tool" {
            // Preserve file paths from the result if present
            let preserved = extract_key_info(&messages[i].content);
            messages[i].content = if preserved.is_empty() {
                "[result cleared]".to_string()
            } else {
                format!("[result cleared — {}]", preserved)
            };
        }
    }

    // Phase 3: If still too many messages, drop old ones (same as before)
    if messages.len() > KEEP_RECENT + 10 {
        let mut compacted = Vec::new();

        // Keep system messages
        let mut non_system_start = 0;
        for (i, msg) in messages.iter().enumerate() {
            if msg.role == "system" {
                compacted.push(msg.clone());
                non_system_start = i + 1;
            } else {
                break;
            }
        }

        // Preserve first user message
        let first_user_msg = messages[non_system_start..].iter().find(|m| m.role == "user");
        if let Some(user_msg) = first_user_msg {
            compacted.push(user_msg.clone());
        }

        // Find clean split point
        let target_start = messages.len().saturating_sub(KEEP_RECENT);
        let mut split_at = target_start;
        for i in target_start..messages.len() {
            let msg = &messages[i];
            if msg.role == "user" || (msg.role == "assistant" && msg.tool_call_id.is_none()) {
                split_at = i;
                break;
            }
        }

        let trimmed_count = split_at.saturating_sub(non_system_start);
        if trimmed_count > 0 {
            compacted.push(ChatMessage::user(format!(
                "[SYSTEM: Context compacted. {} earlier messages were compressed and trimmed. \
                 The original request and recent messages are preserved. Continue with the task.]",
                trimmed_count
            )));
        }

        compacted.extend(messages[split_at..].iter().cloned());
        compacted
    } else {
        // Compression was enough — no need to drop
        messages
    }
}

/// Extract key info (file paths, URLs) from a tool result for restorable compression.
fn extract_key_info(content: &str) -> String {
    let mut info = Vec::new();

    // Extract file paths (common patterns)
    for word in content.split_whitespace() {
        let trimmed = word.trim_matches(|c: char| c == '"' || c == '\'' || c == ',' || c == ':');
        if (trimmed.contains('/') || trimmed.contains('.'))
            && (trimmed.ends_with(".py")
                || trimmed.ends_with(".json")
                || trimmed.ends_with(".csv")
                || trimmed.ends_with(".html")
                || trimmed.ends_with(".md")
                || trimmed.ends_with(".js")
                || trimmed.ends_with(".ts")
                || trimmed.ends_with(".yaml")
                || trimmed.ends_with(".toml"))
        {
            if !info.contains(&trimmed.to_string()) {
                info.push(trimmed.to_string());
            }
        }
        // Extract URLs
        if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            if !info.contains(&trimmed.to_string()) {
                info.push(trimmed.to_string());
            }
        }
    }

    info.join(", ")
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p agent-runtime -- compaction_tests`
Expected: Both tests pass

Run: `cargo test -p agent-runtime`
Expected: All pass

- [ ] **Step 4: Commit**

```bash
git add runtime/agent-runtime/src/executor.rs
git commit -m "feat: compress-first compaction — compress old messages before dropping, preserve file paths"
```

---

### Task 4: Final Verification

- [ ] **Step 1: Run full workspace tests**

Run: `cargo test --workspace 2>&1 | grep -E "FAILED|test result" | grep -v "zero-core.*doc"`
Expected: No failures

- [ ] **Step 2: Build**

Run: `cargo build`
Expected: Clean build
