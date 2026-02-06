//! # Batch Writer
//!
//! Decouples hot-path DB writes from stream event processing by batching
//! token updates and log entries into a background task.
//!
//! Instead of writing to SQLite synchronously during stream callbacks,
//! callers send write requests through an mpsc channel. The background
//! task coalesces token updates (keeping only the latest per execution)
//! and batch-inserts log entries.

use gateway_database::DatabaseManager;
use api_logs::{ExecutionLog, LogService};
use execution_state::StateService;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

/// A write request for the batch writer.
pub enum BatchWrite {
    /// Update token counts for an execution.
    /// Coalesced: only the latest values per execution_id are persisted.
    TokenUpdate {
        execution_id: String,
        tokens_in: u64,
        tokens_out: u64,
    },

    /// Persist an execution log entry.
    LogEntry(ExecutionLog),
}

/// Handle for sending writes to the batch writer.
///
/// Cheap to clone. Sending is non-blocking (unbounded channel).
#[derive(Clone)]
pub struct BatchWriterHandle {
    tx: mpsc::UnboundedSender<BatchWrite>,
}

impl BatchWriterHandle {
    /// Send a write request to the batch writer.
    ///
    /// Returns immediately. The write will be persisted asynchronously.
    pub fn send(&self, write: BatchWrite) {
        if self.tx.send(write).is_err() {
            tracing::warn!("BatchWriter channel closed, write dropped");
        }
    }

    /// Convenience: send a token update.
    pub fn token_update(&self, execution_id: &str, tokens_in: u64, tokens_out: u64) {
        self.send(BatchWrite::TokenUpdate {
            execution_id: execution_id.to_string(),
            tokens_in,
            tokens_out,
        });
    }

    /// Convenience: send a log entry.
    pub fn log(&self, entry: ExecutionLog) {
        self.send(BatchWrite::LogEntry(entry));
    }
}

/// Spawn a batch writer background task.
///
/// Returns a handle for sending writes. The background task runs until
/// the handle (and all clones) are dropped, at which point it flushes
/// remaining writes and exits.
pub fn spawn_batch_writer(
    state_service: Arc<StateService<DatabaseManager>>,
    log_service: Arc<LogService<DatabaseManager>>,
) -> BatchWriterHandle {
    let (tx, rx) = mpsc::unbounded_channel();

    tokio::spawn(batch_writer_loop(rx, state_service, log_service));

    BatchWriterHandle { tx }
}

/// Background loop that processes batched writes.
async fn batch_writer_loop(
    mut rx: mpsc::UnboundedReceiver<BatchWrite>,
    state_service: Arc<StateService<DatabaseManager>>,
    log_service: Arc<LogService<DatabaseManager>>,
) {
    // Pending token updates — coalesced by execution_id (only latest kept)
    let mut token_updates: HashMap<String, (u64, u64)> = HashMap::new();
    // Pending log entries
    let mut log_entries: Vec<ExecutionLog> = Vec::new();

    let mut interval = tokio::time::interval(Duration::from_millis(100));
    // Don't accumulate ticks while we're busy flushing
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Some(BatchWrite::TokenUpdate { execution_id, tokens_in, tokens_out }) => {
                        // Coalesce: overwrite previous value for this execution
                        token_updates.insert(execution_id, (tokens_in, tokens_out));
                    }
                    Some(BatchWrite::LogEntry(entry)) => {
                        log_entries.push(entry);
                    }
                    None => {
                        // Channel closed — flush remaining and exit
                        flush_all(&state_service, &log_service, &mut token_updates, &mut log_entries);
                        tracing::debug!("BatchWriter shutting down after final flush");
                        return;
                    }
                }

                // Flush if we've accumulated enough items
                let total = token_updates.len() + log_entries.len();
                if total >= 10 {
                    flush_all(&state_service, &log_service, &mut token_updates, &mut log_entries);
                }
            }
            _ = interval.tick() => {
                // Periodic flush
                if !token_updates.is_empty() || !log_entries.is_empty() {
                    flush_all(&state_service, &log_service, &mut token_updates, &mut log_entries);
                }
            }
        }
    }
}

/// Flush all pending writes to the database.
fn flush_all(
    state_service: &StateService<DatabaseManager>,
    log_service: &LogService<DatabaseManager>,
    token_updates: &mut HashMap<String, (u64, u64)>,
    log_entries: &mut Vec<ExecutionLog>,
) {
    // Flush token updates (coalesced — one write per execution)
    for (execution_id, (tokens_in, tokens_out)) in token_updates.drain() {
        if let Err(e) = state_service.update_execution_tokens(&execution_id, tokens_in, tokens_out)
        {
            tracing::warn!("BatchWriter: failed to update tokens for {}: {}", execution_id, e);
        }
    }

    // Flush log entries
    for entry in log_entries.drain(..) {
        if let Err(e) = log_service.log(entry) {
            tracing::warn!("BatchWriter: failed to write log: {}", e);
        }
    }
}
