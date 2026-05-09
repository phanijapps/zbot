//! # Test Support
//!
//! Shared mock InvocationContext for workflow agent tests.
//!
//! Centralizing this avoids per-file boilerplate and the resulting coverage drag.

use std::sync::Arc;

use serde_json::Value;
use zero_core::context::{Session, State};
use zero_core::{
    Agent, CallbackContext, Content, EventActions, InvocationContext, ReadonlyContext, RunConfig,
};

pub(crate) struct EmptyState;

impl State for EmptyState {
    fn get(&self, _key: &str) -> Option<Value> {
        None
    }
    fn set(&mut self, _key: String, _value: Value) {}
    fn all(&self) -> std::collections::HashMap<String, Value> {
        std::collections::HashMap::new()
    }
}

pub(crate) struct EmptySession {
    state: EmptyState,
}

impl Session for EmptySession {
    fn id(&self) -> &str {
        "s"
    }
    fn app_name(&self) -> &str {
        "a"
    }
    fn user_id(&self) -> &str {
        "u"
    }
    fn state(&self) -> &dyn State {
        &self.state
    }
    fn conversation_history(&self) -> Vec<Content> {
        vec![]
    }
}

pub(crate) struct StubCtx {
    agent: Arc<dyn Agent>,
    run_config: RunConfig,
    user_content: Content,
    session: Arc<EmptySession>,
    state_map: std::sync::RwLock<std::collections::HashMap<String, Value>>,
}

impl ReadonlyContext for StubCtx {
    fn invocation_id(&self) -> &str {
        "inv"
    }
    fn agent_name(&self) -> &str {
        "test"
    }
    fn user_id(&self) -> &str {
        "u"
    }
    fn app_name(&self) -> &str {
        "a"
    }
    fn session_id(&self) -> &str {
        "s"
    }
    fn branch(&self) -> &str {
        "main"
    }
    fn user_content(&self) -> &Content {
        &self.user_content
    }
}

impl CallbackContext for StubCtx {
    fn get_state(&self, key: &str) -> Option<Value> {
        self.state_map.read().ok().and_then(|m| m.get(key).cloned())
    }
    fn set_state(&self, key: String, value: Value) {
        if let Ok(mut m) = self.state_map.write() {
            m.insert(key, value);
        }
    }
}

impl InvocationContext for StubCtx {
    fn agent(&self) -> Arc<dyn Agent> {
        Arc::clone(&self.agent)
    }
    fn session(&self) -> Arc<dyn Session> {
        Arc::clone(&self.session) as Arc<dyn Session>
    }
    fn run_config(&self) -> &RunConfig {
        &self.run_config
    }
    fn actions(&self) -> EventActions {
        EventActions::default()
    }
    fn set_actions(&self, _actions: EventActions) {}
    fn end_invocation(&self) {}
    fn ended(&self) -> bool {
        false
    }
    fn add_content(&self, _content: Content) {}
}

/// Build a stub `InvocationContext` for tests.
pub(crate) fn make_ctx(agent: Arc<dyn Agent>) -> Arc<dyn InvocationContext> {
    make_ctx_with_state(agent, std::collections::HashMap::new())
}

/// Build a stub `InvocationContext` with pre-seeded state for tests.
pub(crate) fn make_ctx_with_state(
    agent: Arc<dyn Agent>,
    state: std::collections::HashMap<String, Value>,
) -> Arc<dyn InvocationContext> {
    Arc::new(StubCtx {
        agent,
        run_config: RunConfig::default(),
        user_content: Content::user("hi"),
        session: Arc::new(EmptySession { state: EmptyState }),
        state_map: std::sync::RwLock::new(state),
    }) as Arc<dyn InvocationContext>
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use zero_core::{EventStream, Result};

    struct StubAgent;

    #[async_trait]
    impl Agent for StubAgent {
        fn name(&self) -> &str {
            "stub"
        }
        fn description(&self) -> &str {
            ""
        }
        fn sub_agents(&self) -> &[Arc<dyn Agent>] {
            &[]
        }
        async fn run(&self, _ctx: Arc<dyn InvocationContext>) -> Result<EventStream> {
            use async_stream::stream;
            let s = stream! { yield Ok(zero_core::Event::new("x")); };
            Ok(Box::pin(s))
        }
    }

    #[test]
    fn exercise_stub_ctx_methods() {
        // Run every method of the trait to keep test_support coverage high.
        let agent: Arc<dyn Agent> = Arc::new(StubAgent);
        let ctx = make_ctx(Arc::clone(&agent));
        assert_eq!(ctx.invocation_id(), "inv");
        assert_eq!(ctx.agent_name(), "test");
        assert_eq!(ctx.user_id(), "u");
        assert_eq!(ctx.app_name(), "a");
        assert_eq!(ctx.session_id(), "s");
        assert_eq!(ctx.branch(), "main");
        let _ = ctx.user_content();
        assert!(ctx.get_state("absent").is_none());
        ctx.set_state("k".to_string(), Value::Bool(true));
        let _ = ctx.agent();
        let _ = ctx.session();
        let _ = ctx.run_config();
        let _ = ctx.actions();
        ctx.set_actions(EventActions::default());
        assert!(!ctx.ended());
        ctx.end_invocation();
        ctx.add_content(Content::user("x"));

        // Exercise EmptySession + EmptyState directly
        let mut state = EmptyState;
        assert!(state.get("foo").is_none());
        state.set("k".to_string(), Value::Null);
        assert!(state.all().is_empty());
        let session = EmptySession { state: EmptyState };
        assert_eq!(session.id(), "s");
        assert_eq!(session.app_name(), "a");
        assert_eq!(session.user_id(), "u");
        let _ = session.state();
        assert!(session.conversation_history().is_empty());
    }
}
