// ============================================================================
// TOOL GUARDS
// Shared guard functions that redirect tools when preconditions aren't met.
// ============================================================================

use zero_core::ToolContext;

/// Check if the active ward has unfilled placeholder specs.
///
/// Returns `true` when the root agent (not a delegated subagent) has a ward
/// whose `specs/` folder contains files with `Status: placeholder`. This
/// signals that the planning pipeline hasn't been completed yet and shortcut
/// tools (list_skills, load_skill, update_plan) should redirect.
pub(crate) fn has_placeholder_specs(ctx: &dyn ToolContext) -> bool {
    // Only check for root agents (not delegated subagents)
    let is_delegated = ctx
        .get_state("app:is_delegated")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if is_delegated {
        return false; // Subagents can use all tools freely
    }

    // Check ward_id
    let ward_id = match ctx
        .get_state("ward_id")
        .and_then(|v| v.as_str().map(String::from))
    {
        Some(id) if id != "scratch" => id,
        _ => return false,
    };

    // Check for placeholder specs in the ward
    let specs_dir = dirs::document_dir()
        .or_else(dirs::home_dir)
        .map(|d| d.join("zbot").join("wards").join(&ward_id).join("specs"));

    let Some(specs_dir) = specs_dir else {
        return false;
    };
    if !specs_dir.exists() {
        return false;
    }
    let Ok(entries) = std::fs::read_dir(&specs_dir) else {
        return false;
    };

    for entry in entries.filter_map(|e| e.ok()) {
        if entry.path().is_dir() && dir_has_placeholder_spec(&entry.path()) {
            return true;
        }
    }
    false
}

fn dir_has_placeholder_spec(dir: &std::path::Path) -> bool {
    let Ok(files) = std::fs::read_dir(dir) else {
        return false;
    };
    for file in files.filter_map(|f| f.ok()) {
        if let Ok(content) = std::fs::read_to_string(file.path())
            && content.contains("Status: placeholder")
        {
            return true;
        }
    }
    false
}

/// Reject writes to `AGENTS.md` when the caller is a delegated subagent.
///
/// AGENTS.md is durable ward doctrine (purpose, conventions, do-nots).
/// Session-specific handoff content is auto-captured into ctx when a
/// subagent calls respond() — agents must not pollute doctrine with it.
/// Root can always write AGENTS.md (ward creation, user-driven edits).
///
/// Returns `Some(error_message)` to short-circuit the tool, `None` to allow.
pub(crate) fn reject_agents_md_from_subagent(
    is_delegated: bool,
    path: &str,
) -> Option<&'static str> {
    if !is_delegated {
        return None;
    }
    let filename = path.rsplit('/').next().unwrap_or(path);
    if filename != "AGENTS.md" {
        return None;
    }
    Some(
        "AGENTS.md is ward doctrine — subagents cannot modify it. \
         Session-specific handoff notes are auto-captured into ctx when you \
         call respond() with a handoff field; do not write them into AGENTS.md.",
    )
}

/// Context-bound form: reads `app:is_delegated` from the tool context.
pub(crate) fn check_agents_md_write_gate(
    ctx: &dyn ToolContext,
    path: &str,
) -> Option<&'static str> {
    let is_delegated = ctx
        .get_state("app:is_delegated")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    reject_agents_md_from_subagent(is_delegated, path)
}

#[cfg(test)]
mod agents_md_gate_tests {
    use super::reject_agents_md_from_subagent;

    #[test]
    fn root_may_write_agents_md() {
        assert!(reject_agents_md_from_subagent(false, "/wards/w/AGENTS.md").is_none());
    }

    #[test]
    fn subagent_blocked_on_direct_path() {
        let err = reject_agents_md_from_subagent(true, "/wards/w/AGENTS.md").unwrap();
        assert!(err.contains("ward doctrine"));
    }

    #[test]
    fn subagent_blocked_on_bare_filename() {
        assert!(reject_agents_md_from_subagent(true, "AGENTS.md").is_some());
    }

    #[test]
    fn subagent_may_write_other_files() {
        assert!(reject_agents_md_from_subagent(true, "/wards/w/memory-bank/ward.md").is_none());
        assert!(reject_agents_md_from_subagent(true, "/wards/w/core/valuation.py").is_none());
    }

    #[test]
    fn directory_with_agents_md_suffix_allowed() {
        // Writes to a path whose DIRECTORY ends in "AGENTS.md" should
        // still be allowed — we only match the final filename.
        assert!(
            reject_agents_md_from_subagent(true, "/wards/w/MY_AGENTS.md/notes.txt").is_none()
        );
    }
}
