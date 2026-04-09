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
//! For detailed usage examples, see the README.md file.

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

/// Steering queue for mid-execution message injection
pub mod steering;

/// Logging utilities
pub mod logging;

// ============================================================================
// CONVENIENT RE-EXPORTS
// ============================================================================

pub use types::{
    ChatMessage, StreamEvent, ToolCall, ToolResult, ToolError
};

pub use llm::{
    LlmClient, LlmError, LlmConfig, OpenAiClient, StreamChunk, StreamCallback,
    ToolCallChunk, ChatResponse, TokenUsage,
    RetryingLlmClient, RetryPolicy, ThrottledLlmClient,
    ProviderRateLimiter, RateLimitedLlmClient, NonStreamingLlmClient,
    EmbeddingClient, EmbeddingConfig, EmbeddingProviderType, EmbeddingError,
    OpenAiEmbeddingClient, LocalEmbeddingClient, content_hash,
};

pub use tools::{
    Tool, ToolRegistry, ToolContext,
    FileSystemContext, NoFileSystemContext,
    RespondTool, DelegateTool,
};
pub use tools::error::ToolError as ToolExecError;

pub use mcp::{
    McpManager, McpClient, McpServerConfig, McpTool, McpError
};

pub use middleware::{
    MiddlewarePipeline,
    PreProcessMiddleware,
    EventMiddleware,
    MiddlewareContext,
    MiddlewareEffect,
    MiddlewareConfig,
    SummarizationMiddleware,
    ContextEditingMiddleware,
    SummarizationConfig,
    ContextEditingConfig,
    TriggerCondition,
    KeepPolicy,
};

pub use executor::{
    AgentExecutor, ExecutorConfig, ExecutorError, RecallHook, RecallHookResult, create_executor,
    ToolCallDecision, ToolExecutionMode, BeforeToolCallHook, AfterToolCallHook,
    TransformContextHook,
};

pub use steering::{
    SteeringQueue, SteeringHandle, SteeringMessage, SteeringSource, SteeringPriority,
};

pub use logging::{
    init_logging, init_logging_from_env, LogLevel
};

// Logging macros are available at crate root via #[macro_export] in logging module
// Use: agent_info!, agent_warn!, agent_error!, agent_debug!
