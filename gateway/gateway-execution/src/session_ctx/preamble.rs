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
/// The tag is self-describing — it carries the sid + ward for this
/// session AND a `tool=` attribute showing the exact memory call to
/// read ctx fields, with the sid already substituted. This eliminates
/// the need for the LLM to consult the shard before its first read:
/// everything it needs is inline.
///
/// Output looks like:
/// ```text
/// <session_ctx
///   sid="sess-beb261fd"
///   ward="stock-analysis"
///   step="3/7"
///   tool='memory(action="get_fact", key="ctx.sess-beb261fd.<field>")'
///   fields="intent,prompt,plan,state.<exec_id>"
///   prior_states="exec-abc,exec-def"
/// />
/// ```
///
/// On a single line:
/// ```text
/// <session_ctx sid="sess-beb261fd" ward="stock-analysis" tool='memory(action="get_fact", key="ctx.sess-beb261fd.<field>")' fields="intent,prompt,plan,state.<exec_id>" />
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
    let mut attrs = Vec::with_capacity(6);
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

    // Self-describing usage hint. The sid is substituted inline so the
    // LLM can copy-paste the call verbatim. Single-quoted value so the
    // inner JSON-ish args can keep double quotes and remain valid.
    attrs.push(format!(
        r#"tool='memory(action="get_fact", key="ctx.{}.<field>")'"#,
        sid
    ));

    // Enumerate the keys an agent can read. Intent/prompt/plan are
    // always root-owned canonicals; state.<exec_id> is per-subagent.
    // If prior_states is non-empty, the caller can fetch state.<id>
    // for each listed exec — listed separately below so the LLM
    // doesn't have to guess which exec_ids exist.
    attrs.push(r#"fields="intent,prompt,plan,state.<exec_id>""#.to_string());

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
    fn test_build_minimal_includes_sid_tool_fields() {
        let tag = build("sess-1", None, None, None, &[]);
        // Even with no ward or step, the tag carries sid + the
        // self-describing tool + fields attrs so the LLM has what it
        // needs to call memory(get_fact).
        assert!(tag.contains(r#"sid="sess-1""#));
        assert!(tag.contains(r#"tool='memory(action="get_fact", key="ctx.sess-1.<field>")'"#));
        assert!(tag.contains(r#"fields="intent,prompt,plan,state.<exec_id>""#));
        assert!(!tag.contains("ward="));
        assert!(!tag.contains("step="));
        assert!(!tag.contains("prior_states="));
    }

    #[test]
    fn test_build_with_ward_only() {
        let tag = build("sess-1", Some("my-ward"), None, None, &[]);
        assert!(tag.contains(r#"sid="sess-1""#));
        assert!(tag.contains(r#"ward="my-ward""#));
        assert!(tag.contains(r#"tool='memory(action="get_fact", key="ctx.sess-1.<field>")'"#));
        assert!(tag.contains(r#"fields="intent,prompt,plan,state.<exec_id>""#));
    }

    #[test]
    fn test_build_with_step_pair() {
        let tag = build("sess-1", Some("w"), Some(3), Some(7), &[]);
        assert!(tag.contains(r#"sid="sess-1""#));
        assert!(tag.contains(r#"ward="w""#));
        assert!(tag.contains(r#"step="3/7""#));
        assert!(tag.contains(r#"tool='memory(action="get_fact", key="ctx.sess-1.<field>")'"#));
    }

    #[test]
    fn test_build_with_step_current_only() {
        let tag = build("sess-1", Some("w"), Some(2), None, &[]);
        assert!(tag.contains(r#"step="2""#));
        assert!(!tag.contains("step=\"2/"));
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
        assert!(tag.contains(r#"prior_states="exec-abc,exec-def""#));
        // prior_states comes AFTER fields in the tag ordering so readers
        // see the universal field list first, then which exec_ids to
        // substitute into state.<exec_id>.
        let fields_pos = tag.find("fields=").unwrap();
        let prior_pos = tag.find("prior_states=").unwrap();
        assert!(fields_pos < prior_pos);
    }

    #[test]
    fn test_build_empty_ward_dropped() {
        // An empty ward string is treated as absent — the attribute is
        // dropped rather than emitted as ward="".
        let tag = build("sess-1", Some(""), None, None, &[]);
        assert!(!tag.contains("ward="));
        assert!(tag.contains(r#"sid="sess-1""#));
    }

    #[test]
    fn test_tool_attribute_has_sid_substituted_verbatim() {
        // The `tool=` attribute must show the ACTUAL sid, not a
        // placeholder, so the LLM can copy-paste the call without
        // substitution.
        let tag = build("sess-beb261fd", Some("w"), None, None, &[]);
        assert!(tag.contains(r#"ctx.sess-beb261fd.<field>"#));
        assert!(!tag.contains("ctx.<sid>.")); // placeholder should not leak
    }

    #[test]
    fn test_tool_attribute_uses_single_quotes_outside_double_inside() {
        // The `tool=` attribute value contains double-quoted JSON-ish
        // args (action="get_fact", key="..."). It must be wrapped in
        // single quotes so the quoting remains valid.
        let tag = build("sess-1", None, None, None, &[]);
        assert!(tag.contains(
            r#"tool='memory(action="get_fact", key="ctx.sess-1.<field>")'"#
        ));
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
