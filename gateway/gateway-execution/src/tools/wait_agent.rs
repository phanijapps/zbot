use crate::agent_pool::{AgentResultBus, AgentWaitError};
use async_trait::async_trait;
use execution_state::{ExecutionStatus, StateService};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::time::Duration;
use zero_core::{Result, Tool, ToolContext, ZeroError};
use zero_stores_sqlite::{ConversationRepository, DatabaseManager};

pub struct WaitAgentTool {
    bus: Arc<AgentResultBus>,
    state_service: Arc<StateService<DatabaseManager>>,
    conversation_repo: Arc<ConversationRepository>,
}

impl WaitAgentTool {
    pub fn new(
        bus: Arc<AgentResultBus>,
        state_service: Arc<StateService<DatabaseManager>>,
        conversation_repo: Arc<ConversationRepository>,
    ) -> Self {
        Self {
            bus,
            state_service,
            conversation_repo,
        }
    }
}

#[async_trait]
impl Tool for WaitAgentTool {
    fn name(&self) -> &'static str {
        "wait_agent"
    }

    fn description(&self) -> &'static str {
        "Block until a delegated subagent completes and return its result. \
         Pass the execution_id returned by delegate_to_agent. \
         Returns the agent's respond() text. Times out after timeout_secs (default 300). \
         Use this to coordinate sequential steps: delegate, wait, use result, delegate next."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "execution_id": {
                    "type": "string",
                    "description": "The execution_id returned by delegate_to_agent"
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": "Seconds to wait before returning a timeout error (default: 300)"
                }
            },
            "required": ["execution_id"]
        }))
    }

    async fn execute(&self, _ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
        let execution_id = args
            .get("execution_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ZeroError::Tool("execution_id is required".to_string()))?;

        let timeout_secs = args
            .get("timeout_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(300);

        // Fast path: agent already completed before wait_agent was called.
        if let Ok(Some(exec)) = self.state_service.get_execution(execution_id) {
            match exec.status {
                ExecutionStatus::Completed => {
                    let response = exec
                        .child_session_id
                        .as_deref()
                        .and_then(|sid| {
                            self.conversation_repo
                                .get_session_conversation(sid, 10)
                                .ok()
                        })
                        .and_then(|msgs| {
                            msgs.into_iter()
                                .rev()
                                .find(|m| m.role == "assistant")
                                .map(|m| m.content)
                        })
                        .unwrap_or_default();
                    return Ok(json!({
                        "execution_id": execution_id,
                        "result": response
                    }));
                }
                ExecutionStatus::Crashed | ExecutionStatus::Cancelled => {
                    return Ok(json!({
                        "error": "crashed",
                        "execution_id": execution_id,
                        "details": exec.error.unwrap_or_default()
                    }));
                }
                _ => {}
            }
        }

        // Register waiter and block until the execution resolves or timeout.
        let rx = self.bus.register_waiter(execution_id);

        match tokio::time::timeout(Duration::from_secs(timeout_secs), rx).await {
            Ok(Ok(Ok(result))) => Ok(json!({
                "execution_id": result.execution_id,
                "agent_id": result.agent_id,
                "result": result.response
            })),
            Ok(Ok(Err(AgentWaitError::Crashed { error }))) => Ok(json!({
                "error": "crashed",
                "execution_id": execution_id,
                "details": error
            })),
            Ok(Ok(Err(AgentWaitError::NotFound(id)))) => Ok(json!({
                "error": "not_found",
                "execution_id": id
            })),
            Ok(Ok(Err(AgentWaitError::Timeout))) => Ok(json!({
                "error": "timeout",
                "execution_id": execution_id
            })),
            Ok(Err(_)) => Ok(json!({
                "error": "crashed",
                "execution_id": execution_id,
                "details": "agent result channel closed unexpectedly"
            })),
            Err(_elapsed) => Ok(json!({
                "error": "timeout",
                "execution_id": execution_id
            })),
        }
    }
}
