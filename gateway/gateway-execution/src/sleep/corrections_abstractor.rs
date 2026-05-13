//! CorrectionsAbstractor — engine moved to gateway-memory crate.
//! This file now hosts only the production LLM impl (which depends on ProviderService).

pub use gateway_memory::sleep::corrections_abstractor::*;

use std::sync::Arc;

use agent_runtime::llm::{ChatMessage, LlmClient, LlmConfig};
use async_trait::async_trait;
use gateway_services::ProviderService;

use crate::ingest::json_shape::parse_llm_json;

// ============================================================================
// LLM-backed implementation
// ============================================================================

/// Production `AbstractionLlm` wired to the default configured provider.
pub struct LlmCorrectionsAbstractor {
    provider_service: Arc<ProviderService>,
}

impl LlmCorrectionsAbstractor {
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
        .with_max_tokens(512);
        let client = agent_runtime::llm::openai::OpenAiClient::new(config)
            .map_err(|e| format!("build client: {e}"))?;
        Ok(Arc::new(client) as Arc<dyn LlmClient>)
    }
}

#[async_trait]
impl AbstractionLlm for LlmCorrectionsAbstractor {
    async fn abstract_corrections(
        &self,
        corrections: &[String],
    ) -> Result<AbstractionResponse, String> {
        let client = self.build_client()?;
        let formatted = corrections
            .iter()
            .enumerate()
            .map(|(i, c)| format!("{}. {c}", i + 1))
            .collect::<Vec<_>>()
            .join("\n");
        let prompt = format!(
            "You identify common principles from an AI agent's correction history.\n\
             Below are {n} correction facts the agent has accumulated.\n\
             Decide if they share a common theme expressible as one imperative principle.\n\n\
             Return ONLY JSON: \
             {{\"schema\": string, \"confidence\": 0.0-1.0, \
             \"key_fact\": string, \"decision\": \"abstract\" | \"skip\"}}.\n\
             - \"schema\": theme name in snake_case (<5 words)\n\
             - \"key_fact\": the principle as a single imperative sentence\n\
             - \"decision\": \"abstract\" if clear shared principle, \"skip\" if too diverse\n\n\
             Corrections:\n{formatted}",
            n = corrections.len(),
        );
        let messages = vec![
            ChatMessage::system("You return only valid JSON.".to_string()),
            ChatMessage::user(prompt),
        ];
        let response = client
            .chat(messages, None)
            .await
            .map_err(|e| format!("LLM call: {e}"))?;
        parse_llm_json::<AbstractionResponse>(&response.content)
    }
}
