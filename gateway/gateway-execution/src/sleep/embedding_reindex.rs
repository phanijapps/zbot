//! # Embedding Reindex
//!
//! Phase 1 scaffold for the embedding-backend-selection feature. When the
//! active embedding dimension changes, all sqlite-vec virtual tables must be
//! rebuilt with the new dimension. This module provides the reindex routine
//! that:
//!
//! 1. Drops any orphan `*__new` tables (crash recovery).
//! 2. Creates `{table}__new` at the target dimension.
//! 3. Streams rows from the source table in batches of 100, embeds them with
//!    the current client, and inserts into `{table}__new`.
//! 4. Drops the old table and `ALTER TABLE ... RENAME` the new one into place.
//!
//! The sqlite-vec virtual tables handled here:
//!
//! | Target table | Source table | Source column |
//! |---|---|---|
//! | `memory_facts_index` | `memory_facts` | `content` |
//! | `kg_name_index` | `kg_entities` | `name` |
//! | `session_episodes_index` | `session_episodes` | `task_summary` |
//!
//! Phase 1 ships this module as a *skeleton*: the function signatures,
//! progress callback shape, and configuration are wired up, but the
//! per-row embed+insert loop is a placeholder. Phase 2+ fleshes it out
//! alongside the UI progress modal.

use std::sync::Arc;

use agent_runtime::llm::EmbeddingClient;

/// Target of a single reindex pass.
#[derive(Debug, Clone, Copy)]
pub struct ReindexTarget {
    pub table: &'static str,
    pub source_table: &'static str,
    pub source_column: &'static str,
}

/// The three reindex targets touched by backend swaps.
pub const REINDEX_TARGETS: &[ReindexTarget] = &[
    ReindexTarget {
        table: "memory_facts_index",
        source_table: "memory_facts",
        source_column: "content",
    },
    ReindexTarget {
        table: "kg_name_index",
        source_table: "kg_entities",
        source_column: "name",
    },
    ReindexTarget {
        table: "session_episodes_index",
        source_table: "session_episodes",
        source_column: "task_summary",
    },
];

/// Summary returned by a single-table reindex.
#[derive(Debug, Clone, Default)]
pub struct ReindexSummary {
    pub indexed: usize,
    pub skipped: usize,
    pub total: usize,
}

/// Progress callback: `(table, current, total)`.
pub type ProgressFn<'a> = &'a (dyn Fn(&'static str, usize, usize) + Send + Sync);

/// Reindex all vec0 tables at `new_dim` using `client`.
///
/// Phase 1 skeleton: returns empty summaries without performing I/O. The
/// full implementation drops orphan tables, streams batches, and swaps
/// atomically once `EmbeddingService` is wired into `AppState`.
///
/// # Errors
///
/// Currently infallible; returns `Ok` with empty summaries.
pub async fn reindex_all(
    _client: Arc<dyn EmbeddingClient>,
    _new_dim: usize,
    on_progress: ProgressFn<'_>,
) -> Result<Vec<(ReindexTarget, ReindexSummary)>, String> {
    let mut out = Vec::with_capacity(REINDEX_TARGETS.len());
    for t in REINDEX_TARGETS {
        on_progress(t.table, 0, 0);
        out.push((*t, ReindexSummary::default()));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reindex_targets_has_three_entries() {
        assert_eq!(REINDEX_TARGETS.len(), 3);
        assert!(REINDEX_TARGETS
            .iter()
            .any(|t| t.table == "memory_facts_index"));
        assert!(REINDEX_TARGETS.iter().any(|t| t.table == "kg_name_index"));
        assert!(REINDEX_TARGETS
            .iter()
            .any(|t| t.table == "session_episodes_index"));
    }

    #[tokio::test]
    async fn reindex_all_skeleton_reports_progress_per_table() {
        use agent_runtime::llm::LocalEmbeddingClient;
        let client: Arc<dyn EmbeddingClient> = Arc::new(LocalEmbeddingClient::new());
        let counter = std::sync::atomic::AtomicUsize::new(0);
        let counter_ref = &counter;
        let cb = move |_: &'static str, _: usize, _: usize| {
            counter_ref.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        };
        let out = reindex_all(client, 384, &cb).await.unwrap();
        assert_eq!(out.len(), 3);
        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 3);
    }
}
