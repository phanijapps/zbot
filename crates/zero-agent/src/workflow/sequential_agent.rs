//! # Sequential Agent
//!
//! Executes sub-agents in order (A → B → C).

use async_trait::async_trait;
use async_stream::stream;
use futures::StreamExt;
use std::sync::Arc;

use zero_core::{Agent, BeforeAgentCallback, AfterAgentCallback, EventStream, InvocationContext, Result};

/// Sequential agent executes sub-agents once in order.
///
/// # Example
///
/// ```ignore
/// let agent = SequentialAgent::new("pipeline", vec![agent_a, agent_b, agent_c]);
/// ```
///
/// This is equivalent to running LoopAgent with max_iterations=1.
pub struct SequentialAgent {
    name: String,
    description: String,
    sub_agents: Vec<Arc<dyn Agent>>,
    before_callbacks: Vec<BeforeAgentCallback>,
    after_callbacks: Vec<AfterAgentCallback>,
}

impl SequentialAgent {
    /// Create a new sequential agent.
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

    /// Execute before callbacks.
    async fn run_before_callbacks(&self, ctx: Arc<dyn InvocationContext>) {
        for callback in &self.before_callbacks {
            if let Some(content) = callback(ctx.clone()).await {
                // In a real implementation, this would prepend to conversation
                let _ = content;
            }
        }
    }

    /// Execute after callbacks.
    async fn run_after_callbacks(&self, ctx: Arc<dyn InvocationContext>) {
        for callback in &self.after_callbacks {
            if let Some(content) = callback(ctx.clone()).await {
                // In a real implementation, this would append to conversation
                let _ = content;
            }
        }
    }
}

#[async_trait]
impl Agent for SequentialAgent {
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
        use zero_core::Event;

        let sub_agents = self.sub_agents.clone();

        let s = stream! {
            // Run before callbacks
            // Note: We can't await callbacks here in the stream macro directly
            // In a real implementation, these would be run before creating the stream

            for agent in &sub_agents {
                let agent_name = agent.name().to_string();

                // Emit agent start event
                let mut start_event = Event::new(&agent_name);
                start_event.author = format!("workflow:{}", agent_name);
                start_event.metadata.insert("agent_lifecycle".to_string(), serde_json::json!("start"));
                start_event.metadata.insert("agent_id".to_string(), serde_json::json!(agent_name));
                yield Ok(start_event);

                let mut stream = agent.run(ctx.clone()).await?;

                while let Some(result) = stream.next().await {
                    yield result;
                }

                // Emit agent end event
                let mut end_event = Event::new(&agent_name);
                end_event.author = format!("workflow:{}", agent_name);
                end_event.metadata.insert("agent_lifecycle".to_string(), serde_json::json!("end"));
                end_event.metadata.insert("agent_id".to_string(), serde_json::json!(agent_name));
                yield Ok(end_event);
            }
        };

        Ok(Box::pin(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zero_core::{Event, Content};
    use std::sync::Arc;

    // Mock agent for testing
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
    async fn test_sequential_agent_creation() {
        let agent1 = Arc::new(MockAgent {
            name: "agent1".to_string(),
            description: "Agent 1".to_string(),
        }) as Arc<dyn Agent>;
        let agent2 = Arc::new(MockAgent {
            name: "agent2".to_string(),
            description: "Agent 2".to_string(),
        }) as Arc<dyn Agent>;

        let sequential = SequentialAgent::new("seq", vec![agent1, agent2]);

        assert_eq!(sequential.name(), "seq");
        assert_eq!(sequential.sub_agents().len(), 2);
    }

    #[tokio::test]
    async fn test_sequential_agent_with_description() {
        let agent = Arc::new(MockAgent {
            name: "test".to_string(),
            description: "Test".to_string(),
        }) as Arc<dyn Agent>;

        let sequential = SequentialAgent::new("seq", vec![agent])
            .with_description("Sequential pipeline");

        assert_eq!(sequential.description(), "Sequential pipeline");
    }

    #[tokio::test]
    async fn test_sequential_agent_empty() {
        let sequential = SequentialAgent::new("empty", vec![]);
        assert_eq!(sequential.sub_agents().len(), 0);
    }
}
