# Agent Runtime Library

## Overview

`agent-runtime` is a modular, reusable AI agent execution framework designed to be independent of any specific application framework. It provides the core building blocks for executing AI agents with tool calling, MCP (Model Context Protocol) support, and extensible middleware.

## Architecture

This library follows **Clean Architecture** principles with clear separation of concerns:

```
┌─────────────────────────────────────────────────────────────────┐
│                     Application Layer                           │
│                   (Tauri, CLI, Web, etc.)                      │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                   Agent Runtime Library                         │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                    Executor                              │   │
│  │  ┌────────────┐  ┌────────────┐  ┌───────────┐       │   │
│  │  │     LLM    │  │   Tools    │  │ Middleware│       │   │
│  │  │   Client   │  │  Registry  │  │  Pipeline  │       │   │
│  │  └────────────┘  └────────────┘  └───────────┘       │   │
│  │                                                          │   │
│  │  ┌────────────┐  ┌────────────┐                         │   │
│  │  │     MCP    │  │  Types     │                         │   │
│  │  │  Manager   │  │  Module    │                         │   │
│  │  └────────────┘  └────────────┘                         │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                   External Dependencies                         │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐               │
│  │  OpenAI    │  │  Anthropic │  │   Custom   │               │
│  │ Compatible │  │    Claude   │  │    APIs    │               │
│  └────────────┘  └────────────┘  └────────────┘               │
└─────────────────────────────────────────────────────────────────┘
```

## Module Structure

### `types` - Shared Data Structures

Common types used throughout the library.

**Key Types:**
- `ChatMessage` - LLM chat message format
- `StreamEvent` - Events emitted during execution
- `ToolCall` - Tool invocation representation
- `ToolResult` - Tool execution result

**Design Principle:** Types are framework-agnostic and use `serde` for serialization.

### `llm` - LLM Client Abstraction

Abstract interface for LLM providers with OpenAI-compatible implementations.

**Key Types:**
- `LlmClient` - Trait for LLM operations
- `LlmConfig` - Configuration for LLM clients
- `OpenAiClient` - OpenAI-compatible implementation

**Design Principle:** Dependency injection - configuration is passed in, not read from files.

### `tools` - Tool Execution System

Extensible tool registry and execution framework.

**Key Types:**
- `Tool` - Trait for tool implementations
- `ToolRegistry` - Registry of available tools
- `ToolContext` - Execution context with conversation scoping
- `ToolError` - Error type for tool operations

**Design Principle:** Tools are isolated, stateless functions with context injection.

### `mcp` - MCP Protocol Support

Model Context Protocol client for external tool integration.

**Key Types:**
- `McpManager` - Manages MCP server connections
- `McpClient` - Individual MCP client
- `McpServerConfig` - Server configuration
- `McpTool` - Tool provided by MCP server

**Design Principle:** Supports multiple transports (stdio, HTTP, SSE) with clean abstractions.

### `middleware` - Middleware Pipeline

Extensible middleware for preprocessing and event handling.

**Key Types:**
- `MiddlewarePipeline` - Orchestrates middleware execution
- `PreProcessMiddleware` - Trait for message preprocessing
- `EventMiddleware` - Trait for event handling
- `SummarizationMiddleware` - Conversation summarization
- `ContextEditingMiddleware` - Tool result management

**Design Principle:** Chain of responsibility pattern with clean trait boundaries.

### `executor` - Core Execution Engine

Main orchestrator that coordinates LLM, tools, MCP, and middleware.

**Key Types:**
- `AgentExecutor` - Main executor struct
- `ExecutorConfig` - Configuration for execution
- `create_executor` - Factory function

**Design Principle:** Executor depends on traits, not concrete implementations.

## Usage Example

```rust
use agent_runtime::{
    AgentExecutor, ExecutorConfig, create_executor,
    ChatMessage, LogLevel
};

// Initialize logging
agent_runtime::init_logging(LogLevel::Info);

// Create configuration
let config = ExecutorConfig {
    agent_id: "my-agent".to_string(),
    provider_id: "openai".to_string(),
    model: "gpt-4".to_string(),
    temperature: 0.7,
    max_tokens: 2000,
    // ... other fields
};

// Create executor
let executor = create_executor(config).await?;

// Execute with streaming
let history = vec![];
executor.execute_stream(
    "Hello, agent!",
    &history,
    |event| {
        // Handle streaming events
        println!("{:?}", event);
    }
).await?;
```

## Design Principles

### 1. **Dependency Injection**

All dependencies are passed via constructors, not loaded internally:

```rust
// ✅ GOOD: Configuration passed in
let executor = AgentExecutor::new(config, llm_client, tool_registry)?;

// ❌ BAD: Reading from files inside
let executor = AgentExecutor::from_agent_id("agent-id")?;
```

### 2. **Trait-Based Abstractions**

Core functionality uses traits for flexibility:

```rust
pub trait LlmClient: Send + Sync {
    async fn chat(&self, messages: Vec<ChatMessage>) -> Result<ChatResponse>;
    async fn chat_stream(&self, messages: Vec<ChatMessage>) -> StreamEvent;
}
```

### 3. **Framework Independence**

No dependencies on Tauri, web frameworks, or specific applications:

```rust
// Library can be used from:
// - Tauri desktop apps
// - CLI tools
// - Web servers (Axum, Actix)
// - Other Rust applications
```

### 4. **Error Handling with `thiserror`**

Proper error types instead of `String`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum ExecutorError {
    #[error("LLM client error: {0}")]
    LlmError(#[from] LlmError),

    #[error("Tool execution failed: {0}")]
    ToolError(String),
}
```

### 5. **Structured Logging with `tracing`**

Controlled, structured logging instead of `println!`:

```rust
use tracing::{info, warn, error, debug};

info!("Agent {} starting execution", agent_id);
debug!("Processing {} messages", messages.len());
warn("Tool '{}' not found", tool_name);
error!("Execution failed: {}", err);
```

## Configuration Injection Pattern

The library uses configuration injection to remain framework-agnostic:

```rust
// Define a trait for config loading
pub trait ConfigProvider: Send + Sync {
    fn get_llm_config(&self, provider_id: &str) -> Result<LlmConfig>;
    fn get_agent_config(&self, agent_id: &str) -> Result<AgentConfig>;
}

// Application implements this
struct TauriConfigProvider {
    app_dirs: Arc<AppDirs>,
}

impl ConfigProvider for TauriConfigProvider {
    fn get_llm_config(&self, provider_id: &str) -> Result<LlmConfig> {
        // Load from Tauri config directory
    }
}
```

## Testing

The library is designed for easy testing:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    struct MockLlmClient;

    #[async_trait]
    impl LlmClient for MockLlmClient {
        async fn chat(&self, _messages: Vec<ChatMessage>) -> Result<ChatResponse> {
            Ok(ChatResponse {
                content: "Mock response".to_string(),
                tool_calls: None,
            })
        }
    }

    #[tokio::test]
    async fn test_executor_with_mock() {
        let executor = AgentExecutor::new(
            config,
            Arc::new(MockLlmClient),
            Arc::new(ToolRegistry::new()),
        ).unwrap();

        // Test execution...
    }
}
```

## Future Enhancements

1. **Plugin System**: Dynamic loading of custom tools and middleware
2. **Multi-Agent**: Support for agent-to-agent communication
3. **Observability**: Built-in metrics and tracing
4. **Rate Limiting**: Configurable rate limiting for LLM calls
5. **Caching**: Response caching for repeated queries

## Related Documentation

| File | Description |
|------|-------------|
| `src/lib.rs` | Public API exports |
| `src/executor/mod.rs` | Executor implementation |
| `src/tools/mod.rs` | Tool system details |
| `src/llm/mod.rs` | LLM client abstraction |
| `src/middleware/mod.rs` | Middleware pipeline |
| `../../ARCHITECTURE.md` | Overall system architecture |
