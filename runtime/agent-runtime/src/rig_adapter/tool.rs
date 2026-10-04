//! Host tools exposed through Rig's dynamic tool callbacks and private scope.
use super::tool_results::{SharedToolResults, ToolOutcome};
use crate::tools::context::ToolContext;
use agent_primitives::Tool as ZeroTool;
use rig::tool::{DynamicTool, ToolContext as RigToolContext, ToolOutput};
use serde_json::{json, Value};
use std::sync::Arc;

pub type SharedToolContext = Arc<ToolContext>;
pub(super) struct HostToolScope {
    pub context: SharedToolContext,
    pub results: SharedToolResults,
}
#[derive(Clone)]
pub struct RigToolAdapter {
    inner: Arc<dyn ZeroTool>,
}
#[derive(Debug, thiserror::Error)]
#[error("Host tool execution context is missing")]
struct MissingToolContext;
impl RigToolAdapter {
    #[must_use]
    pub fn new(inner: Arc<dyn ZeroTool>) -> Self {
        Self { inner }
    }
    #[must_use]
    pub fn boxed(inner: Arc<dyn ZeroTool>) -> DynamicTool {
        Self::new(inner).into_dynamic()
    }
    pub fn into_dynamic(self) -> DynamicTool {
        let schema = crate::tool_schema::harden_tool_schema(
            self.inner
                .parameters_schema()
                .filter(|v| !v.is_null())
                .unwrap_or_else(empty_object_schema),
        );
        DynamicTool::new_with_context(
            self.inner.name().to_owned(),
            self.inner.description().to_owned(),
            schema,
            move |context: &mut RigToolContext, arguments| {
                let scope = context.scope::<HostToolScope>();
                let inner = self.inner.clone();
                Box::pin(async move {
                    let scope = scope.ok_or_else(|| {
                        rig::tool::ToolExecutionError::other(MissingToolContext.to_string())
                    })?;
                    let id = scope.context.get_function_call_id();
                    RigToolAdapter::new(inner)
                        .dispatch(arguments, scope.context.clone(), scope.results.clone(), id)
                        .await
                        .map(ToolOutput::text)
                })
            },
        )
    }
}
impl RigToolAdapter {
    /// Run a host tool with its private scope and sequential call identity.
    async fn dispatch(
        &self,
        args: Value,
        ctx: SharedToolContext,
        results: SharedToolResults,
        call_id: String,
    ) -> Result<String, rig::tool::ToolExecutionError> {
        let inner = self.inner.clone();
        let started = std::time::Instant::now();
        let result: Result<String, rig::tool::ToolExecutionError> = async {
            if results.terminal() {
                return Ok(json!({"blocked":true,"reason":"terminal_action_committed"}).to_string());
            }
            if results.peer_influenced() && inner.name() != "respond" {
                return Ok(
                    json!({"blocked":true,"reason":"peer_data_authority_boundary"}).to_string(),
                );
            }
            // Providers send null for tools whose arguments are all optional.
            let args_value = if args.is_null() { json!({}) } else { args };

            if agent_tools::guards::planning_gate_blocks_tool(
                ctx.as_ref(),
                inner.name(),
                &args_value,
            ) {
                return Ok(agent_tools::guards::cold_graph_redirect().to_string());
            }

            if let Some(result) = crate::tool_replay::intercept(ctx.as_ref(), inner.name()) {
                return Ok(result);
            }

            let result = inner
                .execute(ctx.clone(), args_value)
                .await
                .map_err(|e| rig::tool::ToolExecutionError::other(e.to_string()))?;

            Ok(serialize_model_visible(result))
        }
        .await;
        let duration_ms = started.elapsed().as_millis() as i64;
        match result {
            Ok(raw) => {
                let actions = ctx.take_actions();
                if actions.respond.is_some()
                    || actions.delegate.as_ref().is_some_and(|a| !a.parallel)
                {
                    results.mark_terminal();
                }

                results.record(
                    &call_id,
                    ToolOutcome {
                        raw: Some(raw.clone()),
                        actions,
                        duration_ms,
                        ..ToolOutcome::default()
                    },
                );
                Ok(raw)
            }
            Err(error) => {
                let _ = ctx.take_actions();
                let error = error.to_string();
                results.record(
                    &call_id,
                    ToolOutcome {
                        raw: Some(String::new()),
                        error: Some(error.clone()),
                        duration_ms,
                        ..ToolOutcome::default()
                    },
                );
                Ok(json!({"error":error}).to_string())
            }
        }
    }
}

/// Render a tool result the way the model should see it.
///
/// Bare JSON string → verbatim; any other JSON value → JSON string. This is
/// the model-visible slice only; raw persistence and UI payloads are shaped
/// downstream by the engine.
fn serialize_model_visible(value: Value) -> String {
    match value {
        Value::String(text) => text,
        other => other.to_string(),
    }
}

/// Empty JSON-Schema object used when an AgentZero tool declares no parameters.
fn empty_object_schema() -> Value {
    json!({"type": "object", "properties": {}})
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_primitives::CallbackContext;
    use agent_primitives::ToolContext as ZeroToolContext;
    use async_trait::async_trait;
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// AgentZero tool that records what it was called with.
    struct RecordingTool {
        name: String,
        description: String,
        schema: Option<Value>,
        seen: Arc<Mutex<Vec<RecordedCall>>>,
    }

    #[derive(Clone, Debug)]
    struct RecordedCall {
        args: Value,
        agent_id: Option<String>,
        conversation_id: Option<String>,
        secret_from_state: Option<Value>,
    }

    impl RecordingTool {
        fn new(seen: Arc<Mutex<Vec<RecordedCall>>>) -> Self {
            Self {
                name: "record".to_string(),
                description: "Records its call".to_string(),
                schema: Some(json!({
                    "type": "object",
                    "properties": {"x": {"type": "number"}},
                    "required": ["x"]
                })),
                seen,
            }
        }
    }

    #[async_trait]
    impl ZeroTool for RecordingTool {
        fn name(&self) -> &str {
            &self.name
        }
        fn description(&self) -> &str {
            &self.description
        }
        fn parameters_schema(&self) -> Option<Value> {
            self.schema.clone()
        }
        async fn execute(
            &self,
            ctx: Arc<dyn ZeroToolContext>,
            args: Value,
        ) -> Result<Value, agent_primitives::error::AgentError> {
            let secret_from_state = ctx.get_state("app:hidden_auth_token");
            let record = RecordedCall {
                args,
                agent_id: ctx
                    .get_state("app:agent_id")
                    .and_then(|v| v.as_str().map(str::to_string)),
                conversation_id: ctx
                    .get_state("app:conversation_id")
                    .and_then(|v| v.as_str().map(str::to_string)),
                secret_from_state,
            };
            self.seen.lock().unwrap().push(record);
            Ok(json!({"ok": true}))
        }
    }

    fn shared_context_with_secret() -> SharedToolContext {
        let mut state = HashMap::new();
        state.insert(
            "app:hidden_auth_token".to_string(),
            json!("sk-secret-never-for-model"),
        );
        state.insert("app:agent_id".to_string(), json!("agent-7"));
        state.insert("app:conversation_id".to_string(), json!("conv-7"));
        Arc::new(ToolContext::full_with_state(
            "agent-7".to_string(),
            Some("conv-7".to_string()),
            Vec::new(),
            state,
        ))
    }

    #[tokio::test]
    async fn definition_maps_name_description_and_schema() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let adapter = RigToolAdapter::new(Arc::new(RecordingTool::new(seen)));
        let def = adapter.into_dynamic().definition();

        assert_eq!(def.name, "record");
        assert_eq!(def.description, "Records its call");
        assert_eq!(
            def.parameters,
            json!({
                "type": "object",
                "properties": {"x": {"type": "number"}},
                "required": ["x"],
                "additionalProperties": false
            })
        );
    }

    #[tokio::test]
    async fn hidden_context_flows_via_extensions_not_args() {
        // AC7/AC10: the secret reaches the tool through the shared context, and
        // neither the args string nor the model-visible schema contains it.
        let seen = Arc::new(Mutex::new(Vec::new()));
        let adapter = RigToolAdapter::new(Arc::new(RecordingTool::new(seen.clone())));

        let mut context = RigToolContext::new();
        context = context.with_scope(Arc::new(HostToolScope {
            context: shared_context_with_secret(),
            results: Arc::new(super::super::tool_results::ToolResults::default()),
        }));

        let args_string = json!({"x": 42}).to_string();
        let model_visible = adapter
            .clone()
            .into_dynamic()
            .execute_with(&mut context, serde_json::from_str(&args_string).unwrap())
            .await
            .expect("tool call")
            .render();

        let call = {
            let calls = seen.lock().unwrap();
            assert_eq!(calls.len(), 1, "tool executed once");
            calls[0].clone()
        };
        // Tool received the model-supplied args verbatim.
        assert_eq!(call.args, json!({"x": 42}));
        // Hidden runtime context reached the tool from extensions.
        assert_eq!(call.agent_id.as_deref(), Some("agent-7"));
        assert_eq!(call.conversation_id.as_deref(), Some("conv-7"));
        assert_eq!(
            call.secret_from_state.as_ref().and_then(|v| v.as_str()),
            Some("sk-secret-never-for-model")
        );
        // The secret never reached the tool through args — it rode the shared
        // context — so what the tool observed as args, and what the model sees
        // as the result, both omit it. (Asserting against the caller-side
        // `args_string` would be tautological: the adapter cannot mutate it.)
        let observed_args = call.args.to_string();
        assert!(!observed_args.contains("sk-secret-never-for-model"));
        assert!(!model_visible.contains("sk-secret-never-for-model"));
        // And it is not in the schema the model sees either.
        let def = adapter.into_dynamic().definition();
        let schema_str = def.parameters.to_string();
        assert!(!schema_str.contains("sk-secret-never-for-model"));
        assert!(!schema_str.contains("auth_token"));
    }

    #[tokio::test]
    async fn cold_graph_gate_blocks_rig_tool_dispatch_before_execution() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let adapter = RigToolAdapter::new(Arc::new(RecordingTool {
            name: "shell".to_string(),
            description: "mutates the workspace".to_string(),
            schema: None,
            seen: seen.clone(),
        }));
        let ctx = shared_context_with_secret();
        ctx.set_state(
            agent_tools::guards::PLANNING_GATE_STATE.to_string(),
            serde_json::to_value(agent_tools::guards::PlanningGate::awaiting_ward(
                "Plan this graph request",
            ))
            .unwrap(),
        );
        let mut context = RigToolContext::new();
        context = context.with_scope(Arc::new(HostToolScope {
            context: ctx,
            results: Arc::new(super::super::tool_results::ToolResults::default()),
        }));

        let result = adapter
            .clone()
            .into_dynamic()
            .execute_with(&mut context, json!({}))
            .await
            .expect("gate returns a redirect")
            .render();

        assert!(result.contains("redirect"));
        assert!(result.contains("planner-agent"));
        // Canonical shared message (guards::cold_graph_redirect) — pins the
        // exact text so this site can never silently drift from the executor's.
        assert!(result.contains("do not call MCP tools or other tools yet."));
        assert!(seen.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn null_args_normalize_to_empty_object() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let adapter = RigToolAdapter::new(Arc::new(RecordingTool::new(seen.clone())));

        let mut context = RigToolContext::new();
        context = context.with_scope(Arc::new(HostToolScope {
            context: shared_context_with_secret(),
            results: Arc::new(super::super::tool_results::ToolResults::default()),
        }));

        adapter
            .clone()
            .into_dynamic()
            .execute_with(&mut context, Value::Null)
            .await
            .expect("tool call")
            .render();

        let calls = seen.lock().unwrap();
        assert_eq!(calls[0].args, Value::Object(Default::default()));
    }

    #[tokio::test]
    async fn direct_dispatch_respects_host_owned_peer_authority() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let adapter = RigToolAdapter::new(Arc::new(RecordingTool::new(seen.clone())));
        let results = Arc::new(super::super::tool_results::ToolResults::default());
        results.mark_peer_influenced();
        let mut context = RigToolContext::new().with_scope(Arc::new(HostToolScope {
            context: shared_context_with_secret(),
            results,
        }));
        let response = adapter
            .clone()
            .into_dynamic()
            .execute_with(&mut context, json!({"peer_influenced":false}))
            .await
            .unwrap()
            .render();
        assert!(response.contains("peer_data_authority_boundary"));
        assert!(seen.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn result_string_passes_through_object_becomes_json() {
        // AC11 (model-visible slice): bare string verbatim, object -> JSON string.
        struct StringTool;
        #[async_trait]
        impl ZeroTool for StringTool {
            fn name(&self) -> &str {
                "string-tool"
            }
            fn description(&self) -> &str {
                "returns text"
            }
            async fn execute(
                &self,
                _ctx: Arc<dyn ZeroToolContext>,
                _args: Value,
            ) -> Result<Value, agent_primitives::error::AgentError> {
                Ok(Value::String("plain text result".to_string()))
            }
        }
        struct ObjectTool;
        #[async_trait]
        impl ZeroTool for ObjectTool {
            fn name(&self) -> &str {
                "object-tool"
            }
            fn description(&self) -> &str {
                "returns object"
            }
            async fn execute(
                &self,
                _ctx: Arc<dyn ZeroToolContext>,
                _args: Value,
            ) -> Result<Value, agent_primitives::error::AgentError> {
                Ok(json!({"path": "/a/b", "bytes": 10}))
            }
        }

        let mut context = RigToolContext::new();
        context = context.with_scope(Arc::new(HostToolScope {
            context: shared_context_with_secret(),
            results: Arc::new(super::super::tool_results::ToolResults::default()),
        }));

        let s = RigToolAdapter::new(Arc::new(StringTool))
            .into_dynamic()
            .execute_with(&mut context, json!({}))
            .await
            .unwrap();
        assert_eq!(s.render(), "plain text result");

        let o = RigToolAdapter::new(Arc::new(ObjectTool))
            .into_dynamic()
            .execute_with(&mut context, json!({}))
            .await
            .unwrap();
        assert_eq!(o.render(), json!({"path": "/a/b", "bytes": 10}).to_string());
    }

    #[tokio::test]
    async fn shared_context_state_persists_across_tool_calls() {
        // load_skill-style: one tool writes state on the shared Arc<ToolContext>,
        // a second tool (separate adapter instance, same context) reads it.
        struct Writer;
        #[async_trait]
        impl ZeroTool for Writer {
            fn name(&self) -> &str {
                "writer"
            }
            fn description(&self) -> &str {
                "writes skill state"
            }
            async fn execute(
                &self,
                ctx: Arc<dyn ZeroToolContext>,
                _args: Value,
            ) -> Result<Value, agent_primitives::error::AgentError> {
                ctx.set_state("skill:loaded".to_string(), json!(["alpha"]));
                Ok(Value::Null)
            }
        }
        let seen = Arc::new(Mutex::new(Vec::new()));

        let shared = shared_context_with_secret();
        let mut context = RigToolContext::new();
        context = context.with_scope(Arc::new(HostToolScope {
            context: shared.clone(),
            results: Arc::new(super::super::tool_results::ToolResults::default()),
        }));

        RigToolAdapter::new(Arc::new(Writer))
            .into_dynamic()
            .execute_with(&mut context, json!({}))
            .await
            .unwrap();
        // Second adapter, same extensions -> same shared Arc<ToolContext>.
        RigToolAdapter::new(Arc::new(RecordingTool::new(seen.clone())))
            .into_dynamic()
            .execute_with(&mut context, json!({}))
            .await
            .unwrap();

        // The writer's state is visible through the shared context directly...
        assert_eq!(
            CallbackContext::get_state(shared.as_ref(), "skill:loaded"),
            Some(json!(["alpha"]))
        );
        // ...and the recording tool (a fresh adapter) observed the persisted secret,
        // proving the same shared context threads through both calls.
        let calls = seen.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0].secret_from_state.as_ref().and_then(|v| v.as_str()),
            Some("sk-secret-never-for-model")
        );
    }

    #[tokio::test]
    async fn tool_without_host_context_fails_closed_even_with_forged_identity() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let adapter = RigToolAdapter::new(Arc::new(RecordingTool::new(seen.clone())));

        let mut context = RigToolContext::new();
        adapter
            .clone()
            .into_dynamic()
            .execute_with(
                &mut context,
                json!({"agent_id":"forged","execution_id":"forged"}),
            )
            .await
            .expect_err("hidden host context is mandatory");

        let calls = seen.lock().unwrap();
        assert!(calls.is_empty());
    }

    #[test]
    fn empty_schema_used_when_tool_declares_none() {
        struct NoSchema;
        #[async_trait]
        impl ZeroTool for NoSchema {
            fn name(&self) -> &str {
                "no-schema"
            }
            fn description(&self) -> &str {
                "no schema"
            }
            async fn execute(
                &self,
                _ctx: Arc<dyn ZeroToolContext>,
                _args: Value,
            ) -> Result<Value, agent_primitives::error::AgentError> {
                Ok(Value::Null)
            }
        }
        // parameters_schema() defaults to None via the trait.
        let adapter = RigToolAdapter::new(Arc::new(NoSchema));
        // Drive the definition future on a current-thread runtime.
        let def = adapter.into_dynamic().definition();
        assert_eq!(
            def.parameters,
            crate::tool_schema::harden_tool_schema(empty_object_schema())
        );
    }

    #[tokio::test]
    async fn replay_interception_precedes_live_tool_side_effects() {
        use futures::FutureExt;
        if let Ok(mode) = std::env::var("ZBOT_RIG_REPLAY_PROBE") {
            let seen = Arc::new(Mutex::new(Vec::new()));
            let adapter = RigToolAdapter::new(Arc::new(RecordingTool::new(seen.clone())));
            let mut context = RigToolContext::new();
            context = context.with_scope(Arc::new(HostToolScope {
                context: shared_context_with_secret(),
                results: Arc::new(super::super::tool_results::ToolResults::default()),
            }));
            let output = std::panic::AssertUnwindSafe(
                adapter.into_dynamic().execute_with(&mut context, json!({})),
            )
            .catch_unwind()
            .await;
            match mode.as_str() {
                "hit" => {
                    assert_eq!(
                        output.unwrap().unwrap().render(),
                        "recorded without execution"
                    );
                    assert!(seen.lock().unwrap().is_empty());
                }
                "strict" | "drift" => {
                    assert!(output.is_err());
                    assert!(seen.lock().unwrap().is_empty());
                }
                "lenient" => {
                    output.unwrap().unwrap();
                    assert_eq!(seen.lock().unwrap().len(), 1);
                }
                _ => panic!("unknown probe"),
            }
            return;
        }
        // The replay store intentionally initializes once per process. Fresh
        // child test processes exercise its real environment boundary without
        // racing unrelated tests or injecting a test-only dispatch mechanism.
        for mode in ["hit", "strict", "drift", "lenient"] {
            let dir = tempfile::tempdir().unwrap();
            let record = if mode == "hit" || mode == "drift" {
                json!({"execution_id":"conv-7","tool_index":0,"tool_name":if mode=="drift" {"other"}else{"record"},"args_hash":"fixture","result":"recorded without execution"}).to_string()
            } else {
                String::new()
            };
            std::fs::write(dir.path().join("tool-results.jsonl"), record).unwrap();
            let result = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "rig_adapter::tool::tests::replay_interception_precedes_live_tool_side_effects",
                    "--nocapture",
                ])
                .env("ZBOT_RIG_REPLAY_PROBE", mode)
                .env("ZBOT_REPLAY_DIR", dir.path())
                .env(
                    "ZBOT_REPLAY_STRICT",
                    if mode == "lenient" { "0" } else { "1" },
                )
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{mode}: {}",
                String::from_utf8_lossy(&result.stdout)
            );
        }
    }
}
