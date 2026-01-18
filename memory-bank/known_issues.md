# Known Issues

This document tracks known issues that need to be addressed.

## ~~Write Tool Path Resolution Issue~~ ✅ RESOLVED

**Status:** Resolved
**Priority:** High
**Component:** `agent-tools` / `WriteTool`
**Resolution Date:** 2025-01-18

### Description

When the agent attempted to write files using the `write` tool, the operation failed with error:
```
Tool execution error: Tool error: Missing 'path' parameter
```

This error occurred even when the LLM was correctly providing the `path` parameter in the tool call.

### Root Cause

The issue was a **state management problem**, not a parameter parsing issue. The `conversation_id` was being:
1. Baked into tool instances during creation (`WriteTool::with_conversation()`)
2. Stored in multiple places (tool struct + filesystem context)
3. Not properly propagated through the execution context

This created tight coupling and made tools non-idempotent.

### Solution

Implemented **state-based conversation ID propagation**:

1. **Application layer defines state keys** (`src-tauri/src/domains/agent_runtime/state_keys.rs`):
   ```rust
   pub const CONVERSATION_ID: &str = "app:conversation_id";
   ```

2. **Executor sets state during initialization** (`executor_v2.rs`):
   ```rust
   session.state_mut().set("app:conversation_id", json!(conversation_id));
   ```

3. **Tools read from context during execution** (`file.rs`):
   ```rust
   let conv_id = ctx.get_state("app:conversation_id")
       .and_then(|v| v.as_str().map(|s| s.to_string()));
   ```

4. **Simplified filesystem context** - Removed `conversation_id` storage from `TauriFileSystemContext`

### Changes Made

- `src-tauri/src/domains/agent_runtime/state_keys.rs` - New file with state key constants
- `src-tauri/src/domains/agent_runtime/executor_v2.rs` - Sets conversation_id in session state
- `application/agent-tools/src/tools/file.rs` - WriteTool/EditTool now stateless, read from context
- `application/agent-tools/src/tools/mod.rs` - Removed conversation_id from `builtin_tools_with_fs()`
- `src-tauri/src/domains/agent_runtime/filesystem.rs` - Simplified TauriFileSystemContext
- `src-tauri/src/commands/agents_runtime.rs` - Unified to use new `run_stream` API
- `memory-bank/learnings.md` - Added "State-Based Conversation ID Propagation" section

### Benefits

1. **Stateless Tools**: Same tool instance works for any conversation
2. **Single Source of Truth**: conversation_id lives in session state only
3. **Scalable**: Future migration to persistent state (FS/SQLite/Parquet) only requires changing the `State` implementation
4. **Clean Separation**: Framework (`zero-*`) provides infrastructure, application defines state keys
