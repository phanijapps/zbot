//! OrphanArchiver — domain-agnostic janitor for the knowledge graph.
//!
//! Archives entities that satisfy ALL of:
//!   - `mention_count = 1` (only seen once)
//!   - `confidence < 0.5`
//!   - `first_seen_at < now() - 24 hours` (grace period for reinforcement)
//!   - zero incoming AND zero outgoing relationships
//!   - not already archived (`compressed_into IS NULL` AND
//!     `epistemic_class != 'archival'`)
//!
//! The archive action is a soft-delete: we set
//! `epistemic_class = 'archival'` and `compressed_into = 'orphan-archive'`
//! and remove the name-index entry (mirroring `GraphStorage::mark_pruned`).
//! Keeping the row preserves referential integrity with episodes that may
//! still reference the entity id.
//!
//! Runs after `Pruner` in the sleep cycle so that decay-driven prunes land
//! first and genuine orphans are identifiable in a single, stable pass.
//!
//! Runaway-protection: at most 100 entities archived per cycle.
//!
//! Audit: every archival records one `kg_compactions` row via
//! [`CompactionRepository::record_prune`] with `reason = "orphan-archival"`.

use std::sync::Arc;

use gateway_database::{CompactionRepository, KnowledgeDatabase};
use rusqlite::params;

/// Cap on archivals per cycle. Prevents a bad criterion from accidentally
/// wiping the graph on first pass.
const ARCHIVE_LIMIT: usize = 100;

/// Reason string recorded in `kg_compactions.reason` for audit rows.
const ARCHIVE_REASON: &str = "orphan-archival";

/// Sentinel written to `kg_entities.compressed_into` — distinct from
/// `Pruner`'s `__pruned__` so operators can tell the two apart.
const ORPHAN_SENTINEL: &str = "orphan-archive";

/// Counts emitted from a single archival pass.
#[derive(Debug, Default, Clone)]
pub struct OrphanArchiverStats {
    pub scanned: usize,
    pub archived: usize,
    pub failed: usize,
}

/// Archives isolated, low-confidence, singleton entities.
pub struct OrphanArchiver {
    db: Arc<KnowledgeDatabase>,
    compaction_repo: Arc<CompactionRepository>,
}

impl OrphanArchiver {
    pub fn new(db: Arc<KnowledgeDatabase>, compaction_repo: Arc<CompactionRepository>) -> Self {
        Self {
            db,
            compaction_repo,
        }
    }

    /// Run one archival pass. Returns aggregate stats. A per-entity failure
    /// is logged and skipped — the cycle never fails hard.
    pub async fn run_cycle(&self, run_id: &str) -> Result<OrphanArchiverStats, String> {
        let candidates = self.load_candidates()?;
        let mut stats = OrphanArchiverStats {
            scanned: candidates.len(),
            ..Default::default()
        };
        for entity_id in &candidates {
            match self.archive_entity(entity_id) {
                Ok(()) => {
                    stats.archived += 1;
                    if let Err(e) =
                        self.compaction_repo
                            .record_prune(run_id, entity_id, ARCHIVE_REASON)
                    {
                        tracing::warn!(
                            entity = %entity_id,
                            error = %e,
                            "orphan_archiver: record_prune failed",
                        );
                    }
                }
                Err(e) => {
                    stats.failed += 1;
                    tracing::warn!(
                        entity = %entity_id,
                        error = %e,
                        "orphan_archiver: archive failed",
                    );
                }
            }
        }
        Ok(stats)
    }

    /// Select up to [`ARCHIVE_LIMIT`] entity ids matching the orphan criteria.
    fn load_candidates(&self) -> Result<Vec<String>, String> {
        let limit = ARCHIVE_LIMIT as i64;
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id FROM kg_entities e
                 WHERE mention_count = 1
                   AND confidence < 0.5
                   AND first_seen_at < datetime('now', '-24 hours')
                   AND compressed_into IS NULL
                   AND epistemic_class != 'archival'
                   AND NOT EXISTS (
                       SELECT 1 FROM kg_relationships r
                       WHERE r.source_entity_id = e.id
                   )
                   AND NOT EXISTS (
                       SELECT 1 FROM kg_relationships r
                       WHERE r.target_entity_id = e.id
                   )
                 LIMIT ?1",
            )?;
            let rows = stmt
                .query_map(params![limit], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
    }

    /// Soft-delete a single entity: flip `epistemic_class` + `compressed_into`
    /// and remove its name-index row. Wrapped in a transaction so readers
    /// never see a half-archived state.
    fn archive_entity(&self, entity_id: &str) -> Result<(), String> {
        let id = entity_id.to_string();
        self.db.with_connection(move |conn| {
            let tx = conn.unchecked_transaction()?;
            tx.execute(
                "UPDATE kg_entities
                 SET epistemic_class = 'archival',
                     compressed_into = ?1
                 WHERE id = ?2",
                params![ORPHAN_SENTINEL, id],
            )?;
            tx.execute(
                "DELETE FROM kg_name_index WHERE entity_id = ?1",
                params![id],
            )?;
            tx.commit()?;
            Ok(())
        })
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use gateway_database::{CompactionRepository, KnowledgeDatabase};
    use gateway_services::VaultPaths;
    use tempfile::TempDir;

    struct Harness {
        _tmp: TempDir,
        db: Arc<KnowledgeDatabase>,
        repo: Arc<CompactionRepository>,
    }

    fn setup() -> Harness {
        let tmp = tempfile::tempdir().expect("tempdir");
        let paths = Arc::new(VaultPaths::new(tmp.path().to_path_buf()));
        std::fs::create_dir_all(paths.conversations_db().parent().expect("parent")).expect("mkdir");
        let db = Arc::new(KnowledgeDatabase::new(paths).expect("knowledge db"));
        let repo = Arc::new(CompactionRepository::new(db.clone()));
        Harness {
            _tmp: tmp,
            db,
            repo,
        }
    }

    /// Insert a kg_entities row with fully-specified attributes so tests can
    /// control age/confidence/mention_count independently.
    #[allow(clippy::too_many_arguments)]
    fn insert_entity(
        db: &KnowledgeDatabase,
        id: &str,
        agent_id: &str,
        name: &str,
        mention_count: i64,
        confidence: f64,
        first_seen_at: &str,
        epistemic_class: &str,
        compressed_into: Option<&str>,
    ) {
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO kg_entities
                    (id, agent_id, entity_type, name, normalized_name, normalized_hash,
                     epistemic_class, confidence, mention_count, access_count,
                     first_seen_at, last_seen_at, compressed_into)
                 VALUES (?1, ?2, 'Concept', ?3, ?3, ?1, ?4, ?5, ?6, 0, ?7, ?7, ?8)",
                params![
                    id,
                    agent_id,
                    name,
                    epistemic_class,
                    confidence,
                    mention_count,
                    first_seen_at,
                    compressed_into,
                ],
            )?;
            Ok(())
        })
        .expect("insert entity");
    }

    fn insert_relationship(db: &KnowledgeDatabase, id: &str, agent_id: &str, src: &str, tgt: &str) {
        let now = chrono::Utc::now().to_rfc3339();
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO kg_relationships
                    (id, agent_id, source_entity_id, target_entity_id, relationship_type,
                     epistemic_class, confidence, mention_count, access_count,
                     first_seen_at, last_seen_at)
                 VALUES (?1, ?2, ?3, ?4, 'relates_to',
                         'current', 0.9, 1, 0, ?5, ?5)",
                params![id, agent_id, src, tgt, now],
            )?;
            Ok(())
        })
        .expect("insert rel");
    }

    fn days_ago(n: i64) -> String {
        (chrono::Utc::now() - chrono::Duration::days(n)).to_rfc3339()
    }

    fn hours_ago(n: i64) -> String {
        (chrono::Utc::now() - chrono::Duration::hours(n)).to_rfc3339()
    }

    #[tokio::test]
    async fn cycle_with_no_orphans_returns_zero() {
        let h = setup();
        let agent = "agent-none";
        // 3 entities, each with a relationship, so none qualify.
        for (i, name) in ["a", "b", "c"].iter().enumerate() {
            insert_entity(
                &h.db,
                &format!("e{i}"),
                agent,
                name,
                1,
                0.3,
                &days_ago(3),
                "current",
                None,
            );
        }
        insert_relationship(&h.db, "r-0", agent, "e0", "e1");
        insert_relationship(&h.db, "r-1", agent, "e1", "e2");
        // e0 has outgoing r-0; e1 has both; e2 has incoming r-1.
        // Only entities with zero in+out qualify. No entity qualifies → 0.

        let archiver = OrphanArchiver::new(h.db.clone(), h.repo.clone());
        let stats = archiver.run_cycle("run-none").await.expect("run");
        assert_eq!(stats.scanned, 0, "no orphans expected: {stats:?}");
        assert_eq!(stats.archived, 0);
        assert_eq!(stats.failed, 0);
    }

    #[tokio::test]
    async fn cycle_archives_mentioned_once_isolated_entity() {
        let h = setup();
        let agent = "agent-solo";
        insert_entity(
            &h.db,
            "lonely",
            agent,
            "lonely",
            1,
            0.3,
            &days_ago(3),
            "current",
            None,
        );

        let archiver = OrphanArchiver::new(h.db.clone(), h.repo.clone());
        let stats = archiver.run_cycle("run-solo").await.expect("run");
        assert_eq!(stats.scanned, 1);
        assert_eq!(stats.archived, 1);
        assert_eq!(stats.failed, 0);

        // Verify sentinel.
        let (class, sentinel): (String, Option<String>) =
            h.db.with_connection(|conn| {
                conn.query_row(
                    "SELECT epistemic_class, compressed_into FROM kg_entities WHERE id = 'lonely'",
                    [],
                    |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)),
                )
            })
            .expect("query");
        assert_eq!(class, "archival");
        assert_eq!(sentinel.as_deref(), Some(ORPHAN_SENTINEL));
    }

    #[tokio::test]
    async fn cycle_respects_confidence_threshold() {
        let h = setup();
        insert_entity(
            &h.db,
            "confident",
            "agent",
            "confident",
            1,
            0.7, // above threshold
            &days_ago(3),
            "current",
            None,
        );
        let archiver = OrphanArchiver::new(h.db.clone(), h.repo.clone());
        let stats = archiver.run_cycle("run-conf").await.expect("run");
        assert_eq!(stats.archived, 0, "high-confidence must survive: {stats:?}");
    }

    #[tokio::test]
    async fn cycle_respects_age_threshold() {
        let h = setup();
        insert_entity(
            &h.db,
            "fresh",
            "agent",
            "fresh",
            1,
            0.3,
            &hours_ago(1), // < 24h
            "current",
            None,
        );
        let archiver = OrphanArchiver::new(h.db.clone(), h.repo.clone());
        let stats = archiver.run_cycle("run-age").await.expect("run");
        assert_eq!(stats.archived, 0, "fresh entity must survive: {stats:?}");
    }

    #[tokio::test]
    async fn cycle_respects_relationship_guard() {
        let h = setup();
        let agent = "agent-rel";
        insert_entity(
            &h.db,
            "linked",
            agent,
            "linked",
            1,
            0.3,
            &days_ago(3),
            "current",
            None,
        );
        insert_entity(
            &h.db,
            "other",
            agent,
            "other",
            3,
            0.9,
            &days_ago(3),
            "current",
            None,
        );
        // Incoming edge into "linked" — disqualifies it.
        insert_relationship(&h.db, "r-in", agent, "other", "linked");

        let archiver = OrphanArchiver::new(h.db.clone(), h.repo.clone());
        let stats = archiver.run_cycle("run-rel").await.expect("run");
        assert_eq!(
            stats.archived, 0,
            "entity with incoming edge must survive: {stats:?}"
        );
    }

    #[tokio::test]
    async fn cycle_caps_at_100_per_run() {
        let h = setup();
        let agent = "agent-flood";
        for i in 0..150 {
            insert_entity(
                &h.db,
                &format!("e-{i}"),
                agent,
                &format!("n-{i}"),
                1,
                0.3,
                &days_ago(3),
                "current",
                None,
            );
        }
        let archiver = OrphanArchiver::new(h.db.clone(), h.repo.clone());
        let stats = archiver.run_cycle("run-flood").await.expect("run");
        assert_eq!(stats.scanned, 100, "cap must hold: {stats:?}");
        assert_eq!(stats.archived, 100);
    }

    #[tokio::test]
    async fn record_orphan_archive_adds_kg_compactions_row() {
        let h = setup();
        insert_entity(
            &h.db,
            "audit-me",
            "agent",
            "audit-me",
            1,
            0.3,
            &days_ago(3),
            "current",
            None,
        );
        let archiver = OrphanArchiver::new(h.db.clone(), h.repo.clone());
        let run_id = "run-audit";
        let stats = archiver.run_cycle(run_id).await.expect("run");
        assert_eq!(stats.archived, 1);
        let rows = h.repo.list_run(run_id).expect("list");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].operation, "prune");
        assert_eq!(rows[0].entity_id.as_deref(), Some("audit-me"));
        assert_eq!(rows[0].reason.as_deref(), Some(ARCHIVE_REASON));
    }

    #[tokio::test]
    async fn cycle_skips_already_archived_entity() {
        let h = setup();
        insert_entity(
            &h.db,
            "already",
            "agent",
            "already",
            1,
            0.3,
            &days_ago(3),
            "archival", // already archived
            Some("orphan-archive"),
        );
        let archiver = OrphanArchiver::new(h.db.clone(), h.repo.clone());
        let stats = archiver.run_cycle("run-skip").await.expect("run");
        assert_eq!(stats.archived, 0);
    }
}
