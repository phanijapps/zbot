// ============================================================================
// OPENAI-COMPATIBLE EMBEDDING CLIENT
// Works with OpenAI, Ollama, Voyage, and any OpenAI-compatible provider
// ============================================================================

use async_trait::async_trait;
use serde_json::json;

use super::embedding::{EmbeddingClient, EmbeddingError};

/// OpenAI-compatible embedding client.
///
/// Calls `POST {base_url}/v1/embeddings` with the standard OpenAI format.
/// Works with: OpenAI, Ollama (`localhost:11434/v1`), Voyage, LiteLLM, etc.
pub struct OpenAiEmbeddingClient {
    base_url: String,
    api_key: String,
    model: String,
    dimensions: usize,
    http_client: reqwest::Client,
}

impl OpenAiEmbeddingClient {
    /// Create a new OpenAI-compatible embedding client.
    pub fn new(
        base_url: String,
        api_key: String,
        model: String,
        dimensions: usize,
    ) -> Self {
        Self {
            base_url,
            api_key,
            model,
            dimensions,
            http_client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl EmbeddingClient for OpenAiEmbeddingClient {
    async fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let url = format!(
            "{}/embeddings",
            self.base_url.trim_end_matches('/')
        );

        let body = json!({
            "model": self.model,
            "input": texts,
        });

        tracing::debug!(
            "Embedding {} text(s) via {} (model: {})",
            texts.len(),
            url,
            self.model
        );

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(EmbeddingError::ApiError(format!(
                "({}): {}",
                status, error_text
            )));
        }

        let json_response: serde_json::Value = response
            .json()
            .await
            .map_err(|e| EmbeddingError::ParseError(format!("Failed to parse response: {}", e)))?;

        // Parse OpenAI embedding response format:
        // { "data": [{ "embedding": [0.1, 0.2, ...], "index": 0 }, ...] }
        let data = json_response
            .get("data")
            .and_then(|d| d.as_array())
            .ok_or_else(|| {
                EmbeddingError::ParseError("Missing 'data' array in response".to_string())
            })?;

        let mut embeddings: Vec<(usize, Vec<f32>)> = Vec::with_capacity(data.len());
        for item in data {
            let index = item
                .get("index")
                .and_then(|i| i.as_u64())
                .unwrap_or(embeddings.len() as u64) as usize;

            let embedding = item
                .get("embedding")
                .and_then(|e| e.as_array())
                .ok_or_else(|| {
                    EmbeddingError::ParseError("Missing 'embedding' array in response item".to_string())
                })?
                .iter()
                .filter_map(|v| v.as_f64().map(|f| f as f32))
                .collect::<Vec<f32>>();

            embeddings.push((index, embedding));
        }

        // Sort by index to match input order
        embeddings.sort_by_key(|(i, _)| *i);

        Ok(embeddings.into_iter().map(|(_, e)| e).collect())
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn model_name(&self) -> &str {
        &self.model
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = OpenAiEmbeddingClient::new(
            "https://api.openai.com/v1".to_string(),
            "sk-test".to_string(),
            "text-embedding-3-small".to_string(),
            1536,
        );

        assert_eq!(client.model_name(), "text-embedding-3-small");
        assert_eq!(client.dimensions(), 1536);
    }

    #[test]
    fn test_ollama_url_format() {
        let client = OpenAiEmbeddingClient::new(
            "http://localhost:11434/v1".to_string(),
            String::new(),
            "nomic-embed-text".to_string(),
            768,
        );

        assert_eq!(client.model_name(), "nomic-embed-text");
        assert_eq!(client.dimensions(), 768);
    }
}
