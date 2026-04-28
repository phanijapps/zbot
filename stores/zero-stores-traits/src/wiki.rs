//! `WikiStore` trait — backend-agnostic interface for ward wiki articles.

use async_trait::async_trait;
use serde_json::Value;

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
}
