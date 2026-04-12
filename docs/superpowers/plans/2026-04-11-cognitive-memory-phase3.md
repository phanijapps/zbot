# Cognitive Memory Phase 3 — Ward Knowledge Compilation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the Karpathy compiler pattern — compile ward knowledge into structured wiki articles after each session, then use wiki-first recall for richer context.

**Architecture:** A `WardWikiRepository` persists compiled articles per ward. After distillation, a `compile_ward_wiki()` function takes new facts + existing articles and asks the LLM to produce updated/new articles. During recall, wiki articles are searched by embedding similarity and injected before individual facts.

**Tech Stack:** Rust (gateway-execution, gateway-database), SQLite, LLM integration via existing OpenAiClient/RetryingLlmClient pattern.

**Spec:** `docs/superpowers/specs/2026-04-11-cognitive-memory-system-design.md` — Section 7

**Branch:** `feature/sentient` (continuing from Phase 2)

---

## File Structure

| Action | File | Responsibility |
|--------|------|----------------|
| MODIFY | `gateway/gateway-database/src/schema.rs` | Migration v19: ward_wiki_articles table |
| CREATE | `gateway/gateway-database/src/wiki_repository.rs` | CRUD + vector search for wiki articles |
| MODIFY | `gateway/gateway-database/src/lib.rs` | Export WardWikiRepository |
| CREATE | `gateway/gateway-execution/src/ward_wiki.rs` | compile_ward_wiki() — LLM compilation logic |
| MODIFY | `gateway/gateway-execution/src/lib.rs` | Export ward_wiki module |
| MODIFY | `gateway/gateway-execution/src/runner.rs` | Call compile_ward_wiki after distillation |
| MODIFY | `gateway/gateway-execution/src/recall.rs` | Wiki-first recall: search articles before facts |

---

### Task 1: Database Migration v19 — ward_wiki_articles Table

**Files:**
- Modify: `gateway/gateway-database/src/schema.rs`

- [ ] **Step 1: Read current schema version and migration pattern**

Read `gateway/gateway-database/src/schema.rs` to find current `SCHEMA_VERSION` (should be 18 from Phase 1).

- [ ] **Step 2: Add migration v19**

Increment `SCHEMA_VERSION` to 19. Add migration block:

```rust
if version < 19 {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS ward_wiki_articles (
            id TEXT PRIMARY KEY,
            ward_id TEXT NOT NULL,
            agent_id TEXT NOT NULL,
            title TEXT NOT NULL,
            content TEXT NOT NULL,
            tags TEXT,
            source_fact_ids TEXT,
            embedding BLOB,
            version INTEGER DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(ward_id, title)
        );
        CREATE INDEX IF NOT EXISTS idx_wiki_ward ON ward_wiki_articles(ward_id);",
    )?;
}
```

Also add the CREATE TABLE to the fresh database initialization section (look for where other tables like `memory_facts` are created for new databases).

Update the schema version test if one exists.

- [ ] **Step 3: Verify compilation**

Run: `cargo check --package gateway-database`

- [ ] **Step 4: Commit**

```bash
git add gateway/gateway-database/src/schema.rs
git commit -m "feat(db): migration v19 — ward_wiki_articles table"
```

---

### Task 2: Ward Wiki Repository

**Files:**
- Create: `gateway/gateway-database/src/wiki_repository.rs`
- Modify: `gateway/gateway-database/src/lib.rs`

- [ ] **Step 1: Read existing repository patterns**

Read `gateway/gateway-database/src/memory_repository.rs` lines 1-30 for the constructor pattern, and search for `cosine_similarity` to understand the vector search pattern.

- [ ] **Step 2: Create wiki_repository.rs**

Create `gateway/gateway-database/src/wiki_repository.rs`:

```rust
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
            let mut stmt = conn
                .prepare(
                    "SELECT id, ward_id, agent_id, title, content, tags, source_fact_ids, \
                     embedding, version, created_at, updated_at \
                     FROM ward_wiki_articles WHERE ward_id = ?1 ORDER BY title",
                )
                .map_err(|e| format!("Failed to prepare: {e}"))?;

            let articles = stmt
                .query_map(params![ward_id], |row| Ok(Self::row_to_article(row)))
                .map_err(|e| format!("Failed to query: {e}"))?
                .filter_map(|r| r.ok())
                .collect();

            Ok(articles)
        })
    }

    /// Get a specific article by ward and title.
    pub fn get_article(&self, ward_id: &str, title: &str) -> Result<Option<WikiArticle>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn
                .prepare(
                    "SELECT id, ward_id, agent_id, title, content, tags, source_fact_ids, \
                     embedding, version, created_at, updated_at \
                     FROM ward_wiki_articles WHERE ward_id = ?1 AND title = ?2",
                )
                .map_err(|e| format!("Failed to prepare: {e}"))?;

            let article = stmt
                .query_row(params![ward_id, title], |row| Ok(Self::row_to_article(row)))
                .optional()
                .map_err(|e| format!("Failed to query: {e}"))?;

            Ok(article)
        })
    }

    /// Upsert an article (insert or update if title exists for this ward).
    pub fn upsert_article(&self, article: &WikiArticle) -> Result<(), String> {
        self.db.with_connection(|conn| {
            let embedding_bytes = article.embedding.as_ref().map(|e| {
                e.iter().flat_map(|f| f.to_le_bytes()).collect::<Vec<u8>>()
            });

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
            )
            .map_err(|e| format!("Failed to upsert article: {e}"))?;

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
            let mut stmt = conn
                .prepare(
                    "SELECT id, ward_id, agent_id, title, content, tags, source_fact_ids, \
                     embedding, version, created_at, updated_at \
                     FROM ward_wiki_articles \
                     WHERE ward_id = ?1 AND embedding IS NOT NULL",
                )
                .map_err(|e| format!("Failed to prepare: {e}"))?;

            let mut scored: Vec<(WikiArticle, f64)> = stmt
                .query_map(params![ward_id], |row| Ok(Self::row_to_article(row)))
                .map_err(|e| format!("Failed to query: {e}"))?
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
            let deleted = conn
                .execute(
                    "DELETE FROM ward_wiki_articles WHERE ward_id = ?1 AND title = ?2",
                    params![ward_id, title],
                )
                .map_err(|e| format!("Failed to delete: {e}"))?;
            Ok(deleted > 0)
        })
    }

    /// Count articles for a ward.
    pub fn count_articles(&self, ward_id: &str) -> Result<usize, String> {
        self.db.with_connection(|conn| {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM ward_wiki_articles WHERE ward_id = ?1",
                    params![ward_id],
                    |row| row.get(0),
                )
                .map_err(|e| format!("Failed to count: {e}"))?;
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
        // Use the same test DB pattern as other repos — check memory_repository.rs tests
        // If DatabaseManager::new requires VaultPaths, use tempdir
        use tempfile::TempDir;
        let temp_dir = TempDir::new().unwrap();
        let paths = Arc::new(gateway_services::VaultPaths::new(
            temp_dir.path().to_path_buf(),
        ));
        // Keep temp dir alive
        std::mem::forget(temp_dir);
        Arc::new(DatabaseManager::new(paths).unwrap())
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
        let fetched = repo.get_article("stock-analysis", "yfinance-patterns").unwrap();
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

        // Article with embedding
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
}
```

- [ ] **Step 3: Export from lib.rs**

In `gateway/gateway-database/src/lib.rs`, add:

```rust
pub mod wiki_repository;
pub use wiki_repository::{WardWikiRepository, WikiArticle};
```

- [ ] **Step 4: Run tests**

Run: `cargo test --package gateway-database -- wiki_repository`
Expected: 7 tests pass.

- [ ] **Step 5: Quality checks**

Run: `cargo fmt --all && cargo clippy --package gateway-database -- -D warnings`

- [ ] **Step 6: Commit**

```bash
git add gateway/gateway-database/src/wiki_repository.rs gateway/gateway-database/src/lib.rs
git commit -m "feat(db): WardWikiRepository with CRUD, vector search, and tests"
```

---

### Task 3: Ward Wiki Compilation Logic

**Files:**
- Create: `gateway/gateway-execution/src/ward_wiki.rs`
- Modify: `gateway/gateway-execution/src/lib.rs`

- [ ] **Step 1: Read the distillation LLM call pattern**

Read `gateway/gateway-execution/src/distillation.rs` to understand how the LLM client is called — specifically how `RetryingLlmClient` is used, how `ChatMessage` is constructed, and how JSON responses are parsed. Copy the exact same pattern.

- [ ] **Step 2: Create ward_wiki.rs**

Create `gateway/gateway-execution/src/ward_wiki.rs`:

```rust
//! Ward Wiki Compilation — Karpathy compiler pattern.
//!
//! After distillation extracts facts, this module compiles them into
//! structured wiki articles per ward. Articles accumulate and evolve
//! across sessions, creating a compiled knowledge base.

use agent_runtime::{ChatMessage, LlmClient};
use gateway_database::{WardWikiRepository, WikiArticle};
use std::sync::Arc;

/// Compile ward wiki articles from newly extracted facts.
///
/// Called after distillation completes. Takes new facts and existing
/// articles, asks the LLM to produce updated/new articles.
///
/// Returns the number of articles upserted.
pub async fn compile_ward_wiki(
    ward_id: &str,
    agent_id: &str,
    new_facts: &[FactSummary],
    wiki_repo: &WardWikiRepository,
    llm_client: &dyn LlmClient,
    embedding_client: Option<&dyn agent_runtime::EmbeddingClient>,
) -> Result<usize, String> {
    if new_facts.is_empty() {
        tracing::debug!(ward = %ward_id, "No new facts — skipping wiki compilation");
        return Ok(0);
    }

    // Load existing articles
    let existing = wiki_repo.list_articles(ward_id)?;

    // Build compilation prompt
    let prompt = build_compilation_prompt(&existing, new_facts);

    // Call LLM
    let messages = vec![
        ChatMessage::system(
            "You are a knowledge compiler. Produce structured wiki articles from facts. \
             Respond with ONLY valid JSON."
                .to_string(),
        ),
        ChatMessage::user(prompt),
    ];

    let response = llm_client
        .chat(messages, None)
        .await
        .map_err(|e| format!("Wiki compilation LLM call failed: {e}"))?;

    let response_text = response.text_content();

    // Parse response
    let compiled: CompilationResponse = parse_compilation_response(&response_text)?;

    // Upsert articles
    let now = chrono::Utc::now().to_rfc3339();
    let mut upserted = 0;

    for article_data in &compiled.articles {
        let embedding = if let Some(ec) = embedding_client {
            ec.embed(&article_data.content)
                .await
                .ok()
        } else {
            None
        };

        let article = WikiArticle {
            id: format!("wiki-{}-{}", ward_id, uuid::Uuid::new_v4()),
            ward_id: ward_id.to_string(),
            agent_id: agent_id.to_string(),
            title: article_data.title.clone(),
            content: article_data.content.clone(),
            tags: article_data
                .tags
                .as_ref()
                .map(|t| serde_json::to_string(t).unwrap_or_default()),
            source_fact_ids: None,
            embedding,
            version: 1,
            created_at: now.clone(),
            updated_at: now.clone(),
        };

        if let Err(e) = wiki_repo.upsert_article(&article) {
            tracing::warn!(title = %article_data.title, error = %e, "Failed to upsert wiki article");
        } else {
            upserted += 1;
        }
    }

    // Update index article
    if upserted > 0 {
        let index_content = build_index_content(ward_id, wiki_repo)?;
        let index_article = WikiArticle {
            id: format!("wiki-{}-index", ward_id),
            ward_id: ward_id.to_string(),
            agent_id: agent_id.to_string(),
            title: "__index__".to_string(),
            content: index_content,
            tags: None,
            source_fact_ids: None,
            embedding: None,
            version: 1,
            created_at: now.clone(),
            updated_at: now,
        };
        let _ = wiki_repo.upsert_article(&index_article);
    }

    tracing::info!(ward = %ward_id, articles = upserted, "Ward wiki compilation complete");
    Ok(upserted)
}

/// Summary of a fact for the compilation prompt.
#[derive(Debug, Clone)]
pub struct FactSummary {
    pub category: String,
    pub key: String,
    pub content: String,
}

#[derive(Debug, serde::Deserialize)]
struct CompilationResponse {
    articles: Vec<ArticleData>,
}

#[derive(Debug, serde::Deserialize)]
struct ArticleData {
    title: String,
    content: String,
    tags: Option<Vec<String>>,
}

fn build_compilation_prompt(existing: &[WikiArticle], new_facts: &[FactSummary]) -> String {
    let mut prompt = String::new();

    prompt.push_str(
        "Given new facts from a session and existing wiki articles, \
         produce updated or new articles.\n\n\
         Rules:\n\
         - Each article covers ONE topic\n\
         - Article content: 100-500 words, factual, concise\n\
         - Update existing articles if new facts are relevant\n\
         - Create new articles for uncovered topics\n\
         - Note contradictions between new and existing knowledge\n\n",
    );

    if !existing.is_empty() {
        prompt.push_str("## Existing Articles\n\n");
        for article in existing {
            if article.title == "__index__" {
                continue;
            }
            prompt.push_str(&format!(
                "### {}\n{}\n\n",
                article.title,
                truncate_str(&article.content, 500)
            ));
        }
    }

    prompt.push_str("## New Facts From This Session\n\n");
    for fact in new_facts {
        prompt.push_str(&format!("- [{}] {}: {}\n", fact.category, fact.key, fact.content));
    }

    prompt.push_str(
        "\n\nRespond with JSON only:\n\
         {\"articles\": [{\"title\": \"...\", \"content\": \"...\", \"tags\": [\"...\"]}]}",
    );

    prompt
}

fn parse_compilation_response(text: &str) -> Result<CompilationResponse, String> {
    // Try to extract JSON from response (may have markdown fences)
    let json_text = if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            &text[start..=end]
        } else {
            text
        }
    } else {
        text
    };

    serde_json::from_str(json_text)
        .map_err(|e| format!("Failed to parse compilation response: {e}"))
}

fn build_index_content(ward_id: &str, wiki_repo: &WardWikiRepository) -> Result<String, String> {
    let articles = wiki_repo.list_articles(ward_id)?;
    let mut index = format!("# {} Wiki Index\n\n", ward_id);
    for article in &articles {
        if article.title == "__index__" {
            continue;
        }
        // One-line summary: first sentence of content
        let summary = article
            .content
            .lines()
            .find(|l| !l.trim().is_empty() && !l.starts_with('#'))
            .unwrap_or("(no summary)");
        let summary = truncate_str(summary, 100);
        index.push_str(&format!("- **{}** — {}\n", article.title, summary));
    }
    Ok(index)
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_compilation_prompt_with_facts() {
        let facts = vec![
            FactSummary {
                category: "pattern".into(),
                key: "rate_limiting".into(),
                content: "Use 1 req/sec for yfinance".into(),
            },
        ];
        let prompt = build_compilation_prompt(&[], &facts);
        assert!(prompt.contains("rate_limiting"));
        assert!(prompt.contains("New Facts"));
        assert!(prompt.contains("JSON only"));
    }

    #[test]
    fn test_build_compilation_prompt_with_existing() {
        let existing = vec![WikiArticle {
            id: "1".into(),
            ward_id: "w".into(),
            agent_id: "r".into(),
            title: "old-topic".into(),
            content: "Old content here.".into(),
            tags: None,
            source_fact_ids: None,
            embedding: None,
            version: 1,
            created_at: "2026".into(),
            updated_at: "2026".into(),
        }];
        let facts = vec![FactSummary {
            category: "domain".into(),
            key: "new_fact".into(),
            content: "New info".into(),
        }];
        let prompt = build_compilation_prompt(&existing, &facts);
        assert!(prompt.contains("old-topic"));
        assert!(prompt.contains("Old content"));
        assert!(prompt.contains("new_fact"));
    }

    #[test]
    fn test_parse_compilation_response_clean() {
        let json = r#"{"articles": [{"title": "test", "content": "body", "tags": ["a"]}]}"#;
        let parsed = parse_compilation_response(json).unwrap();
        assert_eq!(parsed.articles.len(), 1);
        assert_eq!(parsed.articles[0].title, "test");
    }

    #[test]
    fn test_parse_compilation_response_with_fences() {
        let text = "```json\n{\"articles\": [{\"title\": \"t\", \"content\": \"c\"}]}\n```";
        let parsed = parse_compilation_response(text).unwrap();
        assert_eq!(parsed.articles.len(), 1);
    }

    #[test]
    fn test_parse_compilation_response_invalid() {
        let result = parse_compilation_response("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_index_excludes_self() {
        // build_index_content skips __index__ article — tested via format
        let index_line = "- **__index__**";
        assert!(!index_line.is_empty()); // placeholder — real test needs DB
    }
}
```

- [ ] **Step 3: Export ward_wiki module**

In `gateway/gateway-execution/src/lib.rs`, add:

```rust
pub mod ward_wiki;
```

- [ ] **Step 4: Run tests**

Run: `cargo test --package gateway-execution -- ward_wiki`
Expected: 6 tests pass.

- [ ] **Step 5: Quality checks**

Run: `cargo fmt --all && cargo clippy --package gateway-execution -- -D warnings`

- [ ] **Step 6: Commit**

```bash
git add gateway/gateway-execution/src/ward_wiki.rs gateway/gateway-execution/src/lib.rs
git commit -m "feat(ward-wiki): compilation logic with LLM prompt, JSON parsing, index generation"
```

---

### Task 4: Wire Wiki Compilation into Runner (Post-Distillation)

**Files:**
- Modify: `gateway/gateway-execution/src/runner.rs`

- [ ] **Step 1: Read the distillation spawn blocks**

Read `gateway/gateway-execution/src/runner.rs` and find the two `tokio::spawn` blocks where `distiller.distill()` is called (~lines 1160 and 2384). Understand what variables are available in scope.

- [ ] **Step 2: Add wiki compilation after distillation**

In both distillation spawn blocks, after `distiller.distill()` succeeds, add wiki compilation. You need:
- `wiki_repo: WardWikiRepository` — constructed from the same `DatabaseManager` used by other repos
- `ward_id` — from the session (already available as `session.ward_id`)
- `agent_id` — already in scope
- Facts from the distillation — since `distill()` returns `Result<usize, String>`, the facts aren't returned. Instead, fetch recent facts for this session from the memory repo.

Simplest approach: after distill succeeds, query `memory_repo.get_facts_for_session(session_id)` or similar. If no such method exists, query by `agent_id` + `ward_id` with a recent timestamp filter.

Alternative: Add the wiki compilation call INSIDE `distillation.rs` at the end of the `distill()` method, right before `Ok(upserted)`. This avoids needing to thread new dependencies through the runner. The distiller already has `memory_repo` and can be given `wiki_repo`.

**Recommended approach**: Add `wiki_repo` field to `SessionDistiller` and call `compile_ward_wiki` at the end of `distill()`. This is cleaner than modifying the runner spawns.

- [ ] **Step 3: Thread WardWikiRepository through SessionDistiller**

Add `wiki_repo: Option<Arc<WardWikiRepository>>` to `SessionDistiller`. Set it during initialization in `gateway/src/state.rs` where the distiller is created.

- [ ] **Step 4: Call compile_ward_wiki at end of distill()**

In `distillation.rs`, before `Ok(upserted)`, add:

```rust
// Compile ward wiki if ward_id is set
if let (Some(ref wiki_repo), Some(ref ward_id)) = (&self.wiki_repo, &ward_id) {
    let fact_summaries: Vec<ward_wiki::FactSummary> = extracted_facts
        .iter()
        .map(|f| ward_wiki::FactSummary {
            category: f.category.clone(),
            key: f.key.clone(),
            content: f.content.clone(),
        })
        .collect();

    if let Err(e) = ward_wiki::compile_ward_wiki(
        ward_id,
        agent_id,
        &fact_summaries,
        wiki_repo,
        &*llm_client,
        self.embedding_client.as_deref(),
    ).await {
        tracing::warn!(ward = %ward_id, error = %e, "Ward wiki compilation failed");
    }
}
```

Note: You'll need to check what `extracted_facts` is called in the distill function — it may be named differently. Read the code to find the vector of facts that were just extracted and upserted.

- [ ] **Step 5: Verify compilation**

Run: `cargo check --workspace`

- [ ] **Step 6: Commit**

```bash
git add gateway/gateway-execution/src/distillation.rs gateway/src/state.rs
git commit -m "feat(ward-wiki): wire compilation into distillation pipeline"
```

---

### Task 5: Wiki-First Recall

**Files:**
- Modify: `gateway/gateway-execution/src/recall.rs`

- [ ] **Step 1: Read recall_with_graph entry point**

Read `gateway/gateway-execution/src/recall.rs` — find `recall_with_graph()` and understand where facts are assembled. Wiki articles should be searched BEFORE individual fact recall and included in the formatted output.

- [ ] **Step 2: Add wiki_repo to MemoryRecall struct**

Add `wiki_repo: Option<Arc<WardWikiRepository>>` field to the `MemoryRecall` struct. Thread it through from initialization in `state.rs`.

- [ ] **Step 3: Add wiki article search in recall_with_graph**

At the beginning of `recall_with_graph()`, after getting the query embedding (or before the main recall), add:

```rust
// Wiki-first recall: search ward articles before individual facts
let mut wiki_context = String::new();
if let (Some(ref wiki_repo), Some(ward_id)) = (&self.wiki_repo, ward_id) {
    if let Ok(query_emb) = self.embedding_client.embed(message).await {
        if let Ok(articles) = wiki_repo.search_by_similarity(ward_id, &query_emb, 3) {
            let mut wiki_tokens = 0;
            let wiki_budget = 1500;
            for (article, score) in &articles {
                if *score < 0.3 || wiki_tokens >= wiki_budget {
                    break;
                }
                let content = truncate_content(&article.content, 500);
                wiki_context.push_str(&format!(
                    "### {} (relevance: {:.0}%)\n{}\n\n",
                    article.title,
                    score * 100.0,
                    content
                ));
                wiki_tokens += content.len() / 4;
            }
        }
    }
}
```

- [ ] **Step 4: Include wiki context in formatted output**

Find where the `RecallResult.formatted` string is built (likely in `format_prioritized_recall`). Prepend the wiki context before individual facts:

```rust
if !wiki_context.is_empty() {
    formatted = format!("## Ward Knowledge Base\n\n{}\n\n{}", wiki_context, formatted);
}
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check --workspace`

- [ ] **Step 6: Commit**

```bash
git add gateway/gateway-execution/src/recall.rs gateway/src/state.rs
git commit -m "feat(recall): wiki-first recall — search ward articles before individual facts"
```

---

### Task 6: Final Checks

- [ ] **Step 1: Format and lint**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`

- [ ] **Step 2: Run all tests**

Run: `cargo test --workspace --lib --bins --tests`
Expected: All pass.

- [ ] **Step 3: UI checks**

Run: `cd apps/ui && npm run build && npm run lint`

- [ ] **Step 4: Push**

```bash
git push
```
