//! Host policy at Rig's tool dispatch and outcome boundaries.
use super::{
    tool_results::{SharedToolResults, ToolOutcome},
    SharedToolContext,
};
use crate::engine::hooks::{HookSet, ToolDecision};
use crate::ToolResultContextConfig;
use agent_primitives::CallbackContext;
use rig::agent::{
    AgentHook, CompletionCallAction, CompletionCallEvent, DispatchAction, DispatchEvent,
    HookContext, InvalidToolCallAction, InvalidToolCallContext, OutcomeAction, OutcomeEvent,
};
use serde_json::{json, Value};

pub(super) struct RigExecutionHook {
    pub ctx: SharedToolContext,
    pub hooks: std::sync::Arc<HookSet>,
    pub results: SharedToolResults,
    pub context_config: ToolResultContextConfig,
    pub events: Option<ToolLifecycleSender>,
    pub stop: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}
impl AgentHook for RigExecutionHook {
    async fn on_completion_call(
        &self,
        _: &HookContext,
        _: CompletionCallEvent<'_>,
    ) -> CompletionCallAction {
        self.ctx
            .set_state("app:delegation_active".into(), Value::Bool(false));
        CompletionCallAction::Continue
    }
    async fn on_dispatch(&self, _: &HookContext, event: DispatchEvent<'_>) -> DispatchAction {
        let Some(tool_name) = event.tool_name() else {
            return DispatchAction::Proceed;
        };
        if self
            .stop
            .as_ref()
            .is_some_and(|stop| stop.load(std::sync::atomic::Ordering::Acquire))
        {
            return DispatchAction::stop("Host execution stopped");
        }
        let id = event.call_id.map(ToString::to_string).unwrap_or_default();
        self.ctx.set_function_call_id(id.clone());
        let args = serde_json::from_str::<Value>(event.tool_args().unwrap_or("null"))
            .unwrap_or(Value::Null);
        if self.results.terminal() {
            return DispatchAction::skip("terminal_action_committed");
        }
        if let Some(call_id) = event.call_id {
            let call = rig::completion::message::ToolCall::new(
                call_id.clone(),
                rig::completion::message::ToolFunction::new(
                    rig::completion::message::ToolName::new(tool_name)
                        .expect("validated tool name"),
                    args.clone(),
                ),
            );
            if !self.publish(ToolLifecycle::Start(call)).await {
                return DispatchAction::stop("Host event consumer closed");
            }
        }
        let decision = if self.results.peer_influenced() && tool_name != "respond" {
            ToolDecision::Block {
                reason: "peer_data_authority_boundary".into(),
            }
        } else {
            self.hooks.before_tool(tool_name, &args).await
        };
        if let ToolDecision::Block { reason } = decision {
            let context = json!({"blocked":true,"reason":reason}).to_string();
            self.results.record(
                &id,
                ToolOutcome {
                    raw: Some("[blocked by hook]".into()),
                    error: Some("blocked_by_hook".into()),
                    context: Some(context.clone()),
                    ..Default::default()
                },
            );
            return DispatchAction::skip(context);
        }
        DispatchAction::Proceed
    }
    async fn on_invalid_tool_call(
        &self,
        _: &HookContext,
        call: &InvalidToolCallContext,
    ) -> Option<InvalidToolCallAction> {
        let args = call
            .args
            .as_deref()
            .and_then(|args| serde_json::from_str::<Value>(args).ok())
            .unwrap_or(Value::Null);
        let error = format!("Tool not found or not allowed: {}", call.tool_name);
        let context = json!({"error":error}).to_string();
        let context = self
            .hooks
            .after_tool(&call.tool_name, &args, &context, false)
            .await
            .unwrap_or(context);
        let id = call
            .tool_call_id
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_default();
        self.results.record(
            &id,
            ToolOutcome {
                raw: Some(String::new()),
                error: Some(error),
                context: Some(context.clone()),
                rejected_call: Some((call.tool_name.clone(), args)),
                ..Default::default()
            },
        );
        if let Some(id) = &call.tool_call_id {
            self.publish_result(id.clone(), &call.tool_name, &context)
                .await;
        }
        Some(InvalidToolCallAction::skip(context))
    }
    async fn on_outcome(&self, _: &HookContext, event: OutcomeEvent<'_>) -> OutcomeAction {
        let Some(name) = event.tool_name() else {
            return OutcomeAction::Proceed;
        };
        let Some(id) = event.call_id else {
            return OutcomeAction::Proceed;
        };
        let id = id.to_string();
        let args = serde_json::from_str::<Value>(event.tool_args().unwrap_or("null"))
            .unwrap_or(Value::Null);
        let mut outcome = self.results.snapshot(&id);
        if outcome.raw.is_none() {
            if let Some(result) = event.tool_result() {
                outcome.raw = Some(result.output().render());
            } else {
                outcome.raw = Some(String::new());
                outcome.error = Some(format!("Tool not found or not allowed: {name}"));
                outcome.rejected_call = Some((name.to_owned(), args.clone()));
            }
        }
        let context = if let Some(context) = outcome.context.clone() {
            context
        } else {
            let succeeded = outcome.error.is_none();
            let context = if succeeded {
                crate::prepare_tool_result_for_context(
                    name,
                    outcome.raw.clone().unwrap_or_default(),
                    &self.context_config,
                )
            } else {
                json!({"error":outcome.error}).to_string()
            };
            self.hooks
                .after_tool(name, &args, &context, succeeded)
                .await
                .unwrap_or(context)
        };
        outcome.context = Some(context.clone());
        self.results.record(&id, outcome);
        self.publish_result(event.call_id.unwrap().clone(), name, &context)
            .await;
        if event.tool_result().is_none() {
            return OutcomeAction::replace(Ok(rig::effect::Outcome::ToolResult {
                result: rig::tool::ToolResult::skipped(context.clone())
                    .with_output(rig::tool::ToolOutput::text(context)),
            }));
        }
        OutcomeAction::rewrite_tool_result(&event, context)
    }
}

/// Observations are acknowledged before Rig can dispatch the next sibling.
/// Dropping the consumer releases every pending acknowledgment.
pub(super) enum ToolLifecycle {
    Start(rig::completion::message::ToolCall),
    Result(rig::completion::message::ToolResult),
}
pub(super) type ToolLifecycleSender =
    tokio::sync::mpsc::UnboundedSender<(ToolLifecycle, tokio::sync::oneshot::Sender<()>)>;
impl RigExecutionHook {
    async fn publish(&self, event: ToolLifecycle) -> bool {
        let Some(sender) = &self.events else {
            return true;
        };
        let (tx, rx) = tokio::sync::oneshot::channel();
        sender.send((event, tx)).is_ok() && rx.await.is_ok()
    }
    async fn publish_result(&self, id: rig::completion::message::CallId, name: &str, text: &str) {
        let result = rig::completion::message::ToolResult {
            call: id,
            name: rig::completion::message::ToolName::new(name).expect("validated tool name"),
            content: vec![rig::completion::message::ToolResultContent::Text(
                rig::completion::message::Text::new(text),
            )],
        };
        self.publish(ToolLifecycle::Result(result)).await;
    }
}
