# agent-runtime

A modular AI agent execution framework with MCP support.

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

- Use `// ===` banners for major sections
- Use `async_trait` for trait definitions
- Use `Arc<T>` for shared state
- Return `Result<T>` from fallible operations

## Core Concepts

### Executor

The `Executor` is the main runtime that:
1. Loads agent configuration from YAML
2. Creates LLM client
3. Initializes MCP servers
4. Creates and bridges tools
5. Runs the agent loop

### Configuration

Agents are configured via YAML with:
- Model settings
- MCP server definitions
- Tool definitions
- System instructions

### MCP Integration

MCP servers can be configured as:
- **stdio** - Command-based with stdio transport
- **SSE** - HTTP with Server-Sent Events

Tools from MCP servers are automatically bridged to the `Tool` trait.

## Key Components

- `config.rs` - YAML configuration parsing
- `executor.rs` - Main executor implementation
- `mcp/` - MCP client and tool bridging
- `skills/` - Skill file loading and execution

## Testing

Use `tokio::test` for async tests. Mock LLM and MCP for unit tests.

## Important Notes

- Conversation ID should be passed to tools for scoping
- MCP tools are discovered at runtime from configured servers
- Skill files are YAML-based agent/chain definitions
- See `AGENTS.md` in parent project for additional context
