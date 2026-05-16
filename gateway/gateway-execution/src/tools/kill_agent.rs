use crate::agent_pool::AgentResultBus;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;
use zero_core::{Result, Tool, ToolContext, ZeroError};

pub struct KillAgentTool {
    bus: Arc<AgentResultBus>,
}

impl KillAgentTool {
    pub fn new(bus: Arc<AgentResultBus>) -> Self {
        Self { bus }
    }
}

#[async_trait]
impl Tool for KillAgentTool {
    fn name(&self) -> &'static str {
        "kill_agent"
    }

    fn description(&self) -> &'static str {
        "Stop a running delegated subagent immediately. \
         Pass the execution_id returned by delegate_to_agent. \
         Unblocks any pending wait_agent on this execution with a crashed error. \
         Returns status: stopped if the agent was running, not_running otherwise."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "execution_id": {
                    "type": "string",
                    "description": "The execution_id returned by delegate_to_agent"
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

        if self.bus.kill(execution_id) {
            tracing::info!(execution_id = %execution_id, "kill_agent: stopped running subagent");
            Ok(json!({ "status": "stopped", "execution_id": execution_id }))
        } else {
            Ok(json!({ "status": "not_running", "execution_id": execution_id }))
        }
    }
}
