# Known Issues

This document tracks known issues that need to be addressed.

## Write Tool Path Resolution Issue

**Status:** Open
**Priority:** High
**Component:** `agent-tools` / `WriteTool`

### Description

When the agent attempts to write files using the `write` tool, the operation fails with error:
```
Tool execution error: Tool error: Missing 'path' parameter
```

This error occurs even when the LLM is correctly providing the `path` parameter in the tool call.

### Current Behavior

1. LLM generates tool call with arguments: `{"path": "attachments/time-report.html", "content": "..."}`
2. OpenAI LLM client parses the response using `serde_json::from_str`
3. Tool receives the arguments but cannot find the `path` parameter
4. Execution fails with "Missing 'path' parameter" error

### Root Cause Analysis (In Progress)

The issue appears to be related to:
- JSON parsing of tool arguments from OpenAI API responses
- Possible truncation due to `finish_reason: "length"` when content is large
- Conversation ID not being properly propagated to WriteTool

### Attempted Fixes

1. **Conversation ID Propagation** - Updated `builtin_tools_with_fs` to accept and pass conversation_id to WriteTool/EditTool
2. **Debug Logging** - Added detailed logging in OpenAI client to track argument parsing
3. **Owned vs Borrowed** - Fixed ownership issues with conversation_id parameter

### Next Steps

1. Check if arguments JSON is malformed or truncated
2. Verify WriteTool is receiving correct arguments structure
3. Test with smaller content to rule out truncation
4. Consider using `outputs/` prefix pattern which has different code path

### Related Files

- `crates/zero-llm/src/openai.rs` - OpenAI LLM client implementation
- `application/agent-tools/src/tools/file.rs` - WriteTool implementation
- `application/agent-tools/src/tools/mod.rs` - builtin_tools_with_fs function
- `src-tauri/src/domains/agent_runtime/executor_v2.rs` - Executor that creates tools

### Workaround

None currently available. Files cannot be written through the agent tool interface.
