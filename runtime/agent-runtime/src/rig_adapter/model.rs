//! Rig wire and transport over the existing host LlmClient.
use crate::llm::{ChatMessage, LlmClient, StreamCallback, StreamChunk, TokenUsage};
use crate::types::ToolCall as AgentToolCall;
use futures::{channel::mpsc, Stream};
use rig::completion::message::{ToolCall, ToolFunction, ToolName, ToolResultContent};
use rig::completion::{CompletionRequest, Message, ToolDefinition, Usage};
use rig::driver::{Exchange, Opened, Opening, Transport};
use rig::error::{EncodeError, ProviderError};
use rig::operation::{Completion, Finish};
use rig::wire::{Decoder, Descriptor, Flow, Mode, Out, Request, Wire, WireEvent};
use rig::{DynModel, Model};
use serde_json::{json, Value};
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

pub(super) fn usage(usage: Option<&TokenUsage>) -> Usage {
    usage
        .map(|u| Usage {
            input_tokens: Some(u.prompt_tokens.into()),
            output_tokens: Some(u.completion_tokens.into()),
            total_tokens: Some(u.total_tokens.into()),
            cached_input_tokens: u.cached_prompt_tokens.map(Into::into),
            ..Default::default()
        })
        .unwrap_or_default()
}

pub enum HostFrame {
    Text(String),
    Reasoning(String),
    Call(ToolCall),
    End(Usage),
}
#[derive(Clone)]
pub struct HostWire;
pub struct HostDecoder;
impl Wire for HostWire {
    type Op = Completion;
    type Payload = (CompletionRequest, Mode);
    type Frame = HostFrame;
    type Decoder<'id> = HostDecoder;
    fn describe(&self) -> Descriptor<'_> {
        Descriptor::new("agentzero")
    }
    fn encode(&self, request: Request<Self>, mode: Mode) -> Result<Self::Payload, EncodeError> {
        Ok((request, mode))
    }
    fn decoder<'id>(&self) -> Self::Decoder<'id> {
        HostDecoder
    }
}
impl<'id> Decoder<'id, Completion, HostFrame> for HostDecoder {
    type Event = HostFrame;
    fn classify(&self, frame: HostFrame) -> WireEvent<HostFrame> {
        WireEvent::Known(frame)
    }
    fn decode(
        &mut self,
        frame: HostFrame,
        mut out: Out<'id, Completion>,
    ) -> Result<Flow, ProviderError> {
        match frame {
            HostFrame::Text(text) => {
                let part = out.text();
                out.push_text(&part, &text);
                out.close_text(part);
            }
            HostFrame::Reasoning(text) => {
                let part = out.reasoning();
                out.push_reasoning(&part, &text);
                out.close_reasoning(part, Default::default());
            }
            HostFrame::Call(call) => out.tool_call(call)?,
            HostFrame::End(usage) => {
                return Ok(out.end(Finish {
                    usage,
                    ..Default::default()
                }))
            }
        }
        Ok(Flow::More)
    }
}

#[derive(Clone)]
pub struct LlmCompletionModel {
    client: Arc<dyn LlmClient>,
    single_action_mode: bool,
    context_policy: Option<Arc<super::context_policy::ContextPolicy>>,
}
impl LlmCompletionModel {
    #[must_use]
    pub fn new(client: Arc<dyn LlmClient>) -> Self {
        Self {
            client,
            single_action_mode: false,
            context_policy: None,
        }
    }
    pub(super) fn with_single_action_mode(mut self, enabled: bool) -> Self {
        self.single_action_mode = enabled;
        self
    }
    pub(super) fn with_context_policy(
        mut self,
        policy: Arc<super::context_policy::ContextPolicy>,
    ) -> Self {
        self.context_policy = Some(policy);
        self
    }
    pub fn erase(self) -> DynModel<Completion> {
        Model::new(HostWire, self).erase()
    }
}
impl Transport<HostWire> for LlmCompletionModel {
    fn send(&self, (request, mode): (CompletionRequest, Mode), _: Exchange) -> Opening<HostFrame> {
        let this = self.clone();
        Opening::new(async move {
            let tools = convert_tools(&request.tools);
            let super::context_policy::PreparedRequest {
                messages,
                tools,
                acks,
            } = match &this.context_policy {
                Some(policy) => policy.prepare(&request, &tools).await?,
                None => super::context_policy::PreparedRequest {
                    messages: convert_messages(&request),
                    tools,
                    acks: Vec::new(),
                },
            };
            let (tx, rx) = mpsc::unbounded();
            let sender = tx.clone();
            let callback: StreamCallback = Box::new(move |chunk| {
                let frame = match chunk {
                    StreamChunk::Token(t) => HostFrame::Text(t),
                    StreamChunk::Reasoning(t) => HostFrame::Reasoning(t),
                    StreamChunk::ToolCall(_) => return,
                };
                let _ = sender.unbounded_send(Ok(frame));
            });
            let task = tokio::spawn(async move {
                let streaming = matches!(mode, Mode::Streaming);
                let response = if streaming {
                    this.client.chat_stream(messages, tools, callback).await
                } else {
                    this.client
                        .chat_with_schema(
                            messages,
                            tools,
                            request
                                .output_schema
                                .map(|schema| schema.as_value().clone()),
                        )
                        .await
                };
                match response {
                    Ok(mut response) => {
                        if let Some(policy) = &this.context_policy {
                            policy.record_usage(
                                response.usage.as_ref().map(|u| u.prompt_tokens.into()),
                            );
                        }
                        for ack in acks {
                            let _ = ack.send(());
                        }
                        if !streaming && !response.content.is_empty() {
                            let _ = tx.unbounded_send(Ok(HostFrame::Text(response.content)));
                        }
                        if !streaming {
                            if let Some(reasoning) = response.reasoning {
                                let _ = tx.unbounded_send(Ok(HostFrame::Reasoning(reasoning)));
                            }
                        }
                        if this.single_action_mode {
                            if let Some(calls) = &mut response.tool_calls {
                                calls.truncate(1);
                            }
                        }
                        for call in response.tool_calls.unwrap_or_default() {
                            match host_tool_call(call) {
                                Ok(call) => {
                                    let _ = tx.unbounded_send(Ok(HostFrame::Call(call)));
                                }
                                Err(error) => {
                                    let _ = tx.unbounded_send(Err(ProviderError::Request(
                                        Arc::new(error),
                                    )));
                                    return;
                                }
                            }
                        }
                        let _ =
                            tx.unbounded_send(Ok(HostFrame::End(usage(response.usage.as_ref()))));
                    }
                    Err(error) => {
                        let _ = tx.unbounded_send(Err(ProviderError::Request(Arc::new(
                            HostProviderError::new(error),
                        ))));
                    }
                }
            });
            Ok(Opened::new(ProviderStream {
                receiver: rx,
                task,
                joined: false,
            }))
        })
    }
}
struct ProviderStream {
    receiver: mpsc::UnboundedReceiver<Result<HostFrame, ProviderError>>,
    task: tokio::task::JoinHandle<()>,
    joined: bool,
}
impl Stream for ProviderStream {
    type Item = Result<HostFrame, ProviderError>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match Pin::new(&mut self.receiver).poll_next(cx) {
            Poll::Ready(None) if !self.joined => match Pin::new(&mut self.task).poll(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(result) => {
                    self.joined = true;
                    if result.is_err() {
                        Poll::Ready(Some(Err(ProviderError::Response(
                            "Provider task failed".into(),
                        ))))
                    } else {
                        Poll::Ready(None)
                    }
                }
            },
            item => item,
        }
    }
}
impl Drop for ProviderStream {
    fn drop(&mut self) {
        self.task.abort();
    }
}
pub(super) fn host_tool_call(
    call: AgentToolCall,
) -> Result<ToolCall, rig::completion::message::EmptyToolName> {
    let name = ToolName::new(call.name)?;
    Ok(ToolCall::from_wire(
        call.id,
        ToolFunction::new(name, call.arguments),
    ))
}
/// Convert a Rig [`CompletionRequest`] into AgentZero chat messages.
///
/// Preamble is already folded into a leading system message by Rig's request
/// builder, so it needs no separate handling here. Assistant tool calls and
/// tool results are bridged faithfully: a rig assistant `ToolCall` becomes an
/// AgentZero assistant message with `tool_calls`, and a rig user `ToolResult`
/// becomes an AgentZero `role:"tool"` message whose `tool_call_id` matches the
/// originating call. Without this, OpenAI-compatible providers (DeepSeek, GLM,
/// OpenAI) reject the orphaned tool call as a malformed prompt.
pub(crate) fn convert_messages(request: &CompletionRequest) -> Vec<ChatMessage> {
    convert_rig_messages(request.chat_history.iter())
}

pub(super) fn convert_rig_messages<'a>(
    messages: impl IntoIterator<Item = &'a Message>,
) -> Vec<ChatMessage> {
    use agent_primitives::types::Part;
    use rig::completion::message::{AssistantContent, UserContent};

    let mut out = Vec::new();
    for message in messages {
        match message {
            Message::System { content } => out.push(ChatMessage::system(content.clone())),

            Message::User { content } => {
                // A rig user message may carry text and/or tool results. Each
                // tool result becomes its own `role:"tool"` message (OpenAI shape),
                // paired by id with the assistant tool call that requested it.
                for part in content.iter() {
                    match part {
                        UserContent::Text(t) => out.push(ChatMessage::user(t.text.clone())),
                        UserContent::ToolResult(tool_result) => {
                            let text = tool_result
                                .content
                                .iter()
                                .filter_map(|c| match c {
                                    ToolResultContent::Text(t) => Some(t.text.clone()),
                                    _ => None,
                                })
                                .collect::<Vec<_>>()
                                .join("\n");
                            let id = tool_result.call.wire().into_owned();
                            out.push(ChatMessage::tool_result(id, text));
                        }
                        // Images / audio / documents are not yet bridged onto the wire.
                        _ => {}
                    }
                }
            }

            Message::Assistant { content, .. } => {
                let mut text_parts: Vec<String> = Vec::new();
                let mut tool_calls: Vec<AgentToolCall> = Vec::new();
                for part in content.iter() {
                    match part {
                        AssistantContent::Text(t) => text_parts.push(t.text.clone()),
                        AssistantContent::ToolCall(tc) => {
                            tool_calls.push(AgentToolCall {
                                id: tc.id.wire().into_owned(),
                                name: tc.function.name.to_string(),
                                arguments: tc.function.arguments.clone(),
                            });
                        }
                        _ => {}
                    }
                }
                // Providers reject empty assistant content; only emit a content
                // part when there is text. A tool-call-only turn yields an
                // assistant message with empty content + tool_calls.
                let content_parts: Vec<Part> = text_parts
                    .into_iter()
                    .map(|t| Part::Text { text: t })
                    .collect();
                if content_parts.is_empty() && tool_calls.is_empty() {
                    continue;
                }
                out.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: content_parts,
                    tool_calls: if tool_calls.is_empty() {
                        None
                    } else {
                        Some(tool_calls)
                    },
                    tool_call_id: None,
                    is_summary: false,
                });
            }
        }
    }
    out
}

/// Convert Rig tool definitions into the OpenAI-compatible `tools` payload the
/// AgentZero client expects.
pub(crate) fn convert_tools(tools: &[ToolDefinition]) -> Option<Value> {
    if tools.is_empty() {
        return None;
    }
    Some(Value::Array(
        tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters }
                })
            })
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{ChatResponse, LlmError};
    use async_trait::async_trait;
    use futures::StreamExt;
    use rig::completion::AssistantContent;
    use rig::streaming::{Item, StreamEvent as RigStreamEvent};
    use std::sync::Mutex;

    type SeenMessages = Arc<Mutex<Vec<Vec<ChatMessage>>>>;

    /// Stub AgentZero LlmClient that streams canned text then resolves.
    struct StubLlm {
        chunks: Vec<String>,
        final_text: String,
        tool_calls: Vec<AgentToolCall>,
        seen: SeenMessages,
        seen_schema: Arc<Mutex<Vec<Option<Value>>>>,
    }

    impl StubLlm {
        fn text(chunks: &[&str]) -> (Arc<Self>, SeenMessages) {
            let seen = Arc::new(Mutex::new(Vec::new()));
            let stub = Arc::new(Self {
                chunks: chunks.iter().map(|c| (*c).to_string()).collect(),
                final_text: chunks.join(""),
                tool_calls: Vec::new(),
                seen: seen.clone(),
                seen_schema: Arc::new(Mutex::new(Vec::new())),
            });
            (stub, seen)
        }
    }

    #[async_trait]
    impl LlmClient for StubLlm {
        fn model(&self) -> &str {
            "stub"
        }
        fn provider(&self) -> &str {
            "stub"
        }
        async fn chat(
            &self,
            messages: Vec<ChatMessage>,
            _tools: Option<Value>,
        ) -> Result<ChatResponse, LlmError> {
            self.seen.lock().unwrap().push(messages);
            self.seen_schema.lock().unwrap().push(None);
            Ok(ChatResponse {
                content: self.final_text.clone(),
                tool_calls: None,
                reasoning: None,
                usage: None,
            })
        }
        async fn chat_with_schema(
            &self,
            messages: Vec<ChatMessage>,
            _tools: Option<Value>,
            output_schema: Option<Value>,
        ) -> Result<ChatResponse, LlmError> {
            self.seen.lock().unwrap().push(messages);
            self.seen_schema.lock().unwrap().push(output_schema);
            Ok(ChatResponse {
                content: self.final_text.clone(),
                tool_calls: None,
                reasoning: None,
                usage: None,
            })
        }
        async fn chat_stream(
            &self,
            messages: Vec<ChatMessage>,
            _tools: Option<Value>,
            callback: StreamCallback,
        ) -> Result<ChatResponse, LlmError> {
            self.seen.lock().unwrap().push(messages);
            for chunk in &self.chunks {
                callback(StreamChunk::Token(chunk.clone()));
            }
            Ok(ChatResponse {
                content: self.final_text.clone(),
                tool_calls: if self.tool_calls.is_empty() {
                    None
                } else {
                    Some(self.tool_calls.clone())
                },
                reasoning: None,
                usage: None,
            })
        }
    }

    fn agent_tool_call(id: &str, name: &str, args: Value) -> AgentToolCall {
        AgentToolCall {
            id: id.to_string(),
            name: name.to_string(),
            arguments: args,
        }
    }

    #[tokio::test]
    async fn bridge_streams_text_tokens_then_final() {
        let (stub, _seen) = StubLlm::text(&["hel", "lo"]);
        let model = LlmCompletionModel::new(stub as Arc<dyn LlmClient>).erase();

        let stream = model
            .stream(rig_request("hi"))
            .expect("stream should build");

        let mut text = String::new();
        let mut s = stream;
        while let Some(item) = s.next().await {
            if let Item::Event(RigStreamEvent::Text { text: t, .. }) = item.expect("chunk") {
                text.push_str(&t);
            }
        }
        assert_eq!(text, "hello");
        s.finish().await.expect("provider completed the response");
    }

    #[tokio::test]
    async fn bridge_forwards_complete_tool_calls_after_text() {
        let stub = Arc::new(StubLlm {
            chunks: vec!["think".to_string()],
            final_text: "think".to_string(),
            tool_calls: vec![agent_tool_call("call_1", "calculator", json!({"x": 1}))],
            seen: Arc::new(Mutex::new(Vec::new())),
            seen_schema: Arc::new(Mutex::new(Vec::new())),
        });
        let model = LlmCompletionModel::new(stub as Arc<dyn LlmClient>).erase();

        let stream = model.stream(rig_request("use tool")).expect("stream");
        let mut tool_calls = Vec::new();
        let mut s = stream;
        while let Some(item) = s.next().await {
            if let Item::Event(RigStreamEvent::End {
                content: AssistantContent::ToolCall(tool_call),
                ..
            }) = item.expect("chunk")
            {
                tool_calls.push((
                    tool_call.function.name.to_string(),
                    tool_call.function.arguments,
                    tool_call.id.to_string(),
                    tool_call.id.wire().into_owned(),
                ));
            }
        }
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].0, "calculator");
        assert_eq!(tool_calls[0].1, json!({"x": 1}));
        assert_eq!(tool_calls[0].2, "call_1");
        assert_eq!(tool_calls[0].3, "call_1");
    }

    #[tokio::test]
    async fn bridge_surfaces_llm_errors_as_provider_error() {
        struct ErrorLlm;
        #[async_trait]
        impl LlmClient for ErrorLlm {
            fn model(&self) -> &str {
                "err"
            }
            fn provider(&self) -> &str {
                "err"
            }
            async fn chat(
                &self,
                _: Vec<ChatMessage>,
                _: Option<Value>,
            ) -> Result<ChatResponse, LlmError> {
                Err(LlmError::ApiError("boom".to_string()))
            }
            async fn chat_stream(
                &self,
                _: Vec<ChatMessage>,
                _: Option<Value>,
                _: StreamCallback,
            ) -> Result<ChatResponse, LlmError> {
                Err(LlmError::ApiError("boom".to_string()))
            }
        }
        let model = LlmCompletionModel::new(Arc::new(ErrorLlm) as Arc<dyn LlmClient>).erase();
        let mut stream = model.stream(rig_request("hi")).expect("stream");
        match stream.next().await.expect("an item") {
            Err(ProviderError::Request(error)) => assert!(error.to_string().contains("boom")),
            other => panic!("expected provider error, got {other:?}"),
        }
    }

    fn rig_request(prompt: &str) -> CompletionRequest {
        CompletionRequest::new(prompt)
    }

    #[tokio::test]
    async fn dropping_bridge_stream_aborts_pending_provider() {
        struct PendingLlm {
            entered: Arc<tokio::sync::Notify>,
            dropped: Arc<tokio::sync::Notify>,
        }
        struct DropNotice(Arc<tokio::sync::Notify>);
        impl Drop for DropNotice {
            fn drop(&mut self) {
                self.0.notify_one();
            }
        }
        #[async_trait]
        impl LlmClient for PendingLlm {
            fn model(&self) -> &str {
                "pending"
            }
            fn provider(&self) -> &str {
                "fixture"
            }
            async fn chat(
                &self,
                _: Vec<ChatMessage>,
                _: Option<Value>,
            ) -> Result<ChatResponse, LlmError> {
                panic!("streaming fixture");
            }
            async fn chat_stream(
                &self,
                _: Vec<ChatMessage>,
                _: Option<Value>,
                _: StreamCallback,
            ) -> Result<ChatResponse, LlmError> {
                let _notice = DropNotice(self.dropped.clone());
                self.entered.notify_one();
                futures::future::pending().await
            }
        }
        let entered = Arc::new(tokio::sync::Notify::new());
        let dropped = Arc::new(tokio::sync::Notify::new());
        let model = LlmCompletionModel::new(Arc::new(PendingLlm {
            entered: entered.clone(),
            dropped: dropped.clone(),
        }))
        .erase();
        let mut stream = model.stream(rig_request("hello")).unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            tokio::select! { item = stream.next() => panic!("pending provider yielded {item:?}"), () = entered.notified() => {} }
        })
            .await
            .expect("provider task must actually start");
        drop(stream);
        tokio::time::timeout(std::time::Duration::from_secs(1), dropped.notified())
            .await
            .expect("stream owns and cancels the pending provider task");
    }

    fn invalid_tool_request() -> CompletionRequest {
        let mut request = rig_request("hello");
        request.tools = vec![ToolDefinition {
            name: "x".repeat(65),
            description: "invalid provider tool name".to_string(),
            parameters: json!({"type": "object"}),
        }];
        request
    }

    #[tokio::test]
    async fn completion_and_stream_reject_invalid_tools_before_network_io() {
        let config = crate::llm::LlmConfig::new(
            "https://api.example.invalid".to_string(),
            "test-key".to_string(),
            "gpt-4-turbo".to_string(),
            "openai".to_string(),
        );
        let client = crate::llm::OpenAiClient::new(config).expect("test OpenAI client");
        let model = LlmCompletionModel::new(Arc::new(client) as Arc<dyn LlmClient>).erase();

        let completion = model
            .call(invalid_tool_request())
            .await
            .expect_err("Rig completion must surface local validation");
        assert!(completion
            .to_string()
            .contains("tool_schema_rule=function_name"));

        let mut stream = model
            .stream(invalid_tool_request())
            .expect("Rig stream should initialize");
        let stream_error = stream
            .next()
            .await
            .expect("Rig stream must surface an error")
            .expect_err("Rig stream must reject the invalid tool");
        assert!(stream_error
            .to_string()
            .contains("tool_schema_rule=function_name"));
    }

    #[test]
    fn convert_messages_bridges_tool_calls_and_results() {
        // Regression: a history with an assistant tool call + its tool result
        // must produce a valid OpenAI message chain (assistant.tool_calls paired
        // with a role:"tool" message whose tool_call_id matches). Dropping the
        // tool result made strict providers (DeepSeek/GLM) reject the request.
        use rig::completion::message::Text as RigText;
        use rig::completion::message::{
            AssistantContent, ToolResult as RigToolResult, UserContent,
        };

        let call = host_tool_call(agent_tool_call("call_1", "echo", json!({"x":1}))).unwrap();
        let assistant_call = Message::Assistant {
            id: None,
            content: vec![AssistantContent::ToolCall(call.clone())],
        };
        let tool_result = Message::User {
            content: vec![UserContent::ToolResult(RigToolResult {
                call: call.id,
                name: call.function.name,
                content: vec![ToolResultContent::Text(RigText::new("echo-result"))],
            })],
        };
        let mut request = rig_request("please echo");
        request.chat_history.extend([assistant_call, tool_result]);

        let msgs = convert_messages(&request);

        let assistant = msgs
            .iter()
            .find(|m| m.role == "assistant")
            .expect("assistant message present");
        let tool_calls = assistant.tool_calls.as_ref().expect("assistant tool_calls");
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].id, "call_1");
        assert_eq!(tool_calls[0].name, "echo");
        assert_eq!(tool_calls[0].arguments, json!({"x": 1}));

        let tool = msgs
            .iter()
            .find(|m| m.role == "tool")
            .expect("tool result message present");
        // The tool result's tool_call_id must match the assistant's tool call id.
        assert_eq!(tool.tool_call_id.as_deref(), Some("call_1"));
        assert!(tool.text_content().contains("echo-result"));
    }

    #[tokio::test]
    async fn completion_forwards_output_schema_to_llm_client() {
        let (stub, _seen) = StubLlm::text(&[r#"{"value":"ok"}"#]);
        let schema = json!({
            "type": "object",
            "properties": {
                "value": { "type": "string" }
            },
            "required": ["value"]
        });
        let model = LlmCompletionModel::new(stub.clone() as Arc<dyn LlmClient>).erase();
        let mut request = rig_request("typed");
        request.output_schema = Some(schemars::Schema::try_from(schema.clone()).unwrap());

        let response = model.call(request).await.expect("completion");
        assert_eq!(response.usage.input_tokens, None);
        assert_eq!(response.usage.output_tokens, None);
        assert_eq!(response.usage.total_tokens, None);

        let seen_schema = stub.seen_schema.lock().unwrap();
        assert_eq!(seen_schema.as_slice(), &[Some(schema)]);
    }
}

/// Own a non-cloneable provider error across Rig's shared error reports.
#[derive(Debug)]
pub(super) struct HostProviderError {
    message: String,
    error: std::sync::Mutex<Option<crate::llm::LlmError>>,
}
impl HostProviderError {
    fn new(error: crate::llm::LlmError) -> Self {
        Self {
            message: error.to_string(),
            error: std::sync::Mutex::new(Some(error)),
        }
    }
    pub fn take(&self) -> Option<crate::llm::LlmError> {
        self.error.lock().unwrap().take()
    }
}
impl std::fmt::Display for HostProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for HostProviderError {}

impl From<LlmCompletionModel> for DynModel<Completion> {
    fn from(model: LlmCompletionModel) -> Self {
        model.erase()
    }
}
