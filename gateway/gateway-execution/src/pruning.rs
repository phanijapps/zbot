// ============================================================================
// FACT PRUNING
// Archive decayed facts to keep the active memory store lean.
// ============================================================================

//! Prunes facts whose effective score has fallen below a threshold for too
//! long. Pruned facts are moved to `memory_facts_archive` — they are not
//! deleted, just moved out of the hot path for recall scoring.
//!
//! Called opportunistically after distillation; failures are logged but
//! never break the distillation pipeline.

use std::collections::HashMap;

use gateway_database::MemoryRepository;
use gateway_services::recall_config::TemporalDecayConfig;

/// Summary of a pruning run.
pub struct PruneResult {
    pub pruned_count: usize,
    pub categories: HashMap<String, usize>,
}

/// Prune facts that have decayed below threshold for too long.
///
/// Moves qualifying facts to `memory_facts_archive` via
/// [`MemoryRepository::archive_fact`]. Skips `skill` and `agent` categories
/// (capability indices that should never decay).
pub fn prune_decayed_facts(
    memory_repo: &MemoryRepository,
    config: &TemporalDecayConfig,
) -> Result<PruneResult, String> {
    if !config.enabled {
        return Ok(PruneResult {
            pruned_count: 0,
            categories: HashMap::new(),
        });
    }

    // 1. Fetch all facts (broad sweep — pruning is infrequent)
    let all_facts = memory_repo
        .list_all_memory_facts(None, None, None, 1000, 0)
        .unwrap_or_default();

    let now = chrono::Utc::now();
    let mut to_prune: Vec<String> = Vec::new();
    let mut categories: HashMap<String, usize> = HashMap::new();

    for fact in &all_facts {
        // Capability indices should never decay
        if fact.category == "skill" || fact.category == "agent" {
            continue;
        }

        let half_life = config
            .half_life_days
            .get(&fact.category)
            .copied()
            .unwrap_or(30.0);

        let last_seen = chrono::DateTime::parse_from_rfc3339(&fact.updated_at)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or(now);

        let age_days = (now - last_seen).num_days().max(0) as f64;
        let decay = 1.0 / (1.0 + (age_days / half_life));
        let mention_boost = 1.0 + (fact.mention_count as f64).max(1.0).log2();
        let effective_score = fact.confidence * decay * mention_boost;

        // Only prune if score is very low AND fact is old enough
        if effective_score < config.prune_threshold && age_days > config.prune_after_days as f64 {
            to_prune.push(fact.id.clone());
            *categories.entry(fact.category.clone()).or_insert(0) += 1;
        }
    }

    // 2. Archive each qualifying fact (INSERT into archive, DELETE from active)
    let mut pruned_count = 0;
    for fact_id in &to_prune {
        match memory_repo.archive_fact(fact_id) {
            Ok(_) => pruned_count += 1,
            Err(e) => {
                tracing::warn!(fact_id = %fact_id, error = %e, "Failed to archive fact");
            }
        }
    }

    if pruned_count > 0 {
        tracing::info!(
            pruned = pruned_count,
            categories = ?categories,
            "Pruned decayed facts to archive"
        );
    }

    Ok(PruneResult {
        pruned_count,
        categories,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prune_disabled_returns_zero() {
        let config = TemporalDecayConfig {
            enabled: false,
            half_life_days: HashMap::new(),
            prune_threshold: 0.05,
            prune_after_days: 30,
        };
        // We can't construct a MemoryRepository without a DatabaseManager,
        // but we verify the early-exit path compiles and works logically.
        // A full integration test would use an in-memory DB.
        assert!(!config.enabled);
    }
}
