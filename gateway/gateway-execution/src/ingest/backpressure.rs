//! Backpressure and per-source rate limiting for ingestion.
//!
//! Two gates, both checked before enqueue:
//! - Global queue depth (pending + running episodes across all sources)
//! - Per-source pending count (protects against one book starving sessions)
//!
//! Violations return Err; the HTTP layer converts those to 429.

use std::sync::Arc;

use gateway_database::KgEpisodeRepository;

#[derive(Debug, Clone)]
pub struct BackpressureConfig {
    pub max_queue_depth: u64,
    pub max_per_source: u64,
}

impl Default for BackpressureConfig {
    fn default() -> Self {
        Self {
            max_queue_depth: 5_000,
            max_per_source: 500,
        }
    }
}

pub struct Backpressure {
    config: BackpressureConfig,
    episode_repo: Arc<KgEpisodeRepository>,
}

impl Backpressure {
    pub fn new(config: BackpressureConfig, episode_repo: Arc<KgEpisodeRepository>) -> Self {
        Self {
            config,
            episode_repo,
        }
    }

    /// Returns Err(String) with a human-readable reason if either gate
    /// rejects the source. The HTTP caller maps this to 429 Too Many Requests.
    pub fn check(&self, source_ref_prefix: &str) -> Result<(), String> {
        let global_pending = self.episode_repo.count_pending_global().unwrap_or(0);
        if global_pending >= self.config.max_queue_depth {
            return Err(format!(
                "ingestion queue full ({global_pending} pending+running, limit {})",
                self.config.max_queue_depth
            ));
        }
        let per_source_pending = self
            .episode_repo
            .count_pending_for_source(source_ref_prefix)
            .unwrap_or(0);
        if per_source_pending >= self.config.max_per_source {
            return Err(format!(
                "source backpressure ({per_source_pending} pending+running for '{source_ref_prefix}', limit {})",
                self.config.max_per_source
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gateway_database::KnowledgeDatabase;
    use gateway_services::VaultPaths;

    fn setup() -> (tempfile::TempDir, Arc<KgEpisodeRepository>) {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Arc::new(VaultPaths::new(tmp.path().to_path_buf()));
        std::fs::create_dir_all(paths.conversations_db().parent().unwrap()).unwrap();
        let db = Arc::new(KnowledgeDatabase::new(paths).unwrap());
        (tmp, Arc::new(KgEpisodeRepository::new(db)))
    }

    #[test]
    fn allows_when_queue_is_empty() {
        let (_tmp, repo) = setup();
        let bp = Backpressure::new(BackpressureConfig::default(), repo);
        assert!(bp.check("anything").is_ok());
    }

    #[test]
    fn rejects_over_global_cap() {
        let (_tmp, repo) = setup();
        // Seed exactly the cap number of pending episodes.
        let cap = 3;
        for i in 0..cap {
            repo.upsert_pending("t", &format!("x#{i}"), &format!("h{i}"), None, "root")
                .unwrap();
        }
        let bp = Backpressure::new(
            BackpressureConfig {
                max_queue_depth: cap as u64,
                max_per_source: 1_000,
            },
            repo,
        );
        assert!(bp.check("x").is_err());
    }

    #[test]
    fn rejects_over_per_source_cap() {
        let (_tmp, repo) = setup();
        for i in 0..5 {
            repo.upsert_pending("t", &format!("book#{i}"), &format!("h{i}"), None, "root")
                .unwrap();
        }
        let bp = Backpressure::new(
            BackpressureConfig {
                max_queue_depth: 1_000,
                max_per_source: 5,
            },
            repo,
        );
        assert!(bp.check("book").is_err());
        // Different source still allowed.
        assert!(bp.check("other").is_ok());
    }
}
