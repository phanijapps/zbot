use super::*;
use crate::LlmClient;
use serde_json::json;
use std::sync::atomic::AtomicUsize;

#[cfg(target_os = "linux")]
mod model_fallback;

pub struct RecordingSink(pub Mutex<Vec<HookActivity>>);
impl HookActivitySink for RecordingSink {
    fn record(&self, activity: HookActivity) {
        self.0.lock().unwrap().push(activity);
    }
}
async fn invocation(
    events: &[(&str, &str)],
) -> (tempfile::TempDir, Arc<HookInvocation>, Arc<RecordingSink>) {
    let temp = tempfile::tempdir().unwrap();
    let paths = VaultPaths::new(temp.path().to_owned());
    paths.ensure_dirs_exist().unwrap();
    let script = paths.config_dir().join("observe.py");
    std::fs::write(
        &script,
        "import sys,json\njson.load(sys.stdin)\nprint('{\"version\":1,\"action\":\"continue\"}')\n",
    )
    .unwrap();
    let definitions:Vec<_>=events.iter().map(|(id,event)| json!({"id":id,"event":event,"command":["python3",script,id],"timeout_ms":1000})).collect();
    std::fs::write(
        paths.hooks_config(),
        json!({"version":1,"hooks":definitions}).to_string(),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for file in [&script, &paths.hooks_config()] {
            std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
    }
    let snapshot = HookSnapshot::load(&paths, &[]).await.unwrap();
    let sink = Arc::new(RecordingSink(Mutex::new(Vec::new())));
    let owner = HookInvocation::new(HookInvocationConfig {
        consumed_context_bytes: 0,
        context_checkpoint: None,
        invocation_id: uuid::Uuid::new_v4().to_string(),
        session_id: "sess-root".into(),
        snapshot,
        paths,
        forbidden_roots: vec![],
        registered_secrets: Some(vec![]),
        activity_sink: Some(sink.clone()),
    });
    (temp, owner, sink)
}
fn settled(sink: &RecordingSink) -> Vec<HookActivity> {
    sink.0
        .lock()
        .unwrap()
        .iter()
        .filter(|row| row.status != HookStatus::Running)
        .cloned()
        .collect()
}
struct Provider {
    calls: AtomicUsize,
    pending: bool,
}
#[async_trait::async_trait]
impl crate::LlmClient for Provider {
    fn model(&self) -> &str {
        "fixture-model"
    }
    fn provider(&self) -> &str {
        "fixture-provider"
    }
    async fn chat(
        &self,
        _messages: Vec<crate::ChatMessage>,
        _tools: Option<serde_json::Value>,
    ) -> Result<crate::ChatResponse, crate::LlmError> {
        let attempt = self.calls.fetch_add(1, Ordering::SeqCst);
        if self.pending {
            return futures::future::pending().await;
        }
        if attempt == 0 {
            return Err(crate::LlmError::RateLimited);
        }
        Ok(crate::ChatResponse {
            content: "answer".into(),
            tool_calls: None,
            reasoning: None,
            usage: None,
        })
    }
    async fn chat_stream(
        &self,
        messages: Vec<crate::ChatMessage>,
        tools: Option<serde_json::Value>,
        _: crate::StreamCallback,
    ) -> Result<crate::ChatResponse, crate::LlmError> {
        self.chat(messages, tools).await
    }
}
#[cfg(target_os = "linux")]
#[tokio::test]
async fn provider_hooks_count_real_retries_and_terminal_settlement_once() {
    let (_temp, owner, sink) = invocation(&[
        ("start", "run_start"),
        ("pre", "before_model"),
        ("post", "after_model"),
        ("end", "run_end"),
    ])
    .await;
    let run = owner.run(
        "root".into(),
        "exec-root".into(),
        HookMode::Chat,
        Arc::new(AtomicBool::new(false)),
    );
    assert!(!run.start("sdk-authoritative".into()).await);
    run.set_turn("sdk-authoritative".into(), 1);
    let raw = Arc::new(Provider {
        calls: AtomicUsize::new(0),
        pending: false,
    });
    let hooked = Arc::new(HookedLlmClient::new(raw.clone(), run.clone()));
    let retry = crate::RetryingLlmClient::new(
        hooked,
        crate::RetryPolicy {
            base_delay: std::time::Duration::ZERO,
            max_delay: std::time::Duration::ZERO,
            ..Default::default()
        },
    );
    assert_eq!(retry.chat(vec![], None).await.unwrap().content, "answer");
    run.settle(HookRunStatus::Completed).await;
    run.settle(HookRunStatus::Completed).await;
    let rows = settled(&sink);
    assert_eq!(
        rows.iter().map(|row| row.event).collect::<Vec<_>>(),
        [
            HookEvent::RunStart,
            HookEvent::BeforeModel,
            HookEvent::AfterModel,
            HookEvent::BeforeModel,
            HookEvent::AfterModel,
            HookEvent::RunEnd
        ]
    );
    assert!(rows
        .iter()
        .all(|row| row.run_id.as_deref() == Some("sdk-authoritative")));
    assert_eq!(raw.calls.load(Ordering::SeqCst), 2);
}
#[cfg(target_os = "linux")]
#[tokio::test]
async fn aborted_provider_attempt_observes_cancelled_then_run_end_without_duplicate_ingress() {
    let (_temp, owner, sink) = invocation(&[
        ("session", "session_start"),
        ("prompt", "user_prompt"),
        ("start", "run_start"),
        ("pre", "before_model"),
        ("post", "after_model"),
        ("end", "run_end"),
    ])
    .await;
    let run = owner.run(
        "root".into(),
        "exec-root".into(),
        HookMode::Research,
        Arc::new(AtomicBool::new(false)),
    );
    assert!(!run.ingress(true, "hello").await);
    assert!(!run.start("sdk-id".into()).await);
    let raw = Arc::new(Provider {
        calls: AtomicUsize::new(0),
        pending: true,
    });
    let client = HookedLlmClient::new(raw.clone(), run.clone());
    let task =
        tokio::spawn(async move { client.chat_stream(vec![], None, Box::new(|_| {})).await });
    for _ in 0..100 {
        if raw.calls.load(Ordering::SeqCst) > 0 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert_eq!(raw.calls.load(Ordering::SeqCst), 1);
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    run.settle(HookRunStatus::Cancelled).await;
    let rows = settled(&sink);
    assert_eq!(
        rows.iter().map(|row| row.event).collect::<Vec<_>>(),
        [
            HookEvent::SessionStart,
            HookEvent::UserPrompt,
            HookEvent::RunStart,
            HookEvent::BeforeModel,
            HookEvent::AfterModel,
            HookEvent::RunEnd
        ]
    );
    assert!(rows[0].run_id.is_none());
    assert!(rows[1].run_id.is_none());
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn stop_during_before_model_never_starts_raw_provider() {
    let (_temp, owner, sink) = invocation(&[("pre", "before_model")]).await;
    let script = owner.config.paths.config_dir().join("observe.py");
    std::fs::write(
        &script,
        "import sys,json,time\njson.load(sys.stdin)\ntime.sleep(30)\n",
    )
    .unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let run = owner.run(
        "root".into(),
        "exec-root".into(),
        HookMode::Chat,
        stop.clone(),
    );
    let raw = Arc::new(Provider {
        calls: AtomicUsize::new(0),
        pending: false,
    });
    let client = HookedLlmClient::new(raw.clone(), run);
    let task = tokio::spawn(async move { client.chat(vec![], None).await });
    for _ in 0..100 {
        if !sink.0.lock().unwrap().is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    assert!(!sink.0.lock().unwrap().is_empty());
    stop.store(true, Ordering::Release);
    assert!(task.await.unwrap().is_err());
    assert_eq!(raw.calls.load(Ordering::SeqCst), 0);
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn same_event_commands_are_ordered_and_block_short_circuits_without_private_reason() {
    let (_temp, owner, sink) = invocation(&[
        ("first", "run_start"),
        ("second", "run_start"),
        ("third", "run_start"),
    ])
    .await;
    std::fs::write(owner.config.paths.config_dir().join("observe.py"),"import sys,json\njson.load(sys.stdin)\nprint(json.dumps({'version':1,'action':'block','reason':'private-sentinel'} if sys.argv[1]=='second' else {'version':1,'action':'continue','context':'reference'}))\n").unwrap();
    let run = owner.run(
        "root".into(),
        "exec".into(),
        HookMode::Chat,
        Arc::new(AtomicBool::new(false)),
    );
    assert!(run.start("actual-sdk-run".into()).await);
    let rows = settled(&sink);
    assert_eq!(
        rows.iter()
            .map(|row| row.hook_id.as_str())
            .collect::<Vec<_>>(),
        ["first", "second"]
    );
    assert!(rows[0].occurred_at < rows[1].occurred_at);
    assert_eq!(rows[1].status, HookStatus::Blocked);
    assert!(!format!("{rows:?}").contains("private-sentinel"));
}

struct CaptureProvider(Mutex<Vec<Vec<crate::ChatMessage>>>);
#[async_trait::async_trait]
impl crate::LlmClient for CaptureProvider {
    fn model(&self) -> &str {
        "model"
    }
    fn provider(&self) -> &str {
        "provider"
    }
    async fn chat(
        &self,
        messages: Vec<crate::ChatMessage>,
        _: Option<serde_json::Value>,
    ) -> Result<crate::ChatResponse, crate::LlmError> {
        self.0.lock().unwrap().push(messages);
        Ok(crate::ChatResponse {
            content: "done".into(),
            tool_calls: None,
            reasoning: None,
            usage: None,
        })
    }
    async fn chat_stream(
        &self,
        messages: Vec<crate::ChatMessage>,
        tools: Option<serde_json::Value>,
        _: crate::StreamCallback,
    ) -> Result<crate::ChatResponse, crate::LlmError> {
        self.chat(messages, tools).await
    }
}
#[cfg(target_os = "linux")]
#[tokio::test]
async fn restored_context_budget_charges_utf8_before_attributed_user_data_and_omits_empty() {
    let (_temp, original, sink) = invocation(&[
        ("first", "before_model"),
        ("second", "before_model"),
        ("empty", "before_model"),
    ])
    .await;
    std::fs::write(original.config.paths.config_dir().join("observe.py"),"import sys,json\njson.load(sys.stdin)\nprint(json.dumps({'version':1,'action':'continue','context':'' if sys.argv[1]=='empty' else 'é'}))\n").unwrap();
    let checkpoints = Arc::new(Mutex::new(Vec::new()));
    let record = checkpoints.clone();
    let owner = HookInvocation::new(HookInvocationConfig {
        consumed_context_bytes: 8190,
        context_checkpoint: Some(Arc::new(move |used| {
            record.lock().unwrap().push(used);
            Ok(())
        })),
        invocation_id: original.id().to_owned(),
        session_id: original.session_id().to_owned(),
        snapshot: original.snapshot().clone(),
        paths: original.config.paths.clone(),
        forbidden_roots: vec![],
        registered_secrets: Some(vec![]),
        activity_sink: Some(sink.clone()),
    });
    let run = owner.run(
        "root".into(),
        "exec".into(),
        HookMode::Chat,
        Arc::new(AtomicBool::new(false)),
    );
    let raw = Arc::new(CaptureProvider(Mutex::new(Vec::new())));
    let client = HookedLlmClient::new(raw.clone(), run);
    client
        .chat(
            vec![
                crate::ChatMessage::system("protected instructions".into()),
                crate::ChatMessage::user("actual prompt".into()),
            ],
            None,
        )
        .await
        .unwrap();
    assert_eq!(*checkpoints.lock().unwrap(), [8192]);
    let requests = raw.0.lock().unwrap();
    assert_eq!(requests[0].len(), 3);
    assert_eq!(requests[0][0].role, "system");
    assert_eq!(requests[0][0].text_content(), "protected instructions");
    assert_eq!(requests[0][1].text_content(), "actual prompt");
    assert_eq!(requests[0][2].role, "user");
    assert!(requests[0][2].text_content().contains("data from first"));
    assert!(requests[0][2].text_content().contains('é'));
    let rows = settled(&sink);
    assert_eq!(
        rows.iter().map(|row| row.status).collect::<Vec<_>>(),
        [
            HookStatus::Completed,
            HookStatus::Failed,
            HookStatus::Completed
        ]
    );
}
