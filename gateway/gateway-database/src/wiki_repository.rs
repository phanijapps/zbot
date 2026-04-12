//! Repository for ward wiki articles — compiled knowledge per ward.
//!
//! Phase 1b (v22): constructs on `KnowledgeDatabase` and stores embeddings in
//! the `wiki_articles_index` vec0 virtual table through the `VectorIndex` trait.
//! The `embedding` column on `ward_wiki_articles` is gone; callers write
//! normalized vectors through `upsert_article`, which delegates to the injected
//! `VectorIndex`. Vectors MUST be L2-normalized by the caller.
//!
//! To read an embedding back, use [`WardWikiRepository::get_article_embedding`].

use crate::vector_index::VectorIndex;
use crate::KnowledgeDatabase;
use rusqlite::params;
use std::sync::Arc;

/// A compiled wiki article for a ward.
#[derive(Debug, Clone)]
pub struct WikiArticle {
    pub id: String,
    pub ward_id: String,
    pub agent_id: String,
    pub title: String,
    pub content: String,
    pub tags: Option<String>,
    pub source_fact_ids: Option<String>,
    /// Raw f32 embedding. Always `None` when loaded from `ward_wiki_articles`
    /// (the column was removed in schema v22). Callers may set this to `Some(v)`
    /// prior to `upsert_article` to have the vector persisted through the
    /// `VectorIndex` — vectors MUST be L2-normalized by the caller.
    ///
    /// To read an embedding back, use [`WardWikiRepository::get_article_embedding`].
    pub embedding: Option<Vec<f32>>,
    pub version: i32,
    pub created_at: String,
    pub updated_at: String,
}

/// Repository for ward wiki article CRUD and vector search.
pub struct WardWikiRepository {
    db: Arc<KnowledgeDatabase>,
    vec_index: Arc<dyn VectorIndex>,
}

impl WardWikiRepository {
    /// Create a new wiki repository.
    ///
    /// `vec_index` must wrap the `wiki_articles_index` vec0 table (384-dim).
    pub fn new(db: Arc<KnowledgeDatabase>, vec_index: Arc<dyn VectorIndex>) -> Self {
        Self { db, vec_index }
    }

    /// List all articles for a ward.
    pub fn list_articles(&self, ward_id: &str) -> Result<Vec<WikiArticle>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, ward_id, agent_id, title, content, tags, source_fact_ids, \
                 version, created_at, updated_at \
                 FROM ward_wiki_articles WHERE ward_id = ?1 ORDER BY title",
            )?;

            let articles = stmt
                .query_map(params![ward_id], |row| Ok(Self::row_to_article(row)))?
                .filter_map(|r| r.ok())
                .collect();

            Ok(articles)
        })
    }

    /// Get a specific article by ward and title.
    pub fn get_article(&self, ward_id: &str, title: &str) -> Result<Option<WikiArticle>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, ward_id, agent_id, title, content, tags, source_fact_ids, \
                 version, created_at, updated_at \
                 FROM ward_wiki_articles WHERE ward_id = ?1 AND title = ?2",
            )?;

            let mut rows =
                stmt.query_map(params![ward_id, title], |row| Ok(Self::row_to_article(row)))?;

            match rows.next() {
                Some(Ok(article)) => Ok(Some(article)),
                Some(Err(e)) => Err(e),
                None => Ok(None),
            }
        })
    }

    /// Upsert an article (insert or update if title exists for this ward).
    ///
    /// If `article.embedding` is `Some(v)`, the vector is written to
    /// `wiki_articles_index` via the injected `VectorIndex`. **Callers must
    /// L2-normalize the vector first**.
    pub fn upsert_article(&self, article: &WikiArticle) -> Result<(), String> {
        self.db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO ward_wiki_articles \
                 (id, ward_id, agent_id, title, content, tags, source_fact_ids, \
                  version, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10) \
                 ON CONFLICT(ward_id, title) DO UPDATE SET \
                 content = excluded.content, \
                 tags = excluded.tags, \
                 source_fact_ids = excluded.source_fact_ids, \
                 version = version + 1, \
                 updated_at = excluded.updated_at",
                params![
                    article.id,
                    article.ward_id,
                    article.agent_id,
                    article.title,
                    article.content,
                    article.tags,
                    article.source_fact_ids,
                    article.version,
                    article.created_at,
                    article.updated_at,
                ],
            )?;
            Ok(())
        })?;

        if let Some(emb) = article.embedding.as_ref() {
            self.vec_index.upsert(&article.id, emb)?;
        }

        Ok(())
    }

    /// Search articles by embedding similarity for a ward.
    ///
    /// Performs a nearest-neighbor query through `VectorIndex`, then loads the
    /// matching `ward_wiki_articles` rows and filters by ward in Rust. The
    /// returned score is cosine similarity (`1 - L2_sq / 2`), valid because
    /// stored and query vectors are required to be L2-normalized.
    pub fn search_by_similarity(
        &self,
        ward_id: &str,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<(WikiArticle, f64)>, String> {
        // Over-fetch so post-filtering by ward still returns `limit` hits.
        let fetch = limit.saturating_mul(4).max(limit);
        let nearest = self.vec_index.query_nearest(query_embedding, fetch)?;
        if nearest.is_empty() {
            return Ok(Vec::new());
        }

        let ids: Vec<String> = nearest.iter().map(|(id, _)| id.clone()).collect();
        let dist_by_id: std::collections::HashMap<String, f32> =
            nearest.iter().map(|(id, d)| (id.clone(), *d)).collect();

        let placeholders = (0..ids.len())
            .map(|i| format!("?{}", i + 1))
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT id, ward_id, agent_id, title, content, tags, source_fact_ids, \
             version, created_at, updated_at \
             FROM ward_wiki_articles WHERE id IN ({placeholders})"
        );

        let articles: Vec<WikiArticle> = self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(&sql)?;
            let params_iter = rusqlite::params_from_iter(ids.iter());
            let rows = stmt.query_map(params_iter, |row| Ok(Self::row_to_article(row)))?;
            Ok(rows.filter_map(|r| r.ok()).collect::<Vec<_>>())
        })?;

        let mut scored: Vec<(WikiArticle, f64)> = articles
            .into_iter()
            .filter(|a| a.ward_id == ward_id)
            .map(|a| {
                let dist = dist_by_id.get(&a.id).copied().unwrap_or(f32::MAX);
                // L2 squared on normalized vectors → cosine = 1 - dist/2.
                let score = 1.0 - (dist as f64) / 2.0;
                (a, score)
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit);
        Ok(scored)
    }

    /// Delete an article.
    pub fn delete_article(&self, ward_id: &str, title: &str) -> Result<bool, String> {
        self.db.with_connection(|conn| {
            let deleted = conn.execute(
                "DELETE FROM ward_wiki_articles WHERE ward_id = ?1 AND title = ?2",
                params![ward_id, title],
            )?;
            Ok(deleted > 0)
        })
    }

    /// Count articles for a ward.
    pub fn count_articles(&self, ward_id: &str) -> Result<usize, String> {
        self.db.with_connection(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM ward_wiki_articles WHERE ward_id = ?1",
                params![ward_id],
                |row| row.get(0),
            )?;
            Ok(count as usize)
        })
    }

    /// Fetch the stored embedding for an article, if present in `wiki_articles_index`.
    /// Returns `None` if the article has never been indexed.
    ///
    /// `sqlite-vec` stores vectors as `FLOAT[N]` BLOBs (little-endian f32s);
    /// we decode the raw bytes back to `Vec<f32>`.
    pub fn get_article_embedding(&self, article_id: &str) -> Result<Option<Vec<f32>>, String> {
        self.db.with_connection(|conn| {
            let r = conn.query_row(
                "SELECT embedding FROM wiki_articles_index WHERE article_id = ?1",
                params![article_id],
                |row| row.get::<_, Vec<u8>>(0),
            );
            match r {
                Ok(blob) => Ok(Some(blob_to_f32_vec(&blob))),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
    }

    fn row_to_article(row: &rusqlite::Row) -> WikiArticle {
        WikiArticle {
            id: row.get(0).unwrap_or_default(),
            ward_id: row.get(1).unwrap_or_default(),
            agent_id: row.get(2).unwrap_or_default(),
            title: row.get(3).unwrap_or_default(),
            content: row.get(4).unwrap_or_default(),
            tags: row.get(5).ok().flatten(),
            source_fact_ids: row.get(6).ok().flatten(),
            embedding: None,
            version: row.get(7).unwrap_or(1),
            created_at: row.get(8).unwrap_or_default(),
            updated_at: row.get(9).unwrap_or_default(),
        }
    }
}

fn blob_to_f32_vec(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vector_index::SqliteVecIndex;
    use gateway_services::VaultPaths;

    fn setup() -> (tempfile::TempDir, WardWikiRepository) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let paths = Arc::new(VaultPaths::new(tmp.path().to_path_buf()));
        let db = Arc::new(KnowledgeDatabase::new(paths).expect("knowledge db"));
        let vec_index: Arc<dyn VectorIndex> = Arc::new(SqliteVecIndex::new(
            db.clone(),
            "wiki_articles_index",
            "article_id",
            384,
        ));
        let repo = WardWikiRepository::new(db, vec_index);
        (tmp, repo)
    }

    fn normalized(v: Vec<f32>) -> Vec<f32> {
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm < 1e-9 {
            v
        } else {
            v.into_iter().map(|x| x / norm).collect()
        }
    }

    #[test]
    fn test_upsert_and_get_article() {
        let (_tmp, repo) = setup();

        let article = WikiArticle {
            id: "art-1".to_string(),
            ward_id: "stock-analysis".to_string(),
            agent_id: "root".to_string(),
            title: "yfinance-patterns".to_string(),
            content: "# yfinance Patterns\nUse rate limiting...".to_string(),
            tags: Some(r#"["yfinance", "python"]"#.to_string()),
            source_fact_ids: None,
            embedding: None,
            version: 1,
            created_at: "2026-04-11".to_string(),
            updated_at: "2026-04-11".to_string(),
        };

        repo.upsert_article(&article).unwrap();
        let fetched = repo
            .get_article("stock-analysis", "yfinance-patterns")
            .unwrap();
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.title, "yfinance-patterns");
        assert!(fetched.content.contains("rate limiting"));
    }

    #[test]
    fn test_upsert_updates_on_conflict() {
        let (_tmp, repo) = setup();

        let article = WikiArticle {
            id: "art-1".to_string(),
            ward_id: "w1".to_string(),
            agent_id: "root".to_string(),
            title: "topic-a".to_string(),
            content: "version 1".to_string(),
            tags: None,
            source_fact_ids: None,
            embedding: None,
            version: 1,
            created_at: "2026-04-11".to_string(),
            updated_at: "2026-04-11".to_string(),
        };

        repo.upsert_article(&article).unwrap();

        let updated = WikiArticle {
            id: "art-2".to_string(),
            content: "version 2".to_string(),
            ..article.clone()
        };
        repo.upsert_article(&updated).unwrap();

        let fetched = repo.get_article("w1", "topic-a").unwrap().unwrap();
        assert_eq!(fetched.content, "version 2");
        assert_eq!(fetched.version, 2); // incremented on conflict
    }

    #[test]
    fn test_list_articles() {
        let (_tmp, repo) = setup();

        for i in 0..3 {
            let article = WikiArticle {
                id: format!("art-{i}"),
                ward_id: "w1".to_string(),
                agent_id: "root".to_string(),
                title: format!("topic-{i}"),
                content: format!("content {i}"),
                tags: None,
                source_fact_ids: None,
                embedding: None,
                version: 1,
                created_at: "2026-04-11".to_string(),
                updated_at: "2026-04-11".to_string(),
            };
            repo.upsert_article(&article).unwrap();
        }

        let articles = repo.list_articles("w1").unwrap();
        assert_eq!(articles.len(), 3);
    }

    #[test]
    fn test_search_by_similarity() {
        let (_tmp, repo) = setup();

        let emb = normalized(
            (0..384)
                .map(|i| if i == 0 { 1.0_f32 } else { 0.0_f32 })
                .collect(),
        );
        let article = WikiArticle {
            id: "art-1".to_string(),
            ward_id: "w1".to_string(),
            agent_id: "root".to_string(),
            title: "topic-a".to_string(),
            content: "content".to_string(),
            tags: None,
            source_fact_ids: None,
            embedding: Some(emb.clone()),
            version: 1,
            created_at: "2026-04-11".to_string(),
            updated_at: "2026-04-11".to_string(),
        };
        repo.upsert_article(&article).unwrap();

        let results = repo.search_by_similarity("w1", &emb, 5).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].1 > 0.99); // cosine similarity ~1.0
    }

    #[test]
    fn test_delete_article() {
        let (_tmp, repo) = setup();

        let article = WikiArticle {
            id: "art-1".to_string(),
            ward_id: "w1".to_string(),
            agent_id: "root".to_string(),
            title: "topic-a".to_string(),
            content: "content".to_string(),
            tags: None,
            source_fact_ids: None,
            embedding: None,
            version: 1,
            created_at: "2026-04-11".to_string(),
            updated_at: "2026-04-11".to_string(),
        };
        repo.upsert_article(&article).unwrap();

        assert!(repo.delete_article("w1", "topic-a").unwrap());
        assert!(repo.get_article("w1", "topic-a").unwrap().is_none());
    }

    #[test]
    fn test_count_articles() {
        let (_tmp, repo) = setup();

        assert_eq!(repo.count_articles("w1").unwrap(), 0);

        for i in 0..3 {
            let article = WikiArticle {
                id: format!("art-{i}"),
                ward_id: "w1".to_string(),
                agent_id: "root".to_string(),
                title: format!("topic-{i}"),
                content: format!("content {i}"),
                tags: None,
                source_fact_ids: None,
                embedding: None,
                version: 1,
                created_at: "2026-04-11".to_string(),
                updated_at: "2026-04-11".to_string(),
            };
            repo.upsert_article(&article).unwrap();
        }

        assert_eq!(repo.count_articles("w1").unwrap(), 3);
    }
}
