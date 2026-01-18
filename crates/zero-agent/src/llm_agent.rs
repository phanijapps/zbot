//! # LLM Agent
//!
//! Agent implementation using LLM and tools.

use std::sync::Arc;

use async_stream::stream;
use async_trait::async_trait;
use serde_json::Value;
use tracing::{debug, info, warn};
use uuid::Uuid;

use zero_core::{
    Agent, Content, Event, EventActions, EventStream, InvocationContext, Part, Result, ZeroError,
    ReadonlyContext, CallbackContext, ToolContext,
};
use zero_llm::{Llm, LlmRequest, LlmResponse, ToolCall, ToolDefinition};
use zero_core::{Tool, Toolset};

/// LLM-based agent that responds to user input using an LLM and tools.
pub struct LlmAgent {
    name: String,
    description: String,
    llm: Arc<dyn Llm>,
    tools: Arc<dyn Toolset>,
    system_instruction: Option<String>,
}

impl LlmAgent {
    /// Create a new LLM agent.
    ///
    /// # Arguments
    ///
    /// * `name` - Agent name
    /// * `description` - Agent description
    /// * `llm` - LLM implementation
    /// * `tools` - Toolset for the agent
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        llm: Arc<dyn Llm>,
        tools: Arc<dyn Toolset>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            llm,
            tools,
            system_instruction: None,
        }
    }

    /// Set the system instruction for the agent.
    pub fn with_system_instruction(mut self, instruction: impl Into<String>) -> Self {
        self.system_instruction = Some(instruction.into());
        self
    }

    /// Generate LLM request from context.
    async fn build_request(&self, ctx: &Arc<dyn InvocationContext>) -> LlmRequest {
        let session = ctx.session();

        // Get conversation history (already includes user message added by executor)
        let all_contents = session.conversation_history();

        // Build tool definitions
        let tools = self.tools.tools().await.unwrap_or_default();
        let tool_definitions: Vec<ToolDefinition> = tools
            .iter()
            .map(|tool| ToolDefinition {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                parameters: tool.parameters_schema(),
            })
            .collect();

        let mut request = LlmRequest::new();
        request.contents = all_contents;

        if let Some(ref instruction) = self.system_instruction {
            request.system_instruction = Some(instruction.clone());
        }

        if !tool_definitions.is_empty() {
            request.tools = Some(tool_definitions);
        }

        request
    }

    /// Process tool calls and return tool responses.
    async fn process_tool_calls(
        &self,
        ctx: &Arc<dyn InvocationContext>,
        tool_calls: Vec<ToolCall>,
    ) -> Result<Vec<Content>> {
        let mut responses = Vec::new();

        // Get all available tools
        let tools = self.tools.tools().await.unwrap_or_default();
        let tools_map: std::collections::HashMap<String, Arc<dyn Tool>> = tools
            .iter()
            .map(|t| (t.name().to_string(), t.clone()))
            .collect();

        for tool_call in tool_calls {
            debug!("Executing tool: {}", tool_call.name);

            let tool = tools_map.get(&tool_call.name).ok_or_else(|| {
                ZeroError::Tool(format!("Tool not found: {}", tool_call.name))
            })?;

            // Create a ToolContext adapter for this tool call
            let tool_ctx = Arc::new(ToolContextAdapter::new(
                ctx.clone(),
                tool_call.id.clone(),
            ));

            let result = match tool.execute(tool_ctx, tool_call.arguments).await {
                Ok(result) => result.to_string(),
                Err(e) => {
                    warn!("Tool execution error: {}", e);
                    format!("Error: {}", e)
                }
            };

            responses.push(Content::tool_response(
                tool_call.id.clone(),
                result,
            ));
        }

        Ok(responses)
    }

    /// Run a single turn of the agent.
    async fn run_turn(
        &self,
        ctx: &Arc<dyn InvocationContext>,
    ) -> Result<(LlmResponse, Vec<Content>)> {
        let request = self.build_request(ctx).await;

        debug!("Sending request to LLM");
        let response = self.llm.generate(request).await?;

        let tool_responses = if let Some(ref content) = response.content {
            let tool_calls: Vec<ToolCall> = content
                .parts
                .iter()
                .filter_map(|part| match part {
                    Part::FunctionCall { name, args, id } => Some(ToolCall {
                        id: id.clone().unwrap_or_else(|| Uuid::new_v4().to_string()),
                        name: name.clone(),
                        arguments: args.clone(),
                    }),
                    _ => None,
                })
                .collect();

            if !tool_calls.is_empty() {
                Some(self.process_tool_calls(ctx, tool_calls).await?)
            } else {
                None
            }
        } else {
            None
        };

        Ok((response, tool_responses.unwrap_or_default()))
    }

    /// Create an event from LLM response.
    fn create_event(
        &self,
        ctx: &Arc<dyn InvocationContext>,
        response: &LlmResponse,
    ) -> Event {
        Event {
            id: Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            invocation_id: ctx.invocation_id().to_string(),
            branch: ctx.branch().to_string(),
            author: self.name.clone(),
            content: response.content.clone(),
            actions: EventActions::default(),
            turn_complete: response.turn_complete,
            long_running_tool_ids: Vec::new(),
            metadata: Default::default(),
        }
    }
}

#[async_trait]
impl Agent for LlmAgent {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn sub_agents(&self) -> &[Arc<dyn Agent>] {
        &[]
    }

    async fn run(&self, ctx: Arc<dyn InvocationContext>) -> Result<EventStream> {
        info!("Starting agent: {}", self.name);

        let agent = self.clone();
        let ctx_clone = ctx.clone();

        let stream = stream! {
            let max_iterations = ctx.run_config().max_iterations.unwrap_or(50);

            for iteration in 0..max_iterations {
                if ctx.ended() {
                    debug!("Invocation ended, stopping");
                    break;
                }

                debug!("Starting iteration {}", iteration);

                // Run a turn
                let (response, tool_responses) = agent.run_turn(&ctx_clone).await?;

                // Add assistant response to session history
                if let Some(ref content) = response.content {
                    ctx_clone.add_content(content.clone());
                    debug!("Added assistant response to session history");
                }

                // Emit assistant response event
                let event = agent.create_event(&ctx_clone, &response);
                yield Ok(event);

                // If turn is complete, we're done
                if response.turn_complete {
                    debug!("Turn complete after {} iterations", iteration + 1);
                    break;
                }

                // Add tool responses to session history for the next iteration
                for tool_response in tool_responses {
                    ctx_clone.add_content(tool_response);
                    debug!("Added tool response to session history");
                }
            }

            info!("Agent {} finished", agent.name);
        };

        Ok(Box::pin(stream))
    }
}

impl Clone for LlmAgent {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            description: self.description.clone(),
            llm: Arc::clone(&self.llm),
            tools: Arc::clone(&self.tools),
            system_instruction: self.system_instruction.clone(),
        }
    }
}

/// Adapter that converts InvocationContext to ToolContext for tool execution.
struct ToolContextAdapter {
    inner: Arc<dyn InvocationContext>,
    function_call_id: String,
}

impl ToolContextAdapter {
    fn new(ctx: Arc<dyn InvocationContext>, function_call_id: String) -> Self {
        Self {
            inner: ctx,
            function_call_id,
        }
    }
}

impl ReadonlyContext for ToolContextAdapter {
    fn invocation_id(&self) -> &str {
        self.inner.invocation_id()
    }

    fn agent_name(&self) -> &str {
        self.inner.agent_name()
    }

    fn user_id(&self) -> &str {
        self.inner.user_id()
    }

    fn app_name(&self) -> &str {
        self.inner.app_name()
    }

    fn session_id(&self) -> &str {
        self.inner.session_id()
    }

    fn branch(&self) -> &str {
        self.inner.branch()
    }

    fn user_content(&self) -> &Content {
        self.inner.user_content()
    }
}

impl CallbackContext for ToolContextAdapter {
    fn get_state(&self, key: &str) -> Option<Value> {
        self.inner.get_state(key)
    }

    fn set_state(&self, key: String, value: Value) {
        self.inner.set_state(key, value)
    }
}

impl ToolContext for ToolContextAdapter {
    fn function_call_id(&self) -> &str {
        &self.function_call_id
    }

    fn actions(&self) -> EventActions {
        self.inner.actions()
    }

    fn set_actions(&self, actions: EventActions) {
        self.inner.set_actions(actions)
    }
}

/// Builder for creating LlmAgent instances.
pub struct LlmAgentBuilder {
    name: String,
    description: String,
    llm: Option<Arc<dyn Llm>>,
    tools: Option<Arc<dyn Toolset>>,
    system_instruction: Option<String>,
}

impl LlmAgentBuilder {
    /// Create a new builder.
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            llm: None,
            tools: None,
            system_instruction: None,
        }
    }

    /// Set the LLM.
    pub fn with_llm(mut self, llm: Arc<dyn Llm>) -> Self {
        self.llm = Some(llm);
        self
    }

    /// Set the tools.
    pub fn with_tools(mut self, tools: Arc<dyn Toolset>) -> Self {
        self.tools = Some(tools);
        self
    }

    /// Set the system instruction.
    pub fn with_system_instruction(mut self, instruction: impl Into<String>) -> Self {
        self.system_instruction = Some(instruction.into());
        self
    }

    /// Build the agent.
    pub fn build(self) -> Result<LlmAgent> {
        let llm = self.llm.ok_or_else(|| {
            ZeroError::Config("LLM is required".to_string())
        })?;

        let tools = self.tools.unwrap_or_else(|| {
            Arc::new(zero_tool::ToolRegistry::new())
        });

        let mut agent = LlmAgent::new(self.name, self.description, llm, tools);

        if let Some(instruction) = self.system_instruction {
            agent = agent.with_system_instruction(instruction);
        }

        Ok(agent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::pin::Pin;
    use futures::Stream;
    use zero_tool::ToolRegistry;

    // Mock LLM for testing
    struct MockLlm;

    #[async_trait]
    impl Llm for MockLlm {
        async fn generate(&self, _request: LlmRequest) -> Result<LlmResponse> {
            Ok(LlmResponse {
                content: Some(Content::assistant("Hello!")),
                turn_complete: true,
                usage: None,
            })
        }

        async fn generate_stream(
            &self,
            _request: LlmRequest,
        ) -> Result<Pin<Box<dyn Stream<Item = Result<zero_llm::LlmResponseChunk>> + Send>>> {
            // Return a simple stream
            let stream = stream! {
                yield Ok(zero_llm::LlmResponseChunk {
                    delta: Some("Hello!".to_string()),
                    tool_call: None,
                    turn_complete: true,
                    usage: None,
                });
            };
            Ok(Box::pin(stream))
        }
    }

    #[test]
    fn test_agent_creation() {
        let llm = Arc::new(MockLlm);
        let tools = Arc::new(ToolRegistry::new());
        let agent = LlmAgent::new("test", "Test agent", llm, tools);

        assert_eq!(agent.name(), "test");
        assert_eq!(agent.description(), "Test agent");
        assert!(agent.sub_agents().is_empty());
    }

    #[test]
    fn test_agent_with_system_instruction() {
        let llm = Arc::new(MockLlm);
        let tools = Arc::new(ToolRegistry::new());
        let agent = LlmAgent::new("test", "Test agent", llm, tools)
            .with_system_instruction("You are helpful");

        assert!(agent.system_instruction.is_some());
    }

    #[test]
    fn test_builder() {
        let llm = Arc::new(MockLlm);
        let tools = Arc::new(ToolRegistry::new());

        let agent = LlmAgentBuilder::new("test", "Test agent")
            .with_llm(llm)
            .with_tools(tools)
            .with_system_instruction("You are helpful")
            .build()
            .unwrap();

        assert_eq!(agent.name(), "test");
    }

    #[test]
    fn test_builder_missing_llm() {
        let result = LlmAgentBuilder::new("test", "Test agent")
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_clone() {
        let llm = Arc::new(MockLlm);
        let tools = Arc::new(ToolRegistry::new());
        let agent1 = LlmAgent::new("test", "Test agent", llm, tools);
        let agent2 = agent1.clone();

        assert_eq!(agent1.name(), agent2.name());
    }
}
