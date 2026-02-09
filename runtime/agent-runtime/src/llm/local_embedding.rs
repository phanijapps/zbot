// ============================================================================
// LOCAL EMBEDDING CLIENT
// ONNX-based local embeddings via fastembed — zero API calls
// ============================================================================

use async_trait::async_trait;
use fastembed::{InitOptions, TextEmbedding, EmbeddingModel};

use super::embedding::{EmbeddingClient, EmbeddingError};

/// Local embedding client using fastembed (ONNX Runtime).
///
/// Default model: `all-MiniLM-L6-v2` (384 dims, ~100MB, fastest).
/// Runs entirely on CPU — no API key, no network, no cost.
pub struct LocalEmbeddingClient {
    model: TextEmbedding,
    model_name: String,
    dimensions: usize,
}

impl LocalEmbeddingClient {
    /// Create a local embedding client with the default model (all-MiniLM-L6-v2).
    pub fn new() -> Result<Self, EmbeddingError> {
        Self::with_model(EmbeddingModel::AllMiniLML6V2)
    }

    /// Create a local embedding client with a specific fastembed model.
    pub fn with_model(model_id: EmbeddingModel) -> Result<Self, EmbeddingError> {
        let (name, dims) = model_info(&model_id);

        let options = InitOptions::new(model_id)
            .with_show_download_progress(true);

        let model = TextEmbedding::try_new(options)
            .map_err(|e| EmbeddingError::ModelError(format!("Failed to init fastembed model: {}", e)))?;

        tracing::info!(
            "Local embedding model loaded: {} ({}d)",
            name, dims
        );

        Ok(Self {
            model,
            model_name: name.to_string(),
            dimensions: dims,
        })
    }
}

#[async_trait]
impl EmbeddingClient for LocalEmbeddingClient {
    async fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // fastembed expects Vec<String>
        let owned: Vec<String> = texts.iter().map(|s| s.to_string()).collect();

        tracing::debug!("Embedding {} text(s) locally via {}", owned.len(), self.model_name);

        // fastembed is sync + CPU-bound — run on blocking thread pool
        let model_name = self.model_name.clone();
        let embeddings = {
            // TextEmbedding is not Send, so we create it fresh in the blocking closure
            // Actually, we can't move self.model across threads easily.
            // Instead, embed synchronously (fastembed is fast enough for <100 texts).
            self.model
                .embed(owned, None)
                .map_err(|e| EmbeddingError::ModelError(format!(
                    "Embedding failed ({}): {}", model_name, e
                )))?
        };

        Ok(embeddings)
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn model_name(&self) -> &str {
        &self.model_name
    }
}

/// Return (name, dimensions) for known fastembed models.
fn model_info(model: &EmbeddingModel) -> (&'static str, usize) {
    match model {
        EmbeddingModel::AllMiniLML6V2 => ("all-MiniLM-L6-v2", 384),
        EmbeddingModel::BGESmallENV15 => ("bge-small-en-v1.5", 384),
        EmbeddingModel::BGEBaseENV15 => ("bge-base-en-v1.5", 768),
        EmbeddingModel::BGELargeENV15 => ("bge-large-en-v1.5", 1024),
        EmbeddingModel::AllMiniLML12V2 => ("all-MiniLM-L12-v2", 384),
        EmbeddingModel::MultilingualE5Large => ("multilingual-e5-large", 1024),
        _ => ("unknown", 384),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_info() {
        let (name, dims) = model_info(&EmbeddingModel::AllMiniLML6V2);
        assert_eq!(name, "all-MiniLM-L6-v2");
        assert_eq!(dims, 384);
    }

    #[test]
    fn test_model_info_bge() {
        let (name, dims) = model_info(&EmbeddingModel::BGESmallENV15);
        assert_eq!(name, "bge-small-en-v1.5");
        assert_eq!(dims, 384);
    }

    // Integration test: actually loads the model and embeds text.
    // Skipped in CI (model download required).
    #[test]
    #[ignore]
    fn test_local_embedding_end_to_end() {
        let client = LocalEmbeddingClient::new().expect("Should create local client");
        assert_eq!(client.dimensions(), 384);
        assert_eq!(client.model_name(), "all-MiniLM-L6-v2");

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(client.embed(&["hello world", "test embedding"]));
        let embeddings = result.expect("Should embed successfully");
        assert_eq!(embeddings.len(), 2);
        assert_eq!(embeddings[0].len(), 384);
        assert_eq!(embeddings[1].len(), 384);
    }
}
