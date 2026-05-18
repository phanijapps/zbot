//! # Run Procedure Tool
//!
//! Loads a learned procedure by name and executes its steps as a
//! guided sub-loop, dispatching each step against the live tool
//! registry. Strict-mode: each step's `action` must resolve to a
//! registered tool, or the whole procedure aborts with an error
//! (failure_count is bumped).
//!
//! Task 7 lands the surface (struct, schema, validation, 404 path).
//! Task 8 lands the dispatch loop. Task 9 lands argument interpolation.

use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;
use zero_core::{Result, Tool, ToolContext, ZeroError};
use zero_stores_traits::ProcedureStore;

use crate::tools::registry::ToolRegistry;

pub struct RunProcedureTool {
    #[allow(dead_code)] // Task 8 wires this into the dispatch loop.
    registry: Arc<ToolRegistry>,
    procedure_store: Arc<dyn ProcedureStore>,
}

impl RunProcedureTool {
    #[must_use]
    pub fn new(registry: Arc<ToolRegistry>, procedure_store: Arc<dyn ProcedureStore>) -> Self {
        Self {
            registry,
            procedure_store,
        }
    }
}

#[async_trait]
impl Tool for RunProcedureTool {
    fn name(&self) -> &'static str {
        "run_procedure"
    }

    fn description(&self) -> &'static str {
        "Execute a learned procedure by name. Loads the procedure's steps and \
         dispatches each step against the tool registry. Use this when a procedure \
         was recommended in your context. Returns the aggregated result of the \
         final step plus a summary of intermediate steps. On any step failure, \
         the procedure aborts and the failure is recorded."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Procedure name (snake_case)"
                },
                "args": {
                    "type": "object",
                    "description": "Top-level parameters the procedure declares. \
                                    Use the names listed in the procedure's `parameters` field."
                }
            },
            "required": ["name"]
        }))
    }

    async fn execute(&self, ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ZeroError::Tool("name is required".into()))?;

        let agent_id = ctx
            .get_state("agent_id")
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_else(|| "root".into());

        let proc = self
            .procedure_store
            .get_procedure_by_name(&agent_id, name)
            .await
            .map_err(|e| ZeroError::Tool(format!("procedure lookup failed: {e}")))?
            .ok_or_else(|| ZeroError::Tool(format!("procedure '{name}' not found")))?;

        // Task 8 lands the dispatch loop here. For now, fail loud so a
        // skeleton-only deployment can't claim success.
        Err(ZeroError::Tool(format!(
            "run_procedure: dispatch loop not yet implemented (loaded '{}', steps={}B)",
            proc.name,
            proc.steps.len()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::context::ToolContext as ConcreteCtx;

    struct NoOpProcedureStore;
    #[async_trait]
    impl ProcedureStore for NoOpProcedureStore {}

    fn test_ctx() -> Arc<dyn ToolContext> {
        Arc::new(ConcreteCtx::full_with_state(
            "root".into(),
            Some("c1".into()),
            vec![],
            Default::default(),
        ))
    }

    #[test]
    fn run_procedure_tool_schema() {
        let registry = Arc::new(ToolRegistry::new());
        let store = Arc::new(NoOpProcedureStore);
        let tool = RunProcedureTool::new(registry, store);
        assert_eq!(tool.name(), "run_procedure");
        let schema = tool.parameters_schema().unwrap();
        assert_eq!(schema["properties"]["name"]["type"], "string");
        assert_eq!(schema["required"][0], "name");
    }

    #[tokio::test]
    async fn run_procedure_errors_when_name_missing() {
        let registry = Arc::new(ToolRegistry::new());
        let store = Arc::new(NoOpProcedureStore);
        let tool = RunProcedureTool::new(registry, store);
        let res = tool.execute(test_ctx(), json!({})).await;
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("name is required"));
    }

    #[tokio::test]
    async fn run_procedure_errors_when_procedure_missing() {
        let registry = Arc::new(ToolRegistry::new());
        let store = Arc::new(NoOpProcedureStore);
        let tool = RunProcedureTool::new(registry, store);
        let res = tool.execute(test_ctx(), json!({"name": "nope"})).await;
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("not found"));
    }
}
