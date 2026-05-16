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
///
/// Phase B-4: `ItemKind::Belief` is rendered under a dedicated
/// `## Active Beliefs` heading appended after `## Recalled Context`
/// so synthesized stances stand out from raw facts. Belief lines drop
/// the redundant `[belief …]` tag — `belief_to_item` already embeds
/// the confidence into the content string and the heading carries the
/// kind. The two sections only appear when their respective item lists
/// are non-empty.
pub fn format_scored_items(items: &[ScoredItem]) -> String {
    if items.is_empty() {
        return String::new();
    }
    let (beliefs, others): (Vec<&ScoredItem>, Vec<&ScoredItem>) = items
        .iter()
        .partition(|item| matches!(item.kind, ItemKind::Belief));

    let mut sections: Vec<String> = Vec::new();
    if !others.is_empty() {
        let mut lines = Vec::with_capacity(others.len() + 1);
        lines.push("## Recalled Context".to_string());
        for item in &others {
            lines.push(format!(
                "- [{}] {}",
                non_belief_tag(&item.kind),
                item.content
            ));
        }
        sections.push(lines.join("\n"));
    }
    if !beliefs.is_empty() {
        let mut lines = Vec::with_capacity(beliefs.len() + 1);
        lines.push("## Active Beliefs".to_string());
        for item in &beliefs {
            // belief_to_item already prefixes content with
            // `[belief <conf>] ...`; emit verbatim so the consumer sees
            // confidence inline without double-tagging.
            lines.push(format!("- {}", item.content));
        }
        sections.push(lines.join("\n"));
    }
    sections.join("\n\n")
}

/// Map non-belief `ItemKind` values to their inline tag. Belief items
/// never reach this helper — they're emitted under their own heading.
fn non_belief_tag(kind: &ItemKind) -> &'static str {
    match kind {
        ItemKind::Fact => "fact",
        ItemKind::Wiki => "wiki",
        ItemKind::Procedure => "procedure",
        ItemKind::GraphNode => "entity",
        ItemKind::Goal => "goal",
        ItemKind::Episode => "episode",
        // Beliefs are routed to their own section; if one reaches this
        // helper due to future refactoring, fall back to a sensible tag
        // rather than panicking — the heading already disambiguates.
        ItemKind::Belief => "belief",
    }
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
        assert!(
            !out.contains("## Active Beliefs"),
            "no beliefs ⇒ no belief heading"
        );
    }

    /// Phase B-4: belief items render under a dedicated heading,
    /// separate from `## Recalled Context`. The belief content already
    /// includes the `[belief <conf>]` tag (set by `belief_to_item` in
    /// gateway-memory) — the formatter emits it verbatim under the new
    /// heading so the agent sees `## Active Beliefs` distinct from
    /// `## Recalled Context`.
    #[test]
    fn format_scored_items_groups_beliefs_under_active_beliefs_heading() {
        let items = vec![
            mk_item(ItemKind::Fact, "f1", "fact content", 1.0),
            mk_item(
                ItemKind::Belief,
                "b1",
                "[belief 0.92] user.location: User lives in Mason, OH",
                0.9,
            ),
            mk_item(
                ItemKind::Belief,
                "b2",
                "[belief 0.85] user.diet: User is vegetarian",
                0.8,
            ),
        ];
        let out = format_scored_items(&items);

        assert!(out.contains("## Recalled Context"));
        assert!(out.contains("- [fact] fact content"));
        assert!(out.contains("## Active Beliefs"));
        assert!(out.contains("- [belief 0.92] user.location: User lives in Mason, OH"));
        assert!(out.contains("- [belief 0.85] user.diet: User is vegetarian"));

        // Beliefs must NOT be tagged inside `## Recalled Context`.
        let recalled_section = out.split("## Active Beliefs").next().unwrap_or("");
        assert!(
            !recalled_section.contains("user.location"),
            "belief content must not leak into Recalled Context section"
        );
    }

    /// When only beliefs are present, the formatter renders only the
    /// Active Beliefs heading with no empty Recalled Context section.
    #[test]
    fn format_scored_items_belief_only_omits_recalled_context_heading() {
        let items = vec![mk_item(
            ItemKind::Belief,
            "b1",
            "[belief 0.92] user.location: User lives in Mason, OH",
            0.9,
        )];
        let out = format_scored_items(&items);
        assert!(
            !out.contains("## Recalled Context"),
            "no non-belief items ⇒ no Recalled Context heading"
        );
        assert!(out.contains("## Active Beliefs"));
    }
}
