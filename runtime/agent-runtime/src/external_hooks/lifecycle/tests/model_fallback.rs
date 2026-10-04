use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

struct Server {
    url: String,
    requests: Arc<Mutex<Vec<serde_json::Value>>>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn broken_stream_server() -> Server {
    broken_stream_server_with_prefix("data: ").await
}
async fn broken_stream_server_with_prefix(prefix: &str) -> Server {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = requests.clone();
    let broken_response=format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: 10000\r\nConnection: close\r\n\r\n{prefix}");
    let task = tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            let mut buffer = [0; 4096];
            let body_start = loop {
                let n = socket.read(&mut buffer).await.unwrap();
                assert_ne!(n, 0);
                bytes.extend_from_slice(&buffer[..n]);
                if let Some(pos) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    break pos + 4;
                }
            };
            let headers = String::from_utf8_lossy(&bytes[..body_start]);
            let length: usize = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse().unwrap())
                })
                .unwrap();
            while bytes.len() < body_start + length {
                let n = socket.read(&mut buffer).await.unwrap();
                assert_ne!(n, 0);
                bytes.extend_from_slice(&buffer[..n]);
            }
            let request: serde_json::Value = serde_json::from_slice(&bytes[body_start..]).unwrap();
            let first = {
                let mut requests = captured.lock().unwrap();
                let first = requests.is_empty();
                requests.push(request);
                first
            };
            if first {
                // A real HTTP body decode error before a complete SSE event.
                socket.write_all(broken_response.as_bytes()).await.unwrap();
            } else {
                let body =
                    json!({"choices":[{"message":{"content":"fallback answer"}}]}).to_string();
                let response = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                socket.write_all(response.as_bytes()).await.unwrap();
            }
            socket.shutdown().await.unwrap();
        }
    });
    Server {
        url,
        requests,
        task,
    }
}
fn raw_client(server: &Server) -> crate::OpenAiClient {
    crate::OpenAiClient::new(crate::LlmConfig::new(
        server.url.clone(),
        "secret".into(),
        "model".into(),
        "provider".into(),
    ))
    .unwrap()
}

#[tokio::test]
async fn streaming_decode_fallback_has_two_actual_model_pairs_and_shared_context_budget() {
    for mode in [
        "ordinary",
        "budget_continue",
        "budget_block",
        "explicit_block",
    ] {
        let server = broken_stream_server().await;
        let (temp, original, sink) =
            invocation(&[("pre", "before_model"), ("post", "after_model")]).await;
        let paths = original.config.paths.clone();
        if mode == "budget_block" {
            let mut config: serde_json::Value =
                serde_json::from_slice(&std::fs::read(paths.hooks_config()).unwrap()).unwrap();
            config["hooks"][0]["on_failure"] = json!("block");
            std::fs::write(paths.hooks_config(), config.to_string()).unwrap();
        }
        let script=format!("import json,sys,pathlib\ne=json.load(sys.stdin)\np=pathlib.Path(__file__).with_name('events.jsonl')\nwith p.open('a') as f: f.write(json.dumps(e)+'\\n')\nr={{'version':1,'action':'continue'}}\nif e['event']=='before_model':\n n=sum(json.loads(l)['event']=='before_model' for l in p.read_text().splitlines())\n if {mode:?}.startswith('budget'): r['context']='é'*3000\n if {mode:?}=='explicit_block' and n==2: r['action']='block'\nprint(json.dumps(r))\n");
        std::fs::write(paths.config_dir().join("observe.py"), script).unwrap();
        let owner = HookInvocation::new(HookInvocationConfig {
            consumed_context_bytes: 0,
            context_checkpoint: None,
            invocation_id: original.id().into(),
            session_id: original.session_id().into(),
            snapshot: HookSnapshot::load(&paths, &[]).await.unwrap(),
            paths,
            forbidden_roots: vec![],
            registered_secrets: Some(vec![]),
            activity_sink: Some(sink.clone()),
        });
        let run = owner.run(
            "agent".into(),
            "exec".into(),
            HookMode::Chat,
            Arc::new(AtomicBool::new(false)),
        );
        let client = HookedLlmClient::new(Arc::new(raw_client(&server)), run.clone());
        let tokens = Arc::new(Mutex::new(Vec::new()));
        let capture = tokens.clone();
        let result = client
            .chat_stream(
                vec![
                    crate::ChatMessage::system("protected".into()),
                    crate::ChatMessage::user("prompt".into()),
                ],
                None,
                Box::new(move |chunk| {
                    if let crate::StreamChunk::Token(text) = chunk {
                        capture.lock().unwrap().push(text)
                    }
                }),
            )
            .await;
        let blocked = matches!(mode, "budget_block" | "explicit_block");
        assert_eq!(result.is_err(), blocked, "{mode}: {result:?}");
        let requests = server.requests.lock().unwrap();
        assert_eq!(requests.len(), if blocked { 1 } else { 2 }, "{mode}");
        assert_eq!(requests[0]["stream"], true);
        if !blocked {
            assert_ne!(requests[1]["stream"], true);
            assert_eq!(*tokens.lock().unwrap(), ["fallback answer"]);
            assert_eq!(requests[1]["messages"][0]["content"], "protected");
            assert_eq!(
                requests[1]["messages"].as_array().unwrap().len(),
                if mode == "budget_continue" { 3 } else { 2 }
            );
        }
        let events: Vec<serde_json::Value> =
            std::fs::read_to_string(temp.path().join("config/events.jsonl"))
                .unwrap()
                .lines()
                .map(|line| serde_json::from_str(line).unwrap())
                .collect();
        assert_eq!(
            events
                .iter()
                .map(|e| e["event"].as_str().unwrap())
                .collect::<Vec<_>>(),
            if blocked {
                vec!["before_model", "after_model", "before_model"]
            } else {
                vec!["before_model", "after_model", "before_model", "after_model"]
            },
            "{mode}"
        );
        assert_eq!(events[1]["data"]["status"], "failed");
        if !blocked {
            assert_eq!(events[3]["data"]["status"], "completed");
        }
        let ids: std::collections::HashSet<_> = events
            .iter()
            .map(|e| e["event_id"].as_str().unwrap())
            .collect();
        assert_eq!(ids.len(), events.len());
        if mode.starts_with("budget") {
            assert_eq!(owner.budget.lock().unwrap().used(), 6000);
            assert!(
                settled(&sink)
                    .iter()
                    .any(|row| row.event == HookEvent::BeforeModel
                        && row.status == HookStatus::Failed)
            );
        }
    }
}

#[tokio::test]
async fn stop_after_failed_stream_prevents_the_nonstream_provider_fallback() {
    let server = broken_stream_server().await;
    let (temp, owner, sink) = invocation(&[("pre", "before_model"), ("post", "after_model")]).await;
    std::fs::write(owner.config.paths.config_dir().join("observe.py"),"import sys,json,pathlib,time\ne=json.load(sys.stdin)\nif e['event']=='after_model':\n pathlib.Path(__file__).with_name('post.started').write_text('started')\n time.sleep(30)\nprint('{\"version\":1,\"action\":\"continue\"}')\n").unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let run = owner.run("agent".into(), "exec".into(), HookMode::Chat, stop.clone());
    let client = HookedLlmClient::new(Arc::new(raw_client(&server)), run.clone());
    let task =
        tokio::spawn(async move { client.chat_stream(vec![], None, Box::new(|_| {})).await });
    let marker = temp.path().join("config/post.started");
    for _ in 0..200 {
        if marker.exists() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(marker.exists());
    stop.store(true, Ordering::Release);
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap()
            .is_err()
    );
    assert_eq!(server.requests.lock().unwrap().len(), 1);
    assert_eq!(
        settled(&sink)
            .iter()
            .filter(|row| row.event == HookEvent::BeforeModel)
            .count(),
        1
    );
}

#[tokio::test]
async fn unhooked_stream_decode_fallback_keeps_existing_behavior() {
    let server = broken_stream_server().await;
    let result = raw_client(&server)
        .chat_stream(vec![], None, Box::new(|_| {}))
        .await
        .unwrap();
    assert_eq!(result.content, "fallback answer");
    assert_eq!(server.requests.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn committed_partial_text_or_tool_deltas_never_trigger_fallback() {
    for delta in [
        json!({"content":"partial answer"}),
        json!({"tool_calls":[{"index":0,"id":"call","function":{"name":"respond","arguments":"{"}}]}),
    ] {
        for hooked in [false, true] {
            let prefix = format!("data: {}\n\n", json!({"choices":[{"delta":delta}]}));
            let server = broken_stream_server_with_prefix(&prefix).await;
            let (_temp, owner, sink) =
                invocation(&[("pre", "before_model"), ("post", "after_model")]).await;
            let run = owner.run(
                "agent".into(),
                "exec".into(),
                HookMode::Chat,
                Arc::new(AtomicBool::new(false)),
            );
            let raw: Arc<dyn crate::LlmClient> = Arc::new(raw_client(&server));
            let client: Arc<dyn crate::LlmClient> = if hooked {
                Arc::new(HookedLlmClient::new(raw, run))
            } else {
                raw
            };
            assert!(matches!(
                client.chat_stream(vec![], None, Box::new(|_| {})).await,
                Err(crate::LlmError::ApiError(_))
            ));
            assert_eq!(server.requests.lock().unwrap().len(), 1);
            assert_eq!(settled(&sink).len(), if hooked { 2 } else { 0 });
        }
    }
}

#[tokio::test]
async fn malformed_sse_is_ignored_before_decode_failure_with_and_without_hooks() {
    for hooked in [false, true] {
        let server = broken_stream_server_with_prefix("data: not-json\n\n").await;
        let (_temp, owner, sink) =
            invocation(&[("pre", "before_model"), ("post", "after_model")]).await;
        let run = owner.run(
            "agent".into(),
            "exec".into(),
            HookMode::Chat,
            Arc::new(AtomicBool::new(false)),
        );
        let raw: Arc<dyn crate::LlmClient> = Arc::new(raw_client(&server));
        let client: Arc<dyn crate::LlmClient> = if hooked {
            Arc::new(HookedLlmClient::new(raw, run))
        } else {
            raw
        };
        assert_eq!(
            client
                .chat_stream(vec![], None, Box::new(|_| {}))
                .await
                .unwrap()
                .content,
            "fallback answer"
        );
        assert_eq!(server.requests.lock().unwrap().len(), 2);
        assert_eq!(settled(&sink).len(), if hooked { 4 } else { 0 });
    }
}
