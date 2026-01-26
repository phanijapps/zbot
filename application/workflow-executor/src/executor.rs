//! Workflow executor - executes built workflows
//!
//! This module handles the actual execution of workflows, including:
//! - Creating invocation contexts
//! - Managing sessions
//! - Streaming events
//! - Handling execution state

use std::sync::Arc;

use zero_core::{Agent, EventStream, InvocationContext};
use zero_session::{InMemorySession, MutexSession, Session};

use crate::builder::ExecutableWorkflow;
use crate::error::{Result, WorkflowError};

/// Options for workflow execution
#[derive(Debug, Clone)]
pub struct ExecutionOptions {
    /// Maximum iterations for agent loops
    pub max_iterations: Option<usize>,

    /// Session ID (auto-generated if not provided)
    pub session_id: Option<String>,

    /// User ID
    pub user_id: String,

    /// Application name
    pub app_name: String,

    /// Initial state values
    pub initial_state: std::collections::HashMap<String, serde_json::Value>,
}

impl Default for ExecutionOptions {
    fn default() -> Self {
        Self {
            max_iterations: Some(50),
            session_id: None,
            user_id: "default".to_string(),
            app_name: "workflow".to_string(),
            initial_state: std::collections::HashMap::new(),
        }
    }
}

impl ExecutionOptions {
    /// Create new execution options
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum iterations
    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = Some(max);
        self
    }

    /// Set session ID
    pub fn with_session_id(mut self, id: impl Into<String>) -> Self {
        self.session_id = Some(id.into());
        self
    }

    /// Set user ID
    pub fn with_user_id(mut self, id: impl Into<String>) -> Self {
        self.user_id = id.into();
        self
    }

    /// Set application name
    pub fn with_app_name(mut self, name: impl Into<String>) -> Self {
        self.app_name = name.into();
        self
    }

    /// Add initial state value
    pub fn with_state(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.initial_state.insert(key.into(), value);
        self
    }
}

/// Execution result containing the event stream and session
pub struct ExecutionResult {
    /// Stream of events from the workflow
    pub events: EventStream,

    /// Session used for this execution
    pub session: Arc<MutexSession>,

    /// Workflow definition ID
    pub workflow_id: String,

    /// Invocation ID
    pub invocation_id: String,
}

/// Workflow executor
pub struct WorkflowExecutor {
    /// The executable workflow
    workflow: ExecutableWorkflow,
}

impl WorkflowExecutor {
    /// Create a new executor for a workflow
    pub fn new(workflow: ExecutableWorkflow) -> Self {
        Self { workflow }
    }

    /// Execute the workflow with the given user input
    pub async fn execute(
        &self,
        user_input: &str,
        options: ExecutionOptions,
    ) -> Result<ExecutionResult> {
        let invocation_id = uuid::Uuid::new_v4().to_string();
        let session_id = options.session_id
            .unwrap_or_else(|| format!("session_{}", uuid::Uuid::new_v4()));

        tracing::info!(
            "Executing workflow '{}' with invocation_id={}",
            self.workflow.definition.id,
            invocation_id
        );

        // Create session
        let mut session = InMemorySession::new(
            session_id.clone(),
            options.app_name.clone(),
            options.user_id.clone(),
        );

        // Add initial state
        for (key, value) in options.initial_state {
            session.state_mut().set(key, value);
        }

        // Add workflow metadata to state
        session.state_mut().set(
            "app:workflow_id".to_string(),
            serde_json::json!(self.workflow.definition.id),
        );

        // Set agent_id for file tools - use the orchestrator's ID so all subagents
        // write to the same agent data directory
        session.state_mut().set(
            "app:agent_id".to_string(),
            serde_json::json!(self.workflow.definition.id),
        );

        // Add user message to history
        let user_content = zero_core::Content::user(user_input);
        session.add_content(user_content);

        // Wrap session for sharing
        let session = Arc::new(MutexSession::new(session));

        // Create invocation context
        let ctx = self.create_invocation_context(
            &invocation_id,
            session.clone(),
            options.max_iterations,
        )?;

        // Run the root agent
        let events = self.workflow.root_agent.run(ctx).await
            .map_err(|e| WorkflowError::Execution(e.to_string()))?;

        Ok(ExecutionResult {
            events,
            session,
            workflow_id: self.workflow.definition.id.clone(),
            invocation_id,
        })
    }

    /// Continue an existing execution with new user input
    pub async fn continue_execution(
        &self,
        session: Arc<MutexSession>,
        user_input: &str,
        max_iterations: Option<usize>,
    ) -> Result<ExecutionResult> {
        let invocation_id = uuid::Uuid::new_v4().to_string();

        tracing::info!(
            "Continuing workflow '{}' with invocation_id={}",
            self.workflow.definition.id,
            invocation_id
        );

        // Add user message to history using the convenience method
        session.add_content(zero_core::Content::user(user_input));

        // Create invocation context
        let ctx = self.create_invocation_context(
            &invocation_id,
            session.clone(),
            max_iterations,
        )?;

        // Run the root agent
        let events = self.workflow.root_agent.run(ctx).await
            .map_err(|e| WorkflowError::Execution(e.to_string()))?;

        Ok(ExecutionResult {
            events,
            session,
            workflow_id: self.workflow.definition.id.clone(),
            invocation_id,
        })
    }

    /// Create an invocation context
    fn create_invocation_context(
        &self,
        invocation_id: &str,
        session: Arc<MutexSession>,
        max_iterations: Option<usize>,
    ) -> Result<Arc<dyn InvocationContext>> {
        // Create run config
        let run_config = zero_core::RunConfig::new()
            .with_max_iterations(max_iterations.unwrap_or(50));

        // Create context adapter
        let ctx = WorkflowInvocationContext::new(
            invocation_id.to_string(),
            self.workflow.root_agent.clone(),
            session,
            run_config,
        );

        Ok(Arc::new(ctx))
    }

    /// Get the workflow definition
    pub fn definition(&self) -> &crate::WorkflowDefinition {
        &self.workflow.definition
    }

    /// Get the root agent
    pub fn root_agent(&self) -> &Arc<dyn Agent> {
        &self.workflow.root_agent
    }
}

/// Invocation context adapter for workflow execution
struct WorkflowInvocationContext {
    invocation_id: String,
    agent: Arc<dyn Agent>,
    session: Arc<MutexSession>,
    run_config: zero_core::RunConfig,
    actions: std::sync::Mutex<zero_core::EventActions>,
    ended: std::sync::atomic::AtomicBool,
}

impl WorkflowInvocationContext {
    fn new(
        invocation_id: String,
        agent: Arc<dyn Agent>,
        session: Arc<MutexSession>,
        run_config: zero_core::RunConfig,
    ) -> Self {
        Self {
            invocation_id,
            agent,
            session,
            run_config,
            actions: std::sync::Mutex::new(zero_core::EventActions::default()),
            ended: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

impl zero_core::ReadonlyContext for WorkflowInvocationContext {
    fn invocation_id(&self) -> &str {
        &self.invocation_id
    }

    fn agent_name(&self) -> &str {
        self.agent.name()
    }

    fn user_id(&self) -> &str {
        // MutexSession returns placeholder for these - can't hold lock for reference lifetime
        "user"
    }

    fn app_name(&self) -> &str {
        "workflow"
    }

    fn session_id(&self) -> &str {
        "session"
    }

    fn branch(&self) -> &str {
        "main"
    }

    fn user_content(&self) -> &zero_core::Content {
        // Return a static empty content - MutexSession limitation
        static EMPTY: once_cell::sync::Lazy<zero_core::Content> =
            once_cell::sync::Lazy::new(|| zero_core::Content::user(""));
        &EMPTY
    }
}

impl zero_core::CallbackContext for WorkflowInvocationContext {
    fn get_state(&self, key: &str) -> Option<serde_json::Value> {
        if let Ok(locked) = self.session.lock() {
            locked.state().get(key).cloned()
        } else {
            None
        }
    }

    fn set_state(&self, key: String, value: serde_json::Value) {
        if let Ok(mut locked) = self.session.lock() {
            locked.state_mut().set(key, value);
        }
    }
}

impl InvocationContext for WorkflowInvocationContext {
    fn agent(&self) -> Arc<dyn Agent> {
        self.agent.clone()
    }

    fn session(&self) -> Arc<dyn Session> {
        self.session.clone()
    }

    fn run_config(&self) -> &zero_core::RunConfig {
        &self.run_config
    }

    fn actions(&self) -> zero_core::EventActions {
        self.actions.lock().unwrap().clone()
    }

    fn set_actions(&self, actions: zero_core::EventActions) {
        *self.actions.lock().unwrap() = actions;
    }

    fn end_invocation(&self) {
        self.ended.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    fn ended(&self) -> bool {
        self.ended.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn add_content(&self, content: zero_core::Content) {
        self.session.add_content(content);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_options_builder() {
        let options = ExecutionOptions::new()
            .with_max_iterations(100)
            .with_user_id("test_user")
            .with_app_name("test_app")
            .with_state("key", serde_json::json!("value"));

        assert_eq!(options.max_iterations, Some(100));
        assert_eq!(options.user_id, "test_user");
        assert_eq!(options.app_name, "test_app");
        assert!(options.initial_state.contains_key("key"));
    }
}
