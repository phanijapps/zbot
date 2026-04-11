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

    if let Some(specs_dir) = specs_dir {
        if specs_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&specs_dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    if entry.path().is_dir() {
                        if let Ok(files) = std::fs::read_dir(entry.path()) {
                            for file in files.filter_map(|f| f.ok()) {
                                if let Ok(content) = std::fs::read_to_string(file.path()) {
                                    if content.contains("Status: placeholder") {
                                        return true;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    false
}
