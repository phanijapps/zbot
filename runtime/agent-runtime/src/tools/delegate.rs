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
                    "description": "Maximum iterations for the subagent. Default is 1000. Do NOT set this unless you have a specific reason — the system auto-detects stuck agents."
                },
                "output_schema": {
                    "type": "object",
                    "description": "Optional JSON Schema the child agent's response must follow. When provided, the child is instructed to respond with ONLY a JSON object matching this schema."
                },
                "skills": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Skills to pre-load for the subagent. These are loaded into the agent's context automatically."
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

        let output_schema = args.get("output_schema").cloned();

        let skills: Vec<String> = args
            .get("skills")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

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

        // Guard: Block ad-hoc delegations when placeholder specs exist.
        // Only planning subagent delegations are allowed (task contains "planning" or "Spec writer").
        let has_placeholders = ctx
            .get_state("app:has_placeholder_specs")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let is_delegated = ctx
            .get_state("app:is_delegated")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if has_placeholders && !is_delegated {
            let task_lower = task.to_lowercase();
            let is_planning_task = task_lower.contains("planning subagent")
                || task_lower.contains("spec writer")
                || task_lower.contains("plan, not execute")
                || task_lower.contains("fill") && task_lower.contains("spec");
            if !is_planning_task {
                return Ok(json!({
                    "status": "redirect",
                    "message": "Placeholder specs exist in the ward. Delegate to a planning subagent (code-agent) to fill them first. Do not delegate ad-hoc tasks."
                }));
            }
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

        // Enrich task with platform hint so subagents use correct shell syntax
        let platform_hint = match std::env::consts::OS {
            "windows" => "\n\n[PLATFORM: Windows / PowerShell. Do NOT use bash syntax (head, &&, cat, heredocs). Use Get-Content, ';', python.]",
            "macos" => "\n\n[PLATFORM: macOS / zsh.]",
            _ => "\n\n[PLATFORM: Linux / bash.]",
        };
        let enriched_task = format!("{}{}", task, platform_hint);

        // Set delegation action for the executor to pick up
        let mut actions = ctx.actions();
        actions.delegate = Some(zero_core::event::DelegateAction {
            agent_id: target_agent_id.to_string(),
            task: enriched_task,
            context,
            wait_for_result,
            max_iterations,
            output_schema,
            skills,
            complexity: None,
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
