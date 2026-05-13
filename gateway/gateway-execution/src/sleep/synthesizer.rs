//! Synthesizer — engine moved to gateway-memory crate.
//! This file now hosts only the production LLM impl (which depends on ProviderService).

pub use gateway_memory::sleep::synthesizer::*;

use std::sync::Arc;

use agent_runtime::llm::{ChatMessage, LlmClient, LlmConfig};
use async_trait::async_trait;
use gateway_services::ProviderService;

use crate::ingest::json_shape::parse_llm_json;

// ============================================================================
// LLM-backed implementation
// ============================================================================

/// LLM-backed `SynthesisLlm` wired to the default configured provider.
/// Conservative on failure — propagates `Err` so `run_cycle` can log+skip.
pub struct LlmSynthesizer {
    provider_service: Arc<ProviderService>,
}

impl LlmSynthesizer {
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
impl SynthesisLlm for LlmSynthesizer {
    async fn synthesize(&self, input: &SynthesisInput) -> Result<SynthesisResponse, String> {
        let client = self.build_client()?;
        let prompt = format!(
            "You identify reusable cross-session strategies from an agent's knowledge graph.\n\
             The entity below has appeared across {n} distinct sessions within the last 30 days.\n\
             Decide whether the repeated co-occurrence reveals a *strategy* worth memorising \
             as a stable rule (e.g. \"when X times out, retry with backoff\").\n\n\
             Return ONLY JSON: {{\"strategy\": string, \"confidence\": 0.0-1.0, \
             \"key_fact\": string, \"decision\": \"synthesize\" | \"skip\"}}.\n\n\
             Entity: name={name:?} type={etype}\n\
             Recent task summaries:\n{tasks}\n\n\
             Relationships:\n{rels}",
            n = input.session_count,
            name = input.entity_name,
            etype = input.entity_type,
            tasks = input
                .task_summaries
                .iter()
                .map(|t| format!("- {t}"))
                .collect::<Vec<_>>()
                .join("\n"),
            rels = input
                .relationship_summaries
                .iter()
                .map(|r| format!("- {r}"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        let messages = vec![
            ChatMessage::system("You return only valid JSON.".to_string()),
            ChatMessage::user(prompt),
        ];
        let response = client
            .chat(messages, None)
            .await
            .map_err(|e| format!("LLM call: {e}"))?;
        parse_llm_json::<SynthesisResponse>(&response.content)
    }
}
