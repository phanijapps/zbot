// ============================================================================
// AGENT RUNTIME LIBRARY
// A modular AI agent execution framework with MCP support
// ============================================================================

//! # Agent Runtime Library
//!
//! A modular, reusable AI agent execution framework designed to be independent
//! of any specific application framework. It provides the core building blocks
//! for executing AI agents with tool calling, MCP (Model Context Protocol) support,
//! and extensible middleware.
//!
//! ## Architecture
//!
//! The library follows Clean Architecture principles with clear separation of concerns:
//!
//! - **Types**: Shared data structures (messages, events, tool calls)
//! - **LLM**: Abstraction over LLM providers (OpenAI, Anthropic, etc.)
//! - **Tools**: Extensible tool registry and execution framework
//! - **MCP**: Model Context Protocol client for external tool integration
//! - **Middleware**: Pipeline for message preprocessing and event handling
//! - **Executor**: Core orchestrator coordinating all components
//! - **Logging**: Structured, controllable logging utilities
//!
//! ## Usage Example
//!
//! ```rust,no_run
//! use agent_runtime::{
//!     AgentExecutor, ExecutorConfig, create_executor,
//!     ChatMessage, LogLevel
//! };
//!
//! // Initialize logging
//! agent_runtime::init_logging(LogLevel::Info);
//!
//! // Create configuration
//! let config = ExecutorConfig {
//!     agent_id: "my-agent".to_string(),
//!     provider_id: "openai".to_string(),
//!     model: "gpt-4".to_string(),
//!     temperature: 0.7,
//!     max_tokens: 2000,
//!     // ... other fields
//! };
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

// ============================================================================
// PUBLIC API RE-EXPORTS
// ============================================================================

//! # Module Documentation
//!
//! This library is organized into the following modules:

/// Core types
pub mod types;

/// LLM client abstraction
pub mod llm;

/// Tool system
pub mod tools;

/// MCP protocol support
pub mod mcp;

/// Middleware pipeline
pub mod middleware;

/// Executor core
pub mod executor;

/// Logging utilities
pub mod logging;

// ============================================================================
// CONVENIENT RE-EXPORTS
// ============================================================================

pub use types::{
    ChatMessage, StreamEvent, ToolCall, ToolResult, ToolError
};

pub use llm::{
    LlmClient, LlmConfig, LlmModel, OpenAiClient, StreamChunk, StreamCallback
};

pub use tools::{
    Tool, ToolRegistry, ToolContext
};
pub use tools::error::ToolError as ToolExecError;

pub use mcp::{
    McpManager, McpClient, McpServerConfig, McpTool
};

pub use middleware::{
    MiddlewarePipeline,
    PreProcessMiddleware,
    EventMiddleware,
    MiddlewareContext,
    MiddlewareConfig,
    SummarizationMiddleware,
    ContextEditingMiddleware,
};

pub use executor::{
    AgentExecutor, ExecutorConfig, create_executor
};

pub use logging::{
    init_logging, LogLevel
};
