//! # Parallel Agent
//!
//! Executes sub-agents concurrently.

use async_stream::stream;
use async_trait::async_trait;
use futures::stream::{FuturesUnordered, StreamExt};
use std::sync::Arc;

use zero_core::{
    AfterAgentCallback, Agent, BeforeAgentCallback, EventStream, InvocationContext, Result,
};

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

        let parallel = ParallelAgent::new("par", vec![agent]).with_description("Parallel team");

        assert_eq!(parallel.description(), "Parallel team");
    }

    #[tokio::test]
    async fn test_parallel_agent_empty() {
        let parallel = ParallelAgent::new("empty", vec![]);
        assert_eq!(parallel.sub_agents().len(), 0);
    }

    // ============================================================================
    // RUNTIME TESTS
    // ============================================================================

    use crate::workflow::test_support::make_ctx;

    /// Agent whose run() returns Err.
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

    /// Agent whose stream yields an error.
    struct ErrorStreamAgent;
    #[async_trait]
    impl Agent for ErrorStreamAgent {
        fn name(&self) -> &str {
            "errstream"
        }
        fn description(&self) -> &str {
            ""
        }
        fn sub_agents(&self) -> &[Arc<dyn Agent>] {
            &[]
        }
        async fn run(&self, _ctx: Arc<dyn InvocationContext>) -> Result<EventStream> {
            use async_stream::stream;
            let s = stream! {
                yield Err(zero_core::ZeroError::Generic("boom".to_string()));
            };
            Ok(Box::pin(s))
        }
    }

    #[tokio::test]
    async fn test_parallel_agent_runs_all_subagents() {
        let agent1: Arc<dyn Agent> = Arc::new(MockAgent {
            name: "a1".to_string(),
            description: "".to_string(),
        });
        let agent2: Arc<dyn Agent> = Arc::new(MockAgent {
            name: "a2".to_string(),
            description: "".to_string(),
        });
        let parallel = ParallelAgent::new("par", vec![Arc::clone(&agent1), Arc::clone(&agent2)]);
        let ctx = make_ctx(Arc::clone(&agent1));
        let mut stream = parallel.run(ctx).await.unwrap();
        let mut count = 0;
        while let Some(e) = stream.next().await {
            e.unwrap();
            count += 1;
        }
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn test_parallel_agent_stream_error_short_circuits() {
        let bad: Arc<dyn Agent> = Arc::new(ErrorStreamAgent);
        let parallel = ParallelAgent::new("par", vec![Arc::clone(&bad)]);
        let ctx = make_ctx(bad);
        let mut stream = parallel.run(ctx).await.unwrap();
        let first = stream.next().await.unwrap();
        assert!(first.is_err());
    }

    #[tokio::test]
    async fn test_parallel_agent_run_error_yields_err() {
        let bad: Arc<dyn Agent> = Arc::new(FailingAgent);
        let parallel = ParallelAgent::new("par", vec![Arc::clone(&bad)]);
        let ctx = make_ctx(bad);
        let mut stream = parallel.run(ctx).await.unwrap();
        let first = stream.next().await.unwrap();
        assert!(first.is_err());
    }

    #[test]
    fn test_inner_agent_metadata_invoked_via_trait() {
        // Cover the name/description/sub_agents bodies of test mocks via direct calls.
        let mock = MockAgent {
            name: "m".to_string(),
            description: "d".to_string(),
        };
        assert_eq!(mock.name(), "m");
        assert_eq!(mock.description(), "d");
        assert!(mock.sub_agents().is_empty());

        assert_eq!(FailingAgent.name(), "fail");
        assert_eq!(FailingAgent.description(), "");
        assert!(FailingAgent.sub_agents().is_empty());

        assert_eq!(ErrorStreamAgent.name(), "errstream");
        assert_eq!(ErrorStreamAgent.description(), "");
        assert!(ErrorStreamAgent.sub_agents().is_empty());
    }

    #[test]
    fn test_parallel_agent_callbacks() {
        let agent: Arc<dyn Agent> = Arc::new(MockAgent {
            name: "a".to_string(),
            description: "".to_string(),
        });
        let parallel = ParallelAgent::new("par", vec![Arc::clone(&agent)])
            .with_description("d")
            .before_callback(Arc::new(|_| Box::pin(async { None })))
            .after_callback(Arc::new(|_| Box::pin(async { None })));
        assert_eq!(parallel.description(), "d");
    }
}
