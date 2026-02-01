//! # Runtime Service
//! # Runtime Service
//!
//! Service for managing agent execution runtime.
//!
//! This service coordinates agent execution through the ExecutionRunner
//! and provides a high-level API for invoking agents.

use api_logs::LogService;
use execution_state::StateService;
use crate::database::{ConversationRepository, DatabaseManager};
use crate::events::{EventBus, GatewayEvent};
use crate::execution::{ExecutionConfig, ExecutionHandle, ExecutionRunner};
use crate::hooks::HookContext;
use crate::services::{AgentService, McpService, ProviderService, SkillService};
use std::path::PathBuf;
use std::sync::Arc;

/// Execution state for a conversation.
#[derive(Debug, Clone)]
pub struct ExecutionState {
    pub agent_id: String,
    pub conversation_id: String,
    pub is_running: bool,
    pub iteration: u32,
    pub max_iterations: u32,
    pub stop_requested: bool,
}

/// Runtime service for managing agent execution.
pub struct RuntimeService {
    /// Event bus for broadcasting events.
    event_bus: Arc<EventBus>,

    /// Execution runner (optional - set when config_dir is known)
    runner: Option<Arc<ExecutionRunner>>,

    /// Configuration directory
    config_dir: Option<PathBuf>,
}

impl RuntimeService {
    /// Create a new runtime service.
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            event_bus,
            runner: None,
            config_dir: None,
        }
    }

    /// Create a runtime service with an execution runner.
    pub fn with_runner(
        event_bus: Arc<EventBus>,
        agent_service: Arc<AgentService>,
        provider_service: Arc<ProviderService>,
        config_dir: PathBuf,
        conversation_repo: Arc<ConversationRepository>,
        mcp_service: Arc<McpService>,
        skill_service: Arc<SkillService>,
        log_service: Arc<LogService<DatabaseManager>>,
        state_service: Arc<StateService<DatabaseManager>>,
    ) -> Self {
        let runner = Arc::new(ExecutionRunner::new(
            event_bus.clone(),
            agent_service,
            provider_service,
            config_dir.clone(),
            conversation_repo,
            mcp_service,
            skill_service,
            log_service,
            state_service,
        ));
        Self {
            event_bus,
            runner: Some(runner),
            config_dir: Some(config_dir),
        }
    }

    /// Get the event bus.
    pub fn event_bus(&self) -> Arc<EventBus> {
        self.event_bus.clone()
    }

    /// Get the execution runner.
    pub fn runner(&self) -> Option<&Arc<ExecutionRunner>> {
        self.runner.as_ref()
    }

    /// Invoke an agent with a message.
    ///
    /// Returns (ExecutionHandle, session_id).
    /// - If session_id is provided, continues that session
    /// - If session_id is None, creates a new session
    pub async fn invoke(
        &self,
        agent_id: &str,
        conversation_id: &str,
        message: &str,
    ) -> Result<(ExecutionHandle, String), String> {
        self.invoke_with_session(agent_id, conversation_id, message, None).await
    }

    /// Invoke an agent with a message and explicit session ID.
    ///
    /// Returns (ExecutionHandle, session_id).
    pub async fn invoke_with_session(
        &self,
        agent_id: &str,
        conversation_id: &str,
        message: &str,
        session_id: Option<String>,
    ) -> Result<(ExecutionHandle, String), String> {
        let runner = self.runner.as_ref().ok_or_else(|| {
            "Runtime not initialized with executor. Call with_runner() first.".to_string()
        })?;

        let config_dir = self.config_dir.clone().ok_or_else(|| {
            "Config directory not set".to_string()
        })?;

        let mut config = ExecutionConfig::new(
            agent_id.to_string(),
            conversation_id.to_string(),
            config_dir,
        );

        if let Some(sid) = session_id {
            config = config.with_session_id(sid);
        }

        runner.invoke(config, message.to_string()).await
    }

    /// Invoke an agent with a message and hook context.
    ///
    /// The hook context is passed to tools so they can route responses
    /// back to the originating channel (WebSocket, webhook, etc).
    pub async fn invoke_with_hook(
        &self,
        agent_id: &str,
        conversation_id: &str,
        message: &str,
        hook_context: HookContext,
        session_id: Option<String>,
    ) -> Result<(ExecutionHandle, String), String> {
        let runner = self.runner.as_ref().ok_or_else(|| {
            "Runtime not initialized with executor. Call with_runner() first.".to_string()
        })?;

        let config_dir = self.config_dir.clone().ok_or_else(|| {
            "Config directory not set".to_string()
        })?;

        let mut config = ExecutionConfig::new(
            agent_id.to_string(),
            conversation_id.to_string(),
            config_dir,
        ).with_hook_context(hook_context);

        if let Some(sid) = session_id {
            config = config.with_session_id(sid);
        }

        runner.invoke(config, message.to_string()).await
    }

    /// Invoke with a placeholder response (for testing without LLM).
    pub async fn invoke_placeholder(
        &self,
        agent_id: &str,
        conversation_id: &str,
        message: &str,
    ) -> Result<(), String> {
        // Emit start event
        let placeholder_session_id = format!("placeholder-{}", uuid::Uuid::new_v4());
        self.event_bus
            .publish(GatewayEvent::AgentStarted {
                agent_id: agent_id.to_string(),
                conversation_id: conversation_id.to_string(),
                session_id: placeholder_session_id,
            })
            .await;

        // Emit a placeholder completion event after a short delay
        let event_bus = self.event_bus.clone();
        let agent_id = agent_id.to_string();
        let conversation_id = conversation_id.to_string();
        let message = message.to_string();

        tokio::spawn(async move {
            // Simulate processing
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            // Emit completion
            event_bus
                .publish(GatewayEvent::AgentCompleted {
                    agent_id: agent_id.clone(),
                    conversation_id: conversation_id.clone(),
                    result: Some(format!(
                        "Gateway placeholder response. Set OPENAI_API_KEY for real execution. Message: {}",
                        message.chars().take(50).collect::<String>()
                    )),
                })
                .await;
        });

        Ok(())
    }

    /// Stop an agent execution.
    pub async fn stop(&self, conversation_id: &str) -> Result<(), String> {
        if let Some(runner) = &self.runner {
            runner.stop(conversation_id).await
        } else {
            Err("Runtime not initialized with executor".to_string())
        }
    }

    /// Continue an agent execution after max iterations.
    pub async fn continue_execution(
        &self,
        conversation_id: &str,
        additional_iterations: u32,
    ) -> Result<(), String> {
        if let Some(runner) = &self.runner {
            runner.continue_execution(conversation_id, additional_iterations).await
        } else {
            Err("Runtime not initialized with executor".to_string())
        }
    }

    /// Pause an agent execution.
    pub async fn pause(&self, session_id: &str) -> Result<(), String> {
        if let Some(runner) = &self.runner {
            runner.pause(session_id).await
        } else {
            Err("Runtime not initialized with executor".to_string())
        }
    }

    /// Resume a paused agent execution.
    pub async fn resume(&self, session_id: &str) -> Result<(), String> {
        if let Some(runner) = &self.runner {
            runner.resume(session_id).await
        } else {
            Err("Runtime not initialized with executor".to_string())
        }
    }

    /// Cancel an agent execution.
    pub async fn cancel(&self, session_id: &str) -> Result<(), String> {
        if let Some(runner) = &self.runner {
            runner.cancel(session_id).await
        } else {
            Err("Runtime not initialized with executor".to_string())
        }
    }

    /// Get execution handle for a conversation.
    pub async fn get_handle(&self, conversation_id: &str) -> Option<ExecutionHandle> {
        if let Some(runner) = &self.runner {
            runner.get_handle(conversation_id).await
        } else {
            None
        }
    }

    /// Check if an agent is currently executing.
    pub async fn is_running(&self, conversation_id: &str) -> bool {
        if let Some(handle) = self.get_handle(conversation_id).await {
            !handle.is_stop_requested()
        } else {
            false
        }
    }
}

/// Create a shared runtime service.
pub fn shared_runtime_service(event_bus: Arc<EventBus>) -> Arc<RuntimeService> {
    Arc::new(RuntimeService::new(event_bus))
}

/// Create a shared runtime service with execution runner.
pub fn shared_runtime_service_with_runner(
    event_bus: Arc<EventBus>,
    agent_service: Arc<AgentService>,
    provider_service: Arc<ProviderService>,
    config_dir: PathBuf,
    conversation_repo: Arc<ConversationRepository>,
    mcp_service: Arc<McpService>,
    skill_service: Arc<SkillService>,
    log_service: Arc<LogService<DatabaseManager>>,
    state_service: Arc<StateService<DatabaseManager>>,
) -> Arc<RuntimeService> {
    Arc::new(RuntimeService::with_runner(event_bus, agent_service, provider_service, config_dir, conversation_repo, mcp_service, skill_service, log_service, state_service))
}
