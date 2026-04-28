//! Ingestion queue — producers signal "wake up, work exists"; N workers
//! race to `claim_next_pending`, one wins, others resleep. Work lives in
//! the DB so worker restarts recover in-flight pending episodes.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Notify};

use crate::ingest::extractor::Extractor;
use gateway_database::KgEpisodeRepository;
use zero_stores_sqlite::kg::storage::GraphStorage;

const WAKE_CHANNEL_CAPACITY: usize = 256;
const CLAIM_FAILURE_BACKOFF: Duration = Duration::from_millis(500);

pub struct IngestionQueue {
    tx: mpsc::Sender<()>,
}

impl IngestionQueue {
    /// Spawn `workers` background tasks; return a handle producers can use
    /// to signal new work.
    pub fn start(
        workers: usize,
        episode_repo: Arc<KgEpisodeRepository>,
        graph: Arc<GraphStorage>,
        extractor: Arc<dyn Extractor>,
    ) -> Self {
        let (tx, mut rx) = mpsc::channel::<()>(WAKE_CHANNEL_CAPACITY);
        let notify = Arc::new(Notify::new());

        // Fan-out: a small dispatcher drains the mpsc and fires notify_waiters()
        // so every idle worker wakes. Workers race to claim; only one wins,
        // others re-sleep.
        {
            let notify = notify.clone();
            tokio::spawn(async move {
                while rx.recv().await.is_some() {
                    notify.notify_waiters();
                }
            });
        }

        for worker_idx in 0..workers {
            let episode_repo = episode_repo.clone();
            let graph = graph.clone();
            let extractor = extractor.clone();
            let notify = notify.clone();
            tokio::spawn(async move {
                worker_loop(worker_idx, episode_repo, graph, extractor, notify).await;
            });
        }

        Self { tx }
    }

    /// Signal workers that work exists. Non-blocking; if the wake channel
    /// is full, workers will pick up on their next claim attempt anyway.
    pub fn notify(&self) {
        let _ = self.tx.try_send(());
    }
}

async fn worker_loop(
    worker_idx: usize,
    episode_repo: Arc<KgEpisodeRepository>,
    graph: Arc<GraphStorage>,
    extractor: Arc<dyn Extractor>,
    notify: Arc<Notify>,
) {
    tracing::info!(worker_idx, "ingestion worker started");
    loop {
        let repo_for_claim = episode_repo.clone();
        let claim = tokio::task::spawn_blocking(move || repo_for_claim.claim_next_pending()).await;

        let episode = match claim {
            Ok(Ok(Some(e))) => e,
            Ok(Ok(None)) => {
                notify.notified().await;
                continue;
            }
            Ok(Err(e)) => {
                tracing::warn!(worker_idx, error = %e, "claim_next_pending failed");
                tokio::time::sleep(CLAIM_FAILURE_BACKOFF).await;
                continue;
            }
            Err(e) => {
                tracing::warn!(worker_idx, error = %e, "spawn_blocking join failed");
                continue;
            }
        };

        let episode_id = episode.id.clone();
        let payload_fetch = {
            let repo = episode_repo.clone();
            let id = episode_id.clone();
            tokio::task::spawn_blocking(move || repo.get_payload(&id)).await
        };
        let chunk_text = match payload_fetch {
            Ok(Ok(Some(text))) => text,
            Ok(Ok(None)) => {
                tracing::warn!(
                    worker_idx,
                    episode_id = %episode_id,
                    "episode has no payload — marking failed",
                );
                let repo = episode_repo.clone();
                let id = episode_id.clone();
                let _ =
                    tokio::task::spawn_blocking(move || repo.mark_failed(&id, "payload missing"))
                        .await;
                continue;
            }
            Ok(Err(e)) => {
                tracing::warn!(worker_idx, error = %e, "get_payload failed");
                tokio::time::sleep(CLAIM_FAILURE_BACKOFF).await;
                continue;
            }
            Err(e) => {
                tracing::warn!(error = %e, "spawn_blocking join on get_payload");
                continue;
            }
        };
        let result = extractor.process(&episode, &chunk_text, &graph).await;

        let finish = match result {
            Ok(()) => {
                let repo = episode_repo.clone();
                let id = episode_id.clone();
                tokio::task::spawn_blocking(move || repo.mark_done(&id)).await
            }
            Err(err_msg) => {
                tracing::warn!(
                    worker_idx,
                    episode_id = %episode_id,
                    error = %err_msg,
                    "extractor failed; marking episode failed",
                );
                let repo = episode_repo.clone();
                let id = episode_id.clone();
                let err = err_msg.clone();
                tokio::task::spawn_blocking(move || repo.mark_failed(&id, &err)).await
            }
        };

        match finish {
            Ok(Ok(())) => {}
            Ok(Err(db_err)) => {
                tracing::warn!(error = %db_err, "finish status update DB error");
            }
            Err(join_err) => {
                tracing::warn!(error = %join_err, "finish status update join failed");
            }
        }
    }
}
