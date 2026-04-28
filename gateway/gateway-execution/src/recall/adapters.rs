//! Adapters: project each recall source into a uniform [`ScoredItem`].
//!
//! Callers construct source-specific results via existing repository search
//! methods and pass them through these pure functions together with a
//! per-source relevance score. The resulting `Vec<ScoredItem>` lists are
//! consumed by `rrf_merge`.

use crate::recall::scored_item::{ItemKind, Provenance, ScoredItem};
use zero_stores_sqlite::{MemoryFact, Procedure, WikiArticle};

/// Project a [`MemoryFact`] into a [`ScoredItem`].
///
/// The content string combines the category, key, and body so the caller's
/// downstream prompt always has full context without needing extra lookups.
pub fn fact_to_item(fact: &MemoryFact, score: f64) -> ScoredItem {
    ScoredItem {
        kind: ItemKind::Fact,
        id: fact.id.clone(),
        content: format!("[{}] {}: {}", fact.category, fact.key, fact.content),
        score,
        provenance: Provenance {
            source: "memory_facts".to_string(),
            source_id: fact.id.clone(),
            session_id: fact.session_id.clone(),
            ward_id: Some(fact.ward_id.clone()),
        },
    }
}

/// Project a [`WikiArticle`] into a [`ScoredItem`].
///
/// The content string uses a Markdown heading so it renders cleanly when
/// injected into a prompt.
pub fn wiki_to_item(article: &WikiArticle, score: f64) -> ScoredItem {
    ScoredItem {
        kind: ItemKind::Wiki,
        id: article.id.clone(),
        content: format!("# {}\n{}", article.title, article.content),
        score,
        provenance: Provenance {
            source: "ward_wiki".to_string(),
            source_id: article.id.clone(),
            session_id: None,
            ward_id: Some(article.ward_id.clone()),
        },
    }
}

/// Project a [`Procedure`] into a [`ScoredItem`].
///
/// Combines name, description, and steps so the agent receives the full
/// how-to text without a second round-trip to the database.
pub fn procedure_to_item(proc: &Procedure, score: f64) -> ScoredItem {
    ScoredItem {
        kind: ItemKind::Procedure,
        id: proc.id.clone(),
        content: format!(
            "Procedure: {}\n{}\nSteps: {}",
            proc.name, proc.description, proc.steps
        ),
        score,
        provenance: Provenance {
            source: "procedures".to_string(),
            source_id: proc.id.clone(),
            session_id: None,
            ward_id: proc.ward_id.clone(),
        },
    }
}

use std::sync::Arc;
use zero_stores_sqlite::kg::storage::GraphStorage;

/// ANN-query `kg_name_index` for entities whose name embedding is closest to
/// `query_embedding`, and project each hit as a [`ScoredItem::GraphNode`].
///
/// The returned `ScoredItem::score` is rank-discounted by cosine similarity
/// so higher-ranked, higher-similarity entities lead — `rrf_merge` re-scores
/// via rank during fusion, so this per-source score only matters for the
/// adapter-local order, which we preserve by sorting by `(rank, cosine)`.
pub async fn graph_ann_to_items(
    graph: &Arc<GraphStorage>,
    query_embedding: &[f32],
    top_k: usize,
    agent_id: &str,
) -> Result<Vec<ScoredItem>, String> {
    if query_embedding.is_empty() || top_k == 0 {
        return Ok(Vec::new());
    }
    let results = graph
        .search_entities_by_name_embedding(query_embedding, top_k, agent_id)
        .map_err(|e| format!("graph ANN search failed: {e}"))?;

    let mut out = Vec::with_capacity(results.len());
    for (idx, (name, entity_type, dist)) in results.into_iter().enumerate() {
        // L2-squared on normalized vectors → cosine similarity = 1 - dist/2.
        let cosine = 1.0 - (dist as f64) / 2.0;
        let rank_one = (idx as f64) + 1.0;
        let score = (1.0 / rank_one) * cosine;
        out.push(ScoredItem {
            kind: ItemKind::GraphNode,
            id: format!("graph:{name}"),
            content: format!("Entity: {name} [{entity_type}] (cosine ~ {cosine:.2})"),
            score,
            provenance: Provenance {
                source: "kg_name_index".to_string(),
                source_id: name,
                session_id: None,
                ward_id: None,
            },
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_fact() -> MemoryFact {
        MemoryFact {
            id: "fact-1".to_string(),
            session_id: Some("sess-42".to_string()),
            agent_id: "agent-a".to_string(),
            scope: "local".to_string(),
            category: "user".to_string(),
            key: "name".to_string(),
            content: "Alice".to_string(),
            confidence: 0.95,
            mention_count: 3,
            source_summary: None,
            embedding: None,
            ward_id: "ward-x".to_string(),
            contradicted_by: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
            expires_at: None,
            valid_from: None,
            valid_until: None,
            superseded_by: None,
            pinned: false,
            epistemic_class: None,
            source_episode_id: None,
            source_ref: None,
        }
    }

    fn make_article() -> WikiArticle {
        WikiArticle {
            id: "wiki-1".to_string(),
            ward_id: "ward-x".to_string(),
            agent_id: "agent-a".to_string(),
            title: "Project Alpha".to_string(),
            content: "Alpha is the first project.".to_string(),
            tags: None,
            source_fact_ids: None,
            embedding: None,
            version: 1,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        }
    }

    fn make_procedure() -> Procedure {
        Procedure {
            id: "proc-1".to_string(),
            agent_id: "agent-a".to_string(),
            ward_id: Some("ward-x".to_string()),
            name: "Deploy".to_string(),
            description: "Deploy to production.".to_string(),
            trigger_pattern: None,
            steps: "1. Build\n2. Push\n3. Restart".to_string(),
            parameters: None,
            success_count: 5,
            failure_count: 0,
            avg_duration_ms: None,
            avg_token_cost: None,
            last_used: None,
            embedding: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn fact_adapter_populates_all_fields() {
        let fact = make_fact();
        let item = fact_to_item(&fact, 0.88);

        assert_eq!(item.kind, ItemKind::Fact);
        assert_eq!(item.id, "fact-1");
        assert!(!item.content.is_empty());
        assert!(item.content.contains("user"));
        assert!(item.content.contains("name"));
        assert!(item.content.contains("Alice"));
        assert!((item.score - 0.88).abs() < f64::EPSILON);
        assert_eq!(item.provenance.source, "memory_facts");
        assert_eq!(item.provenance.source_id, "fact-1");
        assert_eq!(item.provenance.session_id, Some("sess-42".to_string()));
        assert_eq!(item.provenance.ward_id, Some("ward-x".to_string()));
    }

    #[test]
    fn wiki_adapter_populates_all_fields() {
        let article = make_article();
        let item = wiki_to_item(&article, 0.75);

        assert_eq!(item.kind, ItemKind::Wiki);
        assert_eq!(item.id, "wiki-1");
        assert!(!item.content.is_empty());
        assert!(item.content.contains("Project Alpha"));
        assert!(item.content.contains("Alpha is the first project."));
        assert!((item.score - 0.75).abs() < f64::EPSILON);
        assert_eq!(item.provenance.source, "ward_wiki");
        assert_eq!(item.provenance.source_id, "wiki-1");
        assert_eq!(item.provenance.session_id, None);
        assert_eq!(item.provenance.ward_id, Some("ward-x".to_string()));
    }

    #[test]
    fn procedure_adapter_populates_all_fields() {
        let proc = make_procedure();
        let item = procedure_to_item(&proc, 0.60);

        assert_eq!(item.kind, ItemKind::Procedure);
        assert_eq!(item.id, "proc-1");
        assert!(!item.content.is_empty());
        assert!(item.content.contains("Deploy"));
        assert!(item.content.contains("Deploy to production."));
        assert!(item.content.contains("1. Build"));
        assert!((item.score - 0.60).abs() < f64::EPSILON);
        assert_eq!(item.provenance.source, "procedures");
        assert_eq!(item.provenance.source_id, "proc-1");
        assert_eq!(item.provenance.session_id, None);
        assert_eq!(item.provenance.ward_id, Some("ward-x".to_string()));
    }
}
