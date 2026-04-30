//! `WikiStore` trait — backend-agnostic interface for ward wiki articles.

use async_trait::async_trait;
use serde_json::Value;
// `WikiArticle` lives in `zero-stores-domain`; re-export here so the
// trait surface keeps working for callers that import from this crate.
pub use zero_stores_domain::WikiArticle;

/// Aggregate stats for the wiki subsystem.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WikiStats {
    pub total: i64,
}

/// Backend-agnostic interface for ward wiki articles.
///
/// Each row carries the `WikiArticle` JSON shape from `zero-stores-domain`.
/// Methods returning `Vec<Value>` emit one row per article in the
/// canonical shape; callers deserialize via `serde_json::from_value`.
#[async_trait]
pub trait WikiStore: Send + Sync {
    /// List all articles for a ward. Default returns empty.
    async fn list_articles(&self, _ward_id: &str) -> Result<Vec<Value>, String> {
        Ok(Vec::new())
    }

    /// Get a single article by (ward_id, title). Default returns None.
    async fn get_article(&self, _ward_id: &str, _title: &str) -> Result<Option<Value>, String> {
        Ok(None)
    }

    /// Upsert an article. The `article` Value carries the full
    /// `WikiArticle` shape; `embedding` is optional.
    async fn upsert_article(
        &self,
        _article: Value,
        _embedding: Option<Vec<f32>>,
    ) -> Result<(), String> {
        Err("upsert_article not implemented for this store".to_string())
    }

    /// Delete an article. Returns true if a row was removed.
    async fn delete_article(&self, _ward_id: &str, _title: &str) -> Result<bool, String> {
        Ok(false)
    }

    /// Hybrid FTS + vector search across wiki articles.
    /// Each row carries `article` + `score` + `match_source`.
    async fn search_wiki_hybrid(
        &self,
        _ward_id: Option<&str>,
        _query: &str,
        _limit: usize,
        _query_embedding: Option<&[f32]>,
    ) -> Result<Vec<Value>, String> {
        Ok(Vec::new())
    }

    async fn wiki_stats(&self) -> Result<WikiStats, String> {
        Ok(WikiStats::default())
    }

    /// Pure vector-similarity search scoped to a ward, returning typed
    /// `(WikiArticle, score)` pairs directly. Used by recall paths that
    /// want richer ranking than the hybrid endpoint. Default returns
    /// empty so backends without a dedicated vector index can opt out.
    async fn search_wiki_by_similarity_typed(
        &self,
        _ward_id: &str,
        _embedding: &[f32],
        _limit: usize,
    ) -> Result<Vec<(WikiArticle, f64)>, String> {
        Ok(Vec::new())
    }

    /// Typed variant of `list_articles` returning `Vec<WikiArticle>`
    /// directly. Default deserialises the Value-based result for
    /// backends that haven't overridden.
    async fn list_articles_typed(&self, ward_id: &str) -> Result<Vec<WikiArticle>, String> {
        let rows = self.list_articles(ward_id).await?;
        rows.into_iter()
            .map(|v| serde_json::from_value(v).map_err(|e| format!("decode WikiArticle: {e}")))
            .collect()
    }
}
