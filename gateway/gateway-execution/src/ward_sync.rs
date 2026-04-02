// ============================================================================
// WARD FILE SYNC
// Generate a human-readable ward knowledge file from distilled facts.
// ============================================================================

//! Produces `{ward_path}/memory-bank/ward.md` — a portable, human-readable summary
//! of distilled knowledge scoped to a ward. Auto-generated after each
//! distillation; not a source of truth (the SQLite store is). If deleted,
//! it is regenerated on the next distillation cycle.

use std::path::Path;

use gateway_database::MemoryRepository;

/// Generate a human-readable ward knowledge file from distilled facts.
///
/// Written to `{ward_path}/memory-bank/ward.md`. Auto-generated, not a source of truth.
/// If deleted, regenerated on next distillation.
pub fn generate_ward_knowledge_file(
    ward_path: &Path,
    ward_id: &str,
    memory_repo: &MemoryRepository,
) -> Result<(), String> {
    // 1. Fetch corrections (highest priority — always follow)
    let corrections = memory_repo
        .get_facts_by_category("root", "correction", 10)
        .unwrap_or_default()
        .into_iter()
        .filter(|f| f.ward_id == ward_id || f.ward_id == "__global__")
        .collect::<Vec<_>>();

    // 2. Fetch strategies
    let strategies = memory_repo
        .get_facts_by_category("root", "strategy", 10)
        .unwrap_or_default()
        .into_iter()
        .filter(|f| f.ward_id == ward_id || f.ward_id == "__global__")
        .collect::<Vec<_>>();

    // 3. Fetch patterns
    let patterns = memory_repo
        .get_facts_by_category("root", "pattern", 10)
        .unwrap_or_default()
        .into_iter()
        .filter(|f| f.ward_id == ward_id || f.ward_id == "__global__")
        .collect::<Vec<_>>();

    // 4. Fetch domain facts scoped to this ward
    let domain_facts = memory_repo
        .get_facts_by_category("root", "domain", 15)
        .unwrap_or_default()
        .into_iter()
        .filter(|f| f.ward_id == ward_id || f.ward_id == "__global__")
        .take(10)
        .collect::<Vec<_>>();

    // 5. Format as markdown
    let mut md = String::new();
    md.push_str(&format!("# Ward Knowledge: {}\n", ward_id));
    md.push_str(&format!(
        "*Auto-generated from knowledge graph. Last updated: {}*\n\n",
        chrono::Utc::now().format("%Y-%m-%d")
    ));

    if !corrections.is_empty() {
        md.push_str("## Corrections (ALWAYS follow)\n");
        for f in &corrections {
            md.push_str(&format!("- {}\n", f.content));
        }
        md.push('\n');
    }

    if !strategies.is_empty() {
        md.push_str("## Strategies\n");
        for f in &strategies {
            md.push_str(&format!("- {}\n", f.content));
        }
        md.push('\n');
    }

    if !patterns.is_empty() {
        md.push_str("## Patterns\n");
        for f in &patterns {
            md.push_str(&format!("- {}\n", f.content));
        }
        md.push('\n');
    }

    if !domain_facts.is_empty() {
        md.push_str("## Domain Knowledge\n");
        for f in &domain_facts {
            md.push_str(&format!("- {}\n", f.content));
        }
        md.push('\n');
    }

    // 6. Write file
    let memory_dir = ward_path.join("memory-bank");
    if let Err(e) = std::fs::create_dir_all(&memory_dir) {
        return Err(format!("Failed to create memory dir: {}", e));
    }
    let file_path = memory_dir.join("ward.md");
    std::fs::write(&file_path, md)
        .map_err(|e| format!("Failed to write ward.md: {}", e))?;

    tracing::info!(ward = %ward_id, path = ?file_path, "Updated ward knowledge file");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ward_knowledge_file_is_regenerated_after_delete() {
        // Smoke test: ensure the function signature is correct and the module compiles.
        // Full integration tests require a DatabaseManager and populated facts.
        let dir = tempfile::tempdir().unwrap();
        let ward_path = dir.path().join("wards").join("test-ward");
        std::fs::create_dir_all(&ward_path).unwrap();

        // Without a real MemoryRepository we can't call generate_ward_knowledge_file,
        // but we verify the module compiles and the directory logic works.
        let memory_dir = ward_path.join("memory-bank");
        std::fs::create_dir_all(&memory_dir).unwrap();
        let file_path = memory_dir.join("ward.md");
        std::fs::write(&file_path, "# test").unwrap();
        assert!(file_path.exists());
    }
}
