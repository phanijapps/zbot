use super::*;
#[test]
fn response_is_one_strict_object_with_event_legal_effects() {
    for bytes in [
        b" ".as_slice(),
        b"[]",
        b"{}",
        b"{\"version\":2,\"action\":\"continue\"}",
        b"{\"version\":1,\"action\":{\"continue\":null}}",
        b"{\"version\":1,\"action\":\"continue\",\"reason\":null}",
        b"{\"version\":1,\"action\":\"continue\",\"context\":null}",
        b"{\"version\":1,\"action\":\"continue\",\"other\":true}",
        b"{\"version\":1,\"action\":\"continue\"} {}",
        b"\xff",
    ] {
        assert!(HookResponse::parse(bytes, HookEvent::RunStart).is_err());
    }
    assert_eq!(
        HookResponse::parse(b"", HookEvent::RunEnd).unwrap().action,
        HookAction::Continue
    );
    assert_eq!(
        HookResponse::parse(
            br#"{"version":1,"action":"block","reason":"private"}"#,
            HookEvent::BeforeTool
        )
        .unwrap()
        .action,
        HookAction::Block
    );
    assert!(
        HookResponse::parse(br#"{"version":1,"action":"block"}"#, HookEvent::AfterTool).is_err()
    );
    assert!(HookResponse::parse(
        br#"{"version":1,"action":"block","context":""}"#,
        HookEvent::RunStart
    )
    .is_err());
    assert!(HookResponse::parse(
        br#"{"version":1,"action":"continue","context":"hint"}"#,
        HookEvent::RunEnd
    )
    .is_err());
}
#[test]
fn context_budget_is_bytes_across_repeated_events() {
    let bytes = serde_json::to_vec(
        &serde_json::json!({"version":1,"action":"continue","context":"é".repeat(2048)}),
    )
    .unwrap();
    let response = HookResponse::parse(&bytes, HookEvent::BeforeModel).unwrap();
    let mut budget = HookContextBudget::default();
    assert!(budget.accept(&response).is_ok());
    assert!(budget.accept(&response).is_ok());
    assert_eq!(
        budget.accept(&response).unwrap_err(),
        HookFailure::ContextTooLarge
    );
    let oversized = serde_json::to_vec(
        &serde_json::json!({"version":1,"action":"continue","context":"é".repeat(4097)}),
    )
    .unwrap();
    assert!(HookResponse::parse(&oversized, HookEvent::RunStart).is_err());
}
#[test]
fn envelope_is_schema_shaped_bounded_and_projection_is_mandatory() {
    let mut event = HookEventPayload::new(
        "inv-1".into(),
        "sess-1".into(),
        "root".into(),
        HookEventData::BeforeTool {
            tool: "lookup".into(),
            arguments: serde_json::json!({"token":"secret","query":"safe"}),
            arguments_redacted: false,
        },
    );
    let wire: Value = serde_json::from_slice(&event.encode(&[]).unwrap()).unwrap();
    assert_eq!(wire["event"], "before_tool");
    assert_eq!(wire["version"], 1);
    assert!(wire["run_id"].is_null());
    assert_eq!(
        wire["data"]["arguments"],
        serde_json::json!({"query":"safe"})
    );
    assert_eq!(wire["data"]["arguments_redacted"], true);
    event.agent_id = "x".repeat(129);
    assert_eq!(event.encode(&[]).unwrap_err(), HookFailure::InvalidEvent);
    let event = HookEventPayload::new(
        "inv-1".into(),
        "sess-1".into(),
        "root".into(),
        HookEventData::UserPrompt {
            prompt: "界".repeat(16384),
        },
    );
    assert!(event.encode(&[]).is_ok());
    let event = HookEventPayload::new(
        "inv-1".into(),
        "sess-1".into(),
        "root".into(),
        HookEventData::UserPrompt {
            prompt: "🧠".repeat(16384),
        },
    );
    assert_eq!(event.encode(&[]).unwrap_err(), HookFailure::InputTooLarge);
}
