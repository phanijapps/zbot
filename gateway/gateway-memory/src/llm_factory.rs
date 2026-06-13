//! `MemoryLlmFactory` — abstraction for building LLM clients used by
//! sleep-time memory components.
//!
//! Decouples the production LLM impls in `gateway-memory::sleep` from the
//! concrete `ProviderService` in `gateway-services`. Each component asks the
//! injected factory for a client configured with its preferred temperature
//! and max-tokens; the gateway constructs one factory per process and shares
//! it across every component.

use std::sync::Arc;

use agent_runtime::llm::LlmClient;
use async_trait::async_trait;

/// Per-call configuration each component supplies when asking the factory
/// for a client. Currently captures only the values that vary between
/// production sleep components — temperature and max-tokens.
#[derive(Debug, Clone)]
pub struct LlmClientConfig {
    pub temperature: f64,
    pub max_tokens: u32,
}

impl LlmClientConfig {
    pub fn new(temperature: f64, max_tokens: u32) -> Self {
        Self {
            temperature,
            max_tokens,
        }
    }
}

/// Builds LLM clients on demand for memory sleep components.
///
/// The production implementation lives in `gateway` (where
/// `ProviderService` is defined) to avoid a dependency cycle —
/// `gateway-memory` cannot depend on `gateway-services` because
/// `gateway-services` already depends on `gateway-memory`.
#[async_trait]
pub trait MemoryLlmFactory: Send + Sync {
    /// Build a fresh `LlmClient` configured with the given temperature
    /// and max tokens. Returns a string error on misconfiguration
    /// (no providers, bad credentials, etc.).
    async fn build_client(&self, config: LlmClientConfig) -> Result<Arc<dyn LlmClient>, String>;
}

/// Lazily-cached `LlmClient` bound to a single factory + config (MEM-003).
///
/// Sleep components call `build_client` once per task invocation and use
/// the same `LlmClientConfig` every time — caching the resulting client
/// turns N redundant constructions per cycle into one for the process
/// lifetime. The cache is async-safe via `tokio::sync::OnceCell` so two
/// concurrent first-call paths race-cleanly: only one wins, both observe
/// the same `Arc`.
///
/// Embed one of these per Llm* impl that previously called
/// `factory.build_client(...)` on every method invocation. Misconfigured
/// factories still surface as `Err` (`OnceCell::get_or_try_init` doesn't
/// memoize errors).
pub struct CachedLlmClient {
    factory: Arc<dyn MemoryLlmFactory>,
    config: LlmClientConfig,
    client: tokio::sync::OnceCell<Arc<dyn LlmClient>>,
}

impl CachedLlmClient {
    pub fn new(factory: Arc<dyn MemoryLlmFactory>, config: LlmClientConfig) -> Self {
        Self {
            factory,
            config,
            client: tokio::sync::OnceCell::new(),
        }
    }

    /// Return the cached client, building it via the factory on first
    /// call. Errors are not memoized — a transient factory failure can
    /// be retried by the caller and the next call will try to build
    /// again.
    pub async fn get(&self) -> Result<Arc<dyn LlmClient>, String> {
        self.client
            .get_or_try_init(|| async { self.factory.build_client(self.config.clone()).await })
            .await
            .map(Arc::clone)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_runtime::llm::client::StreamCallback;
    use agent_runtime::llm::{ChatMessage, ChatResponse, LlmError};
    use async_trait::async_trait;
    use serde_json::Value;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct StubLlm;

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
            _msgs: Vec<ChatMessage>,
            _tools: Option<Value>,
        ) -> Result<ChatResponse, LlmError> {
            Err(LlmError::ApiError("test stub".into()))
        }
        async fn chat_stream(
            &self,
            _msgs: Vec<ChatMessage>,
            _tools: Option<Value>,
            _cb: StreamCallback,
        ) -> Result<ChatResponse, LlmError> {
            Err(LlmError::ApiError("test stub".into()))
        }
    }

    /// Counting factory: increments on every `build_client` call.
    /// Returns `Ok` so the cache initializes once and stays populated.
    struct CountingFactory {
        calls: AtomicUsize,
    }

    impl CountingFactory {
        fn new() -> Self {
            Self {
                calls: AtomicUsize::new(0),
            }
        }
        fn call_count(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl MemoryLlmFactory for CountingFactory {
        async fn build_client(
            &self,
            _config: LlmClientConfig,
        ) -> Result<Arc<dyn LlmClient>, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(Arc::new(StubLlm))
        }
    }

    /// Factory that always fails. Used to confirm errors are NOT
    /// memoized — `CachedLlmClient::get` should retry on next call.
    struct FailingFactory {
        calls: AtomicUsize,
    }

    impl FailingFactory {
        fn new() -> Self {
            Self {
                calls: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl MemoryLlmFactory for FailingFactory {
        async fn build_client(
            &self,
            _config: LlmClientConfig,
        ) -> Result<Arc<dyn LlmClient>, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Err("synthetic build failure".into())
        }
    }

    #[tokio::test]
    async fn cached_llm_client_calls_factory_once_across_many_get_calls() {
        let factory = Arc::new(CountingFactory::new());
        let cached = CachedLlmClient::new(factory.clone(), LlmClientConfig::new(0.0, 128));
        for _ in 0..10 {
            let _ = cached.get().await.expect("ok");
        }
        assert_eq!(
            factory.call_count(),
            1,
            "10 cached.get() calls should produce exactly 1 build_client call"
        );
    }

    #[tokio::test]
    async fn cached_llm_client_does_not_memoize_errors() {
        let factory = Arc::new(FailingFactory::new());
        let cached = CachedLlmClient::new(factory.clone(), LlmClientConfig::new(0.0, 128));
        for _ in 0..3 {
            assert!(cached.get().await.is_err(), "factory always fails");
        }
        assert_eq!(
            factory.calls.load(Ordering::SeqCst),
            3,
            "failing factory must be retried on every get() — Err is not memoized"
        );
    }

    #[tokio::test]
    async fn cached_llm_client_concurrent_first_call_only_builds_once() {
        let factory = Arc::new(CountingFactory::new());
        let cached = Arc::new(CachedLlmClient::new(
            factory.clone(),
            LlmClientConfig::new(0.0, 128),
        ));
        // Fire 8 concurrent first calls. OnceCell::get_or_try_init must
        // serialize them so exactly one wins the build.
        let mut handles = Vec::new();
        for _ in 0..8 {
            let c = cached.clone();
            handles.push(tokio::spawn(async move { c.get().await.is_ok() }));
        }
        for h in handles {
            assert!(h.await.unwrap());
        }
        assert_eq!(
            factory.call_count(),
            1,
            "concurrent first-call paths must coalesce to 1 build_client call"
        );
    }
}
