//! Private invocation ownership; never serialized into model tool context.
use super::exec_ctx::ExecCtx;
use agent_runtime::external_hooks::{
    HookInvocation, HookInvocationConfig, HookMode, HookRun, HookSnapshot,
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

struct RetainedInvocation {
    owner: Arc<HookInvocation>,
    conversation: String,
}
#[derive(Default)]
pub(crate) struct HookInvocationRegistry {
    entries: Mutex<HashMap<String, RetainedInvocation>>,
    activity_sink: Mutex<Option<Arc<dyn agent_runtime::external_hooks::HookActivitySink>>>,
}
impl HookInvocationRegistry {
    pub(crate) fn set_activity_sink(
        &self,
        sink: Arc<dyn agent_runtime::external_hooks::HookActivitySink>,
    ) {
        *self
            .activity_sink
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(sink);
    }
    pub(crate) fn activity_sink(
        &self,
    ) -> Option<Arc<dyn agent_runtime::external_hooks::HookActivitySink>> {
        self.activity_sink
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
    pub(crate) fn insert(
        &self,
        owner: Arc<HookInvocation>,
        conversation: &str,
    ) -> Result<(), crate::errors::ExecutionError> {
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if entries.len() >= 1024 && !entries.contains_key(owner.id()) {
            return Err("hook_invocation_capacity".to_string().into());
        }
        entries.insert(
            owner.id().to_owned(),
            RetainedInvocation {
                owner,
                conversation: conversation.to_owned(),
            },
        );
        Ok(())
    }
    pub(crate) fn get(&self, id: &str, session: &str) -> Option<Arc<HookInvocation>> {
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(id)
            .map(|entry| &entry.owner)
            .filter(|owner| owner.session_id() == session && !owner.cancelled())
            .cloned()
    }
    pub(crate) fn cancel_conversation(&self, conversation: &str) -> Vec<Arc<HookInvocation>> {
        let mut cancelled = Vec::new();
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .retain(|_, entry| {
                if entry.conversation == conversation {
                    entry.owner.cancel();
                    cancelled.push(entry.owner.clone());
                    false
                } else {
                    true
                }
            });
        cancelled
    }
    pub(crate) fn cancel_session(&self, session: &str) {
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .retain(|_, entry| {
                if entry.owner.session_id() == session {
                    entry.owner.cancel();
                    false
                } else {
                    true
                }
            });
    }
    pub(crate) fn remove(&self, id: &str) {
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(id);
    }
}

pub(crate) fn mode(chat: bool) -> HookMode {
    if chat {
        HookMode::Chat
    } else {
        HookMode::Research
    }
}

fn secrets(ctx: &ExecCtx) -> Option<Vec<String>> {
    let mut values = ctx
        .provider_service
        .list()
        .ok()?
        .into_iter()
        .map(|provider| provider.api_key)
        .collect::<Vec<_>>();
    for server in ctx.mcp_service.list().ok()? {
        let runtime = ctx.mcp_service.get_for_runtime(&server.id()).ok()?;
        match runtime {
            agent_runtime::mcp::McpServerConfig::Stdio { env: Some(env), .. } => {
                values.extend(env.into_values())
            }
            agent_runtime::mcp::McpServerConfig::Http {
                headers: Some(headers),
                ..
            }
            | agent_runtime::mcp::McpServerConfig::Sse {
                headers: Some(headers),
                ..
            } => values.extend(headers.into_values()),
            _ => {}
        }
        if let Some(token) = ctx.mcp_service.get_oauth_token(&server.id()).ok()? {
            values.push(token.access_token);
            values.extend(token.refresh_token);
            values.extend(token.client_secret);
        }
    }
    values.retain(|value| !value.is_empty());
    Some(values)
}

pub(super) struct AcceptedHooks {
    pub run: Option<Arc<HookRun>>,
    pub ingress_required: bool,
    pub session_start: bool,
    pub retention: HookRetention,
}
pub(super) struct AcceptHooks<'a> {
    pub snapshot: Arc<HookSnapshot>,
    pub session: &'a str,
    pub execution: &'a str,
    pub message: &'a str,
    pub config: &'a crate::config::ExecutionConfig,
    pub handle: &'a crate::ExecutionHandle,
    pub resume: bool,
}
pub(super) fn accept(
    ctx: &ExecCtx,
    args: AcceptHooks<'_>,
) -> Result<AcceptedHooks, crate::errors::ExecutionError> {
    let AcceptHooks {
        snapshot,
        session,
        execution,
        message,
        config,
        handle,
        resume,
    } = args;
    if resume {
        if let Some((_, revision)) = ctx
            .session_meta
            .hook_invocation_identity(session)
            .map_err(|_| crate::errors::ExecutionError::Config("hook_acceptance_failed".into()))?
        {
            if revision != snapshot.revision() {
                return Err("hook_resume_revision_changed".to_string().into());
            }
        } else if snapshot.hooks().iter().any(|hook| hook.enabled) {
            return Err("hook_resume_identity_unavailable".to_string().into());
        }
    }
    if !snapshot.hooks().iter().any(|hook| hook.enabled) {
        if !resume
            && ctx
                .session_meta
                .hook_invocation_identity(session)
                .ok()
                .flatten()
                .is_some()
        {
            ctx.session_meta
                .record_hook_invocation_identity(session, None)
                .map_err(|_| {
                    crate::errors::ExecutionError::Config("hook_acceptance_failed".into())
                })?;
        }
        return Ok(AcceptedHooks {
            run: None,
            ingress_required: false,
            session_start: false,
            retention: HookRetention::new(ctx.hook_invocations.clone(), None),
        });
    }
    let claim = ctx
        .session_meta
        .claim_hook_invocation(
            session,
            message,
            snapshot.revision(),
            &uuid::Uuid::new_v4().to_string(),
            resume,
        )
        .map_err(|_| crate::errors::ExecutionError::Config("hook_acceptance_failed".into()))?;
    let owner = if let Some(owner) = ctx.hook_invocations.get(&claim.invocation_id, session) {
        owner
    } else {
        let store = ctx.session_meta.clone();
        let budget_session = session.to_owned();
        let budget_invocation = claim.invocation_id.clone();
        HookInvocation::new(HookInvocationConfig {
            consumed_context_bytes: claim.consumed_context_bytes,
            context_checkpoint: Some(Arc::new(move |used| {
                store
                    .checkpoint_hook_context_bytes(&budget_session, &budget_invocation, used)
                    .map_err(|_| agent_runtime::external_hooks::HookFailure::ContextCheckpoint)
            })),
            invocation_id: claim.invocation_id,
            session_id: session.to_owned(),
            snapshot,
            paths: ctx.paths.as_ref().clone(),
            forbidden_roots: vec![ctx.paths.wards_dir()],
            registered_secrets: secrets(ctx),
            activity_sink: ctx.hook_invocations.activity_sink(),
        })
    };
    ctx.hook_invocations
        .insert(owner.clone(), &config.conversation_id)?;
    Ok(AcceptedHooks {
        run: Some(owner.run(
            config.agent_id.clone(),
            execution.to_owned(),
            mode(config.is_chat_mode()),
            handle.stop_signal(),
        )),
        ingress_required: claim.ingress_required,
        session_start: claim.session_start,
        retention: HookRetention::new(ctx.hook_invocations.clone(), Some(owner)),
    })
}

pub(super) fn clear_if_terminal(ctx: &ExecCtx, owner: Option<&Arc<HookInvocation>>, stopped: bool) {
    let Some(owner) = owner else { return };
    let pending = ctx
        .state_service
        .get_session(owner.session_id())
        .ok()
        .flatten()
        .is_some_and(|session| session.pending_delegations > 0);
    if stopped || !pending {
        ctx.hook_invocations.remove(owner.id());
    }
}

pub(super) async fn resolve(
    ctx: &ExecCtx,
    session: &str,
    invocation_id: Option<&str>,
) -> Result<Option<Arc<HookInvocation>>, crate::errors::ExecutionError> {
    if let Some(id) = invocation_id {
        return ctx
            .hook_invocations
            .get(id, session)
            .map(Some)
            .ok_or_else(|| "hook_invocation_unavailable".to_string().into());
    }
    if let Some((id, _)) = ctx
        .session_meta
        .hook_invocation_identity(session)
        .map_err(|_| crate::errors::ExecutionError::Config("hook_invocation_unavailable".into()))?
    {
        return ctx
            .hook_invocations
            .get(&id, session)
            .map(Some)
            .ok_or_else(|| "hook_invocation_unavailable".to_string().into());
    }
    let snapshot = HookSnapshot::load(&ctx.paths, &[ctx.paths.wards_dir()])
        .await
        .map_err(|_| crate::errors::ExecutionError::Config("hook_configuration_invalid".into()))?;
    if snapshot.hooks().iter().any(|hook| hook.enabled) {
        return Err("hook_invocation_unavailable".to_string().into());
    }
    Ok(None)
}

/// Failed setup or an aborted host task cannot retain secrets indefinitely.
pub(crate) struct HookRetention {
    registry: Arc<HookInvocationRegistry>,
    owner: Option<Arc<HookInvocation>>,
}
impl HookRetention {
    pub(crate) fn new(
        registry: Arc<HookInvocationRegistry>,
        owner: Option<Arc<HookInvocation>>,
    ) -> Self {
        Self { registry, owner }
    }
    pub(crate) fn finish(mut self, retain: bool) {
        if retain {
            self.owner.take();
        }
    }
}
impl Drop for HookRetention {
    fn drop(&mut self) {
        if let Some(owner) = self.owner.take() {
            owner.cancel();
            self.registry.remove(owner.id());
        }
    }
}
