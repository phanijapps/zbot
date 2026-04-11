// ============================================================================
// LOCAL EMBEDDING CLIENT
// ONNX-based local embeddings via fastembed — zero API calls
// Lazy load/unload: model loads on first embed(), unloads after idle timeout.
// ============================================================================

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

use super::embedding::{EmbeddingClient, EmbeddingError};

/// Local embedding client using fastembed (ONNX Runtime).
///
/// Default model: `all-MiniLM-L6-v2` (384 dims, ~100MB, fastest).
/// Runs entirely on CPU — no API key, no network, no cost.
///
/// The model is loaded lazily on first `embed()` call and unloaded
/// after `idle_timeout_secs` of inactivity to free memory.
pub struct LocalEmbeddingClient {
    model: Mutex<Option<TextEmbedding>>,
    model_id: EmbeddingModel,
    model_name: String,
    dimensions: usize,
    last_used: AtomicU64,
    idle_timeout_secs: u64,
    unload_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl LocalEmbeddingClient {
    /// Create with default model (all-MiniLM-L6-v2), 600s idle timeout.
    /// Does NOT load the model — loads lazily on first embed().
    pub fn new() -> Self {
        Self::with_model(EmbeddingModel::AllMiniLML6V2, 600)
    }

    /// Create with specific model and idle timeout.
    /// Set idle_timeout_secs=0 to never unload.
    pub fn with_model(model_id: EmbeddingModel, idle_timeout_secs: u64) -> Self {
        let (name, dims) = model_info(&model_id);
        tracing::info!(
            "Local embedding client created (lazy): {} ({}d, idle_timeout={}s)",
            name,
            dims,
            idle_timeout_secs
        );
        Self {
            model: Mutex::new(None),
            model_id,
            model_name: name.to_string(),
            dimensions: dims,
            last_used: AtomicU64::new(0),
            idle_timeout_secs,
            unload_handle: Mutex::new(None),
        }
    }

    /// Load model if not loaded, return mutex guard.
    fn ensure_loaded(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, Option<TextEmbedding>>, EmbeddingError> {
        let mut guard = self
            .model
            .lock()
            .map_err(|e| EmbeddingError::ModelError(format!("Mutex poisoned: {}", e)))?;

        if guard.is_none() {
            tracing::info!("Loading embedding model: {} ...", self.model_name);
            let options = InitOptions::new(self.model_id.clone()).with_show_download_progress(true);
            let model = TextEmbedding::try_new(options).map_err(|e| {
                EmbeddingError::ModelError(format!("Failed to load fastembed model: {}", e))
            })?;
            tracing::info!("Embedding model loaded: {}", self.model_name);
            *guard = Some(model);
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.last_used.store(now, Ordering::Relaxed);

        Ok(guard)
    }

    /// Start idle watcher if not running and timeout > 0.
    fn ensure_watcher_running(&self) {
        if self.idle_timeout_secs == 0 {
            return;
        }

        let mut handle_guard = match self.unload_handle.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        if handle_guard.as_ref().is_some_and(|h| !h.is_finished()) {
            return;
        }

        let timeout_secs = self.idle_timeout_secs;
        let last_used = &self.last_used as *const AtomicU64 as usize;
        let model_ptr = &self.model as *const Mutex<Option<TextEmbedding>> as usize;
        let model_name = self.model_name.clone();

        // SAFETY: The watcher only runs while `self` is alive because:
        // - LocalEmbeddingClient is stored in Arc<dyn EmbeddingClient> in AppState
        // - AppState lives for the entire application lifetime
        // - The watcher exits when the model is already None (client dropped)
        let handle = tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;

                let last_used = unsafe { &*(last_used as *const AtomicU64) };
                let model_mutex = unsafe { &*(model_ptr as *const Mutex<Option<TextEmbedding>>) };

                let last = last_used.load(Ordering::Relaxed);
                if last == 0 {
                    continue;
                }

                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                if now.saturating_sub(last) >= timeout_secs {
                    if let Ok(mut guard) = model_mutex.lock() {
                        if guard.is_some() {
                            *guard = None;
                            tracing::info!(
                                "Embedding model unloaded after {}s idle: {}",
                                timeout_secs,
                                model_name
                            );
                        }
                    }
                    break;
                }
            }
        });

        *handle_guard = Some(handle);
    }
}

#[async_trait]
impl EmbeddingClient for LocalEmbeddingClient {
    async fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let owned: Vec<String> = texts.iter().map(|s| s.to_string()).collect();
        tracing::debug!(
            "Embedding {} text(s) locally via {}",
            owned.len(),
            self.model_name
        );

        let guard = self.ensure_loaded()?;
        let embeddings = guard
            .as_ref()
            .expect("ensure_loaded guarantees Some")
            .embed(owned, None)
            .map_err(|e| {
                EmbeddingError::ModelError(format!("Embedding failed ({}): {}", self.model_name, e))
            })?;

        drop(guard);
        self.ensure_watcher_running();

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

    #[test]
    fn test_lazy_construction() {
        let client = LocalEmbeddingClient::new();
        assert_eq!(client.dimensions(), 384);
        assert_eq!(client.model_name(), "all-MiniLM-L6-v2");
        let guard = client.model.lock().unwrap();
        assert!(
            guard.is_none(),
            "Model should be lazy — not loaded at construction"
        );
    }

    #[test]
    fn test_custom_timeout() {
        let client = LocalEmbeddingClient::with_model(EmbeddingModel::AllMiniLML6V2, 0);
        assert_eq!(client.idle_timeout_secs, 0);

        let client2 = LocalEmbeddingClient::with_model(EmbeddingModel::AllMiniLML6V2, 600);
        assert_eq!(client2.idle_timeout_secs, 600);
    }

    #[test]
    #[ignore]
    fn test_local_embedding_end_to_end() {
        let client = LocalEmbeddingClient::new();
        assert_eq!(client.dimensions(), 384);
        assert_eq!(client.model_name(), "all-MiniLM-L6-v2");

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(client.embed(&["hello world", "test embedding"]));
        let embeddings = result.expect("Should embed successfully");
        assert_eq!(embeddings.len(), 2);
        assert_eq!(embeddings[0].len(), 384);
        assert_eq!(embeddings[1].len(), 384);

        let guard = client.model.lock().unwrap();
        assert!(guard.is_some(), "Model should be loaded after embed()");
    }
}
