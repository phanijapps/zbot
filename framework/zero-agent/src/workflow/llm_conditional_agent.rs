//! # LLM Conditional Agent
//!
//! LLM-based intelligent conditional routing agent.

use async_stream::stream;
use async_trait::async_trait;
use futures::StreamExt;
use std::collections::HashMap;
use std::sync::Arc;

use zero_core::{Agent, Content, Event, EventStream, InvocationContext, Part, Result, ZeroError};
use zero_llm::Llm;

/// LLM-based intelligent conditional routing agent.
///
/// Uses an LLM to classify user input and route to the appropriate sub-agent
/// based on the classification result. Supports multi-way routing.
///
/// # Example
///
/// ```ignore
/// let router = LlmConditionalAgent::builder("router", model)
///     .instruction("Classify as 'technical', 'general', or 'creative'.")
///     .route("technical", tech_agent)
///     .route("general", general_agent.clone())
///     .route("creative", creative_agent)
///     .default_route(general_agent)
///     .build()?;
/// ```
///
/// For rule-based routing (A/B testing, feature flags), use [`ConditionalAgent`](crate::workflow::ConditionalAgent).
pub struct LlmConditionalAgent {
    name: String,
    description: String,
    model: Arc<dyn Llm>,
    instruction: String,
    routes: HashMap<String, Arc<dyn Agent>>,
    default_agent: Option<Arc<dyn Agent>>,
}

pub struct LlmConditionalAgentBuilder {
    name: String,
    description: Option<String>,
    model: Arc<dyn Llm>,
    instruction: Option<String>,
    routes: HashMap<String, Arc<dyn Agent>>,
    default_agent: Option<Arc<dyn Agent>>,
}

impl LlmConditionalAgentBuilder {
    /// Create a new builder.
    pub fn new(name: impl Into<String>, model: Arc<dyn Llm>) -> Self {
        Self {
            name: name.into(),
            description: None,
            model,
            instruction: None,
            routes: HashMap::new(),
            default_agent: None,
        }
    }

    /// Set a description for the agent.
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Set the classification instruction.
    ///
    /// The instruction should tell the LLM to classify the user's input
    /// and respond with ONLY the category name (matching a route key).
    pub fn instruction(mut self, instruction: impl Into<String>) -> Self {
        self.instruction = Some(instruction.into());
        self
    }

    /// Add a route mapping a classification label to an agent.
    ///
    /// When the LLM's response contains this label, execution transfers
    /// to the specified agent.
    pub fn route(mut self, label: impl Into<String>, agent: Arc<dyn Agent>) -> Self {
        self.routes.insert(label.into().to_lowercase(), agent);
        self
    }

    /// Set the default agent to use when no route matches.
    pub fn default_route(mut self, agent: Arc<dyn Agent>) -> Self {
        self.default_agent = Some(agent);
        self
    }

    /// Build the LlmConditionalAgent.
    pub fn build(self) -> std::result::Result<LlmConditionalAgent, ZeroError> {
        let instruction = self.instruction.ok_or_else(|| {
            ZeroError::Generic("Instruction is required for LlmConditionalAgent".to_string())
        })?;

        if self.routes.is_empty() {
            return Err(ZeroError::Generic(
                "At least one route is required for LlmConditionalAgent".to_string(),
            ));
        }

        Ok(LlmConditionalAgent {
            name: self.name,
            description: self.description.unwrap_or_default(),
            model: self.model,
            instruction,
            routes: self.routes,
            default_agent: self.default_agent,
        })
    }
}

impl LlmConditionalAgent {
    /// Create a new builder for LlmConditionalAgent.
    pub fn builder(name: impl Into<String>, model: Arc<dyn Llm>) -> LlmConditionalAgentBuilder {
        LlmConditionalAgentBuilder::new(name, model)
    }

    /// Extract text from user content
    fn extract_user_text(content: &Content) -> String {
        content
            .parts
            .iter()
            .filter_map(|p| {
                if let Part::Text { text } = p {
                    Some(text.as_str())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[async_trait]
impl Agent for LlmConditionalAgent {
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
        let model = self.model.clone();
        let instruction = self.instruction.clone();
        let routes = self.routes.clone();
        let default_agent = self.default_agent.clone();
        let invocation_id = ctx.invocation_id().to_string();
        let agent_name = self.name.clone();

        let s = stream! {
            // Build classification request
            let user_content = ctx.user_content().clone();
            let user_text = Self::extract_user_text(&user_content);

            let classification_prompt = format!(
                "{}\n\nUser input: {}",
                instruction,
                user_text
            );

            let request = zero_llm::LlmRequest {
                contents: vec![Content::user(&classification_prompt)],
                system_instruction: None,
                tools: None,
                temperature: None,
                max_tokens: None,
            };

            // Call LLM for classification (streaming)
            let mut response_stream = model.generate_stream(request).await.map_err(|e| ZeroError::Llm(e.to_string()))?;

            // Collect classification response
            let mut classification = String::new();
            while let Some(chunk_result) = response_stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        if let Some(delta) = chunk.delta {
                            classification.push_str(&delta);
                        }
                    }
                    Err(e) => {
                        let mut error_event = Event::new(&invocation_id);
                        error_event.author = agent_name.clone();
                        error_event.content = Some(
                            Content::assistant(format!("Classification error: {}", e))
                        );
                        yield Ok(error_event);
                        return;
                    }
                }
            }

            // Normalize classification
            let classification = classification.trim().to_lowercase();

            // Emit routing event
            let mut routing_event = Event::new(&invocation_id);
            routing_event.author = agent_name.clone();
            routing_event.content = Some(
                Content::assistant(format!("[Routing to: {}]", classification))
            );
            yield Ok(routing_event);

            // Find matching route
            let target_agent = routes.iter()
                .find(|(label, _)| classification.contains(label.as_str()))
                .map(|(_, agent)| agent.clone())
                .or(default_agent);

            // Execute target agent
            if let Some(agent) = target_agent {
                match agent.run(ctx.clone()).await {
                    Ok(mut stream) => {
                        while let Some(event) = stream.next().await {
                            yield event;
                        }
                    }
                    Err(e) => {
                        yield Err(e);
                    }
                }
            } else {
                // No matching route and no default
                let mut error_event = Event::new(&invocation_id);
                error_event.author = agent_name;
                error_event.content = Some(
                    Content::assistant(format!(
                        "No route found for classification '{}'. Available routes: {:?}",
                        classification,
                        routes.keys().collect::<Vec<_>>()
                    ))
                );
                yield Ok(error_event);
            }
        };

        Ok(Box::pin(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_user_text() {
        let content = Content::user("Hello world");
        assert_eq!(
            LlmConditionalAgent::extract_user_text(&content),
            "Hello world"
        );
    }

    #[test]
    fn test_extract_user_text_multi_part() {
        let content = Content {
            role: "user".to_string(),
            parts: vec![
                Part::Text {
                    text: "Hello".to_string(),
                },
                Part::Text {
                    text: "world".to_string(),
                },
            ],
        };
        assert_eq!(
            LlmConditionalAgent::extract_user_text(&content),
            "Hello world"
        );
    }

    #[test]
    fn test_builder_missing_instruction() {
        let mock_model = Arc::new(MockLlm) as Arc<dyn Llm>;
        let agent = Arc::new(MockAgent) as Arc<dyn Agent>;

        let result = LlmConditionalAgent::builder("test", mock_model)
            .route("test", agent)
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_builder_missing_routes() {
        let mock_model = Arc::new(MockLlm) as Arc<dyn Llm>;

        let result = LlmConditionalAgent::builder("test", mock_model)
            .instruction("Test instruction")
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_builder_success() {
        let mock_model = Arc::new(MockLlm) as Arc<dyn Llm>;
        let agent = Arc::new(MockAgent) as Arc<dyn Agent>;

        let result = LlmConditionalAgent::builder("test", mock_model)
            .instruction("Test instruction")
            .route("test", agent)
            .build();

        assert!(result.is_ok());
    }

    #[test]
    fn test_builder_with_description_and_default() {
        let mock_model = Arc::new(MockLlm) as Arc<dyn Llm>;
        let agent = Arc::new(MockAgent) as Arc<dyn Agent>;
        let default_agent = Arc::new(MockAgent) as Arc<dyn Agent>;
        let result = LlmConditionalAgent::builder("test", mock_model)
            .description("desc")
            .instruction("classify")
            .route("a", agent)
            .default_route(default_agent)
            .build()
            .unwrap();
        assert_eq!(result.description(), "desc");
        assert_eq!(result.name(), "test");
        assert!(result.sub_agents().is_empty());
    }

    // ----------------------------------------------------------------------
    // Run-path tests
    // ----------------------------------------------------------------------

    /// LLM that emits a classification (one delta) then completes.
    struct ClassifyingLlm {
        category: &'static str,
    }

    #[async_trait]
    impl zero_llm::Llm for ClassifyingLlm {
        async fn generate(
            &self,
            _request: zero_llm::LlmRequest,
        ) -> zero_core::Result<zero_llm::LlmResponse> {
            Ok(zero_llm::LlmResponse {
                content: Some(Content::assistant(self.category)),
                turn_complete: true,
                usage: None,
            })
        }

        async fn generate_stream(
            &self,
            _request: zero_llm::LlmRequest,
        ) -> zero_core::Result<zero_llm::LlmResponseStream> {
            use async_stream::stream;
            use zero_llm::LlmResponseChunk;
            let cat = self.category.to_string();
            let s = stream! {
                yield Ok(LlmResponseChunk {
                    delta: Some(cat),
                    tool_call: None,
                    turn_complete: true,
                    usage: None,
                });
            };
            Ok(Box::pin(s))
        }
    }

    /// LLM whose stream yields an Err — exercises classification error branch.
    struct StreamErrLlm;

    #[async_trait]
    impl zero_llm::Llm for StreamErrLlm {
        async fn generate(
            &self,
            _request: zero_llm::LlmRequest,
        ) -> zero_core::Result<zero_llm::LlmResponse> {
            Ok(zero_llm::LlmResponse {
                content: None,
                turn_complete: true,
                usage: None,
            })
        }

        async fn generate_stream(
            &self,
            _request: zero_llm::LlmRequest,
        ) -> zero_core::Result<zero_llm::LlmResponseStream> {
            use async_stream::stream;
            let s = stream! {
                yield Err(zero_core::ZeroError::Llm("boom".to_string()));
            };
            Ok(Box::pin(s))
        }
    }

    /// Agent that always errors when run.
    struct FailingAgent;
    #[async_trait]
    impl Agent for FailingAgent {
        fn name(&self) -> &str {
            "fail"
        }
        fn description(&self) -> &str {
            ""
        }
        fn sub_agents(&self) -> &[Arc<dyn Agent>] {
            &[]
        }
        async fn run(&self, _ctx: Arc<dyn InvocationContext>) -> Result<EventStream> {
            Err(zero_core::ZeroError::Generic("nope".to_string()))
        }
    }

    #[tokio::test]
    async fn test_run_routes_to_matching_agent() {
        use crate::workflow::test_support::make_ctx;
        use futures::StreamExt;

        let model = Arc::new(ClassifyingLlm { category: "tech" }) as Arc<dyn zero_llm::Llm>;
        let tech_agent = Arc::new(MockAgent) as Arc<dyn Agent>;
        let agent = LlmConditionalAgent::builder("router", model)
            .instruction("classify")
            .route("tech", tech_agent)
            .build()
            .unwrap();

        let stub_agent: Arc<dyn Agent> = Arc::new(MockAgent);
        let ctx = make_ctx(stub_agent);
        let mut stream = agent.run(ctx).await.unwrap();
        let mut events = Vec::new();
        while let Some(e) = stream.next().await {
            events.push(e.unwrap());
        }
        // routing event + downstream agent event
        assert!(events.len() >= 2);
        // First event is routing message
        assert!(events[0]
            .content
            .as_ref()
            .unwrap()
            .text()
            .unwrap()
            .contains("Routing"));
    }

    #[tokio::test]
    async fn test_run_falls_back_to_default_when_no_match() {
        use crate::workflow::test_support::make_ctx;
        use futures::StreamExt;

        let model = Arc::new(ClassifyingLlm { category: "weird" }) as Arc<dyn zero_llm::Llm>;
        let tech_agent = Arc::new(MockAgent) as Arc<dyn Agent>;
        let default_agent = Arc::new(MockAgent) as Arc<dyn Agent>;
        let agent = LlmConditionalAgent::builder("router", model)
            .instruction("classify")
            .route("tech", tech_agent)
            .default_route(default_agent)
            .build()
            .unwrap();

        let stub: Arc<dyn Agent> = Arc::new(MockAgent);
        let ctx = make_ctx(stub);
        let mut stream = agent.run(ctx).await.unwrap();
        let mut events = Vec::new();
        while let Some(e) = stream.next().await {
            events.push(e.unwrap());
        }
        assert!(events.len() >= 2);
    }

    #[tokio::test]
    async fn test_run_no_match_no_default_emits_error() {
        use crate::workflow::test_support::make_ctx;
        use futures::StreamExt;

        let model = Arc::new(ClassifyingLlm { category: "weird" }) as Arc<dyn zero_llm::Llm>;
        let tech_agent = Arc::new(MockAgent) as Arc<dyn Agent>;
        let agent = LlmConditionalAgent::builder("router", model)
            .instruction("classify")
            .route("tech", tech_agent)
            .build()
            .unwrap();

        let stub: Arc<dyn Agent> = Arc::new(MockAgent);
        let ctx = make_ctx(stub);
        let mut stream = agent.run(ctx).await.unwrap();
        let mut events = Vec::new();
        while let Some(e) = stream.next().await {
            events.push(e.unwrap());
        }
        // routing event + "no route found" message
        assert!(events.len() >= 2);
        let last = events.last().unwrap();
        let text = last.content.as_ref().unwrap().text().unwrap();
        assert!(text.contains("No route found"));
    }

    #[tokio::test]
    async fn test_run_classification_stream_error() {
        use crate::workflow::test_support::make_ctx;
        use futures::StreamExt;

        let model = Arc::new(StreamErrLlm) as Arc<dyn zero_llm::Llm>;
        let tech_agent = Arc::new(MockAgent) as Arc<dyn Agent>;
        let agent = LlmConditionalAgent::builder("router", model)
            .instruction("classify")
            .route("tech", tech_agent)
            .build()
            .unwrap();

        let stub: Arc<dyn Agent> = Arc::new(MockAgent);
        let ctx = make_ctx(stub);
        let mut stream = agent.run(ctx).await.unwrap();
        let event = stream.next().await.unwrap().unwrap();
        // Should be a classification error event with error text
        let text = event.content.as_ref().unwrap().text().unwrap();
        assert!(text.contains("Classification error"));
    }

    #[tokio::test]
    async fn test_run_target_agent_error_propagates() {
        use crate::workflow::test_support::make_ctx;
        use futures::StreamExt;

        let model = Arc::new(ClassifyingLlm { category: "tech" }) as Arc<dyn zero_llm::Llm>;
        let agent = LlmConditionalAgent::builder("router", model)
            .instruction("classify")
            .route("tech", Arc::new(FailingAgent) as Arc<dyn Agent>)
            .build()
            .unwrap();

        let stub: Arc<dyn Agent> = Arc::new(MockAgent);
        let ctx = make_ctx(stub);
        let mut stream = agent.run(ctx).await.unwrap();
        // First event is routing message (Ok), then the agent's run error.
        let routing = stream.next().await.unwrap();
        assert!(routing.is_ok());
        let err_event = stream.next().await.unwrap();
        assert!(err_event.is_err());
    }

    // Mock LLM for testing
    struct MockLlm;

    #[async_trait]
    impl zero_llm::Llm for MockLlm {
        async fn generate(
            &self,
            _request: zero_llm::LlmRequest,
        ) -> zero_core::Result<zero_llm::LlmResponse> {
            Ok(zero_llm::LlmResponse {
                content: Some(Content::assistant("test")),
                turn_complete: true,
                usage: None,
            })
        }

        async fn generate_stream(
            &self,
            _request: zero_llm::LlmRequest,
        ) -> zero_core::Result<zero_llm::LlmResponseStream> {
            use async_stream::stream;
            use zero_llm::LlmResponseChunk;
            let s = stream! {
                yield Ok(LlmResponseChunk {
                    delta: Some("test".to_string()),
                    tool_call: None,
                    turn_complete: true,
                    usage: None,
                });
            };
            Ok(Box::pin(s))
        }
    }

    // Mock Agent for testing
    struct MockAgent;

    #[async_trait]
    impl Agent for MockAgent {
        fn name(&self) -> &str {
            "mock"
        }
        fn description(&self) -> &str {
            "Mock"
        }
        fn sub_agents(&self) -> &[Arc<dyn Agent>] {
            &[]
        }

        async fn run(&self, _ctx: Arc<dyn InvocationContext>) -> zero_core::Result<EventStream> {
            use async_stream::stream;
            let s = stream! {
                yield Ok(Event::new("test"));
            };
            Ok(Box::pin(s))
        }
    }
}
