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
    HookContext, InvalidToolCallAction, InvalidToolCallContext, InvalidToolCallReason,
    OutcomeAction, OutcomeEvent, RunStart, RunStartAction,
};
use serde_json::{json, Value};

pub(super) struct RigExecutionHook {
    pub ctx: SharedToolContext,
    pub hooks: std::sync::Arc<HookSet>,
    pub results: SharedToolResults,
    pub context_config: ToolResultContextConfig,
    pub events: Option<ToolLifecycleSender>,
    pub stop: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    pub external_hooks: Option<std::sync::Arc<crate::external_hooks::HookRun>>,
}
impl RigExecutionHook {
    async fn invalid_call_action(
        &self,
        call: &InvalidToolCallContext,
    ) -> Option<InvalidToolCallAction> {
        if let Some(run) = &self.external_hooks {
            let category = match call.reason {
                InvalidToolCallReason::UnknownTool => {
                    crate::external_hooks::InvalidToolCategory::UnknownTool
                }
                InvalidToolCallReason::MalformedArguments { .. } => {
                    crate::external_hooks::InvalidToolCategory::InvalidArguments
                }
            };
            if run
                .invalid_tool(
                    call.tool_call_id.as_ref().map(ToString::to_string),
                    Some(call.tool_name.clone()),
                    category,
                )
                .await
            {
                return Some(InvalidToolCallAction::stop(
                    "Invalid tool call blocked by external hook",
                ));
            }
        }
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
}

impl AgentHook for RigExecutionHook {
    async fn on_run_start(&self, ctx: &HookContext, _: RunStart<'_>) -> RunStartAction {
        if let Some(run) = &self.external_hooks {
            if run.start(ctx.run_id().to_string()).await {
                return RunStartAction::Stop("Run blocked by external hook".into());
            }
        }
        RunStartAction::Continue
    }
    async fn on_completion_call(
        &self,
        ctx: &HookContext,
        _: CompletionCallEvent<'_>,
    ) -> CompletionCallAction {
        if let Some(run) = &self.external_hooks {
            run.set_turn(ctx.run_id().to_string(), ctx.turn());
        }
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
        if let Some(run) = &self.external_hooks {
            if !args.is_object() && !args.is_null() {
                if run
                    .invalid_tool(
                        Some(id.clone()),
                        Some(tool_name.to_owned()),
                        crate::external_hooks::InvalidToolCategory::InvalidArguments,
                    )
                    .await
                {
                    return DispatchAction::stop("Invalid tool call blocked by external hook");
                }
                let context = json!({"error":"Invalid tool arguments"}).to_string();
                self.results.record(
                    &id,
                    ToolOutcome {
                        raw: Some(String::new()),
                        error: Some("Invalid tool arguments".into()),
                        context: Some(context.clone()),
                        rejected_call: Some((tool_name.to_owned(), args)),
                        ..Default::default()
                    },
                );
                return DispatchAction::skip(context);
            }
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
        let decision = if self.external_hooks.is_some()
            && agent_tools::guards::planning_gate_blocks_tool(self.ctx.as_ref(), tool_name, &args)
        {
            ToolDecision::Block {
                reason: "planning_gate".into(),
            }
        } else if self.results.peer_influenced() && tool_name != "respond" {
            ToolDecision::Block {
                reason: "peer_data_authority_boundary".into(),
            }
        } else {
            self.hooks.before_tool(tool_name, &args).await
        };
        let decision = match decision {
            ToolDecision::Allow => {
                if let Some(run) = &self.external_hooks {
                    if run.before_tool(Some(id.clone()), tool_name, &args).await {
                        ToolDecision::Block {
                            reason: "external_hook_blocked".into(),
                        }
                    } else {
                        ToolDecision::Allow
                    }
                } else {
                    ToolDecision::Allow
                }
            }
            blocked => blocked,
        };
        if let ToolDecision::Block { reason } = decision {
            if let Some(run) = &self.external_hooks {
                if run
                    .invalid_tool(
                        Some(id.clone()),
                        Some(tool_name.to_owned()),
                        crate::external_hooks::InvalidToolCategory::PolicyDenied,
                    )
                    .await
                {
                    return DispatchAction::stop("Invalid tool call blocked by external hook");
                }
            }
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
        self.invalid_call_action(call).await
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
        if event.tool_result().is_some()
            && outcome.error.as_deref() != Some("blocked_by_hook")
            && outcome.rejected_call.is_none()
        {
            if let Some(run) = &self.external_hooks {
                run.after_tool(Some(id.clone()), name, outcome.error.is_none())
                    .await;
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

#[cfg(test)]
mod tests;
