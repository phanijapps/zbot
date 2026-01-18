# zero-mcp

Model Context Protocol (MCP) integration for the Zero framework.

## Setup

```bash
# Build
cargo build

# Run tests
cargo test
```

## Code Style

- Use `async_trait` for async trait methods
- MCP tools are wrapped to implement the `Tool` trait
- Handle stdio and HTTP transport methods

## MCP Client

The MCP client connects to external MCP servers and exposes their tools as `Tool` implementations.

## Tool Bridge

`McpToolBridge` wraps MCP tools to implement the `zero-core::Tool` trait:
- Translates parameter formats
- Handles execution via MCP protocol
- Converts results back to `Value`

## Transport

Supports:
- **stdio** - Standard input/output communication
- **HTTP/SSE** - HTTP with Server-Sent Events

## Testing

Tests may require actual MCP servers for integration testing.

## Important Notes

- MCP tools are dynamically discovered at runtime
- Server connection must be established before tool execution
- Handle connection errors gracefully
