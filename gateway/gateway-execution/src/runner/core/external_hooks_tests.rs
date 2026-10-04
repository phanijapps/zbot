//! Real gateway, Rig, TCP provider and process lifecycle controls.
use super::{test_support::*, *};
use agent_runtime::external_hooks::*;
use serde_json::{json, Value};
use std::sync::Mutex;
use tokio::time::{timeout, Duration};

struct Sink(Mutex<Vec<HookActivity>>);
impl HookActivitySink for Sink {
    fn record(&self, row: HookActivity) {
        self.0.lock().unwrap().push(row);
    }
}
fn install(harness: &Harness, label: &str) -> std::path::PathBuf {
    let script = harness.paths.config_dir().join("observe.py");
    let output = harness.paths.vault_dir().join("observed.jsonl");
    std::fs::write(&script, "import sys,json\ne=json.load(sys.stdin)\ne['label']=sys.argv[2]\nwith open(sys.argv[1],'a') as f:f.write(json.dumps(e)+'\\n')\nprint('{\"version\":1,\"action\":\"continue\"}')\n").unwrap();
    let events = [
        "session_start",
        "user_prompt",
        "run_start",
        "before_model",
        "after_model",
        "before_tool",
        "after_tool",
        "invalid_tool_call",
        "run_end",
    ];
    let hooks: Vec<_> = events.iter().enumerate().map(|(i,event)|json!({"id":format!("observe-{i}"),"event":event,"command":["python3",script,output,label]})).collect();
    std::fs::write(
        harness.paths.hooks_config(),
        json!({"version":1,"hooks":hooks}).to_string(),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for file in [&script, &harness.paths.hooks_config()] {
            std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
    }
    output
}
fn observed(path: &std::path::Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
async fn provider(turns: usize) -> (String, tokio::task::JoinHandle<Vec<Value>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let mut requests = Vec::new();
        for _ in 0..turns {
            let (mut socket, _) = listener.accept().await.unwrap();
            requests.push(serde_json::from_str(&read_request_body(&mut socket).await).unwrap());
            let delta = json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"respond-call","function":{"name":"respond","arguments":"{\"message\":\"answer\"}"}}]},"finish_reason":null}]});
            write_sse(&mut socket,&format!("data: {delta}\n\ndata: {{\"choices\":[{{\"delta\":{{}},\"finish_reason\":\"tool_calls\"}}]}}\n\ndata: [DONE]\n\n")).await;
        }
        requests
    });
    (url, task)
}
async fn completed(
    events: &mut tokio::sync::broadcast::Receiver<GatewayEvent>,
    session: &str,
    agent: &str,
) {
    timeout(Duration::from_secs(10),async {loop {if matches!(events.recv().await.unwrap(),GatewayEvent::AgentCompleted{session_id,agent_id,..} if session_id==session && agent_id==agent) {break;}}}).await.unwrap();
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn new_and_precreated_chat_research_roots_claim_ingress_once_per_message() {
    for mode in ["chat", "research", "fast", "deep"] {
        for precreated in [false, true] {
            let (url, server) = provider(2).await;
            let harness = build_harness(url).await;
            let output = install(&harness, "a");
            let sink = Arc::new(Sink(Mutex::new(Vec::new())));
            harness.runner.set_external_hook_activity_sink(sink.clone());
            let mut events = harness.runner.ctx.event_bus.subscribe_all();
            let existing = precreated.then(|| harness.state.create_session("root").unwrap().0.id);
            assert!(observed(&output).is_empty());
            let mut config = ExecutionConfig::new(
                "root".into(),
                "root-first".into(),
                harness.paths.vault_dir().clone(),
            )
            .with_mode(mode.into());
            config.session_id = existing;
            let (_, session) = harness.runner.invoke(config, "first".into()).await.unwrap();
            completed(&mut events, &session, "root").await;
            let mut config = ExecutionConfig::new(
                "root".into(),
                "root-second".into(),
                harness.paths.vault_dir().clone(),
            )
            .with_mode(mode.into());
            config.session_id = Some(session.clone());
            harness
                .runner
                .invoke(config, "second".into())
                .await
                .unwrap();
            completed(&mut events, &session, "root").await;
            let rows = observed(&output);
            assert_eq!(
                rows.iter()
                    .filter(|row| row["event"] == "session_start")
                    .count(),
                1
            );
            assert_eq!(
                rows.iter()
                    .filter(|row| row["event"] == "user_prompt")
                    .count(),
                2
            );
            for event in [
                "run_start",
                "before_model",
                "after_model",
                "before_tool",
                "after_tool",
                "run_end",
            ] {
                assert_eq!(
                    rows.iter().filter(|row| row["event"] == event).count(),
                    2,
                    "{mode} {precreated} {event}"
                );
            }
            assert!(rows
                .iter()
                .filter(|row| row["event"] == "run_start")
                .all(|row| row["data"]["mode"]
                    == if matches!(mode, "chat" | "fast") {
                        "chat"
                    } else {
                        "research"
                    }));
            let ingress: Vec<_> = rows
                .iter()
                .filter(|row| row["event"] == "user_prompt")
                .collect();
            assert_ne!(ingress[0]["invocation_id"], ingress[1]["invocation_id"]);
            assert!(ingress.iter().all(|row| row["run_id"].is_null()));
            let activity = sink.0.lock().unwrap().clone();
            assert!(activity
                .iter()
                .filter(|row| row.status == HookStatus::Running)
                .all(|running| activity
                    .iter()
                    .any(|row| row.activity_id == running.activity_id
                        && row.status != HookStatus::Running
                        && row.occurred_at == running.occurred_at)));
            assert_eq!(server.await.unwrap().len(), 2);
        }
    }
}

async fn child_request(
    harness: &Harness,
    partial: &super::super::invoke_bootstrap::PartialSetup,
) -> DelegationRequest {
    let execution = execution_state::AgentExecution::new_delegated(
        &partial.session_id,
        "resume-test-agent",
        &partial.execution_id,
        execution_state::DelegationType::Sequential,
        "answer",
    );
    harness.state.create_execution(&execution).unwrap();
    harness
        .state
        .register_delegation(&partial.session_id)
        .unwrap();
    harness
        .state
        .request_continuation(&partial.session_id)
        .unwrap();
    DelegationRequest {
        hook_invocation: partial
            .accepted_hooks
            .run
            .as_ref()
            .map(|run| run.invocation().clone()),
        parent_agent_id: "root".into(),
        session_id: partial.session_id.clone(),
        parent_execution_id: partial.execution_id.clone(),
        parent_conversation_id: partial.session_id.clone(),
        child_agent_id: "resume-test-agent".into(),
        child_execution_id: execution.id,
        task: "answer".into(),
        mode: None,
        context: None,
        max_iterations: Some(5),
        output_schema: None,
        skills: vec![],
        capability_assignment: None,
        planning_capability_catalog: None,
        complexity: None,
        parallel: false,
    }
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn earlier_hooked_child_and_continuation_keep_revision_after_new_root() {
    let (url, server) = provider(2).await;
    let harness = build_harness(url).await;
    let output = install(&harness, "old");
    let mut config = ExecutionConfig::new(
        "root".into(),
        "old".into(),
        harness.paths.vault_dir().clone(),
    )
    .with_mode("fast".into());
    let old = harness
        .runner
        .bootstrap
        .begin_setup(&mut config, "old prompt", None)
        .await
        .unwrap();
    let owner = old
        .accepted_hooks
        .run
        .as_ref()
        .unwrap()
        .invocation()
        .clone();
    install(&harness, "new");
    config.session_id = Some(old.session_id.clone());
    config.conversation_id = "new".into();
    let new = harness
        .runner
        .bootstrap
        .begin_setup(&mut config, "new prompt", None)
        .await
        .unwrap();
    assert_ne!(
        new.accepted_hooks.run.as_ref().unwrap().invocation().id(),
        owner.id()
    );
    let request = child_request(&harness, &old).await;
    let mut events = harness.runner.ctx.event_bus.subscribe_all();
    crate::delegation::spawn::spawn_delegated_agent(&harness.runner.ctx, &request, None)
        .await
        .unwrap();
    let id = timeout(Duration::from_secs(10), async {
        loop {
            if let GatewayEvent::SessionContinuationReady {
                hook_invocation_id, ..
            } = events.recv().await.unwrap()
            {
                break hook_invocation_id;
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(id.as_deref(), Some(owner.id()));
    completed(&mut events, &old.session_id, "root").await;
    let rows = observed(&output);
    assert!(rows
        .iter()
        .filter(|row| row["event"] == "run_start")
        .all(|row| row["data"]["mode"] == "chat"));
    assert!(rows
        .iter()
        .all(|row| row["label"] == "old" && row["invocation_id"] == owner.id()));
    assert_eq!(
        rows.iter()
            .filter(|row| row["event"] == "run_start")
            .count(),
        2
    );
    assert_eq!(
        rows.iter().filter(|row| row["event"] == "run_end").count(),
        2
    );
    assert!(!rows
        .iter()
        .any(|row| row["event"] == "session_start" || row["event"] == "user_prompt"));
    assert_eq!(server.await.unwrap().len(), 2);
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn disabled_root_child_and_continuation_stay_unhooked_after_configuration_edits() {
    for prior_enabled in [false, true] {
        let (url, server) = provider(2).await;
        let harness = build_harness(url).await;
        let mut config = ExecutionConfig::new(
            "root".into(),
            "root".into(),
            harness.paths.vault_dir().clone(),
        )
        .with_mode("chat".into());
        if prior_enabled {
            install(&harness, "previous");
            let first = harness
                .runner
                .bootstrap
                .begin_setup(&mut config, "previous", None)
                .await
                .unwrap();
            config.session_id = Some(first.session_id.clone());
            harness
                .runner
                .ctx
                .hook_invocations
                .remove(first.accepted_hooks.run.as_ref().unwrap().invocation().id());
            std::fs::remove_file(harness.paths.hooks_config()).unwrap();
        }
        let accepted = harness
            .runner
            .bootstrap
            .begin_setup(&mut config, "unhooked", None)
            .await
            .unwrap();
        assert!(accepted.accepted_hooks.run.is_none());
        let request = child_request(&harness, &accepted).await;
        let output = install(&harness, "future");
        let mut events = harness.runner.ctx.event_bus.subscribe_all();
        crate::delegation::spawn::spawn_delegated_agent(&harness.runner.ctx, &request, None)
            .await
            .unwrap();
        let id = timeout(Duration::from_secs(10), async {
            loop {
                if let GatewayEvent::SessionContinuationReady {
                    hook_invocation_id, ..
                } = events.recv().await.unwrap()
                {
                    break hook_invocation_id;
                }
            }
        })
        .await
        .unwrap();
        assert!(id.is_none());
        completed(&mut events, &accepted.session_id, "root").await;
        assert!(observed(&output).is_empty());
        assert_eq!(server.await.unwrap().len(), 2);
    }
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn stop_during_ingress_reaps_command_and_uses_existing_stop_settlement() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let harness = build_harness(format!("http://{}/v1", listener.local_addr().unwrap())).await;
    install(&harness, "ingress");
    let pid = harness.paths.vault_dir().join("ingress.pid");
    let script=format!("import sys,json,os,time\ne=json.load(sys.stdin)\nif e['event']=='user_prompt':\n open({},'w').write(str(os.getpid()))\n time.sleep(30)\nprint('{{\"version\":1,\"action\":\"continue\"}}')\n",serde_json::to_string(pid.to_str().unwrap()).unwrap());
    std::fs::write(harness.paths.config_dir().join("observe.py"), script).unwrap();
    let sink = Arc::new(Sink(Mutex::new(Vec::new())));
    harness.runner.set_external_hook_activity_sink(sink.clone());
    let mut events = harness.runner.ctx.event_bus.subscribe_all();
    let invoke = harness.runner.invoke(
        ExecutionConfig::new(
            "root".into(),
            "ingress-stop".into(),
            harness.paths.vault_dir().clone(),
        )
        .with_mode("chat".into()),
        "hello".into(),
    );
    let stop = async {
        for _ in 0..200 {
            if pid.exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert!(pid.exists());
        harness.runner.stop("ingress-stop").await.unwrap();
    };
    let (result, ()) = tokio::join!(invoke, stop);
    assert!(result.is_err());
    let mut stopped = false;
    let mut crashed = false;
    while let Ok(event) = events.try_recv() {
        match event {
            GatewayEvent::AgentStopped { .. } => stopped = true,
            GatewayEvent::Error { .. } => crashed = true,
            _ => {}
        }
    }
    assert!(stopped);
    assert!(!crashed);
    let session = harness
        .state
        .list_sessions(&Default::default())
        .unwrap()
        .remove(0);
    assert_eq!(
        harness
            .state
            .get_root_execution(&session.id)
            .unwrap()
            .unwrap()
            .status,
        execution_state::ExecutionStatus::Cancelled
    );
    let previous: u32 = std::fs::read_to_string(pid).unwrap().parse().unwrap();
    assert!(!std::path::Path::new(&format!("/proc/{previous}")).exists());
    let rows = sink.0.lock().unwrap().clone();
    assert!(rows
        .iter()
        .any(|row| row.event == HookEvent::UserPrompt && row.status == HookStatus::Cancelled));
    assert!(!rows
        .iter()
        .any(|row| matches!(row.event, HookEvent::RunStart | HookEvent::BeforeModel)));
    assert!(timeout(Duration::from_millis(50), listener.accept())
        .await
        .is_err());
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn persisted_resume_keeps_uuid_ingress_claim_revision_and_consumed_context_budget() {
    let (url, server) = provider(1).await;
    let harness = build_harness(url).await;
    let output = install(&harness, "resume");
    let mut config = ExecutionConfig::new(
        "root".into(),
        "resume".into(),
        harness.paths.vault_dir().clone(),
    )
    .with_mode("chat".into());
    let first = harness
        .runner
        .bootstrap
        .begin_setup(&mut config, "accepted", None)
        .await
        .unwrap();
    let session = first.session_id.clone();
    let execution = first.execution_id.clone();
    let message = first.root_message_id.clone();
    let owner = first
        .accepted_hooks
        .run
        .as_ref()
        .unwrap()
        .invocation()
        .clone();
    let id = owner.id().to_owned();
    assert!(
        !first
            .accepted_hooks
            .run
            .as_ref()
            .unwrap()
            .ingress(true, "accepted")
            .await
    );
    harness
        .runner
        .ctx
        .session_meta
        .checkpoint_hook_context_bytes(&session, &id, 8192)
        .unwrap();
    drop(first);
    assert!(harness
        .runner
        .ctx
        .hook_invocations
        .get(&id, &session)
        .is_none());
    // An overflow response must be rejected using the saved budget after reconstruction.
    let script = harness.paths.config_dir().join("observe.py");
    let source=std::fs::read_to_string(&script).unwrap().replace("print('{\"version\":1,\"action\":\"continue\"}')","print(json.dumps({'version':1,'action':'continue','context':'é'} if e['event']=='before_model' else {'version':1,'action':'continue'}))");
    std::fs::write(script, source).unwrap();
    let sink = Arc::new(Sink(Mutex::new(Vec::new())));
    harness.runner.set_external_hook_activity_sink(sink.clone());
    let mut events = harness.runner.ctx.event_bus.subscribe_all();
    config.session_id = Some(session.clone());
    harness
        .runner
        .invoke_persisted_with_callback(
            config.clone(),
            "accepted".into(),
            execution.clone(),
            message.clone(),
            None,
        )
        .await
        .unwrap();
    completed(&mut events, &session, "root").await;
    let rows = observed(&output);
    assert!(rows.iter().all(|row| row["invocation_id"] == id));
    assert_eq!(
        rows.iter()
            .filter(|row| row["event"] == "session_start")
            .count(),
        1
    );
    assert_eq!(
        rows.iter()
            .filter(|row| row["event"] == "user_prompt")
            .count(),
        1
    );
    assert!(sink
        .0
        .lock()
        .unwrap()
        .iter()
        .any(|row| row.event == HookEvent::BeforeModel && row.status == HookStatus::Failed));
    assert_eq!(server.await.unwrap().len(), 1);
    let count = rows.len();
    install(&harness, "edited");
    assert!(harness
        .runner
        .invoke_persisted_with_callback(config, "accepted".into(), execution, message, None)
        .await
        .is_err());
    assert_eq!(observed(&output).len(), count);
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn interrupted_unhooked_message_cannot_gain_new_hooks_on_persisted_resume() {
    let harness = build_harness("http://unused".into()).await;
    let mut config = ExecutionConfig::new(
        "root".into(),
        "unhooked".into(),
        harness.paths.vault_dir().clone(),
    )
    .with_mode("chat".into());
    let accepted = harness
        .runner
        .bootstrap
        .begin_setup(&mut config, "accepted", None)
        .await
        .unwrap();
    config.session_id = Some(accepted.session_id.clone());
    let output = install(&harness, "new");
    assert!(harness
        .runner
        .invoke_persisted_with_callback(
            config,
            "accepted".into(),
            accepted.execution_id.clone(),
            accepted.root_message_id.clone(),
            None
        )
        .await
        .is_err());
    assert!(observed(&output).is_empty());
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn phase_one_reactivation_failure_drops_retained_owner_before_hooks_run() {
    let harness = build_harness("http://unused".into()).await;
    let output = install(&harness, "failure");
    let (session, root) = harness.state.create_session("root").unwrap();
    harness.state.complete_session(&session.id).unwrap();
    harness.state.complete_execution(&root.id).unwrap();
    let pool =
        zbot_conversation::open_conversation_pool(&harness.paths.conversations_db()).unwrap();
    pool.get().unwrap().execute_batch("CREATE TRIGGER fail_hook_reactivation BEFORE UPDATE OF status ON sessions WHEN NEW.status='running' BEGIN SELECT RAISE(ABORT,'fixture reactivation failure'); END;").unwrap();
    let mut config = ExecutionConfig::new(
        "root".into(),
        "failure".into(),
        harness.paths.vault_dir().clone(),
    )
    .with_mode("chat".into());
    config.session_id = Some(session.id.clone());
    assert!(harness
        .runner
        .bootstrap
        .begin_setup(&mut config, "accepted", None)
        .await
        .is_err());
    let (id, _) = harness
        .runner
        .ctx
        .session_meta
        .hook_invocation_identity(&session.id)
        .unwrap()
        .unwrap();
    assert!(harness
        .runner
        .ctx
        .hook_invocations
        .get(&id, &session.id)
        .is_none());
    assert!(observed(&output).is_empty());
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn stop_after_actual_root_yields_to_child_settles_once_and_cannot_continue() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let (seen_tx, seen_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut root, _) = listener.accept().await.unwrap();
        read_request(&mut root).await;
        let delta = json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"delegate-call","function":{"name":"delegate_to_agent","arguments":"{\"agent_id\":\"resume-test-agent\",\"task\":\"answer\",\"wait_for_result\":true}"}}]},"finish_reason":null}]});
        write_sse(&mut root,&format!("data: {delta}\n\ndata: {{\"choices\":[{{\"delta\":{{}},\"finish_reason\":\"tool_calls\"}}]}}\n\ndata: [DONE]\n\n")).await;
        let (mut child, _) = listener.accept().await.unwrap();
        read_request(&mut child).await;
        seen_tx.send(()).unwrap();
        let _ = release_rx.await;
        drop(child);
        assert!(timeout(Duration::from_millis(250), listener.accept())
            .await
            .is_err());
    });
    let harness = build_harness(url).await;
    let output = install(&harness, "stop-yield");
    let mut events = harness.runner.ctx.event_bus.subscribe_all();
    let (_, session) = harness
        .runner
        .invoke(
            ExecutionConfig::new(
                "root".into(),
                "yield-stop".into(),
                harness.paths.vault_dir().clone(),
            )
            .with_mode("research".into()),
            "delegate".into(),
        )
        .await
        .unwrap();
    timeout(Duration::from_secs(10), seen_rx)
        .await
        .unwrap()
        .unwrap();
    for _ in 0..200 {
        if observed(&output)
            .iter()
            .any(|row| row["agent_id"] == "root" && row["event"] == "run_end")
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(observed(&output)
        .iter()
        .any(|row| row["agent_id"] == "root" && row["event"] == "run_end"));
    tokio::time::sleep(Duration::from_millis(20)).await;
    harness.runner.stop("yield-stop").await.unwrap();
    assert_eq!(
        harness
            .state
            .get_root_execution(&session)
            .unwrap()
            .unwrap()
            .status,
        execution_state::ExecutionStatus::Cancelled
    );
    release_tx.send(()).unwrap();
    server.await.unwrap();
    let mut stopped = 0;
    while let Ok(event) = events.try_recv() {
        if matches!(&event,GatewayEvent::AgentStopped{agent_id,..} if agent_id=="root") {
            stopped += 1;
        }
        assert!(!matches!(
            event,
            GatewayEvent::SessionContinuationReady { .. }
        ));
    }
    assert_eq!(stopped, 1);
}
