//! Repository for ward wiki articles — compiled knowledge per ward.

use crate::connection::DatabaseManager;
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
    pub embedding: Option<Vec<f32>>,
    pub version: i32,
    pub created_at: String,
    pub updated_at: String,
}

/// Repository for ward wiki article CRUD and vector search.
pub struct WardWikiRepository {
    db: Arc<DatabaseManager>,
}

impl WardWikiRepository {
    pub fn new(db: Arc<DatabaseManager>) -> Self {
        Self { db }
    }

    /// List all articles for a ward.
    pub fn list_articles(&self, ward_id: &str) -> Result<Vec<WikiArticle>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, ward_id, agent_id, title, content, tags, source_fact_ids, \
                 embedding, version, created_at, updated_at \
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
                 embedding, version, created_at, updated_at \
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
    pub fn upsert_article(&self, article: &WikiArticle) -> Result<(), String> {
        self.db.with_connection(|conn| {
            let embedding_bytes = article
                .embedding
                .as_ref()
                .map(|e| e.iter().flat_map(|f| f.to_le_bytes()).collect::<Vec<u8>>());

            conn.execute(
                "INSERT INTO ward_wiki_articles \
                 (id, ward_id, agent_id, title, content, tags, source_fact_ids, \
                  embedding, version, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11) \
                 ON CONFLICT(ward_id, title) DO UPDATE SET \
                 content = excluded.content, \
                 tags = excluded.tags, \
                 source_fact_ids = excluded.source_fact_ids, \
                 embedding = excluded.embedding, \
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
                    embedding_bytes,
                    article.version,
                    article.created_at,
                    article.updated_at,
                ],
            )?;

            Ok(())
        })
    }

    /// Search articles by embedding similarity for a ward.
    pub fn search_by_similarity(
        &self,
        ward_id: &str,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<(WikiArticle, f64)>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, ward_id, agent_id, title, content, tags, source_fact_ids, \
                 embedding, version, created_at, updated_at \
                 FROM ward_wiki_articles \
                 WHERE ward_id = ?1 AND embedding IS NOT NULL",
            )?;

            let mut scored: Vec<(WikiArticle, f64)> = stmt
                .query_map(params![ward_id], |row| Ok(Self::row_to_article(row)))?
                .filter_map(|r| r.ok())
                .filter_map(|article| {
                    let embedding = article.embedding.as_ref()?;
                    let sim = cosine_similarity(query_embedding, embedding);
                    Some((article, sim))
                })
                .collect();

            scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            scored.truncate(limit);
            Ok(scored)
        })
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

    fn row_to_article(row: &rusqlite::Row) -> WikiArticle {
        let embedding_blob: Option<Vec<u8>> = row.get(7).ok().flatten();
        let embedding = embedding_blob.map(|blob| {
            blob.chunks_exact(4)
                .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .collect()
        });

        WikiArticle {
            id: row.get(0).unwrap_or_default(),
            ward_id: row.get(1).unwrap_or_default(),
            agent_id: row.get(2).unwrap_or_default(),
            title: row.get(3).unwrap_or_default(),
            content: row.get(4).unwrap_or_default(),
            tags: row.get(5).ok().flatten(),
            source_fact_ids: row.get(6).ok().flatten(),
            embedding,
            version: row.get(8).unwrap_or(1),
            created_at: row.get(9).unwrap_or_default(),
            updated_at: row.get(10).unwrap_or_default(),
        }
    }
}

/// Cosine similarity between two f32 vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0_f64;
    let mut norm_a = 0.0_f64;
    let mut norm_b = 0.0_f64;
    for (x, y) in a.iter().zip(b.iter()) {
        let x = *x as f64;
        let y = *y as f64;
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom == 0.0 {
        0.0
    } else {
        dot / denom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_db() -> Arc<DatabaseManager> {
        use gateway_services::VaultPaths;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let paths = Arc::new(VaultPaths::new(temp_dir.path().to_path_buf()));
        let _ = temp_dir.keep();
        let db = DatabaseManager::new(paths).unwrap();
        Arc::new(db)
    }

    #[test]
    fn test_upsert_and_get_article() {
        let db = setup_test_db();
        let repo = WardWikiRepository::new(db);

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
        let db = setup_test_db();
        let repo = WardWikiRepository::new(db);

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
        let db = setup_test_db();
        let repo = WardWikiRepository::new(db);

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
        let db = setup_test_db();
        let repo = WardWikiRepository::new(db);

        let article = WikiArticle {
            id: "art-1".to_string(),
            ward_id: "w1".to_string(),
            agent_id: "root".to_string(),
            title: "topic-a".to_string(),
            content: "content".to_string(),
            tags: None,
            source_fact_ids: None,
            embedding: Some(vec![1.0, 0.0, 0.0]),
            version: 1,
            created_at: "2026-04-11".to_string(),
            updated_at: "2026-04-11".to_string(),
        };
        repo.upsert_article(&article).unwrap();

        let query = vec![1.0, 0.0, 0.0]; // identical to article
        let results = repo.search_by_similarity("w1", &query, 5).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].1 > 0.99); // cosine similarity ~1.0
    }

    #[test]
    fn test_delete_article() {
        let db = setup_test_db();
        let repo = WardWikiRepository::new(db);

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
    fn test_cosine_similarity() {
        assert!((cosine_similarity(&[1.0, 0.0], &[1.0, 0.0]) - 1.0).abs() < 0.001);
        assert!((cosine_similarity(&[1.0, 0.0], &[0.0, 1.0])).abs() < 0.001);
        assert!((cosine_similarity(&[], &[])).abs() < 0.001);
    }

    #[test]
    fn test_count_articles() {
        let db = setup_test_db();
        let repo = WardWikiRepository::new(db);

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
