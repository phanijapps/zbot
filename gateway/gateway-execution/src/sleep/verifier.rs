//! LLM-backed implementation of `PairwiseVerifier`.
//!
//! Conservative — returns `false` (don't merge) on any LLM failure so a
//! flaky network never triggers an incorrect entity merge.

use std::sync::Arc;

use agent_runtime::llm::{ChatMessage, LlmClient, LlmConfig};
use async_trait::async_trait;
use gateway_services::ProviderService;
use knowledge_graph::Entity;
use serde::Deserialize;

use crate::ingest::json_shape::parse_llm_json;
use crate::sleep::compactor::PairwiseVerifier;

#[derive(Debug, Deserialize)]
struct VerifierResponse {
    same_entity: bool,
    #[allow(dead_code)]
    confidence: Option<f64>,
}

/// LLM-backed pairwise verifier. Defaults to deny on any failure so a
/// flaky network or bad response never causes a wrong merge.
pub struct LlmPairwiseVerifier {
    provider_service: Arc<ProviderService>,
}

impl LlmPairwiseVerifier {
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
        .with_max_tokens(128);
        let client = agent_runtime::llm::openai::OpenAiClient::new(config)
            .map_err(|e| format!("build client: {e}"))?;
        Ok(Arc::new(client) as Arc<dyn LlmClient>)
    }
}

#[async_trait]
impl PairwiseVerifier for LlmPairwiseVerifier {
    async fn should_merge(&self, a: &Entity, b: &Entity) -> bool {
        let client = match self.build_client() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(error = %e, "verifier: build_client failed; default deny");
                return false;
            }
        };
        let prompt = format!(
            "You are an entity-resolution adjudicator. Two entities of the same \
             type were proposed for merge by a similarity threshold. Decide \
             whether they refer to the same real-world thing.\n\n\
             Return ONLY JSON: {{\"same_entity\": bool, \"confidence\": 0.0-1.0}}\n\n\
             Entity A: name={:?}  type={:?}\n\
             Entity B: name={:?}  type={:?}",
            a.name, a.entity_type, b.name, b.entity_type,
        );
        let messages = vec![
            ChatMessage::system("You return only valid JSON.".to_string()),
            ChatMessage::user(prompt),
        ];
        let response = match client.chat(messages, None).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "verifier: LLM call failed; default deny");
                return false;
            }
        };
        match parse_llm_json::<VerifierResponse>(&response.content) {
            Ok(r) => r.same_entity,
            Err(e) => {
                tracing::warn!(error = %e, "verifier: parse failed; default deny");
                false
            }
        }
    }
}
