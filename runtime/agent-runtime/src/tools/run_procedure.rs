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
use zero_stores_traits::{PatternStep, ProcedureStore};

use crate::tools::registry::ToolRegistry;

pub struct RunProcedureTool {
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

        let steps: Vec<PatternStep> = serde_json::from_str(&proc.steps)
            .map_err(|e| ZeroError::Tool(format!("procedure steps unparseable: {e}")))?;

        let mut step_results: Vec<Value> = Vec::with_capacity(steps.len());
        let started = std::time::Instant::now();

        for (i, step) in steps.iter().enumerate() {
            let inner_tool = match self.registry.find(&step.action) {
                Some(t) => t,
                None => {
                    // Strict validation: bump failure_count and return an error
                    if let Err(ee) = self.procedure_store.increment_failure(&proc.id).await {
                        tracing::warn!(error = %ee, "increment_failure failed");
                    }
                    return Err(ZeroError::Tool(format!(
                        "run_procedure '{}' step {} action '{}' is not a registered tool",
                        proc.name, i, step.action
                    )));
                }
            };

            // Task 9 will interpolate {step_N.field} here. For now, args pass through.
            let step_args = Value::Object(step.args.clone());

            let result = match inner_tool.execute(ctx.clone(), step_args).await {
                Ok(v) => v,
                Err(e) => {
                    if let Err(ee) = self.procedure_store.increment_failure(&proc.id).await {
                        tracing::warn!(error = %ee, "increment_failure failed");
                    }
                    return Err(ZeroError::Tool(format!(
                        "run_procedure '{}' step {} ({}) failed: {}",
                        proc.name, i, step.action, e
                    )));
                }
            };
            step_results.push(result);
        }

        let duration_ms = started.elapsed().as_millis() as i64;
        if let Err(e) = self
            .procedure_store
            .increment_success(&proc.id, Some(duration_ms), None)
            .await
        {
            tracing::warn!(error = %e, "increment_success failed");
        }

        Ok(json!({
            "status": "ok",
            "procedure": proc.name,
            "steps_run": step_results.len(),
            "duration_ms": duration_ms,
            "final": step_results.last().cloned().unwrap_or(Value::Null),
            "all_steps": step_results
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::context::ToolContext as ConcreteCtx;
    use tokio::sync::Mutex as TokioMutex;
    use zero_stores_traits::Procedure;

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

    fn test_procedure(id: &str, name: &str, steps_json: &str) -> Procedure {
        Procedure {
            id: id.into(),
            agent_id: "root".into(),
            ward_id: None,
            name: name.into(),
            description: "test".into(),
            trigger_pattern: None,
            steps: steps_json.into(),
            parameters: None,
            success_count: 1,
            failure_count: 0,
            avg_duration_ms: None,
            avg_token_cost: None,
            last_used: None,
            embedding: None,
            created_at: "".into(),
            updated_at: "".into(),
        }
    }

    struct InMemoryProcedureStore {
        proc: TokioMutex<Procedure>,
    }

    impl InMemoryProcedureStore {
        fn with_one(p: Procedure) -> Self {
            Self {
                proc: TokioMutex::new(p),
            }
        }

        async fn success_count_for(&self, id: &str) -> i32 {
            let p = self.proc.lock().await;
            if p.id == id {
                p.success_count
            } else {
                -1
            }
        }

        async fn failure_was_incremented(&self, id: &str) -> bool {
            let p = self.proc.lock().await;
            p.id == id && p.failure_count > 0
        }
    }

    #[async_trait]
    impl ProcedureStore for InMemoryProcedureStore {
        async fn get_procedure_by_name(
            &self,
            _agent_id: &str,
            name: &str,
        ) -> std::result::Result<Option<Procedure>, String> {
            let p = self.proc.lock().await;
            if p.name == name {
                Ok(Some(p.clone()))
            } else {
                Ok(None)
            }
        }
        async fn increment_success(
            &self,
            id: &str,
            _duration_ms: Option<i64>,
            _token_cost: Option<i64>,
        ) -> std::result::Result<(), String> {
            let mut p = self.proc.lock().await;
            if p.id == id {
                p.success_count += 1;
            }
            Ok(())
        }
        async fn increment_failure(&self, id: &str) -> std::result::Result<(), String> {
            let mut p = self.proc.lock().await;
            if p.id == id {
                p.failure_count += 1;
            }
            Ok(())
        }
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

    #[tokio::test]
    async fn dispatch_loop_runs_each_step_in_order() {
        // Two LoggingTools record the call order
        let log: Arc<TokioMutex<Vec<String>>> = Arc::new(TokioMutex::new(Vec::new()));

        struct LoggingTool {
            name: &'static str,
            log: Arc<TokioMutex<Vec<String>>>,
        }
        #[async_trait]
        impl Tool for LoggingTool {
            fn name(&self) -> &'static str {
                self.name
            }
            fn description(&self) -> &'static str {
                "log"
            }
            async fn execute(&self, _ctx: Arc<dyn ToolContext>, _args: Value) -> Result<Value> {
                self.log.lock().await.push(self.name.into());
                Ok(json!({"ok": true}))
            }
        }

        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(LoggingTool {
            name: "step_a",
            log: log.clone(),
        }));
        registry.register(Arc::new(LoggingTool {
            name: "step_b",
            log: log.clone(),
        }));
        let registry = Arc::new(registry);

        let steps_json = serde_json::to_string(&vec![
            json!({"action": "step_a", "args": {}, "binds": []}),
            json!({"action": "step_b", "args": {}, "binds": []}),
        ])
        .unwrap();

        let store = Arc::new(InMemoryProcedureStore::with_one(test_procedure(
            "p1",
            "demo",
            &steps_json,
        )));

        let tool = RunProcedureTool::new(registry, store.clone());
        let res = tool
            .execute(test_ctx(), json!({"name": "demo"}))
            .await
            .unwrap();
        assert_eq!(res["status"], "ok");
        assert_eq!(res["steps_run"], 2);
        let order = log.lock().await.clone();
        assert_eq!(order, vec!["step_a".to_string(), "step_b".to_string()]);

        // success_count incremented (started at 1, now 2)
        assert_eq!(store.success_count_for("p1").await, 2);
    }

    #[tokio::test]
    async fn dispatch_aborts_and_bumps_failure_on_step_error() {
        struct FailingTool;
        #[async_trait]
        impl Tool for FailingTool {
            fn name(&self) -> &'static str {
                "boom"
            }
            fn description(&self) -> &'static str {
                "fails"
            }
            async fn execute(&self, _ctx: Arc<dyn ToolContext>, _args: Value) -> Result<Value> {
                Err(ZeroError::Tool("nope".into()))
            }
        }

        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(FailingTool));
        let registry = Arc::new(registry);

        let steps_json =
            serde_json::to_string(&vec![json!({"action": "boom", "args": {}, "binds": []})])
                .unwrap();
        let store = Arc::new(InMemoryProcedureStore::with_one(test_procedure(
            "p2",
            "demo2",
            &steps_json,
        )));

        let tool = RunProcedureTool::new(registry, store.clone());
        let res = tool.execute(test_ctx(), json!({"name": "demo2"})).await;
        assert!(res.is_err());
        let err_msg = res.unwrap_err().to_string();
        assert!(err_msg.contains("step 0 (boom) failed"), "got: {err_msg}");
        assert!(store.failure_was_incremented("p2").await);
    }

    #[tokio::test]
    async fn dispatch_aborts_when_action_unknown() {
        let registry = Arc::new(ToolRegistry::new()); // empty registry

        let steps_json = serde_json::to_string(&vec![
            json!({"action": "frobnicate", "args": {}, "binds": []}),
        ])
        .unwrap();
        let store = Arc::new(InMemoryProcedureStore::with_one(test_procedure(
            "p3",
            "demo3",
            &steps_json,
        )));

        let tool = RunProcedureTool::new(registry, store.clone());
        let res = tool.execute(test_ctx(), json!({"name": "demo3"})).await;
        assert!(res.is_err());
        let err_msg = res.unwrap_err().to_string();
        assert!(err_msg.contains("not a registered tool"), "got: {err_msg}");
        assert!(store.failure_was_incremented("p3").await);
    }
}
