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
