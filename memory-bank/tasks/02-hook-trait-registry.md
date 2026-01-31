# Task 02: Hook Trait and Registry

## Objective
Create the `Hook` trait that defines how hooks respond, and `HookRegistry` that manages all registered hooks and routes responses.

## Background
After Task 01 created `HookContext` and `HookType`, we need:
1. A trait that hooks implement to handle responses
2. A registry that routes responses to the correct hook based on context

## Current State
- `HookContext` and `HookType` exist in `application/gateway/src/hooks/context.rs`
- No response routing mechanism exists

## Deliverables

### 1. Create `application/gateway/src/hooks/registry.rs`
```rust
use crate::hooks::context::{HookContext, HookType};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Attachment for multimedia responses
#[derive(Clone, Debug)]
pub struct Attachment {
    pub content_type: String,
    pub data: Vec<u8>,
    pub filename: Option<String>,
}

/// Trait that all hooks must implement
#[async_trait]
pub trait Hook: Send + Sync {
    /// Get the hook type this handles
    fn hook_type_name(&self) -> &'static str;

    /// Send a response back through this hook
    async fn respond(
        &self,
        ctx: &HookContext,
        message: &str,
        attachments: Option<Vec<Attachment>>,
    ) -> Result<(), String>;

    /// Check if this hook can handle the given context
    fn can_handle(&self, ctx: &HookContext) -> bool {
        ctx.hook_type.type_name() == self.hook_type_name()
    }
}

/// Registry that manages all hooks and routes responses
pub struct HookRegistry {
    hooks: RwLock<HashMap<String, Arc<dyn Hook>>>,
}

impl HookRegistry {
    pub fn new() -> Self {
        Self {
            hooks: RwLock::new(HashMap::new()),
        }
    }

    /// Register a hook
    pub async fn register(&self, hook: Arc<dyn Hook>) {
        let mut hooks = self.hooks.write().await;
        hooks.insert(hook.hook_type_name().to_string(), hook);
    }

    /// Get a hook by type name
    pub async fn get(&self, type_name: &str) -> Option<Arc<dyn Hook>> {
        let hooks = self.hooks.read().await;
        hooks.get(type_name).cloned()
    }

    /// Route a response to the correct hook based on context
    pub async fn respond(
        &self,
        ctx: &HookContext,
        message: &str,
        attachments: Option<Vec<Attachment>>,
    ) -> Result<(), String> {
        let type_name = ctx.hook_type.type_name();

        let hook = self.get(type_name).await.ok_or_else(|| {
            format!("No hook registered for type: {}", type_name)
        })?;

        if !hook.can_handle(ctx) {
            return Err(format!("Hook cannot handle context: {:?}", ctx.hook_type));
        }

        hook.respond(ctx, message, attachments).await
    }

    /// List all registered hook types
    pub async fn list_hook_types(&self) -> Vec<String> {
        let hooks = self.hooks.read().await;
        hooks.keys().cloned().collect()
    }
}

impl Default for HookRegistry {
    fn default() -> Self {
        Self::new()
    }
}
```

### 2. Update `application/gateway/src/hooks/mod.rs`
```rust
mod context;
mod registry;

pub use context::{HookContext, HookType};
pub use registry::{Attachment, Hook, HookRegistry};
```

### 3. Add to Cargo.toml if not present
```toml
async-trait = "0.1"
```

## Verification
1. Build: `cargo build -p agentzero-gateway`
2. Test registration:
```rust
#[tokio::test]
async fn test_hook_registry() {
    struct TestHook;

    #[async_trait]
    impl Hook for TestHook {
        fn hook_type_name(&self) -> &'static str { "test" }
        async fn respond(&self, _ctx: &HookContext, msg: &str, _: Option<Vec<Attachment>>) -> Result<(), String> {
            println!("Test response: {}", msg);
            Ok(())
        }
    }

    let registry = HookRegistry::new();
    registry.register(Arc::new(TestHook)).await;

    assert!(registry.get("test").await.is_some());
    assert!(registry.get("unknown").await.is_none());
}
```

## Dependencies
- Task 01 complete (HookContext, HookType exist)
- `async-trait` crate

## Next Task
Task 03: Web Hook Implementation
