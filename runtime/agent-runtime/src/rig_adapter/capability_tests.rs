//! Adapter-local contract probes for the locked Rig dependency, not another executor.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::model::{HostFrame, HostWire};
use futures::StreamExt;
use rig::agent::{
    AgentBuilder, AgentHook, CompletionCallAction, CompletionCallEvent, HookContext, OutcomeAction,
    OutcomeEvent, RequestPatch,
};
use rig::completion::{CompletionRequest, Usage};
use rig::driver::{Exchange, Opened, Opening, Transport};
use rig::wire::Mode;
use tokio::sync::Notify;
fn metered_usage() -> Usage {
    Usage {
        input_tokens: Some(11),
        output_tokens: Some(3),
        total_tokens: Some(14),
        ..Default::default()
    }
}

#[derive(Clone, Default)]
struct ProbeModel {
    requests: Arc<Mutex<Vec<CompletionRequest>>>,
    pending: bool,
    entered: Arc<Notify>,
    dropped: Arc<Notify>,
}

struct DropNotice(Arc<Notify>);

/// A request-policy wrapper, not another agent loop. Rig still decides when
/// to request a model response and when to execute tools.
#[derive(Clone)]
struct HistoryPolicyModel(ProbeModel);

impl Transport<HostWire> for HistoryPolicyModel {
    fn send(
        &self,
        (mut request, mode): (CompletionRequest, Mode),
        exchange: Exchange,
    ) -> Opening<HostFrame> {
        request.chat_history.retain(|message| {
            !serde_json::to_string(message)
                .unwrap()
                .contains("obsolete-pair")
        });
        self.0.send((request, mode), exchange)
    }
}

impl Drop for DropNotice {
    fn drop(&mut self) {
        self.0.notify_one();
    }
}

impl Transport<HostWire> for ProbeModel {
    fn send(&self, (request, _): (CompletionRequest, Mode), _: Exchange) -> Opening<HostFrame> {
        let this = self.clone();
        Opening::new(async move {
            this.requests.lock().unwrap().push(request);
            if this.pending {
                let notice = DropNotice(this.dropped.clone());
                return Ok(Opened::new(futures::stream::once(async move {
                    let _notice = notice;
                    this.entered.notify_one();
                    futures::future::pending::<Result<HostFrame, rig::error::ProviderError>>().await
                })));
            }
            Ok(Opened::new(futures::stream::iter([
                Ok(HostFrame::Text("answer".into())),
                Ok(HostFrame::End(metered_usage())),
            ])))
        })
    }
}

#[derive(Clone, Default)]
struct RequestPolicy {
    turns: Arc<Mutex<Vec<usize>>>,
    usage: Arc<Mutex<Vec<Usage>>>,
}

impl AgentHook for RequestPolicy {
    async fn on_completion_call(
        &self,
        _: &HookContext,
        event: CompletionCallEvent<'_>,
    ) -> CompletionCallAction {
        self.turns.lock().unwrap().push(event.turn);
        CompletionCallAction::patch(
            RequestPatch::new()
                .preamble("bounded system context")
                .max_tokens(23),
        )
    }
    async fn on_outcome(&self, _: &HookContext, event: OutcomeEvent<'_>) -> OutcomeAction {
        if let Some(response) = event.completion() {
            self.usage.lock().unwrap().push(response.usage);
        }
        OutcomeAction::Proceed
    }
}

#[tokio::test]
async fn rig_applies_request_policy_before_provider_and_exposes_usage() {
    let model = ProbeModel::default();
    let policy = RequestPolicy::default();
    let agent = AgentBuilder::new(rig::Model::new(HostWire, model.clone()).erase())
        .preamble("original context")
        .max_tokens(99)
        .add_hook(policy.clone())
        .build();
    let mut stream = agent.prompt("hello").stream();
    while let Some(item) = stream.next().await {
        item.expect("Rig streaming turn");
    }
    let requests = model.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    // Rig resolves the override into a system history message; the provider
    // request's separate preamble field is intentionally empty.
    assert!(matches!(
        requests[0].chat_history.first(),
        Some(rig::completion::Message::System { content }) if content == "bounded system context"
    ));
    assert_eq!(requests[0].max_tokens, Some(23));
    assert_eq!(*policy.turns.lock().unwrap(), [1]);
    assert_eq!(*policy.usage.lock().unwrap(), [metered_usage()]);
}

#[tokio::test]
async fn dropping_rig_run_releases_a_provider_stream_that_never_yields() {
    let model = ProbeModel {
        pending: true,
        ..ProbeModel::default()
    };
    let agent = AgentBuilder::new(rig::Model::new(HostWire, model.clone()).erase()).build();
    let mut stream = agent.prompt("hello").stream();
    // The provider must actually be polled before cancellation: an unpolled
    // future would make a resource-release assertion vacuous.
    tokio::time::timeout(Duration::from_secs(1), async {
        tokio::select! {
            biased;
            item = stream.next() => panic!("pending provider yielded {item:?}"),
            () = model.entered.notified() => {}
        }
    })
    .await
    .expect("Rig polls the pending provider");
    drop(stream);
    tokio::time::timeout(Duration::from_secs(1), model.dropped.notified())
        .await
        .expect("dropping the run drops its pending provider stream");
    assert_eq!(model.requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn model_boundary_can_apply_history_policy_before_provider_dispatch() {
    use rig::completion::Message;
    let provider = ProbeModel::default();
    let agent =
        AgentBuilder::new(rig::Model::new(HostWire, HistoryPolicyModel(provider.clone())).erase())
            .preamble("preserved system")
            .build();
    let history = vec![
        Message::user("obsolete-pair question"),
        Message::assistant("obsolete-pair answer"),
    ];
    let mut stream = agent.prompt("current question").history(history).stream();
    while let Some(item) = stream.next().await {
        item.expect("Rig turn through request policy");
    }
    let requests = provider.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    let history = serde_json::to_string(&requests[0].chat_history).unwrap();
    assert!(!history.contains("obsolete-pair"));
    assert!(history.contains("preserved system"));
    assert!(history.contains("current question"));
}
