//! ConflictResolver — engine moved to gateway-memory crate.
//! This file now hosts only the production LLM judge (which depends on ProviderService).

pub use gateway_memory::sleep::conflict_resolver::*;

use std::sync::Arc;

use agent_runtime::llm::{ChatMessage, LlmClient, LlmConfig};
use async_trait::async_trait;
use gateway_services::ProviderService;

use crate::ingest::json_shape::parse_llm_json;

// ============================================================================
// LLM-backed implementation
// ============================================================================

/// Production judge wired to the default configured provider.
pub struct LlmConflictJudge {
    provider_service: Arc<ProviderService>,
}

impl LlmConflictJudge {
    pub fn new(provider_service: Arc<ProviderService>) -> Self {
        Self { provider_service }
    }

    fn build_client(&self) -> Result<Arc<dyn LlmClient>, String> {
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
        let config = LlmConfig::new(
            provider.base_url.clone(),
            provider.api_key.clone(),
            model,
            provider_id,
        )
        .with_temperature(0.0)
        .with_max_tokens(256);
        let client = agent_runtime::llm::openai::OpenAiClient::new(config)
            .map_err(|e| format!("build client: {e}"))?;
        Ok(Arc::new(client) as Arc<dyn LlmClient>)
    }
}

#[async_trait]
impl ConflictJudgeLlm for LlmConflictJudge {
    async fn judge(&self, a: &str, b: &str) -> Result<ConflictResponse, String> {
        let client = self.build_client()?;
        let prompt = format!(
            "You judge whether two principles for an AI agent contradict each other.\n\
             Two principles can be about the same topic and NOT contradict (they may\n\
             cover different cases). Only say \"contradicts\" if one principle's\n\
             prescription would violate the other.\n\n\
             Return ONLY JSON: \
             {{\"decision\": \"contradicts\" | \"compatible\", \
             \"confidence\": 0.0-1.0, \"reason\": string}}.\n\n\
             Principle A: {a}\n\
             Principle B: {b}",
        );
        let messages = vec![
            ChatMessage::system("You return only valid JSON.".to_string()),
            ChatMessage::user(prompt),
        ];
        let response = client
            .chat(messages, None)
            .await
            .map_err(|e| format!("LLM call: {e}"))?;
        parse_llm_json::<ConflictResponse>(&response.content)
    }
}
