# zero-mcp

Model Context Protocol (MCP) client and tool integration for the Zero framework.

## Key Exports

```rust
pub use client::{McpClient, McpServerInfo, McpToolDefinition, MockMcpClient};
pub use config::{McpCommand, McpServerConfig, McpTransport};
pub use connection::{ConnectionState, McpConnection, McpConnectionPool};
pub use error::{McpError, Result};
pub use filter::{accept_all, accept_none, by_names, by_prefix, exclude_names,
    with_property, ToolFilter, ToolPredicate};
pub use schema::{extract_input_schema, sanitize_tool_schema};
pub use tool::{McpTool, McpToolBuilder};
pub use toolset::{McpToolset, McpToolsetBuilder};
pub use zero_core::{Tool, Toolset};  // re-exported for convenience
```

## Architecture

```text
McpToolset  (implements Toolset; applies ToolFilter predicates)
    └── McpConnection  (lifecycle: connecting, ready, error)
          └── McpClient  (RMCP-based protocol: stdio or HTTP/SSE)
```

## Modules

| Module | Purpose |
|--------|---------|
| `client` | `McpClient` trait + `MockMcpClient`; list tools, call tools |
| `config` | `McpServerConfig`, `McpTransport` (Stdio / Http), `McpCommand` |
| `connection` | `McpConnection` + `McpConnectionPool` — lifecycle and pooling |
| `tool` | `McpTool` — wraps an MCP tool as a `zero_core::Tool` |
| `toolset` | `McpToolset` — `Toolset` impl with tool filtering |
| `filter` | `ToolFilter` / `ToolPredicate` — composable predicates for tool selection |
| `schema` | `sanitize_tool_schema()` — strips unsupported JSON Schema constructs for LLM compat |
| `error` | `McpError` variants |

## Transports

| Transport | Config |
|-----------|--------|
| `Stdio` | Spawn a subprocess, communicate over stdin/stdout |
| `Http` | Connect to an HTTP+SSE MCP server |

## Intra-Repo Dependencies

- `zero-core` — `Tool`, `Toolset`, `ToolContext` traits

## Notes

- MCP tools are dynamically discovered by calling `list_tools()` on connect.
- Use `McpToolsetBuilder::with_filter()` to restrict which tools are exposed to the agent.
- `sanitize_tool_schema()` removes JSON Schema features not supported by OpenAI tool calling.
- `MockMcpClient` is available for unit testing without a real server.
