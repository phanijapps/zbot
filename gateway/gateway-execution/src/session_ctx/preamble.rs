//! # Session-ctx preamble builder
//!
//! Constructs the small `<session_ctx ... />` self-closing tag that
//! gets prepended to every subagent's task message. The tag carries
//! the runtime values the static shard's instructions reference:
//! session id, ward, step position, and the execution ids of completed
//! prior subagents (so the new subagent can fetch their handoffs).
//!
//! Pure function — no I/O. Callers (delegation/spawn.rs) assemble the
//! inputs from execution state and pass them in.

/// Build the runtime `<session_ctx ... />` tag for a subagent task.
///
/// Output looks like:
/// ```text
/// <session_ctx sid="sess-beb261fd" ward="stock-analysis" step="3/7" prior_states="exec-abc,exec-def" />
/// ```
///
/// Empty / absent fields are dropped rather than emitted as empty
/// attributes — keeps the tag minimal when context is sparse.
pub fn build(
    sid: &str,
    ward: Option<&str>,
    step_current: Option<u32>,
    step_total: Option<u32>,
    prior_execution_ids: &[String],
) -> String {
    let mut attrs = Vec::with_capacity(4);
    attrs.push(format!(r#"sid="{}""#, sid));

    if let Some(w) = ward {
        if !w.is_empty() {
            attrs.push(format!(r#"ward="{}""#, w));
        }
    }

    if let (Some(cur), Some(total)) = (step_current, step_total) {
        attrs.push(format!(r#"step="{}/{}""#, cur, total));
    } else if let Some(cur) = step_current {
        attrs.push(format!(r#"step="{}""#, cur));
    }

    if !prior_execution_ids.is_empty() {
        attrs.push(format!(r#"prior_states="{}""#, prior_execution_ids.join(",")));
    }

    format!("<session_ctx {} />", attrs.join(" "))
}

/// Prepend the session_ctx tag to a task message.
///
/// Used by spawn.rs when composing the child task. The tag lives on
/// its own line before the original task so it's trivially
/// extractable by readers (human debugging + the agent's own parsing).
pub fn prepend_to_task(
    sid: &str,
    ward: Option<&str>,
    step_current: Option<u32>,
    step_total: Option<u32>,
    prior_execution_ids: &[String],
    task: &str,
) -> String {
    let tag = build(sid, ward, step_current, step_total, prior_execution_ids);
    format!("{}\n\n{}", tag, task)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_minimal_sid_only() {
        let tag = build("sess-1", None, None, None, &[]);
        assert_eq!(tag, r#"<session_ctx sid="sess-1" />"#);
    }

    #[test]
    fn test_build_with_ward_only() {
        let tag = build("sess-1", Some("my-ward"), None, None, &[]);
        assert_eq!(tag, r#"<session_ctx sid="sess-1" ward="my-ward" />"#);
    }

    #[test]
    fn test_build_with_step_pair() {
        let tag = build("sess-1", Some("w"), Some(3), Some(7), &[]);
        assert_eq!(
            tag,
            r#"<session_ctx sid="sess-1" ward="w" step="3/7" />"#
        );
    }

    #[test]
    fn test_build_with_step_current_only() {
        let tag = build("sess-1", Some("w"), Some(2), None, &[]);
        assert_eq!(tag, r#"<session_ctx sid="sess-1" ward="w" step="2" />"#);
    }

    #[test]
    fn test_build_with_prior_states() {
        let tag = build(
            "sess-1",
            Some("w"),
            Some(3),
            Some(7),
            &["exec-abc".to_string(), "exec-def".to_string()],
        );
        assert_eq!(
            tag,
            r#"<session_ctx sid="sess-1" ward="w" step="3/7" prior_states="exec-abc,exec-def" />"#
        );
    }

    #[test]
    fn test_build_empty_ward_dropped() {
        // An empty ward string is treated as absent — the attribute is
        // dropped rather than emitted as ward="".
        let tag = build("sess-1", Some(""), None, None, &[]);
        assert_eq!(tag, r#"<session_ctx sid="sess-1" />"#);
    }

    #[test]
    fn test_prepend_to_task_two_line_separator() {
        let result = prepend_to_task(
            "sess-1",
            Some("w"),
            Some(1),
            Some(3),
            &[],
            "Do the thing.",
        );
        assert!(result.starts_with("<session_ctx "));
        assert!(result.contains("\n\nDo the thing."));
    }

    #[test]
    fn test_prepend_to_task_preserves_task_verbatim() {
        let task = "Line 1\nLine 2\n\n<code>foo</code>";
        let result = prepend_to_task("s", None, None, None, &[], task);
        assert!(result.ends_with(task));
    }
}
