# agent-runtime

Core AI agent execution framework with real streaming, retry logic, parallel tool execution, and MCP support.

## Build & Test

```bash
cargo test -p agent-runtime      # 43 tests
cargo build -p agent-runtime
```

## Key Components

| File | Purpose |
|------|---------|
| `executor.rs` | Main execution loop: LLM call → tool dispatch → repeat. Parallel tool execution via `join_all`. Output truncation (30k chars). |
| `llm/client.rs` | `LlmClient` trait: `chat()` and `chat_stream()` with tools parameter |
| `llm/openai.rs` | OpenAI-compatible streaming client. Handles SSE parsing, tool call assembly, thinking tokens. |
| `llm/retry.rs` | `RetryingLlmClient` wrapper: 3 retries, exponential backoff with jitter, handles 429/5xx/transport errors |
| `llm/config.rs` | `LlmConfig`: base_url, api_key, model, temperature, max_tokens, thinking |
| `types/events.rs` | `StreamEvent` enum: Token, ToolCallStart, ToolResult, WardChanged, ActionDelegate, TurnComplete, etc. |
| `types/messages.rs` | `ChatMessage`, `ToolCall`, `ChatResponse` types |
| `tools/registry.rs` | `ToolRegistry`: register/lookup tools by name |
| `tools/builtin.rs` | `RespondTool`, `DelegateTool` (action tools) |
| `middleware/` | `MiddlewarePipeline`, summarization, context editing |
| `mcp/` | `McpManager`: start/stop MCP servers, bridge tools to Tool trait |

## Execution Flow

```
User message + history
    → Middleware preprocessing
    → LLM streaming call (chat_stream with tools)
    → If tool calls: execute in parallel → append results → loop
    → If respond/delegate action: emit event → break
    → If no tool calls: emit TurnComplete → break
    → Max iterations safety check
```

## StreamEvent Variants

Events emitted during execution (consumed by gateway's stream processor):

- `TurnStart` — New LLM turn beginning
- `Token` — Streamed text content
- `Reasoning` — Thinking/reasoning content
- `ToolCallStart` / `ToolCallArgs` / `ToolResult` — Tool lifecycle
- `ActionRespond` / `ActionDelegate` — Agent decisions
- `WardChanged` — Agent switched code ward
- `TokenUpdate` — Token usage counts
- `TurnComplete` — Turn finished with final message
- `Error` — Recoverable/non-recoverable errors
- `ShowContent` / `RequestInput` — UI interactions

## Code Style

- Use `// ===` banners for major sections
- Use `async_trait` for trait definitions
- Use `Arc<T>` for shared state
- Return `Result<T>` from fallible operations
