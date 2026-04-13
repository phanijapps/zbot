//! Sleep-time worker — background tokio task running compaction + decay + prune.
//!
//! Runs on a configurable interval (default 60 min). Callers can also trigger
//! an immediate run via `SleepTimeWorker::trigger()` (e.g. from a
//! `POST /api/memory/consolidate` handler).
//!
//! One `run_id` per cycle, recorded across all ops via `CompactionRepository`.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;

use crate::sleep::{Compactor, DecayEngine, Pruner};

/// Background worker that orchestrates the full sleep-time pipeline.
///
/// Spawned via [`SleepTimeWorker::start`]. The returned handle exposes
/// [`SleepTimeWorker::trigger`] for callers that want an immediate cycle
/// (e.g. a REST endpoint) rather than waiting for the next periodic tick.
pub struct SleepTimeWorker {
    trigger_tx: mpsc::Sender<()>,
}

impl SleepTimeWorker {
    /// Spawn the worker. Returns a handle that callers can use to force-trigger
    /// a cycle in addition to the periodic one.
    pub fn start(
        compactor: Arc<Compactor>,
        decay_engine: Arc<DecayEngine>,
        pruner: Arc<Pruner>,
        interval: Duration,
        agent_id: String,
    ) -> Self {
        let (tx, mut rx) = mpsc::channel::<()>(8);

        tokio::spawn(async move {
            // Use `interval` but explicitly skip its initial immediate fire so
            // we don't hammer the graph at boot — wait one full period before the
            // first scheduled run. On-demand triggers bypass this.
            let mut ticker = tokio::time::interval(interval);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            // consume the immediate tick fired by interval().
            ticker.tick().await;

            tracing::info!(
                interval_secs = interval.as_secs(),
                agent_id = %agent_id,
                "sleep-time worker started",
            );

            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        run_cycle("scheduled", &compactor, &decay_engine, &pruner, &agent_id).await;
                    }
                    maybe = rx.recv() => {
                        if maybe.is_none() {
                            tracing::info!("sleep-time worker trigger channel closed; exiting");
                            break;
                        }
                        run_cycle("on-demand", &compactor, &decay_engine, &pruner, &agent_id).await;
                    }
                }
            }
        });

        Self { trigger_tx: tx }
    }

    /// Non-blocking on-demand trigger. Drops the signal if the channel is full
    /// (caller can retry — the worker will pick up the next tick anyway).
    pub fn trigger(&self) {
        let _ = self.trigger_tx.try_send(());
    }
}

async fn run_cycle(
    kind: &str,
    compactor: &Arc<Compactor>,
    decay_engine: &Arc<DecayEngine>,
    pruner: &Arc<Pruner>,
    agent_id: &str,
) {
    let run_id = format!("sleep-{}", uuid::Uuid::new_v4());
    tracing::info!(kind, %run_id, agent_id, "sleep-time cycle start");

    let compaction_stats = compactor.run(&run_id, agent_id).await;

    let candidates = decay_engine.list_prune_candidates(agent_id);
    let prune_stats = pruner.prune(&run_id, &candidates);

    tracing::info!(
        kind,
        %run_id,
        candidates_considered = compaction_stats.candidates_considered,
        merges = compaction_stats.merges_performed,
        prune_candidates = candidates.len(),
        pruned = prune_stats.pruned,
        pruned_failed = prune_stats.failed,
        "sleep-time cycle done"
    );
}
