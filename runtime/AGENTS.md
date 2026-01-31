# Runtime

Execution engine for AI agents. Handles the LLM loop, tool dispatch, and middleware application.

## Crates

| Crate | Purpose |
|-------|---------|
| `agent-runtime` | Executor, LLM loop, middleware pipeline, MCP management |
| `agent-tools` | Built-in tool implementations |

## agent-runtime

The core execution library:

- **Executor**: Runs the agent loop (LLM call → tool execution → repeat)
- **LLM Client**: OpenAI-compatible streaming client
- **Middleware**: Summarization, context editing
- **Types**: Messages, events, streaming

## agent-tools

Built-in tools available to agents:

| Tool | Description |
|------|-------------|
| `read_file` | Read file contents |
| `write_file` | Write content to file |
| `list_dir` | List directory contents |
| `execute_command` | Run shell commands |
| `memory` | Persistent key-value store |
| `list_skills` | List available skills |
| `list_agents` | List available agents |
| `delegate_to_agent` | Delegate task to subagent |
| `respond` | Send response to user |

## Responsibilities

- Execute agent invocations
- Manage tool lifecycle
- Handle streaming responses
- Apply middleware transformations

## Does NOT Handle

- Network I/O (that's `gateway/`)
- Persistence (that's `services/`)
- HTTP/WebSocket protocols
