//! Production implementation of `gateway_memory::MemoryLlmFactory` wired to
//! `gateway_services::ProviderService`.
//!
//! Lives in the gateway crate (not `gateway-memory`) because it depends on
//! `ProviderService`, which itself depends on `gateway-memory`. Constructing
//! one of these per process avoids the six copy-pasted `build_client`
//! methods that previously lived inside each sleep-time LLM impl.
//!
//! Honors per-task overrides via `LlmClientConfig::with_task("…")`. Today
//! the only recognised task is `"sleep_time"` → reads
//! `settings.execution.sleepTime.{providerId, model}`. Untagged configs (or
//! tags we don't recognise) fall through to the orchestrator-then-default
//! chain.

use std::sync::Arc;

use agent_runtime::llm::{openai::OpenAiClient, LlmClient, LlmConfig};
use async_trait::async_trait;
use gateway_memory::{LlmClientConfig, MemoryLlmFactory};
use gateway_services::{ProviderService, SettingsService, SharedVaultPaths};

/// Builds OpenAI-compatible LLM clients using the configured provider chain.
/// Per-task overrides (currently `"sleep_time"`) are applied when a config
/// arrives tagged via `LlmClientConfig::with_task(...)`.
pub struct ProviderServiceLlmFactory {
    provider_service: Arc<ProviderService>,
    /// Vault paths used to lazily construct a `SettingsService` per call.
    /// Cheap because the on-disk settings.json is small and the service
    /// has its own internal cache — and avoids the construction-order
    /// problem (the factory is built before `AppState.settings` exists).
    paths: SharedVaultPaths,
}

impl ProviderServiceLlmFactory {
    pub fn new(provider_service: Arc<ProviderService>, paths: SharedVaultPaths) -> Self {
        Self {
            provider_service,
            paths,
        }
    }
}

#[async_trait]
impl MemoryLlmFactory for ProviderServiceLlmFactory {
    async fn build_client(&self, config: LlmClientConfig) -> Result<Arc<dyn LlmClient>, String> {
        let exec = SettingsService::new(self.paths.clone())
            .get_execution_settings()
            .unwrap_or_default();
        let orch = &exec.orchestrator;

        // Resolve the per-task override (only "sleep_time" today). Each
        // field falls through: task → orchestrator → default provider.
        let (task_provider_id, task_model) = match config.task.as_deref() {
            Some("sleep_time") => (
                exec.sleep_time.provider_id.clone(),
                exec.sleep_time.model.clone(),
            ),
            _ => (None, None),
        };

        let provider_id_override = task_provider_id
            .filter(|s| !s.is_empty())
            .or_else(|| orch.provider_id.clone().filter(|s| !s.is_empty()));

        let providers = self
            .provider_service
            .list()
            .map_err(|e| format!("list providers: {e}"))?;
        let provider = match provider_id_override {
            Some(id) => self
                .provider_service
                .get(&id)
                .map_err(|e| format!("provider {id}: {e}"))?,
            None => providers
                .iter()
                .find(|p| p.is_default)
                .or_else(|| providers.first())
                .cloned()
                .ok_or_else(|| "no providers configured".to_string())?,
        };

        let model = task_model
            .filter(|m| !m.is_empty())
            .or_else(|| orch.model.clone().filter(|m| !m.is_empty()))
            .unwrap_or_else(|| provider.default_model().to_string());

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
