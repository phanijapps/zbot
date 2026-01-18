# Zero Framework Crates

This directory contains the modular Zero framework - a set of Rust crates for building AI agent applications.

## Framework Crates

| Crate | Purpose |
|-------|---------|
| `zero-core` | Core traits: Agent, Tool, Session, Event, Content, errors |
| `zero-llm` | LLM trait, OpenAI client, request/response types |
| `zero-agent` | Agent implementations: LlmAgent, workflow agents |
| `zero-tool` | Tool trait and abstractions |
| `zero-session` | Session trait and in-memory implementation |
| `zero-mcp` | MCP client and tool bridging |
| `zero-prompt` | Prompt template system |
| `zero-middleware` | Middleware pipeline for request/response processing |
| `zero-app` | Meta-package importing all zero-* crates |

## Application Crates

| Crate | Purpose |
|-------|---------|
| `agent-runtime` | YAML config, executor, MCP managers, skill loading |
| `agent-tools` | Built-in tools: Read, Write, Edit, Grep, Glob, Python, etc. |

## Development

```bash
# Build all crates
cargo build

# Run tests
cargo test

# Run with logging
RUST_LOG=debug cargo test
```

## Workspace Structure

This is a Cargo workspace. Shared dependencies are defined in the root `Cargo.toml`.

## Quick Reference

- See individual crate `AGENTS.md` files for detailed information
- See `memory-bank/architecture.md` for system architecture
- See `memory-bank/learnings.md` for implementation patterns
