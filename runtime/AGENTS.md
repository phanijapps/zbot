# Runtime

Execution engine for AI agents. Handles the LLM loop, tool dispatch, streaming, retry logic, and parallel tool execution.

## Crates

| Crate | Purpose |
|-------|---------|
| `agent-runtime` | Executor loop, LLM client (OpenAI-compatible), streaming, retry, middleware, MCP |
| `agent-tools` | Built-in tool implementations (12 core + optional) |

## agent-runtime

The core execution library:

- **Executor**: Runs the agent loop (LLM call → tool execution → repeat) with real streaming
- **LLM Client**: OpenAI-compatible streaming client with retry + exponential backoff
- **Parallel Tools**: Multiple tool calls execute concurrently via `join_all`
- **Output Truncation**: Large tool results capped at 30k chars (head 80% + tail 20%)
- **Middleware**: Summarization, context editing pipeline
- **MCP Manager**: Starts and bridges MCP server tools
- **Types**: StreamEvent variants, ChatMessage, ExecutorConfig

Key files: `executor.rs` (main loop), `llm/openai.rs` (streaming client), `llm/retry.rs` (RetryingLlmClient)

## agent-tools

Built-in tools organized by category:

### Core (always enabled)
| Tool | Description |
|------|-------------|
| `shell` | Run shell commands (cwd from ward_id, shared venv) |
| `read` | Read file contents (resolves via ward_dir when ward set) |
| `write` | Write content to file (resolves via ward_dir) |
| `edit` | Search and replace in files (resolves via ward_dir) |
| `memory` | Persistent key-value store (shared/agent/ward scopes) |
| `ward` | Manage code wards (use, list, create, info) |
| `todo` | Task management |
| `list_skills` | List available skills |
| `load_skill` | Load skill instructions into context |
| `grep` | Regex search in files |
| `glob` | File pattern matching |

### Action (always enabled)
| Tool | Description |
|------|-------------|
| `respond` | Send response to user |
| `delegate_to_agent` | Delegate task to subagent |
| `list_agents` | List available agents |

### Optional (configurable)
| Tool | Description |
|------|-------------|
| `python` | Execute Python code |
| `web_fetch` | HTTP requests |
| `ui_tools` | Request input, show content |
| `knowledge_graph` | Entity-relationship storage |
| `create_agent` | Create new agents |
| `introspection` | Agent introspection |

Key files: `tools/ward.rs`, `tools/file.rs`, `tools/execution/shell.rs`, `tools/memory.rs`, `tools/mod.rs` (registration)

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
| `gateway-execution/src/runner.rs` | `spawn_continuation_handler()`, `invoke_continuation()` |
| `gateway-execution/src/lifecycle.rs` | `request_continuation()` on root completion |
| `gateway-execution/src/delegation/spawn.rs` | `complete_delegation()` triggers event |
| `gateway-events/src/lib.rs` | `SessionContinuationReady`, `WardChanged` events |
| `execution-state/src/service.rs` | StateService with delegation tracking |

## Does NOT Handle

- Network I/O (that's `gateway/`)
- Session/execution persistence (that's `services/execution-state`)
- HTTP/WebSocket protocols
