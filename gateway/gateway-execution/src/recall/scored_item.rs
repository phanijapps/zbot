//! Unified retrievable item — every recall source (facts, wiki, procedures,
//! graph, goals) projects into `ScoredItem` so they compete in one pool.
//!
//! Each source adapter produces a `Vec<ScoredItem>` ordered by its own
//! scoring. `rrf_merge(lists, k, budget)` fuses N ranked lists via
//! Reciprocal Rank Fusion: same item across lists sums rank-reciprocal
//! contributions.

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ItemKind {
    Fact,
    Wiki,
    Procedure,
    GraphNode,
    Goal,
}

#[derive(Debug, Clone)]
pub struct Provenance {
    pub source: String,
    pub source_id: String,
    pub session_id: Option<String>,
    pub ward_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ScoredItem {
    pub kind: ItemKind,
    pub id: String,
    pub content: String,
    pub score: f64,
    pub provenance: Provenance,
}

/// Reciprocal Rank Fusion. For each input list, rank r (1-indexed) contributes
/// 1 / (k + r) to the item's fused score. Items appearing in multiple lists
/// accumulate contributions. Returns items sorted by fused score descending,
/// capped to `budget`. Caller pre-sorts each list by its own relevance.
pub fn rrf_merge(lists: Vec<Vec<ScoredItem>>, k: f64, budget: usize) -> Vec<ScoredItem> {
    let mut fused: HashMap<String, (f64, ScoredItem)> = HashMap::new();
    for list in lists {
        for (rank_zero, item) in list.into_iter().enumerate() {
            let rank_one = (rank_zero as f64) + 1.0;
            let contribution = 1.0 / (k + rank_one);
            fused
                .entry(item.id.clone())
                .and_modify(|(score, _)| *score += contribution)
                .or_insert((contribution, item));
        }
    }
    let mut out: Vec<(f64, ScoredItem)> = fused.into_values().collect();
    out.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    out.truncate(budget);
    out.into_iter()
        .map(|(fused_score, mut item)| {
            item.score = fused_score;
            item
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk(id: &str, kind: ItemKind, score: f64) -> ScoredItem {
        ScoredItem {
            kind,
            id: id.to_string(),
            content: id.to_string(),
            score,
            provenance: Provenance {
                source: "test".into(),
                source_id: id.into(),
                session_id: None,
                ward_id: None,
            },
        }
    }

    #[test]
    fn empty_lists_produce_empty_merge() {
        assert!(rrf_merge(vec![], 60.0, 10).is_empty());
    }

    #[test]
    fn single_list_preserves_order() {
        let items: Vec<ScoredItem> = (0..5)
            .map(|i| mk(&format!("i{i}"), ItemKind::Fact, 1.0))
            .collect();
        let merged = rrf_merge(vec![items], 60.0, 10);
        assert_eq!(merged.len(), 5);
        assert_eq!(merged[0].id, "i0");
        assert_eq!(merged[4].id, "i4");
    }

    #[test]
    fn same_item_in_multiple_lists_sums_contributions() {
        let list_a = vec![mk("x", ItemKind::Fact, 1.0), mk("y", ItemKind::Fact, 0.9)];
        let list_b = vec![mk("x", ItemKind::Wiki, 1.0)];
        let merged = rrf_merge(vec![list_a, list_b], 60.0, 10);
        assert_eq!(
            merged[0].id, "x",
            "x present in both lists should rank first"
        );
        // x's score: 1/(60+1) + 1/(60+1) = 2/61
        // y's score: 1/(60+2) = 1/62
        let x_score = merged
            .iter()
            .find(|i| i.id == "x")
            .expect("x present")
            .score;
        let y_score = merged
            .iter()
            .find(|i| i.id == "y")
            .expect("y present")
            .score;
        assert!(x_score > y_score);
    }

    #[test]
    fn budget_caps_result_size() {
        let many: Vec<ScoredItem> = (0..100)
            .map(|i| mk(&format!("i{i}"), ItemKind::Fact, 1.0))
            .collect();
        let merged = rrf_merge(vec![many], 60.0, 5);
        assert_eq!(merged.len(), 5);
    }
}
