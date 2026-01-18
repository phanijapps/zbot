//! # Loop Agent
//!
//! Executes sub-agents repeatedly with exit conditions.

use async_trait::async_trait;
use async_stream::stream;
use futures::StreamExt;
use std::sync::Arc;

use zero_core::{Agent, BeforeAgentCallback, AfterAgentCallback, EventStream, Event, InvocationContext, Result};

/// Loop agent executes sub-agents repeatedly for N iterations or until escalation.
///
/// # Example
///
/// ```ignore
/// let agent = LoopAgent::new("iterator", vec![worker])
///     .with_max_iterations(10);
/// // Runs worker up to 10 times, or until escalation is triggered
/// ```
pub struct LoopAgent {
    name: String,
    description: String,
    sub_agents: Vec<Arc<dyn Agent>>,
    max_iterations: Option<u32>,
    before_callbacks: Vec<BeforeAgentCallback>,
    after_callbacks: Vec<AfterAgentCallback>,
}

impl LoopAgent {
    /// Create a new loop agent.
    pub fn new(name: impl Into<String>, sub_agents: Vec<Arc<dyn Agent>>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            sub_agents,
            max_iterations: None,
            before_callbacks: Vec::new(),
            after_callbacks: Vec::new(),
        }
    }

    /// Set the description.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Set the maximum number of iterations.
    ///
    /// If None, the loop continues until escalation or manual termination.
    pub fn with_max_iterations(mut self, max: u32) -> Self {
        self.max_iterations = Some(max);
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
impl Agent for LoopAgent {
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
        let max_iterations = self.max_iterations;

        let s = stream! {
            let mut count = max_iterations;

            loop {
                let mut should_exit = false;

                for agent in &sub_agents {
                    let mut stream = agent.run(ctx.clone()).await?;

                    while let Some(result) = stream.next().await {
                        match result {
                            Ok(event) => {
                                // Check for escalation flag
                                if event.actions.escalate {
                                    should_exit = true;
                                }
                                yield Ok(event);
                            }
                            Err(e) => {
                                yield Err(e);
                                return;
                            }
                        }
                    }

                    if should_exit {
                        return;
                    }
                }

                if let Some(ref mut c) = count {
                    *c -= 1;
                    if *c == 0 {
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
    use zero_core::{Event, EventActions};

    struct MockAgent {
        name: String,
        description: String,
        escalate_on: Option<u32>,
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
            use std::sync::atomic::{AtomicU32, Ordering};

            static COUNTER: AtomicU32 = AtomicU32::new(0);

            let name = self.name.clone();
            let escalate_on = self.escalate_on;
            let s = stream! {
                let count = COUNTER.fetch_add(1, Ordering::SeqCst);

                let mut event = Event::new("test").with_author(&name);
                if let Some(threshold) = escalate_on {
                    if count >= threshold {
                        event.actions.escalate = true;
                    }
                }
                yield Ok(event);
            };
            Ok(Box::pin(s))
        }
    }

    #[tokio::test]
    async fn test_loop_agent_creation() {
        let agent = Arc::new(MockAgent {
            name: "worker".to_string(),
            description: "Worker".to_string(),
            escalate_on: None,
        }) as Arc<dyn Agent>;

        let loop_agent = LoopAgent::new("iterator", vec![agent])
            .with_max_iterations(5);

        assert_eq!(loop_agent.name(), "iterator");
        assert_eq!(loop_agent.sub_agents().len(), 1);
    }

    #[tokio::test]
    async fn test_loop_agent_with_max_iterations() {
        let agent = Arc::new(MockAgent {
            name: "worker".to_string(),
            description: "Worker".to_string(),
            escalate_on: None,
        }) as Arc<dyn Agent>;

        let loop_agent = LoopAgent::new("iterator", vec![agent])
            .with_max_iterations(3);

        // In a real test, we'd verify the loop stops after 3 iterations
        assert_eq!(loop_agent.max_iterations, Some(3));
    }

    #[tokio::test]
    async fn test_loop_agent_unlimited() {
        let agent = Arc::new(MockAgent {
            name: "worker".to_string(),
            description: "Worker".to_string(),
            escalate_on: None,
        }) as Arc<dyn Agent>;

        let loop_agent = LoopAgent::new("iterator", vec![agent]);

        assert_eq!(loop_agent.max_iterations, None);
    }
}
