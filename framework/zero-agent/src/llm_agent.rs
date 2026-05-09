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
    use futures::{Stream, StreamExt};
    use std::pin::Pin;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::RwLock as StdRwLock;
    use zero_core::context::{Session, State};
    use zero_core::{RunConfig, Tool};
    use zero_llm::LlmResponseStream;
    use zero_tool::{FunctionTool, ToolRegistry};

    // ============================================================================
    // MOCK LLMS
    // ============================================================================

    /// Simple LLM that returns a final text response.
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

    /// LLM that returns a tool call on first invocation, then a final text response.
    struct MockLlmToolThenAnswer {
        calls: AtomicUsize,
        tool_name: String,
    }

    impl MockLlmToolThenAnswer {
        fn new(tool_name: &str) -> Self {
            Self {
                calls: AtomicUsize::new(0),
                tool_name: tool_name.to_string(),
            }
        }
    }

    #[async_trait]
    impl Llm for MockLlmToolThenAnswer {
        async fn generate(&self, _request: LlmRequest) -> Result<LlmResponse> {
            match self.calls.fetch_add(1, Ordering::SeqCst) {
                0 => {
                    // Return a tool call (no text)
                    Ok(LlmResponse {
                        content: Some(Content {
                            role: "assistant".to_string(),
                            parts: vec![Part::FunctionCall {
                                name: self.tool_name.clone(),
                                args: serde_json::json!({"x": 1}),
                                id: Some("call-1".to_string()),
                            }],
                        }),
                        turn_complete: false,
                        usage: None,
                    })
                }
                _ => Ok(LlmResponse {
                    content: Some(Content::assistant("done")),
                    turn_complete: true,
                    usage: None,
                }),
            }
        }

        async fn generate_stream(&self, _request: LlmRequest) -> Result<LlmResponseStream> {
            let stream = stream! { yield Ok(zero_llm::LlmResponseChunk {
                delta: None, tool_call: None, turn_complete: true, usage: None,
            }); };
            Ok(Box::pin(stream))
        }
    }

    /// LLM that returns a function call but with no `id` (exercises the Uuid fallback path).
    struct MockLlmToolNoId {
        calls: AtomicUsize,
    }

    #[async_trait]
    impl Llm for MockLlmToolNoId {
        async fn generate(&self, _request: LlmRequest) -> Result<LlmResponse> {
            match self.calls.fetch_add(1, Ordering::SeqCst) {
                0 => Ok(LlmResponse {
                    content: Some(Content {
                        role: "assistant".to_string(),
                        parts: vec![Part::FunctionCall {
                            name: "echo".to_string(),
                            args: serde_json::json!({}),
                            id: None,
                        }],
                    }),
                    turn_complete: false,
                    usage: None,
                }),
                _ => Ok(LlmResponse {
                    content: Some(Content::assistant("ok")),
                    turn_complete: true,
                    usage: None,
                }),
            }
        }

        async fn generate_stream(&self, _request: LlmRequest) -> Result<LlmResponseStream> {
            let stream = stream! { yield Ok(zero_llm::LlmResponseChunk {
                delta: None, tool_call: None, turn_complete: true, usage: None,
            }); };
            Ok(Box::pin(stream))
        }
    }

    /// LLM that fails with an Llm error.
    struct MockLlmError;

    #[async_trait]
    impl Llm for MockLlmError {
        async fn generate(&self, _request: LlmRequest) -> Result<LlmResponse> {
            Err(ZeroError::Llm("boom".to_string()))
        }

        async fn generate_stream(&self, _request: LlmRequest) -> Result<LlmResponseStream> {
            let stream = stream! { yield Ok(zero_llm::LlmResponseChunk {
                delta: None, tool_call: None, turn_complete: true, usage: None,
            }); };
            Ok(Box::pin(stream))
        }
    }

    /// LLM that yields a response with no content (exercises the `else` branch in run_turn).
    struct MockLlmNoContent;

    #[async_trait]
    impl Llm for MockLlmNoContent {
        async fn generate(&self, _request: LlmRequest) -> Result<LlmResponse> {
            Ok(LlmResponse {
                content: None,
                turn_complete: true,
                usage: None,
            })
        }

        async fn generate_stream(&self, _request: LlmRequest) -> Result<LlmResponseStream> {
            let stream = stream! { yield Ok(zero_llm::LlmResponseChunk {
                delta: None, tool_call: None, turn_complete: true, usage: None,
            }); };
            Ok(Box::pin(stream))
        }
    }

    // ============================================================================
    // MOCK CONTEXT
    // ============================================================================

    struct MockState {
        data: StdRwLock<std::collections::HashMap<String, Value>>,
    }

    impl State for MockState {
        fn get(&self, key: &str) -> Option<Value> {
            self.data.read().ok().and_then(|d| d.get(key).cloned())
        }

        fn set(&mut self, key: String, value: Value) {
            if let Ok(mut d) = self.data.write() {
                d.insert(key, value);
            }
        }

        fn all(&self) -> std::collections::HashMap<String, Value> {
            self.data.read().map(|d| d.clone()).unwrap_or_default()
        }
    }

    struct MockSession {
        id: String,
        history: StdRwLock<Vec<Content>>,
        state: MockState,
    }

    impl MockSession {
        fn new(initial_history: Vec<Content>) -> Self {
            Self {
                id: "test-session".to_string(),
                history: StdRwLock::new(initial_history),
                state: MockState {
                    data: StdRwLock::new(std::collections::HashMap::new()),
                },
            }
        }
    }

    impl Session for MockSession {
        fn id(&self) -> &str {
            &self.id
        }
        fn app_name(&self) -> &str {
            "test-app"
        }
        fn user_id(&self) -> &str {
            "test-user"
        }
        fn state(&self) -> &dyn State {
            &self.state
        }
        fn conversation_history(&self) -> Vec<Content> {
            self.history.read().map(|h| h.clone()).unwrap_or_default()
        }
    }

    struct MockInvocationContext {
        session: Arc<MockSession>,
        agent: Arc<dyn Agent>,
        run_config: RunConfig,
        ended: std::sync::atomic::AtomicBool,
        actions: StdRwLock<EventActions>,
        user_content: Content,
        state_overrides: StdRwLock<std::collections::HashMap<String, Value>>,
    }

    impl MockInvocationContext {
        fn new(session: Arc<MockSession>, agent: Arc<dyn Agent>) -> Self {
            Self {
                session,
                agent,
                run_config: RunConfig::default(),
                ended: std::sync::atomic::AtomicBool::new(false),
                actions: StdRwLock::new(EventActions::default()),
                user_content: Content::user("test"),
                state_overrides: StdRwLock::new(std::collections::HashMap::new()),
            }
        }

        fn with_run_config(mut self, cfg: RunConfig) -> Self {
            self.run_config = cfg;
            self
        }

        fn end_now(&self) {
            self.ended.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }

    impl ReadonlyContext for MockInvocationContext {
        fn invocation_id(&self) -> &str {
            "inv-1"
        }
        fn agent_name(&self) -> &str {
            self.agent.name()
        }
        fn user_id(&self) -> &str {
            "test-user"
        }
        fn app_name(&self) -> &str {
            "test-app"
        }
        fn session_id(&self) -> &str {
            self.session.id()
        }
        fn branch(&self) -> &str {
            "main"
        }
        fn user_content(&self) -> &Content {
            &self.user_content
        }
    }

    impl CallbackContext for MockInvocationContext {
        fn get_state(&self, key: &str) -> Option<Value> {
            // Check overrides first, then session state
            if let Ok(overrides) = self.state_overrides.read() {
                if let Some(v) = overrides.get(key) {
                    return Some(v.clone());
                }
            }
            self.session.state.get(key)
        }
        fn set_state(&self, key: String, value: Value) {
            if let Ok(mut overrides) = self.state_overrides.write() {
                overrides.insert(key, value);
            }
        }
    }

    impl InvocationContext for MockInvocationContext {
        fn agent(&self) -> Arc<dyn Agent> {
            Arc::clone(&self.agent)
        }
        fn session(&self) -> Arc<dyn Session> {
            Arc::clone(&self.session) as Arc<dyn Session>
        }
        fn run_config(&self) -> &RunConfig {
            &self.run_config
        }
        fn actions(&self) -> EventActions {
            self.actions.read().map(|a| a.clone()).unwrap_or_default()
        }
        fn set_actions(&self, actions: EventActions) {
            if let Ok(mut a) = self.actions.write() {
                *a = actions;
            }
        }
        fn end_invocation(&self) {
            self.ended.store(true, std::sync::atomic::Ordering::SeqCst);
        }
        fn ended(&self) -> bool {
            self.ended.load(std::sync::atomic::Ordering::SeqCst)
        }
        fn add_content(&self, content: Content) {
            if let Ok(mut h) = self.session.history.write() {
                h.push(content);
            }
        }
    }

    /// Stub agent used to satisfy ctx.agent() — never run directly.
    struct StubAgent;

    #[async_trait]
    impl Agent for StubAgent {
        fn name(&self) -> &str {
            "stub"
        }
        fn description(&self) -> &str {
            "stub"
        }
        fn sub_agents(&self) -> &[Arc<dyn Agent>] {
            &[]
        }
        async fn run(&self, _ctx: Arc<dyn InvocationContext>) -> Result<EventStream> {
            let s = stream! { yield Ok(zero_core::Event::new("stub")); };
            Ok(Box::pin(s))
        }
    }

    fn make_ctx(history: Vec<Content>) -> Arc<MockInvocationContext> {
        let session = Arc::new(MockSession::new(history));
        let agent: Arc<dyn Agent> = Arc::new(StubAgent);
        Arc::new(MockInvocationContext::new(session, agent))
    }

    fn make_echo_tool() -> Arc<dyn Tool> {
        Arc::new(FunctionTool::new("echo", "Echo input", |_ctx, args| {
            Box::pin(async move { Ok(serde_json::json!({"echo": args})) })
        }))
    }

    fn make_failing_tool() -> Arc<dyn Tool> {
        Arc::new(FunctionTool::new("fail", "Always fails", |_ctx, _args| {
            Box::pin(async move { Err(ZeroError::Tool("kaboom".to_string())) })
        }))
    }

    // ============================================================================
    // EXISTING TESTS (kept)
    // ============================================================================

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
    fn test_builder_default_tools() {
        // No tools provided — builder should default to empty ToolRegistry.
        let llm = Arc::new(MockLlm);
        let agent = LlmAgentBuilder::new("test", "Test")
            .with_llm(llm)
            .build()
            .unwrap();
        assert_eq!(agent.name(), "test");
    }

    #[test]
    fn test_clone() {
        let llm = Arc::new(MockLlm);
        let tools = Arc::new(ToolRegistry::new());
        let agent1 = LlmAgent::new("test", "Test agent", llm, tools).with_system_instruction("sys");
        let agent2 = agent1.clone();

        assert_eq!(agent1.name(), agent2.name());
        assert_eq!(agent1.description(), agent2.description());
        assert_eq!(agent1.system_instruction, agent2.system_instruction);
    }

    // ============================================================================
    // VALIDATE_CONVERSATION_HISTORY
    // ============================================================================

    #[test]
    fn test_validate_history_empty() {
        let result = LlmAgent::validate_conversation_history(vec![]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_validate_history_no_tool_calls() {
        let history = vec![Content::user("hi"), Content::assistant("hello back")];
        let result = LlmAgent::validate_conversation_history(history);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_validate_history_paired_tool_call() {
        // assistant tool call followed by tool response — should pass through.
        let history = vec![
            Content::user("call something"),
            Content {
                role: "assistant".to_string(),
                parts: vec![Part::FunctionCall {
                    name: "echo".to_string(),
                    args: serde_json::json!({}),
                    id: Some("c1".to_string()),
                }],
            },
            Content::tool_response("c1", "done"),
        ];
        let result = LlmAgent::validate_conversation_history(history);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_validate_history_orphaned_at_end() {
        // assistant tool call with no matching response should get a placeholder added.
        let history = vec![
            Content::user("call something"),
            Content {
                role: "assistant".to_string(),
                parts: vec![Part::FunctionCall {
                    name: "echo".to_string(),
                    args: serde_json::json!({}),
                    id: Some("orphan-1".to_string()),
                }],
            },
        ];
        let result = LlmAgent::validate_conversation_history(history);
        assert_eq!(result.len(), 3);
        // Last item should be a placeholder tool_response.
        let last = &result[2];
        assert_eq!(last.role, "tool");
        match &last.parts[0] {
            Part::FunctionResponse { id, response } => {
                assert_eq!(id, "orphan-1");
                assert!(response.contains("interrupted"));
            }
            _ => panic!("Expected FunctionResponse part"),
        }
    }

    #[test]
    fn test_validate_history_orphaned_then_user_message() {
        // assistant tool call followed by another user message (no tool response)
        // — should insert a placeholder before the user message.
        let history = vec![
            Content {
                role: "assistant".to_string(),
                parts: vec![Part::FunctionCall {
                    name: "echo".to_string(),
                    args: serde_json::json!({}),
                    id: Some("c1".to_string()),
                }],
            },
            Content::user("nevermind, do this instead"),
        ];
        let result = LlmAgent::validate_conversation_history(history);
        assert_eq!(result.len(), 3);
        // [assistant tool_call, placeholder tool_response, user]
        assert_eq!(result[0].role, "assistant");
        assert_eq!(result[1].role, "tool");
        assert_eq!(result[2].role, "user");
    }

    #[test]
    fn test_validate_history_tool_call_no_id() {
        // Function call with no id — uses synthetic "unknown-{name}" id and is detected as orphan.
        let history = vec![Content {
            role: "assistant".to_string(),
            parts: vec![Part::FunctionCall {
                name: "noisy".to_string(),
                args: serde_json::json!({}),
                id: None,
            }],
        }];
        let result = LlmAgent::validate_conversation_history(history);
        assert_eq!(result.len(), 2);
        match &result[1].parts[0] {
            Part::FunctionResponse { id, .. } => {
                assert!(id.contains("unknown-noisy"));
            }
            _ => panic!("Expected FunctionResponse"),
        }
    }

    // ============================================================================
    // BUILD_REQUEST
    // ============================================================================

    #[tokio::test]
    async fn test_build_request_no_tools_no_instruction() {
        let llm = Arc::new(MockLlm);
        let tools = Arc::new(ToolRegistry::new());
        let agent = LlmAgent::new("a", "A", llm, tools);
        let ctx: Arc<dyn InvocationContext> = make_ctx(vec![Content::user("hi")]);
        let req = agent.build_request(&ctx).await;
        assert_eq!(req.contents.len(), 1);
        assert!(req.system_instruction.is_none());
        assert!(req.tools.is_none());
    }

    #[tokio::test]
    async fn test_build_request_with_tools_and_instruction() {
        let mut registry = ToolRegistry::new();
        registry.register(make_echo_tool());
        let llm = Arc::new(MockLlm);
        let agent =
            LlmAgent::new("a", "A", llm, Arc::new(registry)).with_system_instruction("you are X");
        let ctx: Arc<dyn InvocationContext> = make_ctx(vec![Content::user("hi")]);
        let req = agent.build_request(&ctx).await;
        assert_eq!(req.system_instruction, Some("you are X".to_string()));
        assert!(req.tools.is_some());
        assert_eq!(req.tools.unwrap().len(), 1);
    }

    // ============================================================================
    // PROCESS_TOOL_CALLS
    // ============================================================================

    #[tokio::test]
    async fn test_process_tool_calls_success() {
        let mut registry = ToolRegistry::new();
        registry.register(make_echo_tool());
        let llm = Arc::new(MockLlm);
        let agent = LlmAgent::new("a", "A", llm, Arc::new(registry));
        let ctx: Arc<dyn InvocationContext> = make_ctx(vec![]);
        let calls = vec![ToolCall {
            id: "id-1".to_string(),
            name: "echo".to_string(),
            arguments: serde_json::json!({"x": 42}),
        }];
        let responses = agent.process_tool_calls(&ctx, calls).await.unwrap();
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].role, "tool");
    }

    #[tokio::test]
    async fn test_process_tool_calls_unknown_tool() {
        let registry = ToolRegistry::new();
        let llm = Arc::new(MockLlm);
        let agent = LlmAgent::new("a", "A", llm, Arc::new(registry));
        let ctx: Arc<dyn InvocationContext> = make_ctx(vec![]);
        let calls = vec![ToolCall {
            id: "id-1".to_string(),
            name: "ghost".to_string(),
            arguments: serde_json::json!({}),
        }];
        let result = agent.process_tool_calls(&ctx, calls).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_process_tool_calls_failing_tool() {
        // Tool execute returns Err — should be reported as an error string in response.
        let mut registry = ToolRegistry::new();
        registry.register(make_failing_tool());
        let llm = Arc::new(MockLlm);
        let agent = LlmAgent::new("a", "A", llm, Arc::new(registry));
        let ctx: Arc<dyn InvocationContext> = make_ctx(vec![]);
        let calls = vec![ToolCall {
            id: "id-fail".to_string(),
            name: "fail".to_string(),
            arguments: serde_json::json!({}),
        }];
        let responses = agent.process_tool_calls(&ctx, calls).await.unwrap();
        assert_eq!(responses.len(), 1);
        match &responses[0].parts[0] {
            Part::FunctionResponse { response, .. } => {
                assert!(response.starts_with("Error:"));
            }
            _ => panic!("Expected FunctionResponse"),
        }
    }

    // ============================================================================
    // CREATE_EVENT
    // ============================================================================

    #[test]
    fn test_create_event() {
        let llm = Arc::new(MockLlm);
        let tools = Arc::new(ToolRegistry::new());
        let agent = LlmAgent::new("agent-x", "X", llm, tools);
        let ctx: Arc<dyn InvocationContext> = make_ctx(vec![]);
        let response = LlmResponse {
            content: Some(Content::assistant("Hi")),
            turn_complete: true,
            usage: None,
        };
        let event = agent.create_event(&ctx, &response);
        assert_eq!(event.author, "agent-x");
        assert_eq!(event.invocation_id, "inv-1");
        assert!(event.turn_complete);
    }

    // ============================================================================
    // RUN — END-TO-END STREAM TESTS
    // ============================================================================

    #[tokio::test]
    async fn test_run_simple_text_response() {
        let llm = Arc::new(MockLlm);
        let tools = Arc::new(ToolRegistry::new());
        let agent = LlmAgent::new("a", "A", llm, tools);
        let ctx: Arc<dyn InvocationContext> = make_ctx(vec![Content::user("hi")]);
        let mut stream = agent.run(ctx).await.unwrap();
        let mut events = Vec::new();
        while let Some(e) = stream.next().await {
            events.push(e.unwrap());
        }
        // One event with assistant content and turn_complete = true.
        assert_eq!(events.len(), 1);
        assert!(events[0].turn_complete);
    }

    #[tokio::test]
    async fn test_run_with_tool_call_then_completes() {
        let mut registry = ToolRegistry::new();
        registry.register(make_echo_tool());
        let llm = Arc::new(MockLlmToolThenAnswer::new("echo"));
        let agent = LlmAgent::new("a", "A", llm, Arc::new(registry));
        let ctx: Arc<dyn InvocationContext> = make_ctx(vec![Content::user("call echo")]);
        let mut stream = agent.run(ctx.clone()).await.unwrap();
        let mut events = Vec::new();
        while let Some(e) = stream.next().await {
            events.push(e.unwrap());
        }
        // Iter 0: assistant tool call event + tool result event
        // Iter 1: assistant final text (turn_complete)
        // So: 3 events total
        assert!(events.len() >= 3);
        assert!(events.last().unwrap().turn_complete);
    }

    #[tokio::test]
    async fn test_run_with_tool_call_no_id() {
        let mut registry = ToolRegistry::new();
        registry.register(make_echo_tool());
        let llm = Arc::new(MockLlmToolNoId {
            calls: AtomicUsize::new(0),
        });
        let agent = LlmAgent::new("a", "A", llm, Arc::new(registry));
        let ctx: Arc<dyn InvocationContext> = make_ctx(vec![Content::user("call echo")]);
        let mut stream = agent.run(ctx).await.unwrap();
        let mut count = 0;
        while let Some(e) = stream.next().await {
            e.unwrap();
            count += 1;
        }
        assert!(count >= 2);
    }

    #[tokio::test]
    async fn test_run_llm_error_propagates() {
        let llm = Arc::new(MockLlmError);
        let tools = Arc::new(ToolRegistry::new());
        let agent = LlmAgent::new("a", "A", llm, tools);
        let ctx: Arc<dyn InvocationContext> = make_ctx(vec![Content::user("hi")]);
        let mut stream = agent.run(ctx).await.unwrap();
        let first = stream.next().await.unwrap();
        assert!(first.is_err());
    }

    #[tokio::test]
    async fn test_run_no_content_response_emits_event() {
        // LLM returns no content but turn_complete=true — should still emit one event.
        let llm = Arc::new(MockLlmNoContent);
        let tools = Arc::new(ToolRegistry::new());
        let agent = LlmAgent::new("a", "A", llm, tools);
        let ctx: Arc<dyn InvocationContext> = make_ctx(vec![Content::user("hi")]);
        let mut stream = agent.run(ctx).await.unwrap();
        let mut events = Vec::new();
        while let Some(e) = stream.next().await {
            events.push(e.unwrap());
        }
        assert_eq!(events.len(), 1);
        assert!(events[0].turn_complete);
    }

    #[tokio::test]
    async fn test_run_stops_when_invocation_ended() {
        // Pre-end the invocation — run should exit on first iteration.
        let llm = Arc::new(MockLlm);
        let tools = Arc::new(ToolRegistry::new());
        let agent = LlmAgent::new("a", "A", llm, tools);
        let session = Arc::new(MockSession::new(vec![Content::user("hi")]));
        let stub: Arc<dyn Agent> = Arc::new(StubAgent);
        let ctx_concrete = Arc::new(MockInvocationContext::new(session, stub));
        ctx_concrete.end_now();
        let ctx: Arc<dyn InvocationContext> = ctx_concrete;
        let mut stream = agent.run(ctx).await.unwrap();
        let mut events = Vec::new();
        while let Some(e) = stream.next().await {
            events.push(e.unwrap());
        }
        // Loop never runs because ended() returns true on first check.
        assert_eq!(events.len(), 0);
    }

    #[tokio::test]
    async fn test_run_stops_on_session_state_stop() {
        // Set execution_control::stop=true via state.
        let llm = Arc::new(MockLlm);
        let tools = Arc::new(ToolRegistry::new());
        let agent = LlmAgent::new("a", "A", llm, tools);
        let session = Arc::new(MockSession::new(vec![Content::user("hi")]));
        let stub: Arc<dyn Agent> = Arc::new(StubAgent);
        let ctx_concrete = Arc::new(MockInvocationContext::new(session, stub));
        ctx_concrete.set_state("execution_control::stop".to_string(), Value::Bool(true));
        let ctx: Arc<dyn InvocationContext> = ctx_concrete;
        let mut stream = agent.run(ctx).await.unwrap();
        let mut events = Vec::new();
        while let Some(e) = stream.next().await {
            events.push(e.unwrap());
        }
        assert_eq!(events.len(), 0);
    }

    #[tokio::test]
    async fn test_run_respects_max_iterations() {
        // Tool-call-only LLM combined with max_iterations=1 means we get exactly 1 iteration.
        let mut registry = ToolRegistry::new();
        registry.register(make_echo_tool());
        let llm = Arc::new(MockLlmToolThenAnswer::new("echo"));
        let agent = LlmAgent::new("a", "A", llm, Arc::new(registry));
        let cfg = RunConfig::default().with_max_iterations(1);
        let session = Arc::new(MockSession::new(vec![Content::user("hi")]));
        let stub: Arc<dyn Agent> = Arc::new(StubAgent);
        let ctx_concrete = Arc::new(MockInvocationContext::new(session, stub).with_run_config(cfg));
        let ctx: Arc<dyn InvocationContext> = ctx_concrete;
        let mut stream = agent.run(ctx).await.unwrap();
        let mut events = Vec::new();
        while let Some(e) = stream.next().await {
            events.push(e.unwrap());
        }
        // Exactly one iteration: assistant tool call + tool result, then loop exits at max.
        assert!(events.len() <= 3);
    }

    // ============================================================================
    // TOOL CONTEXT ADAPTER
    // ============================================================================

    #[test]
    fn test_tool_context_adapter() {
        let ctx: Arc<dyn InvocationContext> = make_ctx(vec![]);
        let adapter = ToolContextAdapter::new(ctx, "fc-id".to_string());
        assert_eq!(adapter.function_call_id(), "fc-id");
        assert_eq!(adapter.invocation_id(), "inv-1");
        assert_eq!(adapter.agent_name(), "stub");
        assert_eq!(adapter.user_id(), "test-user");
        assert_eq!(adapter.app_name(), "test-app");
        assert_eq!(adapter.session_id(), "test-session");
        assert_eq!(adapter.branch(), "main");
        // user_content() shouldn't panic
        assert!(!adapter.user_content().role.is_empty());
        // get_state / set_state via adapter
        adapter.set_state("foo".to_string(), Value::String("bar".to_string()));
        assert_eq!(
            adapter.get_state("foo"),
            Some(Value::String("bar".to_string()))
        );
        // actions/set_actions
        let actions = EventActions::default();
        adapter.set_actions(actions.clone());
        let _ = adapter.actions();
    }
}
