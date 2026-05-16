//! Production implementation of `gateway_memory::MemoryLlmFactory` wired to
//! `gateway_services::ProviderService`.
//!
//! Lives in the gateway crate (not `gateway-memory`) because it depends on
//! `ProviderService`, which itself depends on `gateway-memory`. Constructing
//! one of these per process avoids the six copy-pasted `build_client`
//! methods that previously lived inside each sleep-time LLM impl.

use std::sync::Arc;

use agent_runtime::llm::{openai::OpenAiClient, LlmClient, LlmConfig};
use async_trait::async_trait;
use gateway_memory::{LlmClientConfig, MemoryLlmFactory};
use gateway_services::ProviderService;

/// Builds OpenAI-compatible LLM clients using the default configured
/// provider from `ProviderService`. Falls back to the first listed
/// provider when none is marked default.
pub struct ProviderServiceLlmFactory {
    provider_service: Arc<ProviderService>,
}

impl ProviderServiceLlmFactory {
    pub fn new(provider_service: Arc<ProviderService>) -> Self {
        Self { provider_service }
    }
}

#[async_trait]
impl MemoryLlmFactory for ProviderServiceLlmFactory {
    async fn build_client(&self, config: LlmClientConfig) -> Result<Arc<dyn LlmClient>, String> {
        let providers = self
            .provider_service
            .list()
            .map_err(|e| format!("list providers: {e}"))?;
        let provider = providers
            .iter()
            .find(|p| p.is_default)
            .or_else(|| providers.first())
            .ok_or_else(|| "no providers configured".to_string())?;
        let model = provider.default_model().to_string();
        let provider_id = provider.id.clone().unwrap_or_else(|| "default".to_string());
        let llm_config = LlmConfig::new(
            provider.base_url.clone(),
            provider.api_key.clone(),
            model,
            provider_id,
        )
        .with_temperature(config.temperature)
        .with_max_tokens(config.max_tokens);
        let client = OpenAiClient::new(llm_config).map_err(|e| format!("build client: {e}"))?;
        Ok(Arc::new(client) as Arc<dyn LlmClient>)
    }
}
