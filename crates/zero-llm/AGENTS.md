# zero-llm

LLM abstractions and OpenAI-compatible client for the Zero framework.

## Setup

```bash
# Build
cargo build

# Run tests
cargo test

# Run with logging
RUST_LOG=debug cargo test
```

## Code Style

- Use `async_trait` for the `Llm` trait
- Stream responses using `async_stream::stream!` macro
- Log request/response details at `INFO` level for debugging
- Use `serde_json::Value` for tool arguments to remain flexible

## Core Traits

- `Llm` - LLM client trait with `generate()` and `generate_stream()` methods
- `LlmRequest` - Request with messages, tools, temperature, etc.
- `LlmResponse` - Response with content, tool calls, usage info
- `ToolCall` - Individual tool call with id, name, and arguments

## OpenAI Client

The `OpenAiLlm` client implements the `Llm` trait and works with any OpenAI-compatible API.

**Configuration:**
```rust
let config = LlmConfig::new(api_key, model)
    .with_base_url("https://api.openai.com/v1")  // optional
    .with_temperature(0.7)
    .with_max_tokens(4096);
```

## Tool Calls

Tool calls are parsed from OpenAI's response format. Arguments are serialized as JSON strings and must be parsed using `serde_json::from_str`.

**Debug logging is added for tracking:**
- Tool call count
- Argument parsing success/failure
- Keys present in parsed arguments

## Testing

Tests cover basic client creation and request conversion. For API tests, use environment variables for credentials.

## Important Notes

- The `finish_reason` field indicates if content was truncated ("length")
- Tool calls are returned in `choices[0].message.tool_calls`
- Check `errorlog.txt` for detailed request/response logging when debugging
