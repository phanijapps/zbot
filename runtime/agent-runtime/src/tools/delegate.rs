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
    #[must_use]
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
    fn name(&self) -> &'static str {
        "delegate_to_agent"
    }

    fn description(&self) -> &'static str {
        "Delegate a task to a specialized subagent. The subagent will work on the task \
         independently. Returns an execution_id you can pass to wait_agent (block until result), \
         steer_agent (send mid-run instructions), or kill_agent (stop it). \
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
                },
                "parallel": {
                    "type": "boolean",
                    "default": false,
                    "description": "Set true for independent tasks that can run simultaneously. Use false (default) when tasks must run in order or share files."
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
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);

        let max_iterations = args
            .get("max_iterations")
            .and_then(serde_json::Value::as_u64)
            .map(|v| v as u32);

        let output_schema = args.get("output_schema").cloned();

        let skills: Vec<String> = args
            .get("skills")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(std::string::ToString::to_string))
                    .collect()
            })
            .unwrap_or_default();

        let parallel = args
            .get("parallel")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);

        // Get parent context from state
        let parent_agent_id = ctx
            .get_state("agent_id")
            .and_then(|v| v.as_str().map(std::string::ToString::to_string))
            .unwrap_or_else(|| "unknown".to_string());

        // Guard: Prevent self-delegation
        if target_agent_id == parent_agent_id {
            return Err(zero_core::ZeroError::Tool(
                "Cannot delegate to yourself. Use a different agent or handle the task directly."
                    .to_string(),
            ));
        }

        // Guard: Only one sequential delegation at a time per session.
        // Parallel delegations bypass this — the global semaphore (max_parallel_agents in
        // settings.json) controls concurrency; excess parallel requests queue in the dispatcher
        // until a permit is available.
        if !parallel && !ctx.try_claim("app:delegation_active") {
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
            .and_then(|v| v.as_str().map(std::string::ToString::to_string))
            .unwrap_or_else(|| "unknown".to_string());

        // Generate child conversation ID
        let child_conversation_id = format!(
            "{}-sub-{}",
            parent_conversation_id,
            uuid::Uuid::new_v4()
                .to_string()
                .split('-')
                .next()
                .unwrap_or("0")
        );

        // Generate stable execution ID so the parent can steer this child via steer_agent
        let child_execution_id = format!("exec-{}", uuid::Uuid::new_v4());

        // Enrich task with platform hint so subagents use correct shell syntax
        let platform_hint = match std::env::consts::OS {
            "windows" => "\n\n[PLATFORM: Windows / PowerShell. Do NOT use bash syntax (head, &&, cat, heredocs). Use Get-Content, ';', python.]",
            "macos" => "\n\n[PLATFORM: macOS / zsh.]",
            _ => "\n\n[PLATFORM: Linux / bash.]",
        };
        let enriched_task = format!("{task}{platform_hint}");

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
            parallel,
            child_execution_id: Some(child_execution_id.clone()),
        });
        ctx.set_actions(actions);

        Ok(json!({
            "execution_id": child_execution_id,
            "convid": child_conversation_id,
            "status": "delegated",
            "agent_id": target_agent_id,
            "task": task,
            "parallel": parallel,
            "message": format!("Task delegated to {}. Use execution_id with wait_agent to block until it completes and get its result, steer_agent to send mid-run instructions, or kill_agent to stop it.", target_agent_id)
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::context::ToolContext as ConcreteCtx;

    fn ctx_for(parent_agent: &str) -> Arc<dyn ToolContext> {
        let cc = ConcreteCtx::full_with_state(
            parent_agent.to_string(),
            Some("conv-test".to_string()),
            vec![],
            std::collections::HashMap::new(),
        );
        Arc::new(cc)
    }

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

    // ------------------------------------------------------------------------
    // Argument-validation guards
    // ------------------------------------------------------------------------

    #[tokio::test]
    async fn missing_agent_id_returns_tool_error() {
        let tool = DelegateTool::new();
        let ctx = ctx_for("root");
        let res = tool.execute(ctx, json!({ "task": "do thing" })).await;
        let err = res.expect_err("must error");
        assert!(matches!(err, zero_core::ZeroError::Tool(_)));
        assert!(format!("{err}").contains("agent_id"));
    }

    #[tokio::test]
    async fn missing_task_returns_tool_error() {
        let tool = DelegateTool::new();
        let ctx = ctx_for("root");
        let res = tool
            .execute(ctx, json!({ "agent_id": "writer-agent" }))
            .await;
        let err = res.expect_err("must error");
        assert!(format!("{err}").contains("task"));
    }

    #[tokio::test]
    async fn oversized_task_is_rejected() {
        let tool = DelegateTool::new();
        let ctx = ctx_for("root");
        let big = "x".repeat(4001);
        let res = tool
            .execute(ctx, json!({ "agent_id": "writer-agent", "task": big }))
            .await;
        let err = res.expect_err("must error on >4000 chars");
        let msg = format!("{err}");
        assert!(msg.contains("Task too large"));
        assert!(msg.contains("4000"));
    }

    #[tokio::test]
    async fn oversized_context_is_rejected() {
        let tool = DelegateTool::new();
        let ctx = ctx_for("root");
        // Build a context object whose serialized form > 4000 chars.
        let big_string = "y".repeat(4100);
        let res = tool
            .execute(
                ctx,
                json!({
                    "agent_id": "writer-agent",
                    "task": "tiny task",
                    "context": { "blob": big_string }
                }),
            )
            .await;
        let err = res.expect_err("must error on oversized context");
        assert!(format!("{err}").contains("Context too large"));
    }

    #[tokio::test]
    async fn self_delegation_is_rejected() {
        let tool = DelegateTool::new();
        let ctx = ctx_for("root");
        let res = tool
            .execute(
                ctx,
                json!({ "agent_id": "root", "task": "delegate to self" }),
            )
            .await;
        let err = res.expect_err("self-delegation must fail");
        assert!(format!("{err}").contains("Cannot delegate to yourself"));
    }

    // ------------------------------------------------------------------------
    // try_claim — only one delegation per turn
    // ------------------------------------------------------------------------

    #[tokio::test]
    async fn second_concurrent_delegate_returns_queued() {
        let tool = DelegateTool::new();
        let ctx = ctx_for("root");

        // First call must succeed (claim acquired).
        let first = tool
            .execute(
                ctx.clone(),
                json!({ "agent_id": "writer-agent", "task": "first" }),
            )
            .await
            .expect("first delegate ok");
        assert_eq!(
            first.get("status").and_then(|v| v.as_str()),
            Some("delegated")
        );

        // Second call within the same context (= same loop iteration) must
        // be soft-rejected with status="queued" — NOT a hard error.
        let second = tool
            .execute(ctx, json!({ "agent_id": "writer-agent", "task": "second" }))
            .await
            .expect("second delegate returns Ok with queued status");
        assert_eq!(
            second.get("status").and_then(|v| v.as_str()),
            Some("queued"),
            "second delegate must be queued: {second}"
        );
    }

    #[tokio::test]
    async fn parallel_delegates_are_not_blocked_by_claim() {
        let tool = DelegateTool::new();
        let ctx = ctx_for("root");

        // First parallel call succeeds and sets the delegate action.
        let first = tool
            .execute(
                ctx.clone(),
                json!({ "agent_id": "writer-agent", "task": "first", "parallel": true }),
            )
            .await
            .expect("first parallel delegate ok");
        assert_eq!(
            first.get("status").and_then(|v| v.as_str()),
            Some("delegated"),
            "first parallel must succeed: {first}"
        );

        // Second parallel call within the same session context must ALSO succeed —
        // the semaphore (max_parallel_agents setting) controls concurrency; the
        // try_claim guard must not block parallel calls.
        let ctx2 = ctx_for("root"); // fresh context simulating next tool call
        let second = tool
            .execute(
                ctx2.clone(),
                json!({ "agent_id": "analyst-agent", "task": "second", "parallel": true }),
            )
            .await
            .expect("second parallel delegate ok");
        assert_eq!(
            second.get("status").and_then(|v| v.as_str()),
            Some("delegated"),
            "second parallel must not be blocked: {second}"
        );
    }

    // ------------------------------------------------------------------------
    // Happy path: success populates the DelegateAction the executor reads
    // ------------------------------------------------------------------------

    #[tokio::test]
    async fn delegate_returns_execution_id() {
        let tool = DelegateTool::new();
        let ctx = ctx_for("root");

        let result = tool
            .execute(
                ctx.clone(),
                json!({ "agent_id": "writer-agent", "task": "do work" }),
            )
            .await
            .expect("must succeed");

        let execution_id = result
            .get("execution_id")
            .and_then(|v| v.as_str())
            .expect("execution_id must be in result");
        assert!(
            execution_id.starts_with("exec-"),
            "must have exec- prefix: {execution_id}"
        );

        let action = ctx.actions().delegate.expect("delegate action must be set");
        assert_eq!(action.child_execution_id.as_deref(), Some(execution_id));
    }

    #[tokio::test]
    async fn successful_delegate_sets_actions_and_returns_convid() {
        let tool = DelegateTool::new();
        let ctx = ctx_for("root");

        let result = tool
            .execute(
                ctx.clone(),
                json!({
                    "agent_id": "writer-agent",
                    "task": "compose summary",
                    "skills": ["html-report"],
                    "parallel": false,
                }),
            )
            .await
            .expect("delegate must succeed");

        // Tool result shape
        assert_eq!(
            result.get("status").and_then(|v| v.as_str()),
            Some("delegated")
        );
        assert!(
            result.get("convid").and_then(|v| v.as_str()).is_some(),
            "convid must be present: {result}"
        );

        // The executor reads ctx.actions().delegate — must be populated.
        let action = ctx.actions().delegate.expect("delegate action set");
        assert_eq!(action.agent_id, "writer-agent");
        assert!(action.task.starts_with("compose summary"));
        assert!(
            action.task.contains("[PLATFORM:"),
            "task must be enriched with platform hint"
        );
        assert_eq!(action.skills, vec!["html-report".to_string()]);
        assert!(!action.parallel);
        assert!(!action.wait_for_result);
    }
}
