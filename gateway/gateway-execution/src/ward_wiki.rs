// ============================================================================
// WARD WIKI COMPILATION — Karpathy compiler pattern
// ============================================================================

//! After distillation extracts facts, this module compiles them into
//! structured wiki articles per ward. Articles accumulate and evolve
//! across sessions, creating a compiled knowledge base.

use agent_runtime::llm::client::LlmClient;
use agent_runtime::llm::embedding::EmbeddingClient;
use agent_runtime::types::ChatMessage;
use gateway_database::{WardWikiRepository, WikiArticle};
use serde::Deserialize;

/// Summary of a fact for the compilation prompt.
#[derive(Debug, Clone)]
pub struct FactSummary {
    pub category: String,
    pub key: String,
    pub content: String,
}

/// LLM compilation output.
#[derive(Debug, Deserialize)]
struct CompilationResponse {
    articles: Vec<ArticleData>,
}

/// A single article from the LLM response.
#[derive(Debug, Deserialize)]
struct ArticleData {
    title: String,
    content: String,
    tags: Option<Vec<String>>,
}

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
    embedding_client: Option<&dyn EmbeddingClient>,
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

    let response_text = &response.content;

    // Parse response
    let compiled: CompilationResponse = parse_compilation_response(response_text)?;

    // Upsert articles
    let now = chrono::Utc::now().to_rfc3339();
    let mut upserted = 0;

    for article_data in &compiled.articles {
        let embedding = match embedding_client {
            Some(ec) => ec
                .embed(&[article_data.content.as_str()])
                .await
                .ok()
                .and_then(|mut vecs| {
                    if vecs.is_empty() {
                        None
                    } else {
                        Some(vecs.remove(0))
                    }
                }),
            None => None,
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
        prompt.push_str(&format!(
            "- [{}] {}: {}\n",
            fact.category, fact.key, fact.content
        ));
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

/// Truncate a string to `max_len` characters, appending "..." if truncated.
/// Uses char boundaries to avoid panicking on multi-byte characters.
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let end: usize = s
            .char_indices()
            .nth(max_len)
            .map_or(s.len(), |(idx, _)| idx);
        format!("{}...", &s[..end])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_compilation_prompt_with_facts() {
        let facts = vec![FactSummary {
            category: "pattern".into(),
            key: "rate_limiting".into(),
            content: "Use 1 req/sec for yfinance".into(),
        }];
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
    fn test_truncate_str() {
        assert_eq!(truncate_str("hello", 10), "hello");
        assert_eq!(truncate_str("hello world", 5), "hello...");
        // Multi-byte safety
        assert_eq!(truncate_str("héllo", 3), "hél...");
    }
}
