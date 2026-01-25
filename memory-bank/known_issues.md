# Known Issues

This document tracks known issues that need to be addressed.

## ~~__awaiting_input__ Execution Error~~ ✅ RESOLVED

**Status:** Resolved
**Priority:** High
**Component:** `executor_v2.rs` / `agents_runtime.rs`
**Resolution Date:** 2025-01-21

### Description

After removing special agent-creator handling, agent execution failed with:
```
Error: Agent execution failed: __awaiting_input__
```

### Root Cause

Leftover `AwaitingInput` variant and handling code remained in the executor after removing agent-creator special execution path.

### Solution

Removed all AwaitingInput-related code:
- Removed `AwaitingInput` variant from `ZeroAppStreamEvent` enum
- Removed AwaitingInput detection in `run_stream`
- Removed AwaitingInput event handling from `agents_runtime.rs`

### Changes Made

- `src-tauri/src/domains/agent_runtime/executor_v2.rs` - Removed AwaitingInput variant and detection
- `src-tauri/src/commands/agents_runtime.rs` - Removed AwaitingInput event handling

---

## ~~Conversation History Not Loading on Agent Selection~~ ✅ RESOLVED

**Status:** Resolved
**Priority:** Medium
**Component:** Frontend / `AgentChannelPanel.tsx`
**Resolution Date:** 2025-01-21

### Description

When selecting an agent, the day's conversation history wasn't loading automatically.

### Root Cause

`loadTodaySession` function was missing from the useEffect dependency array in `AgentChannelPanel.tsx`, causing a stale closure issue.

### Solution

Added `loadTodaySession` to the useEffect dependency array:
```typescript
}, [selectedAgent, loadTodaySession]);
```

### Changes Made

- `src/features/agent-channels/AgentChannelPanel.tsx` - Added missing dependency

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

---

## Agent Creator request_input Tool Response Format Issue

**Status:** Workaround Implemented
**Priority:** Medium
**Component:** `agent-creator` / OpenAI API
**Reported Date:** 2025-01-21

### Description

When the agent-creator uses the `request_input` tool to collect user information via forms, submitting the form results in an OpenAI API error:

```
"An assistant message with 'tool_calls' must be followed by tool messages responding to each 'tool_call_id'. (insufficient tool messages following tool_calls message)"
```

### Root Cause

The zero-agent framework doesn't natively support "pausing" for user input. When `request_input` is called:
1. LLM returns a `tool_calls` response
2. Framework executes the tool, which returns `__request_input: true` marker
3. Framework automatically continues to next LLM call without waiting
4. When user submits form, the data is sent as a **user message** instead of a **tool response**

OpenAI's API requires that after a `tool_calls` message, the next message MUST be a `tool` response with matching `tool_call_id`.

### Current Workaround ✅

**Implemented January 2025**: The agent-creator instructions have been updated to work conversationally without using `request_input`. Users provide information through regular chat messages instead of forms.

**Agent-creator is now a regular agent** using the standard workflow:
- Located in `src-tauri/templates/default-agents/agent-creator/`
- Uses `zero-agent-creator` skill for agent creation tools
- No special execution path required

### Potential Solutions

1. **Modify Backend**: Inject form data as tool response directly into session history before continuing
   - Requires storing `tool_call_id` when `request_input` is detected
   - Create separate command to handle form submissions as tool responses
   - Complexity: High - requires session history manipulation

2. **Modify request_input Tool**: Make it return a special marker that pauses execution
   - Requires changes to zero-agent framework
   - Framework would need to support "awaiting input" state
   - Complexity: Very High - framework-level change

3. **Avoid request_input**: Keep current workaround - use conversational approach
   - Agent asks questions one at a time in natural language
   - User responds with plain text
   - Simpler implementation, works with existing framework

### Related Files

- `src-tauri/src/commands/agents_runtime.rs` - Agent execution
- `src-tauri/src/domains/agent_runtime/executor_v2.rs` - Executor implementation
- `src-tauri/templates/default-agents/agent-creator/AGENTS.md` - Agent instructions (conversational approach)
- `src-tauri/templates/default-agents/agent-creator/config.yaml` - Agent configuration
- `src-tauri/templates/default-skills/zero-agent-creator/SKILL.md` - Skill with agent creation tools

### Notes

The executor cache (`EXECUTOR_CACHE`) maintains session across multiple message turns, allowing multi-turn conversations. The current conversational approach works well without requiring framework-level changes for pausing execution.

---

## ~~Agent Channel Navigation Loop After Workflow IDE~~ ✅ RESOLVED

**Status:** Resolved
**Priority:** Medium
**Component:** Frontend / `AgentChannelPanel.tsx`
**Resolution Date:** 2025-01-25

### Description

When returning from the Workflow IDE to the Agent Channel Panel, users could only select the agent they came from. Selecting any other agent would automatically revert back to the previously active agent. The issue would only resolve after reloading the application.

### Root Cause

In `AgentChannelPanel.tsx`, the `selectedAgent` state was included in the dependency array of the restoration useEffect hook. This caused a problematic loop:

1. User navigates to Workflow IDE with `location.state.restoreAgentId = agentA`
2. User returns to Agent Channel Panel
3. Restoration useEffect runs, sets `selectedAgent = agentA`
4. Because `selectedAgent` is in the dependency array, this triggers the useEffect again
5. The useEffect sees `restoreAgentId` is still `agentA` and sets `selectedAgent = agentA` again
6. When user tries to select a different agent, the restoration logic overwrites it

### Solution

Removed `selectedAgent` from the dependency array of the restoration useEffect. The restoration should only run when `location.state` changes (navigation) or when `agents` list changes, not when `selectedAgent` changes.

**Before:**
```typescript
useEffect(() => {
  const state = location.state as { restoreAgentId?: string } | null;
  const restoreAgentId = state?.restoreAgentId;
  // ... restoration logic
}, [location.state, agents, selectedAgent]);  // selectedAgent caused the loop
```

**After:**
```typescript
useEffect(() => {
  const state = location.state as { restoreAgentId?: string } | null;
  const restoreAgentId = state?.restoreAgentId;
  // ... restoration logic
  // eslint-disable-next-line react-hooks/exhaustive-deps
}, [location.state, agents]);  // Removed selectedAgent from deps
```

### Changes Made

- `src/features/agent-channels/AgentChannelPanel.tsx` - Removed `selectedAgent` from dependency array

---

## Draft Features

The following features are marked as **DRAFT** and are not yet fully implemented:

### Conditional Node
- **Location:** `src/features/workflow-ide/components/nodes/ConditionalNode.tsx`
- **Status:** DRAFT/PLACEHOLDER
- **Purpose:** BPMN-style diamond gateway for branching logic in workflows
- **TODO:** Implement conditional branching logic, condition evaluation, and routing

## Current TODOs

The following TODO comments exist in the codebase:

1. **ConditionalNode.tsx**: Display condition on node
2. **Workflow IDE execution visualization**: Complete execution flow visualization (see `WorkflowStore.ts`)

## Known Limitations

1. **Conditional Execution**: The Conditional node is a draft placeholder and does not yet execute conditional logic
2. **Parallel Execution**: Workflow templates show parallel patterns, but execution is currently sequential
3. **Workflow Execution**: Backend execution of workflow-defined orchestrators is not yet implemented
