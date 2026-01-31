# zero-app

Meta-package that combines all Zero framework crates.

## Purpose

This crate provides a convenient dependency that includes all Zero framework modules. Depend on `zero-app` instead of individual crates when you need the full framework.

## Included Crates

- `zero-core` - Core traits and types
- `zero-llm` - LLM abstractions and OpenAI client
- `zero-tool` - Tool definitions
- `zero-session` - Session management
- `zero-agent` - Agent implementations
- `zero-mcp` - MCP integration
- `zero-prompt` - Prompt templates
- `zero-middleware` - Request/response processing

## Usage

```toml
[dependencies]
zero-app = "0.1"
```

This replaces needing to specify each zero-* crate individually.

## Setup

```bash
# Build
cargo build

# Run tests
cargo test
```

## Important Notes

- This is a convenience crate only
- Each sub-crate has its own AGENTS.md with specific details
- Refer to individual crate documentation for implementation details
