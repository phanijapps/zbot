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
    use gateway_database::vector_index::VectorIndex;
    use gateway_database::{KnowledgeDatabase, MemoryFact, SqliteVecIndex};
    use gateway_services::VaultPaths;
    use std::sync::Arc;

    struct Harness {
        _tmp: tempfile::TempDir,
        repo: MemoryRepository,
    }

    fn setup() -> Harness {
        let tmp = tempfile::tempdir().expect("tempdir");
        let paths = Arc::new(VaultPaths::new(tmp.path().to_path_buf()));
        std::fs::create_dir_all(paths.conversations_db().parent().expect("parent")).expect("mkdir");
        let db = Arc::new(KnowledgeDatabase::new(paths).expect("knowledge db"));
        let vec: Arc<dyn VectorIndex> =
            Arc::new(SqliteVecIndex::new(db.clone(), "memory_facts_index", "fact_id").unwrap());
        let repo = MemoryRepository::new(db, vec);
        Harness { _tmp: tmp, repo }
    }

    /// Build a fact whose `updated_at` is `age_days` days in the past, with
    /// the supplied category, confidence and mention_count. Everything else
    /// gets sensible test defaults.
    fn fact(
        id: &str,
        category: &str,
        confidence: f64,
        mention_count: i32,
        age_days: i64,
    ) -> MemoryFact {
        let updated_at = (chrono::Utc::now() - chrono::Duration::days(age_days)).to_rfc3339();
        MemoryFact {
            id: id.into(),
            session_id: None,
            agent_id: "agent:root".into(),
            scope: "agent".into(),
            category: category.into(),
            key: format!("test.{id}"),
            content: "irrelevant".into(),
            confidence,
            mention_count,
            source_summary: None,
            embedding: None,
            ward_id: "__global__".into(),
            contradicted_by: None,
            created_at: updated_at.clone(),
            updated_at,
            expires_at: None,
            valid_from: None,
            valid_until: None,
            superseded_by: None,
            pinned: false,
            epistemic_class: Some("current".into()),
            source_episode_id: None,
            source_ref: None,
        }
    }

    fn disabled_config() -> TemporalDecayConfig {
        TemporalDecayConfig {
            enabled: false,
            half_life_days: HashMap::new(),
            prune_threshold: 0.05,
            prune_after_days: 30,
        }
    }

    fn aggressive_config() -> TemporalDecayConfig {
        // High threshold + short prune_after so any non-pinned decayed fact
        // qualifies. Used to exercise the archive path.
        TemporalDecayConfig {
            enabled: true,
            half_life_days: HashMap::from([("domain".into(), 1.0)]),
            prune_threshold: 1.0,
            prune_after_days: 1,
        }
    }

    #[test]
    fn prune_disabled_short_circuits_without_fetching_facts() {
        let h = setup();
        let out = prune_decayed_facts(&h.repo, &disabled_config()).expect("prune");
        assert_eq!(out.pruned_count, 0);
        assert!(out.categories.is_empty());
    }

    #[test]
    fn prune_empty_store_is_noop() {
        let h = setup();
        let out = prune_decayed_facts(&h.repo, &aggressive_config()).expect("prune");
        assert_eq!(out.pruned_count, 0);
        assert!(out.categories.is_empty());
    }

    #[test]
    fn prune_skips_skill_and_agent_categories_even_when_decayed() {
        let h = setup();
        // Old, low-confidence facts in the protected categories — they should
        // never be pruned even when the rest of the config says "prune everything."
        h.repo
            .upsert_memory_fact(&fact("s1", "skill", 0.01, 1, 999))
            .unwrap();
        h.repo
            .upsert_memory_fact(&fact("a1", "agent", 0.01, 1, 999))
            .unwrap();

        let out = prune_decayed_facts(&h.repo, &aggressive_config()).expect("prune");
        assert_eq!(
            out.pruned_count, 0,
            "skill/agent must be capability-preserved"
        );
    }

    #[test]
    fn prune_archives_decayed_facts_and_counts_per_category() {
        let h = setup();
        // 3 decayed `domain` facts, 1 decayed `pattern` fact (uses default
        // half-life of 30.0 since the config only maps `domain`).
        for id in ["d1", "d2", "d3"] {
            h.repo
                .upsert_memory_fact(&fact(id, "domain", 0.1, 1, 365))
                .unwrap();
        }
        h.repo
            .upsert_memory_fact(&fact("p1", "pattern", 0.1, 1, 365))
            .unwrap();

        let out = prune_decayed_facts(&h.repo, &aggressive_config()).expect("prune");

        assert_eq!(out.pruned_count, 4);
        assert_eq!(out.categories.get("domain").copied(), Some(3));
        assert_eq!(out.categories.get("pattern").copied(), Some(1));
    }

    #[test]
    fn prune_keeps_young_facts_even_with_low_confidence() {
        let h = setup();
        // Young fact (age 0 days) — won't meet the `age_days > prune_after_days`
        // gate no matter how decayed.
        h.repo
            .upsert_memory_fact(&fact("young", "domain", 0.01, 1, 0))
            .unwrap();

        let out = prune_decayed_facts(&h.repo, &aggressive_config()).expect("prune");
        assert_eq!(out.pruned_count, 0);
    }

    #[test]
    fn prune_keeps_high_confidence_facts_even_when_old() {
        // With domain half-life=1 day and age=365 days, a confidence=1.0 fact
        // with a heavy mention_count still sits above the 1.0 threshold only
        // when the boost is enough — set mention_count high so score stays
        // above threshold and the age-filter alone shouldn't prune it.
        //
        // Using the DEFAULT config (threshold 0.05), a high-confidence fact
        // lands well above threshold and should be kept.
        let h = setup();
        h.repo
            .upsert_memory_fact(&fact("tough", "domain", 1.0, 100, 365))
            .unwrap();

        let out = prune_decayed_facts(&h.repo, &TemporalDecayConfig::default()).expect("prune");
        assert_eq!(
            out.pruned_count, 0,
            "high-confidence + mention-boosted fact should survive default pruning"
        );
    }
}
