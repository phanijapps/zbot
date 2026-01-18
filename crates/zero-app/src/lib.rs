//! # Zero App
//!
//! Integration crate for the Zero framework. This crate aggregates all zero-* crates
//! and provides a unified API for building agent applications.
//!
//! ## Example
//!
//! ```ignore
//! use zero_app::prelude::*;
//!
//! let model = Arc::new(OpenAiLlm::new(config));
//! let agent = LlmAgent::builder("assistant", model)
//!     .system_instruction("You are a helpful assistant")
//!     .build();
//! ```

// Re-export core types from zero-core
pub use zero_core::{
    Agent,
    Content,
    Part,
    Event,
    EventStream,
    EventActions,
    Result,
    ZeroError,
    ReadonlyContext,
    CallbackContext,
    InvocationContext,
    RunConfig,
    BeforeAgentCallback,
    AfterAgentCallback,
    Tool,
    Toolset,
    ToolContext,
    FileSystemContext,
    NoFileSystemContext,
    context::Session,
};

// Re-export LLM types
pub use zero_llm::{
    Llm,
    LlmRequest,
    LlmResponse,
    LlmResponseChunk,
    LlmResponseStream,
    ToolCall,
    TokenUsage,
    ToolDefinition,
    LlmConfig,
    OpenAiLlm,
};

// Re-export tool types
pub use zero_tool::{
    ToolRegistry,
    FunctionTool,
};

// Re-export session types
pub use zero_session::{
    InMemorySession,
    MutexSession,
    SessionService,
    InMemoryState,
    State,
    Session as SessionTrait,
};

// Re-export agent types
pub use zero_agent::{
    LlmAgent,
    LlmAgentBuilder,
};

// Re-export workflow agents
pub use zero_agent::workflow::{
    SequentialAgent,
    ParallelAgent,
    LoopAgent,
    ConditionalAgent,
    LlmConditionalAgent,
    LlmConditionalAgentBuilder,
    CustomAgent,
    CustomAgentBuilder,
};

// Re-export MCP types
pub use zero_mcp::{
    McpClient,
    McpToolset,
    McpServerConfig,
    McpTransport,
    McpCommand,
    filter::ToolFilter,
    connection::McpConnectionPool,
};

// Re-export prompt types
pub use zero_prompt::{
    Template,
    TemplateRenderer,
    inject_session_state,
    PromptError,
};

// Re-export middleware types
pub use zero_middleware::{
    MiddlewarePipeline,
    PreProcessMiddleware,
    EventMiddleware,
    MiddlewareContext,
    MiddlewareEffect,
    MiddlewareConfig,
    SummarizationMiddleware,
    ContextEditingMiddleware,
    TriggerCondition,
    KeepPolicy,
};

/// Prelude module for convenient imports
pub mod prelude {
    // Core
    pub use zero_core::{
        Agent,
        Content,
        Part,
        Event,
        EventStream,
        EventActions,
        Result,
        ZeroError,
        ReadonlyContext,
        CallbackContext,
        InvocationContext,
        RunConfig,
        Tool,
        Toolset,
        ToolContext,
        FileSystemContext,
        NoFileSystemContext,
        context::Session,
    };

    // LLM
    pub use zero_llm::{Llm, OpenAiLlm, LlmConfig};

    // Agents
    pub use zero_agent::{LlmAgent, LlmAgentBuilder};

    // Workflow
    pub use zero_agent::workflow::{
        SequentialAgent,
        ParallelAgent,
        LoopAgent,
        ConditionalAgent,
        LlmConditionalAgent,
        CustomAgent,
    };

    // Tools
    pub use zero_tool::{ToolRegistry, FunctionTool, ToolContextImpl};

    // Session
    pub use zero_session::{InMemorySession, SessionService, InMemoryState};

    // MCP
    pub use zero_mcp::{McpToolset, McpServerConfig, McpTransport, McpCommand};

    // Prompt
    pub use zero_prompt::{Template, TemplateRenderer};

    // Middleware
    pub use zero_middleware::{
        MiddlewarePipeline,
        MiddlewareConfig,
        PreProcessMiddleware,
        EventMiddleware,
        MiddlewareContext,
        MiddlewareEffect,
        SummarizationMiddleware,
        ContextEditingMiddleware,
        SummarizationConfig,
        ContextEditingConfig,
        TriggerCondition,
        KeepPolicy,
    };
}

/// Application builder for creating a complete agent application
///
/// This provides a high-level API for configuring all components of the application.
pub struct ZeroAppBuilder {
    llm_config: Option<LlmConfig>,
    mcp_servers: Vec<McpServerConfig>,
    middleware_config: Option<MiddlewareConfig>,
}

impl Default for ZeroAppBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ZeroAppBuilder {
    /// Create a new application builder
    pub fn new() -> Self {
        Self {
            llm_config: None,
            mcp_servers: Vec::new(),
            middleware_config: None,
        }
    }

    /// Set the LLM configuration
    pub fn with_llm_config(mut self, config: LlmConfig) -> Self {
        self.llm_config = Some(config);
        self
    }

    /// Add an MCP server configuration
    pub fn with_mcp_server(mut self, config: McpServerConfig) -> Self {
        self.mcp_servers.push(config);
        self
    }

    /// Add multiple MCP server configurations
    pub fn with_mcp_servers(mut self, configs: Vec<McpServerConfig>) -> Self {
        self.mcp_servers.extend(configs);
        self
    }

    /// Set the middleware configuration
    pub fn with_middleware_config(mut self, config: MiddlewareConfig) -> Self {
        self.middleware_config = Some(config);
        self
    }

    /// Build the application context
    pub fn build(self) -> std::result::Result<ZeroApp, ZeroError> {
        Ok(ZeroApp {
            llm_config: self.llm_config,
            mcp_servers: self.mcp_servers,
            middleware_config: self.middleware_config,
        })
    }
}

/// Application context containing all configured components
pub struct ZeroApp {
    llm_config: Option<LlmConfig>,
    mcp_servers: Vec<McpServerConfig>,
    middleware_config: Option<MiddlewareConfig>,
}

impl ZeroApp {
    /// Get the LLM configuration
    pub fn llm_config(&self) -> Option<&LlmConfig> {
        self.llm_config.as_ref()
    }

    /// Get the MCP server configurations
    pub fn mcp_servers(&self) -> &[McpServerConfig] {
        &self.mcp_servers
    }

    /// Get the middleware configuration
    pub fn middleware_config(&self) -> Option<&MiddlewareConfig> {
        self.middleware_config.as_ref()
    }

    /// Create a new session with this app's configuration
    pub fn create_session(&self, session_id: String, app_name: String, user_id: String) -> InMemorySession {
        InMemorySession::new(session_id, app_name, user_id)
    }

    /// Create a tool registry with MCP tools
    pub async fn create_tool_registry(&self) -> std::result::Result<ToolRegistry, ZeroError> {
        let registry = ToolRegistry::new();

        // TODO: Add MCP tools from configured servers
        // This requires creating MCP clients and connections,
        // which is better done at runtime when needed

        Ok(registry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_builder() {
        let app = ZeroAppBuilder::new()
            .with_llm_config(LlmConfig::new("test-key", "gpt-4"))
            .build()
            .unwrap();

        assert!(app.llm_config().is_some());
        assert!(app.mcp_servers().is_empty());
    }

    #[test]
    fn test_app_builder_with_mcp() {
        let server = McpServerConfig::stdio("test-server", "Test Server", "/path/to/server");

        let app = ZeroAppBuilder::new()
            .with_mcp_server(server)
            .build()
            .unwrap();

        assert_eq!(app.mcp_servers().len(), 1);
    }

    #[test]
    fn test_create_session() {
        let app = ZeroAppBuilder::new().build().unwrap();
        let session = app.create_session("test-session".to_string(), "test-app".to_string(), "test-user".to_string());

        assert_eq!(session.id(), "test-session");
    }
}
