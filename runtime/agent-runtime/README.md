# Agent Runtime Library

A modular, reusable AI agent execution framework with MCP (Model Context Protocol) support.

## Overview

This library provides a clean, framework-agnostic foundation for building AI agent applications. It separates the core agent execution logic from any specific application framework (Tauri, web, CLI, etc.), making it reusable across different projects.

## Features

- **LLM Abstraction**: Unified interface for multiple LLM providers
- **Tool System**: Extensible registry for built-in and custom tools
- **MCP Support**: Model Context Protocol client for external tools
- **Middleware Pipeline**: Preprocessing and event handling
- **Structured Logging**: Configurable, controllable logging
- **Framework Independent**: Use with Tauri, Axum, Actix, or standalone

## Architecture

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
│  ┌────────────┐  ┌────────────┐  ┌───────────┐               │
│  │     LLM    │  │   Tools    │  │ Middleware│               │
│  │   Client   │  │  Registry  │  │  Pipeline  │               │
│  └────────────┘  └────────────┘  └───────────┘               │
│                                                                  │
│  ┌────────────┐  ┌────────────┐                                 │
│  │     MCP    │  │  Executor  │                                 │
│  │  Manager   │  │            │                                 │
│  └────────────┘  └────────────┘                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Usage

```rust,no_run
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
        println!("{:?}", event);
    }
).await?;
```

## Documentation

See [AGENTS.md](AGENTS.md) for detailed architecture documentation.

## License

MIT
