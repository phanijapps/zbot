//! PatternExtractor — engine moved to gateway-memory crate.
//! This file now hosts only the production LLM impl (which depends on ProviderService).

pub use gateway_memory::sleep::pattern_extractor::*;

use std::sync::Arc;

use agent_runtime::llm::{ChatMessage, LlmClient, LlmConfig};
use async_trait::async_trait;
use gateway_services::ProviderService;

use crate::ingest::json_shape::parse_llm_json;

// ============================================================================
// LLM-backed implementation
// ============================================================================

/// LLM-backed `PatternExtractLlm`. Conservative on failure — propagates `Err`
/// so `run_cycle` can log+skip.
pub struct LlmPatternExtractor {
    provider_service: Arc<ProviderService>,
}

impl LlmPatternExtractor {
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
        .with_max_tokens(1024);
        let client = agent_runtime::llm::openai::OpenAiClient::new(config)
            .map_err(|e| format!("build client: {e}"))?;
        Ok(Arc::new(client) as Arc<dyn LlmClient>)
    }
}

#[async_trait]
impl PatternExtractLlm for LlmPatternExtractor {
    async fn generalize(&self, input: &PatternInput) -> Result<PatternResponse, String> {
        let client = self.build_client()?;
        let prompt = format!(
            "Two recent successful agent sessions shared a recurring tool-call \
             sequence. Generalize it into a reusable procedure.\n\n\
             Return ONLY JSON: {{\"name\": snake_case_string, \"description\": string, \
             \"trigger_pattern\": string, \"parameters\": [string], \
             \"steps\": [{{\"action\": string, \"agent\": string|null, \
             \"note\": string|null, \"task_template\": string|null}}]}}.\n\n\
             Session A task: {sa}\n\
             Session A tool sequence: {ta:?}\n\n\
             Session B task: {sb}\n\
             Session B tool sequence: {tb:?}\n\n\
             Matched prefix: {mp:?}",
            sa = input.task_summary_a,
            sb = input.task_summary_b,
            ta = input.tool_sequence_a,
            tb = input.tool_sequence_b,
            mp = input.matched_prefix,
        );
        let messages = vec![
            ChatMessage::system("You return only valid JSON.".to_string()),
            ChatMessage::user(prompt),
        ];
        let response = client
            .chat(messages, None)
            .await
            .map_err(|e| format!("LLM call: {e}"))?;
        parse_llm_json::<PatternResponse>(&response.content)
    }
}
