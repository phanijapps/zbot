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
//! - **LLM**: Abstraction over LLM providers (`OpenAI`, Anthropic, etc.)
//! - **Tools**: Extensible tool registry and execution framework
//! - **MCP**: Model Context Protocol client for external tool integration
//! - **Middleware**: Pipeline for message preprocessing and event handling
//! - **Executor**: Core orchestrator coordinating all components
//! - **Logging**: Structured, controllable logging utilities
//!
//! For detailed usage examples, see the README.md file.

// Note: clippy::all and clippy::pedantic lints are managed via crate-level allows above
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

/// Progress tracking for loop detection
pub(crate) mod progress;

/// Tool schema normalization and hardening helpers
pub(crate) mod tool_schema;

/// Context management helpers (compaction, sanitization, truncation)
pub(crate) mod context_management;

/// Steering queue for mid-execution message injection
pub mod steering;

/// Registry mapping execution IDs to live SteeringHandles
pub mod steering_registry;

/// Logging utilities
pub mod logging;

// ============================================================================
// CONVENIENT RE-EXPORTS
// ============================================================================

pub use types::{ChatMessage, StreamEvent, ToolCall, ToolError, ToolResult};

pub use llm::{
    content_hash, ChatResponse, EmbeddingClient, EmbeddingConfig, EmbeddingError,
    EmbeddingProviderType, LlmClient, LlmConfig, LlmError, LocalEmbeddingClient,
    NonStreamingLlmClient, OpenAiClient, OpenAiEmbeddingClient, ProviderRateLimiter,
    RateLimitedLlmClient, RetryPolicy, RetryingLlmClient, StreamCallback, StreamChunk,
    ThrottledLlmClient, TokenUsage, ToolCallChunk,
};

pub use tools::error::ToolError as ToolExecError;
pub use tools::{
    DelegateTool, FileSystemContext, NoFileSystemContext, RespondTool, Tool, ToolContext,
    ToolRegistry,
};

pub use mcp::{McpClient, McpError, McpManager, McpServerConfig, McpTool};

pub use middleware::{
    ContextEditingConfig, ContextEditingMiddleware, EventMiddleware, KeepPolicy, MiddlewareConfig,
    MiddlewareContext, MiddlewareEffect, MiddlewarePipeline, PlanBlockMiddleware,
    PreProcessMiddleware, SummarizationConfig, SummarizationMiddleware, TriggerCondition,
};

pub use executor::{
    create_executor, AfterToolCallHook, AgentExecutor, BeforeToolCallHook, ExecutorConfig,
    ExecutorError, RecallHook, RecallHookResult, ToolCallDecision, ToolExecutionMode,
    TransformContextHook,
};

pub use steering::{
    SteeringHandle, SteeringMessage, SteeringPriority, SteeringQueue, SteeringSource,
};

pub use steering_registry::{SteerResult, SteeringRegistry};

pub use logging::{init_logging, init_logging_from_env, LogLevel};

// Logging macros are available at crate root via #[macro_export] in logging module
// Use: agent_info!, agent_warn!, agent_error!, agent_debug!
