// ============================================================================
// AGENT RUNTIME DOMAIN
// Re-exports agent-runtime library with Tauri-specific adaptations
// ============================================================================

// Re-export everything from agent-runtime library
// Note: executor module is provided locally (Tauri-specific config loading)
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

// Re-export builtin_tools_with_fs from zerotools
pub use zerotools::builtin_tools_with_fs;

// Tauri-specific modules
pub mod filesystem;

// Tauri-specific executor factory (uses agent-runtime types)
pub mod executor;

// Re-export AgentExecutor from executor module
pub use self::executor::AgentExecutor;
