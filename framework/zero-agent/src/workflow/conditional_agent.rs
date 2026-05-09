//! # Conditional Agent (Rule-Based)
//!
//! Rule-based conditional routing agent.

use async_trait::async_trait;
use std::sync::Arc;

use zero_core::{
    AfterAgentCallback, Agent, BeforeAgentCallback, CallbackContext, EventStream,
    InvocationContext, Result,
};

/// Rule-based conditional routing agent.
///
/// Executes one of two sub-agents based on a synchronous condition function.
///
/// # Example
///
/// ```ignore
/// let router = ConditionalAgent::new(
///     "premium_router",
///     |ctx| ctx.session().state().get("is_premium").map(|v| v.as_bool()).flatten().unwrap_or(false),
///     Arc::new(premium_agent),
/// )
/// .with_else(Arc::new(basic_agent));
/// ```
///
/// For LLM-based intelligent routing, use [`LlmConditionalAgent`](crate::workflow::LlmConditionalAgent).
pub type ConditionFn = Arc<dyn Fn(&dyn CallbackContext) -> bool + Send + Sync>;

pub struct ConditionalAgent {
    name: String,
    description: String,
    condition: ConditionFn,
    if_agent: Arc<dyn Agent>,
    else_agent: Option<Arc<dyn Agent>>,
    before_callbacks: Vec<BeforeAgentCallback>,
    after_callbacks: Vec<AfterAgentCallback>,
}

impl ConditionalAgent {
    /// Create a new conditional agent.
    ///
    /// # Arguments
    ///
    /// * `name` - Agent name
    /// * `condition` - Function that evaluates to true/false
    /// * `if_agent` - Agent to run if condition is true
    pub fn new<F>(name: impl Into<String>, condition: F, if_agent: Arc<dyn Agent>) -> Self
    where
        F: Fn(&dyn CallbackContext) -> bool + Send + Sync + 'static,
    {
        Self {
            name: name.into(),
            description: String::new(),
            condition: Arc::new(condition),
            if_agent,
            else_agent: None,
            before_callbacks: Vec::new(),
            after_callbacks: Vec::new(),
        }
    }

    /// Set the description.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Set the agent to run if condition is false.
    pub fn with_else(mut self, else_agent: Arc<dyn Agent>) -> Self {
        self.else_agent = Some(else_agent);
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
}

#[async_trait]
impl Agent for ConditionalAgent {
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
        // Convert InvocationContext to CallbackContext for the condition check
        // We need to evaluate the condition synchronously
        let agent = if (self.condition)(ctx.as_ref() as &dyn CallbackContext) {
            self.if_agent.clone()
        } else if let Some(else_agent) = &self.else_agent {
            else_agent.clone()
        } else {
            // No else agent - return empty stream
            use futures::stream;
            return Ok(Box::pin(stream::empty()));
        };

        agent.run(ctx).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::test_support::{make_ctx, make_ctx_with_state};
    use futures::StreamExt;
    use serde_json::Value;
    use zero_core::Event;

    struct MockAgent {
        name: String,
        description: String,
    }

    #[async_trait]
    impl Agent for MockAgent {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            &self.description
        }

        fn sub_agents(&self) -> &[Arc<dyn Agent>] {
            &[]
        }

        async fn run(&self, _ctx: Arc<dyn InvocationContext>) -> Result<EventStream> {
            use async_stream::stream;
            let name = self.name.clone();
            let s = stream! {
                yield Ok(Event::new("test").with_author(&name));
            };
            Ok(Box::pin(s))
        }
    }

    fn premium_basic() -> (Arc<dyn Agent>, Arc<dyn Agent>) {
        let premium = Arc::new(MockAgent {
            name: "premium".to_string(),
            description: "Premium agent".to_string(),
        }) as Arc<dyn Agent>;
        let basic = Arc::new(MockAgent {
            name: "basic".to_string(),
            description: "Basic agent".to_string(),
        }) as Arc<dyn Agent>;
        (premium, basic)
    }

    #[tokio::test]
    async fn test_conditional_agent_true() {
        let (premium, basic) = premium_basic();
        let router = Arc::new(
            ConditionalAgent::new(
                "router",
                |ctx| {
                    ctx.get_state("is_premium")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                },
                premium.clone(),
            )
            .with_else(basic.clone()),
        );
        let mut state = std::collections::HashMap::new();
        state.insert("is_premium".to_string(), Value::Bool(true));
        let ctx = make_ctx_with_state(router.clone() as Arc<dyn Agent>, state);
        let mut stream = router.run(ctx).await.unwrap();
        let first_event = stream.next().await.unwrap().unwrap();
        assert_eq!(first_event.author, "premium");
    }

    #[tokio::test]
    async fn test_conditional_agent_false() {
        let (premium, basic) = premium_basic();
        let router = Arc::new(
            ConditionalAgent::new(
                "router",
                |ctx| {
                    ctx.get_state("is_premium")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                },
                premium.clone(),
            )
            .with_else(basic.clone()),
        );
        // No state set → defaults to false.
        let ctx = make_ctx(router.clone() as Arc<dyn Agent>);
        let mut stream = router.run(ctx).await.unwrap();
        let first_event = stream.next().await.unwrap().unwrap();
        assert_eq!(first_event.author, "basic");
    }

    #[tokio::test]
    async fn test_conditional_agent_no_else_returns_empty_stream() {
        // Condition is false and no else agent set — should yield empty stream.
        let if_agent = Arc::new(MockAgent {
            name: "if".to_string(),
            description: "if".to_string(),
        }) as Arc<dyn Agent>;

        let router = Arc::new(ConditionalAgent::new(
            "router",
            |_ctx| false,
            if_agent.clone(),
        ));
        let ctx = make_ctx(router.clone() as Arc<dyn Agent>);
        let mut stream = router.run(ctx).await.unwrap();
        assert!(stream.next().await.is_none());
    }

    #[test]
    fn test_conditional_agent_metadata_setters() {
        let if_agent = Arc::new(MockAgent {
            name: "if".to_string(),
            description: "if".to_string(),
        }) as Arc<dyn Agent>;

        let router = ConditionalAgent::new("r", |_| true, if_agent)
            .with_description("desc")
            .before_callback(Arc::new(|_| Box::pin(async { None })))
            .after_callback(Arc::new(|_| Box::pin(async { None })));
        assert_eq!(router.name(), "r");
        assert_eq!(router.description(), "desc");
        assert!(router.sub_agents().is_empty());
    }
}
