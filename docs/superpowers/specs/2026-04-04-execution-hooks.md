# Execution Hooks — Runtime Enforcement for Agent Behavior

## Overview

The executor supports two hooks on `ExecutorConfig` that intercept every tool call:

- `before_tool_call: Option<BeforeToolCallHook>` — called BEFORE each tool executes. Can block the call.
- `after_tool_call: Option<AfterToolCallHook>` — called AFTER each tool executes. Can transform the result.

Both are `Arc<dyn Fn>` closures, set during executor construction in `gateway/gateway-execution/src/invoke/executor.rs`.

## Current Hooks (subagents only)

### beforeToolCall: Shell File Write Blocker

Blocks shell commands that create files via redirects:
- `> file.py`, `cat << EOF`, `tee`, `printf > file`, `echo "..." > file`
- Returns: "Use write_file to create files, not shell redirects"

**Why:** Agents bypass `write_file` by using shell to write files directly. Shell-written files have escaping bugs, no path sanitization, and bypass the ward directory constraint.

### afterToolCall: Error Guidance Injector

On tool failure, appends guidance to the error message:
- Shell failures: "Fix the ROOT CAUSE in your code. Fix the file first with edit_file."
- Any failure: "Read the error carefully before retrying."

**Why:** Agents enter a fix-retry loop where they retry the same command 5-10 times instead of fixing the underlying code. The injected guidance steers toward `edit_file` for surgical fixes.

## How to Add New Hooks

In `gateway/gateway-execution/src/invoke/executor.rs`, after `executor_config.single_action_mode`:

```rust
executor_config.before_tool_call = Some(Arc::new(|tool_name, args| {
    // Return ToolCallDecision::Allow or ToolCallDecision::Block { reason }
    ToolCallDecision::Allow
}));

executor_config.after_tool_call = Some(Arc::new(|tool_name, args, result, succeeded| {
    // Return None (pass through) or Some(new_result) to replace
    None
}));
```

## Future Hook Ideas

- **Code quality check:** Before `write_file`, scan content for common bugs (undefined variables, missing imports)
- **Rate limiter:** Before any LLM-calling tool, enforce per-provider rate limits
- **Token budget:** After each tool, track cumulative tokens and warn agent when approaching limit
- **File size guard:** Before `write_file`, reject files > 3KB
- **Duplicate detection:** Before `write_file`, check if a similar file already exists in core/
