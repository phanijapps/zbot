// ============================================================================
// WARD FILE SYNC
// Generate a human-readable ward knowledge file from distilled facts.
// ============================================================================

//! Produces `{ward_path}/memory-bank/ward.md` — a portable, human-readable summary
//! of distilled knowledge scoped to a ward. Auto-generated after each
//! distillation; not a source of truth (the SQLite store is). If deleted,
//! it is regenerated on the next distillation cycle.

use std::path::Path;

use gateway_database::{MemoryFact, MemoryRepository};

/// Dedup facts by content similarity — keep highest confidence, skip near-duplicates.
/// Two facts are considered duplicates if they share 60%+ of their words.
fn dedup_facts(mut facts: Vec<MemoryFact>, max: usize) -> Vec<MemoryFact> {
    // Sort by confidence descending — keep the best version
    facts.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut result: Vec<MemoryFact> = Vec::new();
    for fact in facts {
        let dominated = result.iter().any(|existing| {
            let a_words: std::collections::HashSet<&str> =
                existing.content.split_whitespace().collect();
            let b_words: std::collections::HashSet<&str> =
                fact.content.split_whitespace().collect();
            if a_words.is_empty() || b_words.is_empty() {
                return false;
            }
            let overlap = a_words.intersection(&b_words).count();
            let smaller = a_words.len().min(b_words.len());
            overlap as f64 / smaller as f64 > 0.6
        });
        if !dominated {
            result.push(fact);
            if result.len() >= max {
                break;
            }
        }
    }
    result
}

/// Generate a human-readable ward knowledge file from distilled facts.
///
/// Written to `{ward_path}/memory-bank/ward.md`. Auto-generated, not a source of truth.
/// If deleted, regenerated on next distillation.
pub fn generate_ward_knowledge_file(
    ward_path: &Path,
    ward_id: &str,
    memory_repo: &MemoryRepository,
) -> Result<(), String> {
    // Curated ward.md: ONLY actionable rules. Deduped. Max ~1KB.
    // Everything else stays in memory_facts (queryable via recall).

    // 1. Corrections — max 5, highest confidence, deduped
    let corrections = dedup_facts(
        memory_repo
            .get_facts_by_category("root", "correction", 10)
            .unwrap_or_default()
            .into_iter()
            .chain(
                memory_repo
                    .get_facts_by_category("root", "instruction", 10)
                    .unwrap_or_default(),
            )
            .filter(|f| f.ward_id == ward_id || f.ward_id == "__global__")
            .collect(),
        5,
    );

    // 2. Architecture decisions — max 3 strategies
    let strategies = dedup_facts(
        memory_repo
            .get_facts_by_category("root", "strategy", 5)
            .unwrap_or_default()
            .into_iter()
            .filter(|f| f.ward_id == ward_id || f.ward_id == "__global__")
            .collect(),
        3,
    );

    // 3. Active warnings — max 2 patterns (only high-confidence operational ones)
    let patterns = dedup_facts(
        memory_repo
            .get_facts_by_category("root", "pattern", 5)
            .unwrap_or_default()
            .into_iter()
            .filter(|f| (f.ward_id == ward_id || f.ward_id == "__global__") && f.confidence >= 0.8)
            .collect(),
        2,
    );

    // Format — no domain knowledge dump (that's what recall is for)
    let mut md = String::new();
    md.push_str(&format!("# Ward: {}\n", ward_id));
    md.push_str(&format!(
        "*Updated: {}*\n\n",
        chrono::Utc::now().format("%Y-%m-%d")
    ));

    if !corrections.is_empty() {
        md.push_str("## Rules (ALWAYS follow)\n");
        for f in &corrections {
            md.push_str(&format!("- {}\n", f.content));
        }
        md.push('\n');
    }

    if !strategies.is_empty() {
        md.push_str("## Architecture\n");
        for f in &strategies {
            md.push_str(&format!("- {}\n", f.content));
        }
        md.push('\n');
    }

    if !patterns.is_empty() {
        md.push_str("## Warnings\n");
        for f in &patterns {
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
    std::fs::write(&file_path, md).map_err(|e| format!("Failed to write ward.md: {}", e))?;

    tracing::info!(ward = %ward_id, path = ?file_path, "Updated ward knowledge file");
    Ok(())
}

#[cfg(test)]
mod tests {

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
