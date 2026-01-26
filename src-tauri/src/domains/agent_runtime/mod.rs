// ============================================================================
// AGENT RUNTIME DOMAIN
// Re-exports agent-runtime library with Tauri-specific adaptations
// Now integrated with zero-app framework
// ============================================================================

// Re-export everything from agent-runtime library
pub use agent_runtime::{
    // Types
    ChatMessage, StreamEvent, ToolCall, ToolResult, ToolError,

    // LLM
    LlmClient, LlmConfig, OpenAiClient, ChatResponse, StreamChunk, StreamCallback, TokenUsage,

    // Tools
    Tool, ToolRegistry, ToolContext,
    FileSystemContext, NoFileSystemContext,

    // MCP
    McpManager, McpServerConfig, McpClient, McpTool, McpError,

    // Middleware
    MiddlewarePipeline, PreProcessMiddleware, EventMiddleware, MiddlewareContext, MiddlewareEffect,
    MiddlewareConfig, SummarizationMiddleware, ContextEditingMiddleware,
    SummarizationConfig, ContextEditingConfig, TriggerCondition, KeepPolicy,

    // Executor (types only, not the factory function)
    ExecutorConfig, ExecutorError,

    // Logging
    init_logging, LogLevel,
};

// Zero framework integration
pub use zero_app::prelude::*;

// Tauri-specific modules
pub mod filesystem;
pub mod config_adapter;
pub mod types;
pub mod middleware_integration;
pub mod executor_v2;
pub mod subagent_tool;
pub mod state_keys;
pub mod workflow_integration;

// Re-export zero-app executor
pub use self::executor_v2::{
    ZeroAppExecutor,
    ZeroAppStreamEvent,
    create_zero_executor,
    create_subagent_executor,
};

// Re-export Tauri-specific types
pub use self::types::{
    TauriAgentEvent,
    TauriContent,
    TauriLlmConfig,
    TauriMcpServerConfig,
    TauriMiddlewareConfig,
    TauriStreamEvent,
    TauriToolCallInfo,
};
