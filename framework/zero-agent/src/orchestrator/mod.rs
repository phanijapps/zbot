//! # Orchestrator Module
//!
//! Intelligent task orchestration using capability-based agent routing.
//!
//! ## Overview
//!
//! The orchestrator provides:
//! - Capability-based agent discovery and routing
//! - Task graph execution with dependency management
//! - Parallel execution where dependencies allow
//! - Comprehensive execution tracing
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────┐
//! │              OrchestratorAgent              │
//! │  ┌───────────────────────────────────────┐  │
//! │  │         CapabilityRegistry            │  │
//! │  │   (agents indexed by capabilities)    │  │
//! │  └───────────────────────────────────────┘  │
//! │  ┌───────────────────────────────────────┐  │
//! │  │            TaskGraph                  │  │
//! │  │    (DAG of tasks with deps)          │  │
//! │  └───────────────────────────────────────┘  │
//! │  ┌───────────────────────────────────────┐  │
//! │  │         ExecutionTrace                │  │
//! │  │     (observability & debugging)       │  │
//! │  └───────────────────────────────────────┘  │
//! └─────────────────────────────────────────────┘
//! ```
//!
//! ## Example
//!
//! ```rust,no_run
//! use zero_agent::orchestrator::{
//!     OrchestratorAgent, OrchestratorConfig,
//!     task_graph::{TaskGraph, TaskNode},
//! };
//! use zero_core::{CapabilityRegistry, AgentCapabilities, capability::common};
//! use std::sync::Arc;
//!
//! // Create registry and register agents
//! let registry = Arc::new(CapabilityRegistry::new());
//! registry.register(
//!     AgentCapabilities::builder("code-agent")
//!         .add_capability(common::code_review())
//!         .build()
//! );
//!
//! // Create orchestrator
//! let orchestrator = OrchestratorAgent::new(registry);
//! ```

pub mod task_graph;
pub mod trace;

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex as TokioMutex;

use zero_core::{
    context::Session, Agent, AgentCapabilities, CallbackContext, CapabilityQuery,
    CapabilityRegistry, CapabilityRouter, Content, Event, EventActions, EventStream,
    InvocationContext, ReadonlyContext, Result, RunConfig, ZeroError,
};

pub use task_graph::{TaskGraph, TaskGraphError, TaskNode, TaskStatus};
pub use trace::{
    ExecutionTrace, TraceBuilder, TraceEvent, TraceEventKind, TraceMetrics, TraceOutcome,
};

// ============================================================================
// ORCHESTRATOR CONFIG
// ============================================================================

/// Configuration for the orchestrator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorConfig {
    /// Maximum number of parallel tasks
    #[serde(default = "default_max_parallel")]
    pub max_parallel_tasks: usize,

    /// Maximum retries per task
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    /// Whether to continue on task failure
    #[serde(default)]
    pub continue_on_failure: bool,

    /// Timeout per task in seconds
    #[serde(default)]
    pub task_timeout_secs: Option<u64>,

    /// Whether to enable detailed tracing
    #[serde(default = "default_true")]
    pub enable_tracing: bool,
}

fn default_max_parallel() -> usize {
    4
}

fn default_max_retries() -> u32 {
    3
}

fn default_true() -> bool {
    true
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            max_parallel_tasks: 4,
            max_retries: 3,
            continue_on_failure: false,
            task_timeout_secs: Some(300),
            enable_tracing: true,
        }
    }
}

// ============================================================================
// TASK STATE
// ============================================================================

use zero_core::context::State;

/// State storage for task execution.
struct TaskState {
    data: RwLock<HashMap<String, serde_json::Value>>,
}

impl TaskState {
    fn new() -> Self {
        Self {
            data: RwLock::new(HashMap::new()),
        }
    }
}

impl State for TaskState {
    fn get(&self, key: &str) -> Option<serde_json::Value> {
        self.data.read().ok().and_then(|s| s.get(key).cloned())
    }

    fn set(&mut self, key: String, value: serde_json::Value) {
        if let Ok(mut data) = self.data.write() {
            data.insert(key, value);
        }
    }

    fn all(&self) -> HashMap<String, serde_json::Value> {
        self.data.read().map(|s| s.clone()).unwrap_or_default()
    }
}

// ============================================================================
// TASK SESSION
// ============================================================================

/// Session for task execution within the orchestrator.
struct TaskSession {
    id: String,
    app_name: String,
    user_id: String,
    state: TaskState,
    history: RwLock<Vec<Content>>,
}

impl TaskSession {
    fn new(task_id: &str) -> Self {
        Self {
            id: format!("task-session-{}", task_id),
            app_name: "orchestrator".to_string(),
            user_id: "orchestrator".to_string(),
            state: TaskState::new(),
            history: RwLock::new(Vec::new()),
        }
    }

    fn add_content(&self, content: Content) {
        if let Ok(mut history) = self.history.write() {
            history.push(content);
        }
    }
}

impl Session for TaskSession {
    fn id(&self) -> &str {
        &self.id
    }

    fn app_name(&self) -> &str {
        &self.app_name
    }

    fn user_id(&self) -> &str {
        &self.user_id
    }

    fn state(&self) -> &dyn State {
        &self.state
    }

    fn conversation_history(&self) -> Vec<Content> {
        self.history.read().map(|h| h.clone()).unwrap_or_default()
    }
}

// ============================================================================
// TASK INVOCATION CONTEXT
// ============================================================================

/// Invocation context for executing a task.
struct TaskInvocationContext {
    invocation_id: String,
    agent: Arc<dyn Agent>,
    session: Arc<TaskSession>,
    run_config: RunConfig,
    actions: TokioMutex<EventActions>,
    ended: std::sync::atomic::AtomicBool,
    user_content: Content,
}

impl TaskInvocationContext {
    fn new(agent: Arc<dyn Agent>, task_id: &str, task_description: &str) -> Self {
        let session = TaskSession::new(task_id);
        let user_content = Content::user(task_description);

        // Add task description as initial user message
        session.add_content(user_content.clone());

        Self {
            invocation_id: format!("task-invocation-{}", task_id),
            agent,
            session: Arc::new(session),
            run_config: RunConfig::default(),
            actions: TokioMutex::new(EventActions::default()),
            ended: std::sync::atomic::AtomicBool::new(false),
            user_content,
        }
    }
}

impl ReadonlyContext for TaskInvocationContext {
    fn invocation_id(&self) -> &str {
        &self.invocation_id
    }

    fn agent_name(&self) -> &str {
        self.agent.name()
    }

    fn user_id(&self) -> &str {
        "orchestrator"
    }

    fn app_name(&self) -> &str {
        "orchestrator"
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

impl CallbackContext for TaskInvocationContext {
    fn get_state(&self, key: &str) -> Option<serde_json::Value> {
        self.session.state().get(key)
    }

    fn set_state(&self, key: String, value: serde_json::Value) {
        // Note: State::set takes &mut self, but we can't mutate through the session reference
        // This is a limitation - we'd need interior mutability or a different design
        // For now, we use the TaskState's RwLock directly through a workaround
        if let Ok(mut data) = self.session.state.data.write() {
            data.insert(key, value);
        }
    }
}

impl InvocationContext for TaskInvocationContext {
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
        self.actions
            .try_lock()
            .map(|a| a.clone())
            .unwrap_or_default()
    }

    fn set_actions(&self, actions: EventActions) {
        if let Ok(mut a) = self.actions.try_lock() {
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
        // Use TaskSession's add_content method directly
        if let Ok(mut history) = self.session.history.write() {
            history.push(content);
        }
    }
}

// ============================================================================
// ORCHESTRATOR AGENT
// ============================================================================

/// Agent that orchestrates task execution across multiple agents.
///
/// The orchestrator:
/// 1. Receives a goal or task graph
/// 2. Routes tasks to capable agents
/// 3. Manages execution order and parallelism
/// 4. Handles failures and retries
/// 5. Collects and returns results
pub struct OrchestratorAgent {
    name: String,
    description: String,
    registry: Arc<CapabilityRegistry>,
    router: CapabilityRouter,
    config: OrchestratorConfig,
    sub_agents: Vec<Arc<dyn Agent>>,
    /// Agent store: maps agent_id to actual agent instance
    agent_store: RwLock<HashMap<String, Arc<dyn Agent>>>,
}

impl OrchestratorAgent {
    /// Create a new orchestrator with the given capability registry.
    pub fn new(registry: Arc<CapabilityRegistry>) -> Self {
        let router = CapabilityRouter::new(Arc::clone(&registry));
        Self {
            name: "orchestrator".to_string(),
            description: "Orchestrates task execution across multiple agents".to_string(),
            registry,
            router,
            config: OrchestratorConfig::default(),
            sub_agents: Vec::new(),
            agent_store: RwLock::new(HashMap::new()),
        }
    }

    /// Set the orchestrator name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Set the orchestrator description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Set the configuration.
    pub fn with_config(mut self, config: OrchestratorConfig) -> Self {
        self.config = config;
        self
    }

    /// Add a sub-agent (for composition).
    pub fn with_agent(mut self, agent: Arc<dyn Agent>) -> Self {
        // Store in agent_store by name
        {
            let mut store = self.agent_store.write().unwrap();
            store.insert(agent.name().to_string(), Arc::clone(&agent));
        }
        self.sub_agents.push(agent);
        self
    }

    /// Register an agent with its capabilities.
    ///
    /// This both stores the agent instance and registers its capabilities
    /// in the registry for capability-based routing.
    pub fn register_agent(&self, agent: Arc<dyn Agent>, capabilities: AgentCapabilities) {
        // Store the agent instance
        {
            let mut store = self.agent_store.write().unwrap();
            store.insert(capabilities.agent_id.clone(), Arc::clone(&agent));
        }
        // Register capabilities
        self.registry.register(capabilities);
    }

    /// Get an agent by ID.
    pub fn get_agent(&self, agent_id: &str) -> Option<Arc<dyn Agent>> {
        let store = self.agent_store.read().unwrap();
        store.get(agent_id).cloned()
    }

    /// Get the capability registry.
    pub fn registry(&self) -> &Arc<CapabilityRegistry> {
        &self.registry
    }

    /// Get the router.
    pub fn router(&self) -> &CapabilityRouter {
        &self.router
    }

    /// Find the best agent for a capability query.
    pub fn find_agent(&self, query: &CapabilityQuery) -> Option<AgentCapabilities> {
        self.registry.find_best_agent(query)
    }

    /// Execute a task graph.
    ///
    /// This is the core orchestration method that:
    /// 1. Computes parallel execution groups
    /// 2. Assigns tasks to agents
    /// 3. Executes in dependency order
    /// 4. Collects results
    pub async fn execute_graph(
        &self,
        graph: &mut TaskGraph,
        _ctx: Arc<dyn InvocationContext>,
    ) -> Result<ExecutionTrace> {
        let mut trace = TraceBuilder::new(&graph.id);

        trace.trace_mut().record(TraceEvent::new(
            TraceEventKind::PlanCreated,
            format!("Executing task graph with {} tasks", graph.len()),
        ));

        // Get parallel execution groups - collect task info to avoid borrow issues
        // Tuple: (task_id, required_capability, description, input)
        let groups: Vec<Vec<(String, Option<String>, String, Option<serde_json::Value>)>> = {
            let raw_groups = graph.parallel_groups().map_err(|e| {
                ZeroError::Config(format!("Failed to compute execution order: {}", e))
            })?;

            raw_groups
                .into_iter()
                .map(|group| {
                    group
                        .into_iter()
                        .map(|task| {
                            (
                                task.id.clone(),
                                task.required_capability.clone(),
                                task.description.clone(),
                                task.input.clone(),
                            )
                        })
                        .collect()
                })
                .collect()
        };

        for (group_idx, group) in groups.iter().enumerate() {
            trace.begin_span(format!("group-{}", group_idx));

            // Execute tasks in this group (can be parallel)
            for (task_id, required_capability, description, input) in group {
                // Find agent for this task
                let agent_caps = if let Some(cap_id) = required_capability {
                    let query = CapabilityQuery::new().with_capability_ids(vec![cap_id.clone()]);
                    self.find_agent(&query)
                } else {
                    // Use first available agent
                    self.registry.available_agents().into_iter().next()
                };

                let agent_id = match agent_caps {
                    Some(caps) => {
                        trace.agent_selected(
                            &caps.agent_id,
                            format!("Selected {} for task {}", caps.agent_name, task_id),
                        );
                        caps.agent_id.clone()
                    }
                    None => {
                        trace.error(format!("No agent found for task {}", task_id));
                        if let Some(t) = graph.get_task_mut(task_id) {
                            t.fail("No capable agent available");
                        }
                        continue;
                    }
                };

                // Get the actual agent instance
                let agent = match self.get_agent(&agent_id) {
                    Some(a) => a,
                    None => {
                        trace.error(format!(
                            "Agent {} not found in store for task {}",
                            agent_id, task_id
                        ));
                        if let Some(t) = graph.get_task_mut(task_id) {
                            t.fail(&format!("Agent {} not found in store", agent_id));
                        }
                        continue;
                    }
                };

                // Update task with assigned agent
                if let Some(t) = graph.get_task_mut(task_id) {
                    t.assigned_agent = Some(agent_id.clone());
                    t.start();
                }
                trace.task_started(task_id, format!("Executing task: {}", task_id));

                // Build task prompt from description and input
                let task_prompt = if let Some(input_data) = input {
                    format!(
                        "{}\n\nInput data:\n{}",
                        description,
                        serde_json::to_string_pretty(input_data).unwrap_or_default()
                    )
                } else {
                    description.clone()
                };

                // Execute the task by invoking the agent
                let start_time = std::time::Instant::now();
                let ctx = Arc::new(TaskInvocationContext::new(
                    Arc::clone(&agent),
                    task_id,
                    &task_prompt,
                ));

                // Run the agent and collect results
                let result = match agent.run(ctx).await {
                    Ok(mut stream) => {
                        let mut collected_content = String::new();
                        while let Some(event_result) = stream.next().await {
                            match event_result {
                                Ok(event) => {
                                    if let Some(content) = &event.content {
                                        // Extract text from content parts
                                        for part in &content.parts {
                                            if let zero_core::Part::Text { text } = part {
                                                collected_content.push_str(text);
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    trace
                                        .error(format!("Stream error for task {}: {}", task_id, e));
                                }
                            }
                        }
                        Ok(serde_json::json!({
                            "status": "completed",
                            "output": collected_content,
                        }))
                    }
                    Err(e) => Err(e),
                };

                let duration_ms = start_time.elapsed().as_millis() as i64;

                match result {
                    Ok(output) => {
                        if let Some(t) = graph.get_task_mut(task_id) {
                            t.complete(output);
                        }
                        trace.task_completed(
                            task_id,
                            format!("Task {} completed", task_id),
                            duration_ms,
                        );
                    }
                    Err(e) => {
                        let error_msg = e.to_string();
                        trace.error(format!("Task {} failed: {}", task_id, error_msg));
                        if let Some(t) = graph.get_task_mut(task_id) {
                            t.fail(&error_msg);
                        }
                        if !self.config.continue_on_failure {
                            return Err(e);
                        }
                    }
                }
            }

            trace.end_span(format!("group-{}", group_idx));
        }

        // Determine overall outcome
        let trace = if graph.has_failures() {
            trace.fail("Execution completed with failures")
        } else {
            trace.complete("Execution completed successfully")
        };

        Ok(trace)
    }
}

#[async_trait]
impl Agent for OrchestratorAgent {
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
        // The orchestrator receives instructions and creates a task graph
        // For now, emit a simple event indicating the orchestrator is ready

        let event = Event {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            invocation_id: ctx.invocation_id().to_string(),
            branch: "main".to_string(),
            author: self.name.clone(),
            content: Some(Content::assistant(
                "Orchestrator ready. Provide a task graph or goal to execute.",
            )),
            actions: EventActions::default(),
            turn_complete: true,
            long_running_tool_ids: Vec::new(),
            metadata: HashMap::new(),
        };

        let stream = async_stream::stream! {
            yield Ok(event);
        };

        Ok(Box::pin(stream))
    }
}

// ============================================================================
// ORCHESTRATOR BUILDER
// ============================================================================

/// Builder for creating orchestrators with a fluent API.
pub struct OrchestratorBuilder {
    name: String,
    description: String,
    registry: Arc<CapabilityRegistry>,
    config: OrchestratorConfig,
    sub_agents: Vec<Arc<dyn Agent>>,
    agent_store: HashMap<String, Arc<dyn Agent>>,
}

impl OrchestratorBuilder {
    /// Create a new builder with the given registry.
    pub fn new(registry: Arc<CapabilityRegistry>) -> Self {
        Self {
            name: "orchestrator".to_string(),
            description: "Task orchestrator".to_string(),
            registry,
            config: OrchestratorConfig::default(),
            sub_agents: Vec::new(),
            agent_store: HashMap::new(),
        }
    }

    /// Set the name.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Set the description.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Set the config.
    pub fn config(mut self, config: OrchestratorConfig) -> Self {
        self.config = config;
        self
    }

    /// Add a sub-agent.
    pub fn agent(mut self, agent: Arc<dyn Agent>) -> Self {
        // Store in agent_store by name
        self.agent_store
            .insert(agent.name().to_string(), Arc::clone(&agent));
        self.sub_agents.push(agent);
        self
    }

    /// Register an agent with its capabilities.
    pub fn register_agent(
        mut self,
        agent: Arc<dyn Agent>,
        capabilities: AgentCapabilities,
    ) -> Self {
        self.agent_store
            .insert(capabilities.agent_id.clone(), Arc::clone(&agent));
        self.registry.register(capabilities);
        self
    }

    /// Build the orchestrator.
    pub fn build(self) -> OrchestratorAgent {
        OrchestratorAgent {
            name: self.name,
            description: self.description,
            router: CapabilityRouter::new(Arc::clone(&self.registry)),
            registry: self.registry,
            config: self.config,
            sub_agents: self.sub_agents,
            agent_store: RwLock::new(self.agent_store),
        }
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use zero_core::capability::common;

    fn setup_registry() -> Arc<CapabilityRegistry> {
        let registry = Arc::new(CapabilityRegistry::new());

        registry.register(
            AgentCapabilities::builder("code-agent")
                .add_capability(common::code_review())
                .add_capability(common::code_generation())
                .build(),
        );

        registry.register(
            AgentCapabilities::builder("research-agent")
                .add_capability(common::web_search())
                .add_capability(common::research())
                .build(),
        );

        registry
    }

    #[test]
    fn test_orchestrator_creation() {
        let registry = setup_registry();
        let orchestrator = OrchestratorAgent::new(registry);

        assert_eq!(orchestrator.name(), "orchestrator");
    }

    #[test]
    fn test_orchestrator_builder() {
        let registry = setup_registry();
        let orchestrator = OrchestratorBuilder::new(registry)
            .name("my-orchestrator")
            .description("Custom orchestrator")
            .config(OrchestratorConfig {
                max_parallel_tasks: 8,
                ..Default::default()
            })
            .build();

        assert_eq!(orchestrator.name(), "my-orchestrator");
    }

    #[test]
    fn test_find_agent() {
        let registry = setup_registry();
        let orchestrator = OrchestratorAgent::new(registry);

        let query = CapabilityQuery::new().with_capability_ids(vec!["code_review"]);
        let agent = orchestrator.find_agent(&query);

        assert!(agent.is_some());
        assert_eq!(agent.unwrap().agent_id, "code-agent");
    }

    #[tokio::test]
    async fn test_execute_simple_graph() {
        use async_stream::stream;

        let registry = setup_registry();

        // Create a mock agent that will be registered
        struct MockAgent {
            name: String,
        }

        #[async_trait]
        impl Agent for MockAgent {
            fn name(&self) -> &str {
                &self.name
            }
            fn description(&self) -> &str {
                "Mock agent"
            }
            fn sub_agents(&self) -> &[Arc<dyn Agent>] {
                &[]
            }
            async fn run(&self, _ctx: Arc<dyn InvocationContext>) -> Result<EventStream> {
                let s = stream! {
                    yield Ok(Event::new("test"));
                };
                Ok(Box::pin(s))
            }
        }

        // Create orchestrator and register agents with capabilities
        let orchestrator = OrchestratorAgent::new(Arc::clone(&registry));

        let code_agent: Arc<dyn Agent> = Arc::new(MockAgent {
            name: "code-agent".to_string(),
        });
        let research_agent: Arc<dyn Agent> = Arc::new(MockAgent {
            name: "research-agent".to_string(),
        });

        orchestrator.register_agent(
            code_agent,
            AgentCapabilities::builder("code-agent")
                .add_capability(common::code_review())
                .build(),
        );
        orchestrator.register_agent(
            research_agent,
            AgentCapabilities::builder("research-agent")
                .add_capability(common::web_search())
                .build(),
        );

        let mut graph = TaskGraph::new("test-graph");
        graph.add_task(TaskNode::new("t1", "Review code").with_capability("code_review"));
        graph.add_task(TaskNode::new("t2", "Search docs").with_capability("web_search"));

        // Create a task invocation context for the orchestrator itself
        struct MockOrchestratorAgent;
        #[async_trait]
        impl Agent for MockOrchestratorAgent {
            fn name(&self) -> &str {
                "orchestrator"
            }
            fn description(&self) -> &str {
                "Test orchestrator"
            }
            fn sub_agents(&self) -> &[Arc<dyn Agent>] {
                &[]
            }
            async fn run(&self, _ctx: Arc<dyn InvocationContext>) -> Result<EventStream> {
                let s = stream! {
                    yield Ok(Event::new("orchestrator-test"));
                };
                Ok(Box::pin(s))
            }
        }

        let mock_orch: Arc<dyn Agent> = Arc::new(MockOrchestratorAgent);
        let ctx: Arc<dyn InvocationContext> = Arc::new(TaskInvocationContext::new(
            mock_orch,
            "test",
            "Execute test graph",
        ));

        let trace = orchestrator.execute_graph(&mut graph, ctx).await.unwrap();

        assert_eq!(trace.outcome, TraceOutcome::Success);
    }
}
