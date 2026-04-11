//! # Batch Writer
//!
//! Decouples hot-path DB writes from stream event processing by batching
//! token updates and log entries into a background task.
//!
//! Instead of writing to SQLite synchronously during stream callbacks,
//! callers send write requests through an mpsc channel. The background
//! task coalesces token updates (keeping only the latest per execution)
//! and batch-inserts log entries.

use api_logs::{ExecutionLog, LogService};
use execution_state::StateService;
use gateway_database::{ConversationRepository, DatabaseManager};
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

    /// Append a message to a session's conversation stream.
    SessionMessage {
        session_id: String,
        execution_id: String,
        role: String,
        content: String,
        tool_calls: Option<String>,
        tool_call_id: Option<String>,
    },
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

    /// Convenience: append a message to the session conversation stream.
    pub fn session_message(
        &self,
        session_id: &str,
        execution_id: &str,
        role: &str,
        content: &str,
        tool_calls: Option<&str>,
        tool_call_id: Option<&str>,
    ) {
        self.send(BatchWrite::SessionMessage {
            session_id: session_id.to_string(),
            execution_id: execution_id.to_string(),
            role: role.to_string(),
            content: content.to_string(),
            tool_calls: tool_calls.map(String::from),
            tool_call_id: tool_call_id.map(String::from),
        });
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
    spawn_batch_writer_with_repo(state_service, log_service, None)
}

/// Spawn a batch writer with optional conversation repository for session messages.
pub fn spawn_batch_writer_with_repo(
    state_service: Arc<StateService<DatabaseManager>>,
    log_service: Arc<LogService<DatabaseManager>>,
    conversation_repo: Option<Arc<ConversationRepository>>,
) -> BatchWriterHandle {
    let (tx, rx) = mpsc::unbounded_channel();

    tokio::spawn(batch_writer_loop(
        rx,
        state_service,
        log_service,
        conversation_repo,
    ));

    BatchWriterHandle { tx }
}

/// Background loop that processes batched writes.
async fn batch_writer_loop(
    mut rx: mpsc::UnboundedReceiver<BatchWrite>,
    state_service: Arc<StateService<DatabaseManager>>,
    log_service: Arc<LogService<DatabaseManager>>,
    conversation_repo: Option<Arc<ConversationRepository>>,
) {
    // Pending token updates — coalesced by execution_id (only latest kept)
    let mut token_updates: HashMap<String, (u64, u64)> = HashMap::new();
    // Pending log entries
    let mut log_entries: Vec<ExecutionLog> = Vec::new();
    // Pending session messages (NOT coalesced — each is unique)
    #[allow(clippy::type_complexity)]
    let mut session_messages: Vec<(
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
    )> = Vec::new();

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
                    Some(BatchWrite::SessionMessage { session_id, execution_id, role, content, tool_calls, tool_call_id }) => {
                        session_messages.push((session_id, execution_id, role, content, tool_calls, tool_call_id));
                    }
                    None => {
                        // Channel closed — flush remaining and exit
                        flush_all(&state_service, &log_service, conversation_repo.as_deref(), &mut token_updates, &mut log_entries, &mut session_messages);
                        tracing::debug!("BatchWriter shutting down after final flush");
                        return;
                    }
                }

                // Flush if we've accumulated enough items
                let total = token_updates.len() + log_entries.len() + session_messages.len();
                if total >= 10 {
                    flush_all(&state_service, &log_service, conversation_repo.as_deref(), &mut token_updates, &mut log_entries, &mut session_messages);
                }
            }
            _ = interval.tick() => {
                // Periodic flush
                if !token_updates.is_empty() || !log_entries.is_empty() || !session_messages.is_empty() {
                    flush_all(&state_service, &log_service, conversation_repo.as_deref(), &mut token_updates, &mut log_entries, &mut session_messages);
                }
            }
        }
    }
}

/// Flush all pending writes to the database.
#[allow(clippy::type_complexity)]
fn flush_all(
    state_service: &StateService<DatabaseManager>,
    log_service: &LogService<DatabaseManager>,
    conversation_repo: Option<&ConversationRepository>,
    token_updates: &mut HashMap<String, (u64, u64)>,
    log_entries: &mut Vec<ExecutionLog>,
    session_messages: &mut Vec<(
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
    )>,
) {
    // Flush token updates (coalesced — one write per execution)
    for (execution_id, (tokens_in, tokens_out)) in token_updates.drain() {
        if let Err(e) = state_service.update_execution_tokens(&execution_id, tokens_in, tokens_out)
        {
            tracing::warn!(
                "BatchWriter: failed to update tokens for {}: {}",
                execution_id,
                e
            );
        }
    }

    // Flush log entries
    for entry in log_entries.drain(..) {
        if let Err(e) = log_service.log(entry) {
            tracing::warn!("BatchWriter: failed to write log: {}", e);
        }
    }

    // Flush session messages (order-preserving)
    if let Some(repo) = conversation_repo {
        for (session_id, execution_id, role, content, tool_calls, tool_call_id) in
            session_messages.drain(..)
        {
            if let Err(e) = repo.append_session_message(
                &session_id,
                &execution_id,
                &role,
                &content,
                tool_calls.as_deref(),
                tool_call_id.as_deref(),
            ) {
                tracing::warn!("BatchWriter: failed to append session message: {}", e);
            }
        }
    } else if !session_messages.is_empty() {
        tracing::warn!(
            "BatchWriter: {} session messages dropped (no conversation repo)",
            session_messages.len()
        );
        session_messages.clear();
    }
}
