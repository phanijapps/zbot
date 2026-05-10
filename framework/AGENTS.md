# Zero Framework

Core abstractions for building AI agents. Designed to be publishable as a standalone crate (`zero-agent-framework`).

## Crates

| Crate | Purpose |
|-------|---------|
| `zero-core` | Core traits: `Agent`, `Tool`, `Toolset`, `Event`, `FileSystemContext`, errors |
| `zero-llm` | LLM abstractions (`Llm` trait) and OpenAI-compatible client + encoder |
| `zero-tool` | Tool registry (`ToolRegistry`), `FunctionTool`, `ToolContextImpl` |
| `zero-mcp` | Model Context Protocol — client, connection pool, tool wrapping, filtering |
| `zero-session` | Session and state management (`InMemorySession`, `InMemoryState`) |
| `zero-prompt` | Template rendering with `{var}` syntax, session state injection |
| `zero-middleware` | Re-export shim — all logic lives in `agent_runtime::middleware` |
| `zero-agent` | Agent implementations: `LlmAgent`, workflow agents, `OrchestratorAgent` |
| `zero-app` | Convenience aggregator: re-exports all above + `ZeroAppBuilder`/`ZeroApp` |

## Usage

```rust
use zero_app::prelude::*;
```

## Design Principles

1. **Trait-based abstractions** — Implement `Tool`, `Agent`, `Toolset` for custom behavior
2. **No I/O in core** — Network/storage injected via traits
3. **Async-first** — Built on tokio
4. **Composable** — Mix and match crates as needed

## Dependency Graph

```
zero-core (foundation)
    ├── zero-llm
    ├── zero-tool
    ├── zero-mcp
    ├── zero-session
    ├── zero-prompt
    └── zero-middleware  ← re-exports agent-runtime::middleware
            │
            └── zero-agent  (LlmAgent, OrchestratorAgent, workflow agents)
                    │
                    └── zero-app (aggregator prelude + ZeroAppBuilder)
```

## Does NOT Contain

- Network I/O (that's `gateway/`)
- Concrete tool implementations (that's `runtime/agent-tools`)
- SQLite or persistence (that's `stores/`)
- Application-specific logic
