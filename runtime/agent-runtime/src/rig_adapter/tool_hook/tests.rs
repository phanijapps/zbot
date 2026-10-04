use super::*;
use crate::external_hooks::*;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};

struct Recovery(Arc<AtomicUsize>);
#[async_trait::async_trait]
impl crate::EngineHook for Recovery {
    async fn after_tool(&self, _: &str, _: &Value, _: &str, _: bool) -> Option<String> {
        self.0.fetch_add(1, Ordering::SeqCst);
        None
    }
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn malformed_sdk_invalid_call_veto_precedes_recovery_and_outcome_storage() {
    for action in ["block", "failure_block", "continue", "failure_continue"] {
        let temp = tempfile::tempdir().unwrap();
        let paths = agent_primitives::vault_paths::VaultPaths::new(temp.path().to_owned());
        paths.ensure_dirs_exist().unwrap();
        let script = paths.config_dir().join("invalid.py");
        std::fs::write(&script, format!("import json,sys\ne=json.load(sys.stdin)\nassert e['data']['error']=='invalid_arguments'\nif {action:?}.startswith('failure'): sys.exit(3)\nprint(json.dumps({{'version':1,'action': 'block' if {action:?}=='block' else 'continue','reason':'private-reason'}}))\n")).unwrap();
        std::fs::write(paths.hooks_config(),json!({"version":1,"hooks":[{"id":"invalid","event":"invalid_tool_call","command":["python3",script],"on_failure":if action=="failure_block"{"block"}else{"continue"}}]}).to_string()).unwrap();
        use std::os::unix::fs::PermissionsExt;
        for path in [&script, &paths.hooks_config()] {
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        let snapshot = HookSnapshot::load(&paths, &[]).await.unwrap();
        let owner = HookInvocation::new(HookInvocationConfig {
            consumed_context_bytes: 0,
            context_checkpoint: None,
            invocation_id: uuid::Uuid::new_v4().to_string(),
            session_id: "sess".into(),
            snapshot,
            paths,
            forbidden_roots: vec![],
            registered_secrets: Some(vec![]),
            activity_sink: None,
        });
        let run = owner.run(
            "agent".into(),
            "exec".into(),
            HookMode::Chat,
            Arc::new(AtomicBool::new(false)),
        );
        let recovery = Arc::new(AtomicUsize::new(0));
        let mut hooks = HookSet::new();
        hooks.add(Arc::new(Recovery(recovery.clone())));
        let results = Arc::new(super::super::tool_results::ToolResults::default());
        let hook = RigExecutionHook {
            ctx: Arc::new(crate::tools::ToolContext::new()),
            hooks: Arc::new(hooks),
            results: results.clone(),
            context_config: Default::default(),
            events: None,
            stop: None,
            external_hooks: Some(run.clone()),
        };
        let call = InvalidToolCallContext {
            tool_name: "respond".into(),
            tool_call_id: Some(rig::completion::message::CallId::from_wire(
                "malformed-call",
            )),
            args: Some("{broken-private-arguments".into()),
            available_tools: vec!["respond".into()],
            allowed_tools: vec!["respond".into()],
            tool_choice: None,
            chat_history: vec![],
            is_streaming: true,
            reason: InvalidToolCallReason::MalformedArguments {
                error: "invalid JSON".into(),
            },
        };
        let result = hook.invalid_call_action(&call).await.unwrap();
        let blocks = matches!(action, "block" | "failure_block");
        assert_eq!(
            matches!(result, InvalidToolCallAction::Stop { .. }),
            blocks,
            "{action}: {result:?}"
        );
        assert_eq!(run.blocked(), blocks);
        assert_eq!(recovery.load(Ordering::SeqCst), usize::from(!blocks));
        assert_eq!(
            results.snapshot("malformed-call").rejected_call.is_none(),
            blocks
        );
        assert!(!format!("{result:?}").contains("private-reason"));
    }
}
