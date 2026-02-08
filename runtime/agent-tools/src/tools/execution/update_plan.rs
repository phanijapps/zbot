// ============================================================================
// UPDATE PLAN TOOL
// Lightweight fire-and-forget plan tracking.
// No persistence, no UUIDs — just a status checklist for the model to track progress.
// ============================================================================

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use zero_core::{Result, Tool, ToolContext, ZeroError};

// ============================================================================
// UPDATE PLAN TOOL
// ============================================================================

/// Lightweight plan tool that accepts a checklist of steps with statuses.
/// Returns "Plan updated" immediately — fire-and-forget.
pub struct UpdatePlanTool;

impl UpdatePlanTool {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for UpdatePlanTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for UpdatePlanTool {
    fn name(&self) -> &str {
        "update_plan"
    }

    fn description(&self) -> &str {
        "Track task progress with a lightweight checklist. Each step has a status: pending, in_progress, or completed. Use for complex tasks (5+ steps). Skip for simple tasks."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "explanation": {
                    "type": "string",
                    "description": "Brief explanation of plan changes (optional)"
                },
                "plan": {
                    "type": "array",
                    "description": "Task checklist with step descriptions and statuses",
                    "items": {
                        "type": "object",
                        "properties": {
                            "step": {
                                "type": "string",
                                "description": "Description of the step"
                            },
                            "status": {
                                "type": "string",
                                "enum": ["pending", "in_progress", "completed"],
                                "description": "Current status of this step"
                            }
                        },
                        "required": ["step", "status"]
                    }
                }
            },
            "required": ["plan"]
        }))
    }

    async fn execute(&self, ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
        // Check for error markers from truncated/malformed tool calls
        if let Some(error_type) = args.get("__error__").and_then(|v| v.as_str()) {
            let message = args.get("__message__").and_then(|v| v.as_str()).unwrap_or("Unknown error");
            return Err(ZeroError::Tool(format!("{}: {}", error_type, message)));
        }

        let plan = args
            .get("plan")
            .and_then(|v| v.as_array())
            .ok_or_else(|| ZeroError::Tool("Missing 'plan' array parameter".to_string()))?;

        if plan.is_empty() {
            return Err(ZeroError::Tool("Plan cannot be empty".to_string()));
        }

        // Store the plan in session state for UI rendering
        ctx.set_state("app:plan".to_string(), args.clone());

        tracing::debug!("Plan updated: {} steps", plan.len());

        // Return minimal response — fire-and-forget
        Ok(json!({
            "__plan_update": true,
            "plan": plan,
            "message": "Plan updated"
        }))
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_plan_schema() {
        let tool = UpdatePlanTool::new();
        assert_eq!(tool.name(), "update_plan");
        let schema = tool.parameters_schema().unwrap();
        assert!(schema.get("properties").unwrap().get("plan").is_some());
    }
}
