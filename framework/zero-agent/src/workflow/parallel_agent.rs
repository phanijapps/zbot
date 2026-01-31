//! # Parallel Agent
//!
//! Executes sub-agents concurrently.

use async_trait::async_trait;
use async_stream::stream;
use futures::stream::{FuturesUnordered, StreamExt};
use std::sync::Arc;

use zero_core::{Agent, BeforeAgentCallback, AfterAgentCallback, EventStream, InvocationContext, Result};

/// Parallel agent executes sub-agents concurrently.
///
/// # Example
///
/// ```ignore
/// let agent = ParallelAgent::new("team", vec![agent_a, agent_b, agent_c]);
/// // All agents run simultaneously, events are streamed as they arrive
/// ```
pub struct ParallelAgent {
    name: String,
    description: String,
    sub_agents: Vec<Arc<dyn Agent>>,
    before_callbacks: Vec<BeforeAgentCallback>,
    after_callbacks: Vec<AfterAgentCallback>,
}

impl ParallelAgent {
    /// Create a new parallel agent.
    pub fn new(name: impl Into<String>, sub_agents: Vec<Arc<dyn Agent>>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            sub_agents,
            before_callbacks: Vec::new(),
            after_callbacks: Vec::new(),
        }
    }

    /// Set the description.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
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
impl Agent for ParallelAgent {
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
        let sub_agents = self.sub_agents.clone();

        let s = stream! {
            let mut futures = FuturesUnordered::new();

            for agent in sub_agents {
                let ctx = ctx.clone();
                futures.push(async move {
                    agent.run(ctx).await
                });
            }

            while let Some(result) = futures.next().await {
                match result {
                    Ok(mut stream) => {
                        while let Some(event_result) = stream.next().await {
                            yield event_result;
                        }
                    }
                    Err(e) => {
                        yield Err(e);
                        return;
                    }
                }
            }
        };

        Ok(Box::pin(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
                let mut event = Event::new("test");
                event.author = name;
                yield Ok(event);
            };
            Ok(Box::pin(s))
        }
    }

    #[tokio::test]
    async fn test_parallel_agent_creation() {
        let agent1 = Arc::new(MockAgent {
            name: "agent1".to_string(),
            description: "Agent 1".to_string(),
        }) as Arc<dyn Agent>;
        let agent2 = Arc::new(MockAgent {
            name: "agent2".to_string(),
            description: "Agent 2".to_string(),
        }) as Arc<dyn Agent>;

        let parallel = ParallelAgent::new("par", vec![agent1, agent2]);

        assert_eq!(parallel.name(), "par");
        assert_eq!(parallel.sub_agents().len(), 2);
    }

    #[tokio::test]
    async fn test_parallel_agent_with_description() {
        let agent = Arc::new(MockAgent {
            name: "test".to_string(),
            description: "Test".to_string(),
        }) as Arc<dyn Agent>;

        let parallel = ParallelAgent::new("par", vec![agent])
            .with_description("Parallel team");

        assert_eq!(parallel.description(), "Parallel team");
    }

    #[tokio::test]
    async fn test_parallel_agent_empty() {
        let parallel = ParallelAgent::new("empty", vec![]);
        assert_eq!(parallel.sub_agents().len(), 0);
    }
}
