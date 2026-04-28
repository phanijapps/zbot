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
use zero_stores_sqlite::{ConversationRepository, DatabaseManager};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

/// An appended row on a session's conversation stream.
#[derive(Debug, Clone)]
pub struct SessionMessage {
    pub session_id: String,
    pub execution_id: String,
    pub role: String,
    pub content: String,
    pub tool_calls: Option<String>,
    pub tool_call_id: Option<String>,
}

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
    SessionMessage(SessionMessage),
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
        self.send(BatchWrite::SessionMessage(SessionMessage {
            session_id: session_id.to_string(),
            execution_id: execution_id.to_string(),
            role: role.to_string(),
            content: content.to_string(),
            tool_calls: tool_calls.map(String::from),
            tool_call_id: tool_call_id.map(String::from),
        }));
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
    let mut session_messages: Vec<SessionMessage> = Vec::new();

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
                    Some(BatchWrite::SessionMessage(msg)) => {
                        session_messages.push(msg);
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
fn flush_all(
    state_service: &StateService<DatabaseManager>,
    log_service: &LogService<DatabaseManager>,
    conversation_repo: Option<&ConversationRepository>,
    token_updates: &mut HashMap<String, (u64, u64)>,
    log_entries: &mut Vec<ExecutionLog>,
    session_messages: &mut Vec<SessionMessage>,
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
        for msg in session_messages.drain(..) {
            if let Err(e) = repo.append_session_message(
                &msg.session_id,
                &msg.execution_id,
                &msg.role,
                &msg.content,
                msg.tool_calls.as_deref(),
                msg.tool_call_id.as_deref(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use api_logs::{LogCategory, LogLevel};
    use gateway_services::VaultPaths;
    use tempfile::TempDir;

    /// Full wiring: temp vault, real DB, services, a seeded session/execution
    /// so FKs (artifacts / token updates / session_messages) are satisfied.
    struct Harness {
        _tmp: TempDir,
        state: Arc<StateService<DatabaseManager>>,
        logs: Arc<LogService<DatabaseManager>>,
        convo: Arc<ConversationRepository>,
        session_id: String,
        execution_id: String,
    }

    fn setup() -> Harness {
        let tmp = TempDir::new().expect("tempdir");
        let paths = Arc::new(VaultPaths::new(tmp.path().to_path_buf()));
        paths.ensure_dirs_exist().expect("ensure vault dirs");
        let db = Arc::new(DatabaseManager::new(paths).expect("db init"));
        let state = Arc::new(StateService::new(db.clone()));
        let logs = Arc::new(LogService::new(db.clone()));
        let convo = Arc::new(ConversationRepository::new(db));
        let (session, execution) = state.create_session("agent-test").expect("seed session");
        Harness {
            _tmp: tmp,
            state,
            logs,
            convo,
            session_id: session.id,
            execution_id: execution.id,
        }
    }

    // ------------------------------------------------------------------
    // Handle: channel behaviour + convenience methods serialize correctly
    // ------------------------------------------------------------------

    #[test]
    fn send_on_closed_channel_does_not_panic() {
        let (tx, rx) = mpsc::unbounded_channel();
        drop(rx); // receiver gone — any send is a dead-letter
        let handle = BatchWriterHandle { tx };
        // Covers both the convenience method and the silent-drop branch in
        // BatchWriterHandle::send.
        handle.token_update("e1", 1, 2);
        handle.log(ExecutionLog::new(
            "s1",
            "c1",
            "a1",
            LogLevel::Info,
            LogCategory::Session,
            "msg",
        ));
        handle.session_message("s1", "e1", "user", "hi", None, None);
    }

    #[tokio::test]
    async fn convenience_methods_enqueue_correct_variants() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let handle = BatchWriterHandle { tx };

        handle.token_update("exec-9", 11, 22);
        match rx.recv().await.expect("token_update queued") {
            BatchWrite::TokenUpdate {
                execution_id,
                tokens_in,
                tokens_out,
            } => {
                assert_eq!(execution_id, "exec-9");
                assert_eq!(tokens_in, 11);
                assert_eq!(tokens_out, 22);
            }
            other => panic!(
                "expected TokenUpdate, got {:?}",
                std::mem::discriminant(&other)
            ),
        }

        handle.session_message("s1", "e1", "user", "hello", Some("tc"), Some("tcid"));
        match rx.recv().await.expect("session_message queued") {
            BatchWrite::SessionMessage(msg) => {
                assert_eq!(msg.session_id, "s1");
                assert_eq!(msg.role, "user");
                assert_eq!(msg.content, "hello");
                assert_eq!(msg.tool_calls.as_deref(), Some("tc"));
                assert_eq!(msg.tool_call_id.as_deref(), Some("tcid"));
            }
            other => panic!(
                "expected SessionMessage, got {:?}",
                std::mem::discriminant(&other)
            ),
        }
    }

    // ------------------------------------------------------------------
    // Loop: runs until channel closes, flushes on shutdown
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn loop_exits_and_flushes_when_channel_closes() {
        let h = setup();
        let (tx, rx) = mpsc::unbounded_channel();

        let task = tokio::spawn(batch_writer_loop(
            rx,
            h.state.clone(),
            h.logs.clone(),
            Some(h.convo.clone()),
        ));

        // Enqueue a log and a session message. Neither is on the 10-item fast
        // path — the shutdown-flush branch is what persists them.
        tx.send(BatchWrite::LogEntry(ExecutionLog::new(
            &h.session_id,
            "conv-1",
            "agent-test",
            LogLevel::Info,
            LogCategory::Session,
            "hello",
        )))
        .expect("send log");
        tx.send(BatchWrite::SessionMessage(SessionMessage {
            session_id: h.session_id.clone(),
            execution_id: h.execution_id.clone(),
            role: "user".into(),
            content: "from-batch".into(),
            tool_calls: None,
            tool_call_id: None,
        }))
        .expect("send msg");

        drop(tx);
        task.await.expect("task joins cleanly");

        // Final-flush branch must have written the session message.
        let msgs = h.convo.get_messages(&h.execution_id).expect("get_messages");
        assert!(
            msgs.iter().any(|m| m.content == "from-batch"),
            "expected flushed session message in {msgs:?}"
        );
    }

    #[tokio::test]
    async fn token_updates_coalesce_to_latest_value_per_execution() {
        let h = setup();
        let (tx, rx) = mpsc::unbounded_channel();

        let task = tokio::spawn(batch_writer_loop(
            rx,
            h.state.clone(),
            h.logs.clone(),
            Some(h.convo.clone()),
        ));

        for (tin, tout) in [(1, 2), (3, 4), (5, 6), (7, 8)] {
            tx.send(BatchWrite::TokenUpdate {
                execution_id: h.execution_id.clone(),
                tokens_in: tin,
                tokens_out: tout,
            })
            .expect("send");
        }
        drop(tx);
        task.await.expect("task joins");

        let execution = h
            .state
            .get_execution(&h.execution_id)
            .expect("get_execution")
            .expect("execution exists");
        assert_eq!(
            execution.tokens_in, 7,
            "coalesce must keep the LAST tokens_in"
        );
        assert_eq!(
            execution.tokens_out, 8,
            "coalesce must keep the LAST tokens_out"
        );
    }

    #[tokio::test]
    async fn count_threshold_flushes_mid_loop() {
        let h = setup();
        let (tx, rx) = mpsc::unbounded_channel();

        let task = tokio::spawn(batch_writer_loop(
            rx,
            h.state.clone(),
            h.logs.clone(),
            Some(h.convo.clone()),
        ));

        // Ten session messages pushes the pending-count gate at ≥10. The
        // in-loop flush branch must fire before channel close.
        for i in 0..10 {
            tx.send(BatchWrite::SessionMessage(SessionMessage {
                session_id: h.session_id.clone(),
                execution_id: h.execution_id.clone(),
                role: "user".into(),
                content: format!("msg-{i}"),
                tool_calls: None,
                tool_call_id: None,
            }))
            .expect("send");
        }
        drop(tx);
        task.await.expect("task joins");

        let msgs = h.convo.get_messages(&h.execution_id).expect("get_messages");
        // 10 sent, 10 must land. Order preserved by the Vec.
        let batch_msgs: Vec<_> = msgs
            .iter()
            .filter(|m| m.content.starts_with("msg-"))
            .collect();
        assert_eq!(batch_msgs.len(), 10);
        for (i, m) in batch_msgs.iter().enumerate() {
            assert_eq!(m.content, format!("msg-{i}"));
        }
    }

    #[tokio::test]
    async fn session_messages_dropped_when_no_conversation_repo() {
        let h = setup();
        let (tx, rx) = mpsc::unbounded_channel();

        // Spawn WITHOUT a conversation repo — session messages should be
        // dropped on flush with a warn!(), not crash.
        let task = tokio::spawn(batch_writer_loop(rx, h.state.clone(), h.logs.clone(), None));

        tx.send(BatchWrite::SessionMessage(SessionMessage {
            session_id: h.session_id.clone(),
            execution_id: h.execution_id.clone(),
            role: "user".into(),
            content: "orphaned".into(),
            tool_calls: None,
            tool_call_id: None,
        }))
        .expect("send");
        drop(tx);
        task.await.expect("task joins cleanly");

        // And no row should have been written by any side channel.
        let msgs = h.convo.get_messages(&h.execution_id).expect("get_messages");
        assert!(
            msgs.iter().all(|m| m.content != "orphaned"),
            "orphaned message must NOT have been written to the DB"
        );
    }

    #[tokio::test]
    async fn spawn_batch_writer_returns_working_handle() {
        // Integration smoke test of the public spawn helpers — just covers the
        // `spawn_batch_writer` and `spawn_batch_writer_with_repo` entry points
        // so those aren't 0%.
        let h = setup();
        let handle =
            spawn_batch_writer_with_repo(h.state.clone(), h.logs.clone(), Some(h.convo.clone()));
        handle.token_update(&h.execution_id, 100, 200);
        drop(handle);

        // Give the spawned task a moment to observe channel close + flush.
        // 300ms comfortably exceeds the 100ms periodic tick + flush_all work.
        tokio::time::sleep(Duration::from_millis(300)).await;

        let execution = h
            .state
            .get_execution(&h.execution_id)
            .expect("get_execution")
            .expect("execution exists");
        assert_eq!(execution.tokens_in, 100);
        assert_eq!(execution.tokens_out, 200);
    }
}
