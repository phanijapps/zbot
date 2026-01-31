# Zero Framework

Core abstractions for building AI agents. This layer is designed to be publishable as a standalone crate (`zero-agent-framework`).

## Crates

| Crate | Purpose |
|-------|---------|
| `zero-core` | Core traits: Agent, Tool, Toolset, Event, errors |
| `zero-llm` | LLM abstractions and OpenAI client |
| `zero-tool` | Tool registry and execution |
| `zero-mcp` | Model Context Protocol integration |
| `zero-session` | State and session management |
| `zero-prompt` | Template rendering with state injection |
| `zero-middleware` | Message preprocessing pipelines |
| `zero-agent` | Agent implementations (LlmAgent, workflow agents) |
| `zero-app` | Convenience prelude re-exporting all crates |

## Usage

```rust
use zero_app::prelude::*;
```

## Design Principles

1. **Trait-based abstractions** - Implement `Tool`, `Agent`, `Toolset` for custom behavior
2. **No I/O in core** - Network/storage injected via traits
3. **Async-first** - Built on tokio
4. **Composable** - Mix and match crates as needed

## Dependencies

```
zero-core (foundation)
    ├── zero-llm
    ├── zero-tool
    ├── zero-mcp
    ├── zero-session
    ├── zero-prompt
    └── zero-middleware
            │
            └── zero-agent
                    │
                    └── zero-app (aggregator)
```

## Does NOT Contain

- Network I/O (that's `gateway/`)
- Tool implementations (that's `runtime/agent-tools`)
- Application-specific logic
