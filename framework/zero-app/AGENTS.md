# zero-app

Integration crate for the Zero framework. Aggregates all `zero-*` crates into a unified re-export surface and provides `ZeroAppBuilder` / `ZeroApp` for high-level application setup.

## Usage

```toml
[dependencies]
zero-app = { path = "framework/zero-app" }
```

```rust
use zero_app::prelude::*;

let model = Arc::new(OpenAiLlm::new(config));
let agent = LlmAgent::builder("assistant", model)
    .system_instruction("You are helpful.")
    .build();
```

## Key Re-exports (flat)

```rust
// Core
pub use zero_core::{Agent, Tool, Toolset, Event, Result, ZeroError, FileSystemContext, ...};
// LLM
pub use zero_llm::{Llm, LlmConfig, OpenAiLlm, LlmRequest, LlmResponse, ToolCall, ...};
// Tools
pub use zero_tool::{FunctionTool, ToolRegistry};
// Session
pub use zero_session::{InMemorySession, InMemoryState, SessionService, ...};
// Agents
pub use zero_agent::{LlmAgent, LlmAgentBuilder, OrchestratorAgent, OrchestratorBuilder, ...};
// Workflow agents
pub use zero_agent::workflow::{ConditionalAgent, LoopAgent, ParallelAgent, SequentialAgent, ...};
// MCP
pub use zero_mcp::{McpClient, McpServerConfig, McpToolset, McpTransport, McpCommand, ...};
// Prompt
pub use zero_prompt::{Template, TemplateRenderer, inject_session_state};
// Middleware
pub use zero_middleware::{MiddlewarePipeline, MiddlewareConfig, SummarizationMiddleware, ...};
```

A `prelude` module provides the most common types for glob import.

## Application Builder

```rust
let app = ZeroAppBuilder::new()
    .with_llm_config(LlmConfig::new(api_key, model))
    .with_mcp_server(McpServerConfig::stdio("name", "Desc", "/path"))
    .with_middleware_config(config)
    .build()?;

let session = app.create_session(session_id, app_name, user_id);
let registry = app.create_tool_registry().await?;
```

## Intra-Repo Dependencies

- All `framework/zero-*` crates (re-exports their public surfaces)
