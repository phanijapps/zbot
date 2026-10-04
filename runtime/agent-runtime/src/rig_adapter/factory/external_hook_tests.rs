use super::*;
use crate::{
    external_hooks::*, AgentEngine, ChatMessage, ChatResponse, ExecutorConfig, LlmClient,
    LlmConfig, LlmError, StreamCallback,
};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Mutex,
};
struct Sink(Mutex<Vec<HookActivity>>);
impl HookActivitySink for Sink {
    fn record(&self, row: HookActivity) {
        self.0.lock().unwrap().push(row);
    }
}
struct Provider {
    responses: Mutex<Vec<ChatResponse>>,
    calls: AtomicUsize,
}
#[async_trait::async_trait]
impl LlmClient for Provider {
    fn provider(&self) -> &str {
        "provider"
    }
    fn model(&self) -> &str {
        "model"
    }
    async fn chat(&self, _: Vec<ChatMessage>, _: Option<Value>) -> Result<ChatResponse, LlmError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.responses.lock().unwrap().remove(0))
    }
    async fn chat_stream(
        &self,
        m: Vec<ChatMessage>,
        t: Option<Value>,
        cb: StreamCallback,
    ) -> Result<ChatResponse, LlmError> {
        let result = self.chat(m, t).await?;
        if !result.content.is_empty() {
            cb(crate::StreamChunk::Token(result.content.clone()));
        }
        Ok(result)
    }
}
fn reply(tool: Option<(&str, Value)>) -> ChatResponse {
    ChatResponse {
        content: if tool.is_none() {
            "done".into()
        } else {
            String::new()
        },
        tool_calls: tool.map(|(name, arguments)| {
            vec![crate::ToolCall {
                id: "wire-call".into(),
                name: name.into(),
                arguments,
            }]
        }),
        reasoning: None,
        usage: None,
    }
}
async fn fixture(
    config: ExecutorConfig,
    responses: Vec<ChatResponse>,
) -> (
    tempfile::TempDir,
    super::RigAgentEngine,
    Arc<Sink>,
    Arc<Provider>,
    Arc<AtomicBool>,
) {
    fixture_with_failure_policy(config, responses, "continue").await
}
async fn fixture_with_failure_policy(
    config: ExecutorConfig,
    responses: Vec<ChatResponse>,
    failure_policy: &str,
) -> (
    tempfile::TempDir,
    super::RigAgentEngine,
    Arc<Sink>,
    Arc<Provider>,
    Arc<AtomicBool>,
) {
    let temp = tempfile::tempdir().unwrap();
    let paths = agent_primitives::vault_paths::VaultPaths::new(temp.path().to_owned());
    paths.ensure_dirs_exist().unwrap();
    let script = paths.config_dir().join("observe.py");
    std::fs::write(
        &script,
        "import sys,json\njson.load(sys.stdin)\nprint('{\"version\":1,\"action\":\"continue\"}')\n",
    )
    .unwrap();
    let events = [
        ("start", "run_start"),
        ("pre-model", "before_model"),
        ("post-model", "after_model"),
        ("pre-tool", "before_tool"),
        ("post-tool", "after_tool"),
        ("invalid", "invalid_tool_call"),
        ("end", "run_end"),
    ];
    let hooks: Vec<_> = events
        .into_iter()
        .map(|(id, event)| json!({"id":id,"event":event,"command":["python3",script],"on_failure": if event == "invalid_tool_call" { failure_policy } else { "continue" }}))
        .collect();
    std::fs::write(
        paths.hooks_config(),
        json!({"version":1,"hooks":hooks}).to_string(),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for path in [&script, &paths.hooks_config()] {
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
    }
    let sink = Arc::new(Sink(Mutex::new(Vec::new())));
    let snapshot = HookSnapshot::load(&paths, &[]).await.unwrap();
    let invocation = HookInvocation::new(HookInvocationConfig {
        consumed_context_bytes: 0,
        context_checkpoint: None,
        invocation_id: uuid::Uuid::new_v4().to_string(),
        session_id: "sess".into(),
        snapshot,
        paths,
        forbidden_roots: vec![],
        registered_secrets: Some(vec![]),
        activity_sink: Some(sink.clone()),
    });
    let stop = Arc::new(AtomicBool::new(false));
    let run = invocation.run("agent".into(), "exec".into(), HookMode::Chat, stop.clone());
    let provider = Arc::new(Provider {
        responses: Mutex::new(responses),
        calls: AtomicUsize::new(0),
    });
    let client = Arc::new(HookedLlmClient::new(provider.clone(), run.clone()));
    let mut registry = crate::ToolRegistry::new();
    registry.register(Arc::new(crate::RespondTool::new()));
    let mut prepared = PreparedExecution::new(
        config,
        client,
        Arc::new(registry),
        Arc::new(crate::McpManager::new()),
        Arc::new(crate::MiddlewarePipeline::new()),
    );
    prepared.external_hooks = Some(run);
    let rig_config = RigAgentConfig::new(
        "agent",
        "Agent",
        "",
        "",
        super::super::RigModelConfig::from_llm_config(
            &LlmConfig::new(
                "http://unused".into(),
                String::new(),
                "model".into(),
                "provider".into(),
            ),
            8192,
        ),
    );
    (
        temp,
        build_engine(prepared, rig_config),
        sink,
        provider,
        stop,
    )
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn invalid_tool_hook_veto_is_terminal_for_unknown_nonobject_and_policy_denied() {
    for category in ["unknown", "nonobject", "policy_denied"] {
        for action in ["block", "failure_block", "continue", "failure_continue"] {
            let mut config = ExecutorConfig::new("agent".into(), "provider".into(), "model".into());
            if category == "policy_denied" {
                config.hooks.add(Arc::new(super::test_hooks::DenyAllHook));
            }
            let call = match category {
                "unknown" => ("missing_tool", json!({})),
                "nonobject" => ("respond", json!(["invalid"])),
                _ => ("respond", json!({"message":"must not execute"})),
            };
            let blocks = matches!(action, "block" | "failure_block");
            let mut first = reply(Some(call));
            if blocks {
                first.tool_calls.as_mut().unwrap().push(crate::ToolCall {
                    id: "pending-sibling".into(),
                    name: "respond".into(),
                    arguments: json!({"message":"pending sibling must not execute"}),
                });
            }
            let (temp, engine, sink, provider, _) = fixture_with_failure_policy(
                config,
                vec![first, reply(None)],
                if action == "failure_block" {
                    "block"
                } else {
                    "continue"
                },
            )
            .await;
            let script = format!("import json,sys,pathlib\ne=json.load(sys.stdin)\nif e['event']=='run_end': pathlib.Path(__file__).with_name('end.json').write_text(json.dumps(e))\nif e['event']=='invalid_tool_call':\n if {action:?}.startswith('failure'): sys.exit(3)\n if {action:?}=='block':\n  print(json.dumps({{'version':1,'action':'block','reason':'private-veto-sentinel'}}))\n  sys.exit(0)\nprint('{{\"version\":1,\"action\":\"continue\"}}')\n");
            std::fs::write(temp.path().join("config/observe.py"), script).unwrap();
            let result = engine.execute("hello", &[]).await;
            assert_eq!(result.is_err(), blocks, "{category}/{action}: {result:?}");
            assert_eq!(
                provider.calls.load(Ordering::SeqCst),
                if blocks { 1 } else { 2 },
                "{category}/{action}"
            );
            let rows = settled(&sink);
            assert_eq!(
                rows.iter()
                    .filter(|row| row.event == HookEvent::InvalidToolCall)
                    .count(),
                1
            );
            assert!(!rows
                .iter()
                .any(|row| matches!(row.event, HookEvent::BeforeTool | HookEvent::AfterTool)));
            let end: Value = serde_json::from_slice(
                &std::fs::read(temp.path().join("config/end.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(
                end["data"]["status"],
                if blocks { "blocked" } else { "completed" }
            );
            if let Err(error) = result {
                assert!(!error.to_string().contains("private-veto-sentinel"));
            }
        }
    }
}
fn settled(sink: &Sink) -> Vec<HookActivity> {
    sink.0
        .lock()
        .unwrap()
        .iter()
        .filter(|row| row.status != HookStatus::Running)
        .cloned()
        .collect()
}
#[cfg(target_os = "linux")]
#[tokio::test]
async fn sdk_start_terminal_respond_and_tool_hooks_are_actually_wired_once() {
    let (_temp, engine, sink, provider, _) = fixture(
        ExecutorConfig::new("agent".into(), "provider".into(), "model".into()),
        vec![reply(Some(("respond", json!({"message":"terminal"}))))],
    )
    .await;
    let mut events = Vec::new();
    engine
        .execute_stream("hello", &[], &mut |event| events.push(event))
        .await
        .unwrap();
    assert!(events.iter().any(|event|matches!(event,crate::StreamEvent::ActionRespond {message,..} if message=="terminal")));
    let rows = settled(&sink);
    assert_eq!(
        rows.iter().map(|row| row.event).collect::<Vec<_>>(),
        [
            HookEvent::RunStart,
            HookEvent::BeforeModel,
            HookEvent::AfterModel,
            HookEvent::BeforeTool,
            HookEvent::AfterTool,
            HookEvent::RunEnd
        ]
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    assert!(rows[0].run_id.is_some());
    assert!(rows.iter().all(|row| row.run_id == rows[0].run_id));
    assert!(rows[3].tool_call_id.is_some());
}
#[cfg(target_os = "linux")]
#[tokio::test]
async fn protected_veto_prevents_external_pretool_and_posttool_effects() {
    let mut config = ExecutorConfig::new("agent".into(), "provider".into(), "model".into());
    config.hooks.add(Arc::new(super::test_hooks::DenyAllHook));
    let (_temp, engine, sink, _, _) = fixture(
        config,
        vec![
            reply(Some(("respond", json!({"message":"should not happen"})))),
            reply(None),
        ],
    )
    .await;
    assert_eq!(engine.execute("hello", &[]).await.unwrap(), "done");
    let rows = settled(&sink);
    assert_eq!(
        rows.iter()
            .filter(|row| row.event == HookEvent::BeforeTool)
            .count(),
        0
    );
    assert_eq!(
        rows.iter()
            .filter(|row| row.event == HookEvent::AfterTool)
            .count(),
        0
    );
    assert_eq!(
        rows.iter()
            .filter(|row| row.event == HookEvent::InvalidToolCall)
            .count(),
        1
    );
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn actual_rig_stop_and_task_abort_reap_pre_model_command_before_run_end() {
    for abort in [false, true] {
        let (temp, engine, sink, provider, stop) = fixture(
            ExecutorConfig::new("agent".into(), "provider".into(), "model".into()),
            vec![reply(None)],
        )
        .await;
        let pid_file = temp.path().join("command.pid");
        let script = format!("import sys,json,os,time\ne=json.load(sys.stdin)\npidfile={}\nif e['event']=='before_model':\n open(pidfile,'w').write(str(os.getpid()))\n time.sleep(30)\nif e['event']=='run_end':\n pid=int(open(pidfile).read())\n assert not os.path.exists('/proc/'+str(pid)), 'previous process remains'\nprint('{{\"version\":1,\"action\":\"continue\"}}')\n", serde_json::to_string(pid_file.to_str().unwrap()).unwrap());
        std::fs::write(temp.path().join("config/observe.py"), script).unwrap();
        let signal = stop.clone();
        let task = tokio::spawn(async move {
            engine
                .execute_stream_with_stop_flag("hello", &[], Some(signal), &mut |_| {})
                .await
        });
        for _ in 0..200 {
            if pid_file.exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        assert!(pid_file.exists());
        let started = std::time::Instant::now();
        if abort {
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
        } else {
            stop.store(true, Ordering::Release);
            assert!(matches!(
                task.await.unwrap(),
                Err(crate::ExecutorError::Stopped)
            ));
        }
        assert!(started.elapsed() < std::time::Duration::from_millis(500));
        for _ in 0..200 {
            if settled(&sink)
                .iter()
                .any(|row| row.event == HookEvent::RunEnd)
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        let rows = settled(&sink);
        let end = rows
            .iter()
            .find(|row| row.event == HookEvent::RunEnd)
            .expect("terminal observer settled");
        assert_eq!(end.status, HookStatus::Completed);
        assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
        assert!(sink
            .0
            .lock()
            .unwrap()
            .iter()
            .filter(|row| row.status == HookStatus::Running)
            .all(|running| rows
                .iter()
                .any(|settled| settled.activity_id == running.activity_id)));
    }
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn configured_run_start_block_is_safe_terminal_and_never_calls_provider() {
    let (temp, engine, sink, provider, _) = fixture(
        ExecutorConfig::new("agent".into(), "provider".into(), "model".into()),
        vec![reply(None)],
    )
    .await;
    std::fs::write(temp.path().join("config/observe.py"),"import json,sys\ne=json.load(sys.stdin)\nprint(json.dumps({'version':1,'action':'block','reason':'private-block-sentinel'} if e['event']=='run_start' else {'version':1,'action':'continue'}))\n").unwrap();
    let error = engine.execute("hello", &[]).await.unwrap_err().to_string();
    assert!(!error.contains("private-block-sentinel"));
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    let rows = settled(&sink);
    assert_eq!(
        rows.iter()
            .filter(|row| row.event == HookEvent::RunStart && row.status == HookStatus::Blocked)
            .count(),
        1
    );
    assert_eq!(
        rows.iter()
            .filter(|row| row.event == HookEvent::RunEnd)
            .count(),
        1
    );
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn invalid_nonobject_arguments_emit_invalid_event_before_external_pretool() {
    let (_temp, engine, sink, _, _) = fixture(
        ExecutorConfig::new("agent".into(), "provider".into(), "model".into()),
        vec![reply(Some(("respond", json!(["invalid"])))), reply(None)],
    )
    .await;
    assert_eq!(engine.execute("hello", &[]).await.unwrap(), "done");
    let rows = settled(&sink);
    assert_eq!(
        rows.iter()
            .filter(|row| row.event == HookEvent::BeforeTool)
            .count(),
        0
    );
    assert_eq!(
        rows.iter()
            .filter(|row| row.event == HookEvent::AfterTool)
            .count(),
        0
    );
    assert_eq!(
        rows.iter()
            .filter(|row| row.event == HookEvent::InvalidToolCall)
            .count(),
        1
    );
}
