//! # LLM Agent
//!
//! Agent implementation using LLM and tools.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use tracing::{debug, info, warn};
use uuid::Uuid;

use zero_core::{
    Agent, CallbackContext, Content, Event, EventActions, EventStream, InvocationContext, Part,
    ReadonlyContext, Result, ToolContext, ZeroError,
};
use zero_core::{Tool, Toolset};
use zero_llm::{Llm, LlmRequest, LlmResponse, ToolCall, ToolDefinition};

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

    /// Validate and repair conversation history to ensure all tool_calls have responses.
    ///
    /// Some LLM APIs (like DeepSeek/OpenAI) require that every assistant message with
    /// tool_calls must be followed by tool response messages. If execution was interrupted
    /// (e.g., user stopped, error occurred), the history can be left in an invalid state.
    ///
    /// This function scans the history and adds placeholder error responses for any
    /// tool_calls that don't have corresponding tool responses.
    fn validate_conversation_history(contents: Vec<Content>) -> Vec<Content> {
        let mut result = Vec::new();
        let mut pending_tool_calls: Vec<(String, String)> = Vec::new(); // (id, name)

        debug!(
            "Validating conversation history with {} contents",
            contents.len()
        );
        for content in contents {
            // First, check if we have pending tool calls that need responses
            if !pending_tool_calls.is_empty() {
                // Check if this content is a tool response
                let is_tool_response = content
                    .parts
                    .iter()
                    .any(|part| matches!(part, Part::FunctionResponse { .. }));

                if is_tool_response {
                    // Collect which tool_call_ids are being responded to
                    let responded_ids: std::collections::HashSet<String> = content
                        .parts
                        .iter()
                        .filter_map(|part| match part {
                            Part::FunctionResponse { id, .. } => Some(id.clone()),
                            _ => None,
                        })
                        .collect();

                    debug!(
                        "Tool response found with IDs: {:?}, pending: {:?}",
                        responded_ids, pending_tool_calls
                    );

                    // Remove responded tool calls from pending
                    pending_tool_calls.retain(|(id, _)| !responded_ids.contains(id));
                } else {
                    // Not a tool response but we have pending calls - add placeholder responses
                    debug!(
                        "Non-tool-response content after tool calls, pending: {:?}",
                        pending_tool_calls
                    );
                    for (tool_id, tool_name) in pending_tool_calls.drain(..) {
                        warn!(
                            "Adding placeholder response for orphaned tool call: {} ({})",
                            tool_name, tool_id
                        );
                        result.push(Content::tool_response(
                            tool_id,
                            format!("Error: Tool execution was interrupted for '{}'", tool_name),
                        ));
                    }
                }
            }

            // Check if this content has tool calls
            if content.role == "assistant" {
                for part in &content.parts {
                    if let Part::FunctionCall { name, id, .. } = part {
                        let tool_id = id.clone().unwrap_or_else(|| format!("unknown-{}", name));
                        debug!(
                            "Found tool call in assistant message: {} ({})",
                            name, tool_id
                        );
                        pending_tool_calls.push((tool_id, name.clone()));
                    }
                }
            }

            result.push(content);
        }

        // Handle any remaining pending tool calls at the end
        for (tool_id, tool_name) in pending_tool_calls {
            warn!(
                "Adding placeholder response for orphaned tool call at end: {} ({})",
                tool_name, tool_id
            );
            result.push(Content::tool_response(
                tool_id,
                format!("Error: Tool execution was interrupted for '{}'", tool_name),
            ));
        }

        result
    }

    /// Generate LLM request from context.
    async fn build_request(&self, ctx: &Arc<dyn InvocationContext>) -> LlmRequest {
        let session = ctx.session();

        // Get conversation history (already includes user message added by executor)
        let all_contents = session.conversation_history();

        debug!("build_request: History has {} items", all_contents.len());

        // Validate and repair conversation history to ensure tool_calls have responses
        let all_contents = Self::validate_conversation_history(all_contents);

        // Build tool definitions
        let tools = self.tools.tools().await.unwrap_or_default();
        debug!("Available tools for {}: {} tools", self.name, tools.len());
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

            let tool = tools_map
                .get(&tool_call.name)
                .ok_or_else(|| ZeroError::Tool(format!("Tool not found: {}", tool_call.name)))?;

            // Create a ToolContext adapter for this tool call
            let tool_ctx = Arc::new(ToolContextAdapter::new(ctx.clone(), tool_call.id.clone()));

            let result = match tool.execute(tool_ctx, tool_call.arguments).await {
                Ok(result) => result.to_string(),
                Err(e) => {
                    warn!("Tool execution error: {}", e);
                    format!("Error: {}", e)
                }
            };

            responses.push(Content::tool_response(tool_call.id.clone(), result));
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
                info!(
                    "LLM returned {} tool calls: {}",
                    tool_calls.len(),
                    tool_calls
                        .iter()
                        .map(|t| t.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                Some(self.process_tool_calls(ctx, tool_calls).await?)
            } else {
                info!("LLM returned text response instead of tool calls (should have used request_input for multi-field queries)");
                None
            }
        } else {
            None
        };

        Ok((response, tool_responses.unwrap_or_default()))
    }

    /// Create an event from LLM response.
    fn create_event(&self, ctx: &Arc<dyn InvocationContext>, response: &LlmResponse) -> Event {
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

        let stream = async_stream::try_stream! {
            let max_iterations = ctx_clone.run_config().max_iterations.unwrap_or(25);

            for iteration in 0..max_iterations {
                // Check for stop signal (either via ended() or execution_control::stop state)
                if ctx_clone.ended() {
                    debug!("Invocation ended, stopping at iteration {}", iteration);
                    break;
                }

                // Also check for stop via session state (for mid-stream stop requests)
                if let Some(stop_value) = ctx_clone.get_state("execution_control::stop") {
                    if stop_value.as_bool().unwrap_or(false) {
                        info!("Stop requested via session state at iteration {}", iteration);
                        break;
                    }
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
                yield event;

                // If turn is complete, we're done
                if response.turn_complete {
                    debug!("Turn complete after {} iterations", iteration + 1);
                    break;
                }

                // Emit separate events for tool results (for streaming to frontend)
                // Clone into a separate vector to avoid lifetime issues in stream
                let tool_responses_for_events: Vec<Content> = tool_responses.to_vec();
                for tool_response in tool_responses_for_events {
                    let tool_result_event = Event {
                        id: Uuid::new_v4().to_string(),
                        timestamp: chrono::Utc::now(),
                        invocation_id: ctx_clone.invocation_id().to_string(),
                        branch: ctx_clone.branch().to_string(),
                        author: agent.name.clone(),
                        content: Some(tool_response),
                        actions: EventActions::default(),
                        turn_complete: false,
                        long_running_tool_ids: Vec::new(),
                        metadata: Default::default(),
                    };
                    debug!("Emitting tool result event");
                    yield tool_result_event;
                }

                // Add tool responses to session history for the next iteration
                debug!("Adding {} tool responses to session history", tool_responses.len());
                for tool_response in tool_responses {
                    ctx_clone.add_content(tool_response);
                }
            }

            debug!("Agent {} finished", agent.name);
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
    fn function_call_id(&self) -> String {
        self.function_call_id.clone()
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
        let llm = self
            .llm
            .ok_or_else(|| ZeroError::Config("LLM is required".to_string()))?;

        let tools = self
            .tools
            .unwrap_or_else(|| Arc::new(zero_tool::ToolRegistry::new()));

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
    use async_stream::stream;
    use futures::Stream;
    use std::pin::Pin;
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
        ) -> Result<Pin<Box<dyn Stream<Item = Result<zero_llm::LlmResponseChunk>> + Send>>>
        {
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
        let result = LlmAgentBuilder::new("test", "Test agent").build();

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
