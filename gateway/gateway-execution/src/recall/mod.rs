//! Recall pipeline — most logic lives in `gateway-memory`.
//!
//! This module re-exports the generic memory types and adds the
//! consumer-side prompt-formatting helpers (chat-prompt headings, zbot tool
//! names) that are specific to how gateway-execution injects recalled
//! context into the agent.
pub use gateway_memory::recall::*;

/// Format the system message surfaced to the agent when the automatic
/// session-start recall fails with an error.
///
/// Phase 7 (T-D): empty recall results stay quiet — only genuine errors
/// produce a surface message so the agent knows memory retrieval was
/// attempted and can call `memory(action="recall", ...)` manually.
pub fn format_recall_failure_message(err: &str) -> String {
    format!(
        "[Memory retrieval failed: {}. You can call memory(action=\"recall\", query=...) manually if you need past context.]",
        err
    )
}

/// Format a unified scored-item list as a prompt-ready context block.
///
/// Items are emitted in input order (caller should already have them ranked
/// by `recall_unified`). Each line is prefixed with the item kind so the
/// downstream LLM can reason about provenance. Empty input yields an empty
/// string so callers can short-circuit with `.is_empty()`.
pub fn format_scored_items(items: &[ScoredItem]) -> String {
    if items.is_empty() {
        return String::new();
    }
    let mut lines = Vec::with_capacity(items.len() + 1);
    lines.push("## Recalled Context".to_string());
    for item in items {
        let tag = match item.kind {
            ItemKind::Fact => "fact",
            ItemKind::Wiki => "wiki",
            ItemKind::Procedure => "procedure",
            ItemKind::GraphNode => "entity",
            ItemKind::Goal => "goal",
            ItemKind::Episode => "episode",
        };
        lines.push(format!("- [{}] {}", tag, item.content));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_item(kind: ItemKind, id: &str, content: &str, score: f64) -> ScoredItem {
        ScoredItem {
            kind,
            id: id.to_string(),
            content: content.to_string(),
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
    fn format_scored_items_empty_returns_empty_string() {
        assert!(format_scored_items(&[]).is_empty());
    }

    #[test]
    fn format_recall_failure_message_includes_error_and_guidance() {
        let msg = format_recall_failure_message("database timeout");
        assert!(msg.contains("database timeout"));
        assert!(msg.contains("Memory retrieval failed"));
        assert!(msg.contains("memory(action=\"recall\""));
    }

    #[test]
    fn format_scored_items_tags_each_kind() {
        let items = vec![
            mk_item(ItemKind::Fact, "f1", "fact content", 1.0),
            mk_item(ItemKind::Wiki, "w1", "wiki content", 0.9),
            mk_item(ItemKind::Procedure, "p1", "proc content", 0.8),
            mk_item(ItemKind::GraphNode, "g1", "node content", 0.7),
            mk_item(ItemKind::Goal, "go1", "goal content", 0.6),
            mk_item(ItemKind::Episode, "e1", "ep content", 0.5),
        ];
        let out = format_scored_items(&items);
        assert!(out.starts_with("## Recalled Context"));
        assert!(out.contains("- [fact] fact content"));
        assert!(out.contains("- [wiki] wiki content"));
        assert!(out.contains("- [procedure] proc content"));
        assert!(out.contains("- [entity] node content"));
        assert!(out.contains("- [goal] goal content"));
        assert!(out.contains("- [episode] ep content"));
    }
}
