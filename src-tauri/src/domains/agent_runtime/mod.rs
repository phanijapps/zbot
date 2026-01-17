// ============================================================================
// AGENT RUNTIME DOMAIN
// Rust-based agent execution
// ============================================================================

pub mod executor;
pub mod tools;
pub mod llm;
pub mod mcp_manager;

// Re-exports
pub use executor::{AgentExecutor, ExecutorConfig, StreamEvent, create_executor};
pub use tools::{ToolRegistry, Tool, ToolContext, ToolError, ToolResult, builtin_tools};
pub use llm::{LlmClient, LlmModel, LlmConfig, ChatMessage, ChatResponse, ToolCall, OpenAiClient};
pub use mcp_manager::{McpManager, McpClient, McpServerConfig, McpTool};
