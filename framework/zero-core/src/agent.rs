//! # Agent Trait
//!
//! Core agent interface for the Zero framework.

use crate::context::InvocationContext;
use crate::error::Result;
use async_trait::async_trait;
use futures::stream::Stream;
use std::pin::Pin;
use std::sync::Arc;

/// Event stream type returned by agents.
pub type EventStream = Pin<Box<dyn Stream<Item = Result<Event>> + Send>>;

/// Core agent trait.
///
/// All agents must implement this trait. Agents receive an invocation context
/// and return a stream of events.
#[async_trait]
pub trait Agent: Send + Sync {
    /// Get the agent's name.
    fn name(&self) -> &str;

    /// Get the agent's description.
    fn description(&self) -> &str;

    /// Get sub-agents if this is a composite agent.
    fn sub_agents(&self) -> &[Arc<dyn Agent>];

    /// Run the agent with the given context.
    ///
    /// Returns a stream of events representing the agent's execution.
    async fn run(&self, ctx: Arc<dyn InvocationContext>) -> Result<EventStream>;
}

// Re-export Event at crate level for convenience
use crate::event::Event;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{
        CallbackContext, InvocationContext, ReadonlyContext, RunConfig, Session,
    };
    use crate::event::Event;
    use crate::event::EventActions;
    use crate::types::{Content, Part};

    struct TestAgent {
        name: String,
        description: String,
    }

    // Mock implementation for testing
    #[allow(dead_code)]
    struct MockInvocationContext {
        agent: Arc<dyn Agent>,
        run_config: RunConfig,
    }

    impl ReadonlyContext for MockInvocationContext {
        fn invocation_id(&self) -> &str {
            "test"
        }
        fn agent_name(&self) -> &str {
            "test"
        }
        fn user_id(&self) -> &str {
            "user"
        }
        fn app_name(&self) -> &str {
            "app"
        }
        fn session_id(&self) -> &str {
            "session"
        }
        fn branch(&self) -> &str {
            ""
        }
        fn user_content(&self) -> &Content {
            use std::sync::LazyLock;
            static CONTENT: LazyLock<Content> = LazyLock::new(|| Content {
                role: "user".to_string(),
                parts: vec![Part::Text {
                    text: "test".to_string(),
                }],
            });
            &CONTENT
        }
    }

    impl CallbackContext for MockInvocationContext {
        fn get_state(&self, _key: &str) -> Option<serde_json::Value> {
            None
        }
        fn set_state(&self, _key: String, _value: serde_json::Value) {}
    }

    impl InvocationContext for MockInvocationContext {
        fn agent(&self) -> Arc<dyn Agent> {
            self.agent.clone()
        }
        fn session(&self) -> Arc<dyn Session> {
            unimplemented!()
        }
        fn run_config(&self) -> &RunConfig {
            &self.run_config
        }
        fn actions(&self) -> EventActions {
            EventActions::default()
        }
        fn set_actions(&self, _actions: EventActions) {}
        fn end_invocation(&self) {}
        fn ended(&self) -> bool {
            false
        }
        fn add_content(&self, _content: Content) {
            // Mock implementation - does nothing
        }
    }

    #[async_trait]
    impl Agent for TestAgent {
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
            let s = async_stream::stream! {
                yield Ok(Event::new("test").with_author("agent"));
            };
            Ok(Box::pin(s))
        }
    }

    #[tokio::test]
    async fn test_agent_trait() {
        let agent = TestAgent {
            name: "test".to_string(),
            description: "Test agent".to_string(),
        };

        assert_eq!(agent.name(), "test");
        assert_eq!(agent.description(), "Test agent");
        assert!(agent.sub_agents().is_empty());
    }
}
