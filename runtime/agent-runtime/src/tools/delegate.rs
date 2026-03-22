//! # Delegate Tool
//!
//! Tool for delegating tasks to subagents with fire-and-forget pattern.
//!
//! When an agent delegates to a subagent:
//! 1. A new conversation is created for the subagent
//! 2. The task is passed with optional context
//! 3. Parent receives a callback when subagent completes

use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;
use zero_core::{Tool, ToolContext};

/// Tool for delegating tasks to subagents.
///
/// This enables orchestrator agents to spawn specialized agents
/// for specific tasks. The delegation is fire-and-forget by default,
/// with an optional callback on completion.
///
/// # Example
///
/// ```json
/// {
///   "agent_id": "research-agent",
///   "task": "Research the latest trends in AI safety",
///   "context": { "topic": "alignment", "depth": "comprehensive" },
///   "wait_for_result": false
/// }
/// ```
pub struct DelegateTool;

impl DelegateTool {
    /// Create a new delegate tool.
    pub fn new() -> Self {
        Self
    }
}

impl Default for DelegateTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for DelegateTool {
    fn name(&self) -> &str {
        "delegate_to_agent"
    }

    fn description(&self) -> &str {
        "Delegate a task to a specialized subagent. The subagent will work on the task \
         independently and you will receive a callback message when it completes. \
         Use this for complex subtasks that require specialized expertise."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "agent_id": {
                    "type": "string",
                    "description": "ID of the subagent to delegate to"
                },
                "task": {
                    "type": "string",
                    "description": "Task description for the subagent. Be specific about what you need."
                },
                "context": {
                    "type": "object",
                    "description": "Optional task-scoped context to pass to the subagent"
                },
                "wait_for_result": {
                    "type": "boolean",
                    "default": false,
                    "description": "If true, wait for the subagent to complete before continuing. Default is fire-and-forget."
                },
                "max_iterations": {
                    "type": "integer",
                    "description": "Maximum number of iterations the subagent can run. Defaults to 25 if not specified."
                }
            },
            "required": ["agent_id", "task"]
        }))
    }

    async fn execute(&self, ctx: Arc<dyn ToolContext>, args: Value) -> zero_core::Result<Value> {
        let target_agent_id = args
            .get("agent_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| zero_core::ZeroError::Tool("agent_id is required".to_string()))?;

        let task = args
            .get("task")
            .and_then(|v| v.as_str())
            .ok_or_else(|| zero_core::ZeroError::Tool("task is required".to_string()))?;

        // Guard: Limit task size to prevent context explosion
        const MAX_TASK_CHARS: usize = 4000;
        if task.len() > MAX_TASK_CHARS {
            return Err(zero_core::ZeroError::Tool(format!(
                "Task too large ({} chars). Maximum is {} chars. Be concise in your delegation.",
                task.len(),
                MAX_TASK_CHARS
            )));
        }

        let context = args.get("context").cloned();

        // Guard: Limit context size
        if let Some(ctx_val) = &context {
            let ctx_str = serde_json::to_string(ctx_val).unwrap_or_default();
            if ctx_str.len() > MAX_TASK_CHARS {
                return Err(zero_core::ZeroError::Tool(format!(
                    "Context too large ({} chars). Maximum is {} chars. Only pass essential context.",
                    ctx_str.len(),
                    MAX_TASK_CHARS
                )));
            }
        }

        let wait_for_result = args
            .get("wait_for_result")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let max_iterations = args
            .get("max_iterations")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);

        // Get parent context from state
        let parent_agent_id = ctx
            .get_state("agent_id")
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "unknown".to_string());

        // Guard: Prevent self-delegation
        if target_agent_id == parent_agent_id {
            return Err(zero_core::ZeroError::Tool(
                "Cannot delegate to yourself. Use a different agent or handle the task directly.".to_string()
            ));
        }

        // Guard: Only one delegation at a time per session.
        // Atomic claim — first concurrent delegate_to_agent wins, rest return "queued".
        if !ctx.try_claim("app:delegation_active") {
            return Ok(json!({
                "status": "queued",
                "message": "You already have an active delegation. Wait for it to complete — the system will resume you automatically. Do NOT delegate another step until you see the result."
            }));
        }

        let parent_conversation_id = ctx
            .get_state("conversation_id")
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "unknown".to_string());

        // Generate child conversation ID
        let child_conversation_id = format!(
            "{}-sub-{}",
            parent_conversation_id,
            uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("0")
        );

        // Set delegation action for the executor to pick up
        let mut actions = ctx.actions();
        actions.delegate = Some(zero_core::event::DelegateAction {
            agent_id: target_agent_id.to_string(),
            task: task.to_string(),
            context,
            wait_for_result,
            max_iterations,
        });
        ctx.set_actions(actions);

        Ok(json!({
            "convid": child_conversation_id,
            "status": "delegated",
            "message": format!("Task delegated to {}. You will receive a callback with results when complete.", target_agent_id)
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delegate_tool_schema() {
        let tool = DelegateTool::new();
        assert_eq!(tool.name(), "delegate_to_agent");

        let schema = tool.parameters_schema().unwrap();
        let properties = schema.get("properties").unwrap();
        assert!(properties.get("agent_id").is_some());
        assert!(properties.get("task").is_some());
        assert!(properties.get("context").is_some());
        assert!(properties.get("wait_for_result").is_some());

        let required = schema.get("required").unwrap().as_array().unwrap();
        assert!(required.iter().any(|v| v == "agent_id"));
        assert!(required.iter().any(|v| v == "task"));
    }
}
