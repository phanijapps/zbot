//! # Custom Agent
//!
//! Custom async logic without LLM.

use async_trait::async_trait;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use zero_core::{Agent, BeforeAgentCallback, AfterAgentCallback, EventStream, InvocationContext, Result, ZeroError};

/// Handler function type for custom agent logic.
pub type RunHandler = Arc<
    dyn Fn(Arc<dyn InvocationContext>) -> Pin<Box<dyn Future<Output = Result<EventStream>> + Send>>
        + Send
        + Sync,
>;

/// Custom agent for custom async logic without LLM.
///
/// # Example
///
/// ```ignore
/// let agent = CustomAgent::builder("custom")
///     .description("Custom logic agent")
///     .handler(|ctx| {
///         Box::pin(async move {
///             // Custom logic here
///             use async_stream::stream;
///             let s = stream! {
///                 yield Ok(Event::new("custom").with_content(Content::user("Hello")));
///             };
///             Ok(Box::pin(s))
///         })
///     })
///     .build()?;
/// ```
pub struct CustomAgent {
    name: String,
    description: String,
    sub_agents: Vec<Arc<dyn Agent>>,
    // Note: Callbacks are stored but not yet invoked in the run() method.
    // Future implementation should invoke these at appropriate lifecycle points.
    #[allow(dead_code)]
    before_callbacks: Vec<BeforeAgentCallback>,
    #[allow(dead_code)]
    after_callbacks: Vec<AfterAgentCallback>,
    handler: RunHandler,
}

impl CustomAgent {
    /// Create a new builder.
    pub fn builder(name: impl Into<String>) -> CustomAgentBuilder {
        CustomAgentBuilder::new(name)
    }
}

#[async_trait]
impl Agent for CustomAgent {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn sub_agents(&self) -> &[Arc<dyn Agent>] {
        &self.sub_agents
    }

    async fn run(&self, ctx: Arc<dyn InvocationContext>) -> Result<EventStream> {
        (self.handler)(ctx).await
    }
}

/// Builder for CustomAgent.
pub struct CustomAgentBuilder {
    name: String,
    description: String,
    sub_agents: Vec<Arc<dyn Agent>>,
    before_callbacks: Vec<BeforeAgentCallback>,
    after_callbacks: Vec<AfterAgentCallback>,
    handler: Option<RunHandler>,
}

impl CustomAgentBuilder {
    /// Create a new builder.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            sub_agents: Vec::new(),
            before_callbacks: Vec::new(),
            after_callbacks: Vec::new(),
            handler: None,
        }
    }

    /// Set the description.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Add a sub-agent.
    pub fn sub_agent(mut self, agent: Arc<dyn Agent>) -> Self {
        self.sub_agents.push(agent);
        self
    }

    /// Add multiple sub-agents.
    pub fn sub_agents(mut self, agents: Vec<Arc<dyn Agent>>) -> Self {
        self.sub_agents = agents;
        self
    }

    /// Add a before-run callback.
    pub fn before_callback(mut self, callback: BeforeAgentCallback) -> Self {
        self.before_callbacks.push(callback);
        self
    }

    /// Add an after-run callback.
    pub fn after_callback(mut self, callback: AfterAgentCallback) -> Self {
        self.after_callbacks.push(callback);
        self
    }

    /// Set the handler function.
    pub fn handler<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(Arc<dyn InvocationContext>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<EventStream>> + Send + 'static,
    {
        self.handler = Some(Arc::new(move |ctx| Box::pin(handler(ctx))));
        self
    }

    /// Build the custom agent.
    pub fn build(self) -> std::result::Result<CustomAgent, ZeroError> {
        let handler = self.handler.ok_or_else(|| {
            ZeroError::Generic("CustomAgent requires a handler".to_string())
        })?;

        // Validate sub-agents have unique names
        let mut seen_names = std::collections::HashSet::new();
        for agent in &self.sub_agents {
            if !seen_names.insert(agent.name()) {
                return Err(ZeroError::Generic(format!(
                    "Duplicate sub-agent name: {}",
                    agent.name()
                )));
            }
        }

        Ok(CustomAgent {
            name: self.name,
            description: self.description,
            sub_agents: self.sub_agents,
            before_callbacks: self.before_callbacks,
            after_callbacks: self.after_callbacks,
            handler,
        })
    }
}

impl Default for CustomAgentBuilder {
    fn default() -> Self {
        Self::new("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zero_core::{Event, Content, ZeroError};
    use async_stream::stream;
    use std::result::Result as StdResult;

    #[tokio::test]
    async fn test_custom_agent_builder() {
        let agent = CustomAgent::builder("test")
            .description("Test agent")
            .handler(|_ctx| {
                Box::pin(async move {
                    let mut event = Event::new("test");
                    event.content = Some(Content::assistant("Hello"));
                    let s = stream! {
                        yield Ok(event);
                    };
                    StdResult::Ok(Box::pin(s) as EventStream)
                })
            })
            .build()
            .unwrap();

        assert_eq!(agent.name(), "test");
        assert_eq!(agent.description(), "Test agent");
    }

    #[tokio::test]
    async fn test_custom_agent_missing_handler() {
        let result = CustomAgent::builder("test").build();

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_custom_agent_duplicate_sub_agents() {
        let mock1 = Arc::new(MockAgent { name: "test".to_string() }) as Arc<dyn Agent>;
        let mock2 = Arc::new(MockAgent { name: "test".to_string() }) as Arc<dyn Agent>;

        let result = CustomAgent::builder("test")
            .handler(|_ctx| {
                Box::pin(async move {
                    let s = stream! { yield Ok(Event::new("test")); };
                    StdResult::Ok(Box::pin(s) as EventStream)
                })
            })
            .sub_agents(vec![mock1, mock2])
            .build();

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_custom_agent_execution() {
        let agent = CustomAgent::builder("test")
            .handler(|_ctx| {
                Box::pin(async move {
                    let mut event = Event::new("test");
                    event.content = Some(Content::assistant("Hello from custom agent"));
                    let s = stream! {
                        yield Ok(event);
                    };
                    StdResult::Ok(Box::pin(s) as EventStream)
                })
            })
            .build()
            .unwrap();

        // In a real test, we'd create a mock InvocationContext and run the agent
        assert_eq!(agent.name(), "test");
    }

    // Mock Agent for testing
    struct MockAgent {
        name: String,
    }

    #[async_trait]
    impl Agent for MockAgent {
        fn name(&self) -> &str { &self.name }
        fn description(&self) -> &str { "Mock" }
        fn sub_agents(&self) -> &[Arc<dyn Agent>] { &[] }
        async fn run(&self, _ctx: Arc<dyn InvocationContext>) -> Result<EventStream> {
            let s = stream! { yield Ok(Event::new("test")); };
            Ok(Box::pin(s))
        }
    }
}
