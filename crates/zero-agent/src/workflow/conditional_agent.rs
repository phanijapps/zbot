//! # Conditional Agent (Rule-Based)
//!
//! Rule-based conditional routing agent.

use async_trait::async_trait;
use std::sync::Arc;

use zero_core::{Agent, BeforeAgentCallback, AfterAgentCallback, EventStream, InvocationContext, Result, CallbackContext};

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
    use zero_core::{Event, ReadonlyContext, CallbackContext, Content, context::Session};
    use serde_json::Value;
    use futures::StreamExt;

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

    struct TestContext {
        is_premium: bool,
    }

    impl ReadonlyContext for TestContext {
        fn invocation_id(&self) -> &str { "test" }
        fn agent_name(&self) -> &str { "test" }
        fn user_id(&self) -> &str { "test" }
        fn app_name(&self) -> &str { "test" }
        fn session_id(&self) -> &str { "test" }
        fn branch(&self) -> &str { "test" }
        fn user_content(&self) -> &Content {
            static CONTENT: Content = Content { role: String::new(), parts: Vec::new() };
            &CONTENT
        }
    }

    impl CallbackContext for TestContext {
        fn get_state(&self, key: &str) -> Option<Value> {
            if key == "is_premium" {
                Some(Value::Bool(self.is_premium))
            } else {
                None
            }
        }
        fn set_state(&self, _key: String, _value: Value) {}
    }

    // Mock InvocationContext that implements both InvocationContext and TestContext
    struct MockInvocationContext {
        agent: Arc<dyn Agent>,
        test_ctx: TestContext,
    }

    impl ReadonlyContext for MockInvocationContext {
        fn invocation_id(&self) -> &str { self.test_ctx.invocation_id() }
        fn agent_name(&self) -> &str { self.test_ctx.agent_name() }
        fn user_id(&self) -> &str { self.test_ctx.user_id() }
        fn app_name(&self) -> &str { self.test_ctx.app_name() }
        fn session_id(&self) -> &str { self.test_ctx.session_id() }
        fn branch(&self) -> &str { self.test_ctx.branch() }
        fn user_content(&self) -> &Content { self.test_ctx.user_content() }
    }

    impl CallbackContext for MockInvocationContext {
        fn get_state(&self, key: &str) -> Option<Value> { self.test_ctx.get_state(key) }
        fn set_state(&self, key: String, value: Value) { self.test_ctx.set_state(key, value) }
    }

    impl zero_core::InvocationContext for MockInvocationContext {
        fn agent(&self) -> Arc<dyn Agent> { self.agent.clone() }
        fn session(&self) -> Arc<dyn Session> { unimplemented!() }
        fn run_config(&self) -> &zero_core::RunConfig { unimplemented!() }
        fn actions(&self) -> zero_core::EventActions { zero_core::EventActions::default() }
        fn set_actions(&self, _actions: zero_core::EventActions) {}
        fn end_invocation(&self) {}
        fn ended(&self) -> bool { false }
        fn add_content(&self, _content: zero_core::Content) {
            // Mock implementation - does nothing
        }
    }

    #[tokio::test]
    async fn test_conditional_agent_true() {
        let premium = Arc::new(MockAgent {
            name: "premium".to_string(),
            description: "Premium agent".to_string(),
        }) as Arc<dyn Agent>;
        let basic = Arc::new(MockAgent {
            name: "basic".to_string(),
            description: "Basic agent".to_string(),
        }) as Arc<dyn Agent>;

        let router = Arc::new(ConditionalAgent::new(
            "router",
            |ctx| ctx.get_state("is_premium").and_then(|v| v.as_bool()).unwrap_or(false),
            premium.clone(),
        )
        .with_else(basic.clone()));

        let test_ctx = TestContext { is_premium: true };
        let inv_ctx = Arc::new(MockInvocationContext {
            agent: router.clone() as Arc<dyn Agent>,
            test_ctx,
        }) as Arc<dyn zero_core::InvocationContext>;

        let mut stream = router.run(inv_ctx).await.unwrap();
        let first_event = stream.next().await.unwrap().unwrap();
        assert_eq!(first_event.author, "premium");
    }

    #[tokio::test]
    async fn test_conditional_agent_false() {
        let premium = Arc::new(MockAgent {
            name: "premium".to_string(),
            description: "Premium agent".to_string(),
        }) as Arc<dyn Agent>;
        let basic = Arc::new(MockAgent {
            name: "basic".to_string(),
            description: "Basic agent".to_string(),
        }) as Arc<dyn Agent>;

        let router = Arc::new(ConditionalAgent::new(
            "router",
            |ctx| ctx.get_state("is_premium").and_then(|v| v.as_bool()).unwrap_or(false),
            premium.clone(),
        )
        .with_else(basic.clone()));

        let test_ctx = TestContext { is_premium: false };
        let inv_ctx = Arc::new(MockInvocationContext {
            agent: router.clone() as Arc<dyn Agent>,
            test_ctx,
        }) as Arc<dyn zero_core::InvocationContext>;

        let mut stream = router.run(inv_ctx).await.unwrap();
        let first_event = stream.next().await.unwrap().unwrap();
        assert_eq!(first_event.author, "basic");
    }
}
