# Runtime

Execution engine for AI agents. Handles the LLM loop, tool dispatch, streaming, retry logic, and parallel tool execution.

## Crates

| Crate | Purpose |
|-------|---------|
| `agent-runtime` | Executor loop, LLM client (OpenAI-compatible), streaming, retry, middleware, MCP |
| `agent-tools` | Built-in tool implementations |

## agent-runtime

The core execution library:

- **Executor**: Runs the agent loop (LLM call → tool execution → repeat) with real streaming
- **LLM Client**: OpenAI-compatible streaming client with retry + exponential backoff, rate limiter
- **Parallel Tools**: Multiple tool calls execute concurrently via `join_all`
- **Output Truncation**: Large tool results capped at 30k chars (head 80% + tail 20%)
- **Middleware**: `MiddlewarePipeline` — summarization, context editing, token counting
- **MCP Manager**: Starts and bridges MCP server tools
- **Steering**: `SteeringQueue` for mid-execution message injection
- **Types**: `StreamEvent` variants, `ChatMessage`, `ExecutorConfig`

Key files: `executor.rs` (main loop), `llm/openai.rs` (streaming client), `llm/retry.rs` (`RetryingLlmClient`)

## agent-tools

Built-in tools organized into core (always registered) and optional (configurable via `ToolSettings`).

### Core Tools (always enabled)

| Tool | Description |
|------|-------------|
| `shell` | Run shell commands (cwd from ward_id, shared venv) |
| `write_file` | Create or overwrite a file |
| `edit_file` | Targeted find-and-replace in existing files |
| `memory` | Persistent key-value store (shared/agent/ward scopes) |
| `ward` | Manage code wards (use, list, create, info) |
| `update_plan` | Lightweight task checklist |
| `set_session_title` | Set human-readable session label |
| `execution_graph` | DAG workflow engine for multi-step orchestration |
| `list_skills` | List available skills |
| `load_skill` | Load skill instructions into context |
| `grep` | Regex search in files |

### Action Tools (registered separately by runner)

| Tool | Description |
|------|-------------|
| `respond` | Send response to user (routes via HookContext) |
| `delegate_to_agent` | Delegate task to a subagent |
| `list_agents` | List available agents |

### Optional Tools (configurable)

| Tool | Setting flag | Description |
|------|-------------|-------------|
| `read`, `write`, `edit`, `glob` | `file_tools` | Additional file operations |
| `todo` | `todos` | Heavy todo list (legacy, replaced by update_plan) |
| `python` | `python` | Execute Python code |
| `web_fetch` | `web_fetch` | HTTP requests |
| `request_input`, `show_content` | `ui_tools` | Request user input / show content |
| `create_agent` | `create_agent` | Create new agents |
| `list_tools`, `list_mcps` | `introspection` | Agent introspection |
| `multimodal_analyze` | always | Vision fallback (always in optional set) |

Key registration functions: `core_tools()`, `optional_tools()`, `builtin_tools_with_fs()`

## Orchestration & Delegation

Root agent acts as orchestrator. The delegation flow:

```
User message → Root agent invoked
  → Root calls delegate_to_agent → spawns subagent execution
  → Root completes (pending_delegations > 0 → request_continuation)
  → Subagent(s) execute in parallel
  → Each subagent completes → callback added to root context
  → Last subagent done → SessionContinuationReady event
  → Root agent re-invoked (continuation turn)
  → Root sees callbacks, decides: respond or delegate more
```

Key files for delegation:

| File | Purpose |
|------|---------|
| `gateway-execution/src/runner/core.rs` | `ExecutionRunner`, lifecycle methods |
| `gateway-execution/src/runner/continuation_watcher.rs` | Listens for `SessionContinuationReady` |
| `gateway-execution/src/lifecycle.rs` | `request_continuation()` on root completion |
| `gateway-execution/src/delegation/spawn.rs` | `complete_delegation()` triggers event |
| `gateway-events/src/lib.rs` | `SessionContinuationReady`, `WardChanged` events |
| `execution-state/src/service.rs` | `StateService` with delegation tracking |

## Does NOT Handle

- Network I/O (that's `gateway/`)
- Session/execution persistence (that's `services/execution-state`)
- HTTP/WebSocket protocols
