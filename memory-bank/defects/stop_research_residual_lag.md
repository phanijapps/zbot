# Investigation — Research stop UX has residual non-seamlessness

**Severity:** Low (functional; UX-only)
**Discovered:** 2026-05-03 after PR #99 (cooperative-stop fix) shipped
**Status:** Open, **investigation only — not a blocker**

## Symptom

After PR #99 lands, clicking Stop on a `/research` session **does**
terminate the session — confirmed working — but it's not seamless.
There's a perceptible-but-tolerable lag or visible imperfection between
click and full UI reflection of the stopped state. The functional
outcome is correct (session ends in `cancelled` status, stream halts),
but the experience isn't yet on par with the snappier quick-chat stop.

User judgment: *"kind of works but not seamless but it is ok"*.

## What PR #99 did

- Plumbed `Option<Arc<AtomicBool>>` through
  `AgentExecutor::execute_stream_with_stop_flag` so the chunk-receive
  `select!` loop polls the flag every 100 ms; on observation it
  `stream_handle.abort()`s the spawned LLM task and returns
  `Err(ExecutorError::Stopped)`.
- Three gateway call sites pass `Some(handle.stop_signal())`. The
  `Stopped` arm logs and lets the existing trailing
  `if handle.is_stop_requested() { stop_execution(...) }` block do the
  finalization (single canonical path; no double-cancel warn).
- `ExecutionRunner::stop` cascades the signal to delegated subagents
  via `DelegationRegistry::get_children`.

What that does NOT do: cancel the upstream LLM HTTP request synchronously
sub-100 ms, propagate stop into the LLM provider's reqwest body stream
mid-await, or zero-latency notify the UI before final batch-writer
flushes complete.

## Suspected residual contributors

Listing in roughly decreasing-likelihood order. Need empirical
investigation to confirm which dominates.

1. **The 100 ms poll is the floor.** Worst-case latency: 99 ms before
   the recv-loop sees the flag. Average: ~50 ms. Together with
   `stream_handle.abort()` propagation through `tokio::JoinHandle` and
   `reqwest`'s body-stream drop, observed end-to-end can land closer to
   200-300 ms depending on TLS state. Sub-100 ms requires either:
   - Replacing `Option<Arc<AtomicBool>>` with `tokio::sync::watch::Receiver<bool>`
     (event-driven, no polling).
   - Or threading `tokio_util::sync::CancellationToken` through
     `chat_stream` itself so the underlying provider HTTP request drops
     when the token is cancelled (most invasive but architecturally
     correct).

2. **Final batch-writer flush after stop.** `BatchWriter` accumulates
   messages and flushes on a debounce. After `Err(Stopped)` returns,
   the trailing `stop_execution` runs immediately, but pending writes
   in the batch writer's queue (assistant message fragments, tool
   results from the in-flight turn) keep arriving for a beat. The UI
   may see 1-2 trailing token events between click and final
   `AgentStopped`. Look at `gateway-execution/src/invoke/batch_writer.rs`.

3. **Continuation re-entry on delegated sessions.** Research often has
   root paused for a planner delegation. When user clicks Stop:
   - Cascade signals planner → planner aborts.
   - Planner's `spawn.rs` Stopped arm runs (just logs).
   - `state_service.complete_session(child_session_id)` fires at
     spawn.rs:789 on every return path including Stopped — this might
     trigger the continuation watcher to re-enter root.
   - Root's continuation runs through `core.rs:1265` with stop already
     set; recv-loop polls and aborts within 100 ms, but the round-trip
     adds latency to the user-visible "research is now stopped" moment.
   - Possible fix: have `complete_session` skip the
     continuation-trigger when the child's exit was a cooperative stop.
     Requires a "stop reason" on the session record; not present today.

4. **UI event-routing latency.** The `AgentStopped` event published in
   `lifecycle.rs::stop_execution` flows through `EventBus` →
   `WebSocketHandler::run_event_router` → SubscriptionManager →
   client. Several hops, each with its own (small) overhead. Not
   blocking, but a noticeable hop count when wall-clock matters.
   Verify: how long between `stop_execution` running on the daemon and
   the UI's `state.pillState` flipping to `idle`?

5. **Tokens already in the receive channel buffer.** The mpsc channel
   between the spawned LLM task and the recv loop is unbounded. When
   `stream_handle.abort()` fires, in-flight chunks already in the
   channel are still drained by the loop's `chunk = rx.recv()` arm
   *until* the next 100 ms tick or the channel closes. Could observe
   a small number of trailing tokens streamed to the UI after click.
   Cheap fix: on stop_observed, drop `rx` immediately to close the
   channel.

## Investigation steps (when picked up)

1. **Quantify the latency.** Add a `tracing::info!` with
   `Instant::now()` at three points:
   - WS handler receives `Stop` (`gateway/src/websocket/handler.rs:481`)
   - Recv loop observes `stop_flag` (`runtime/agent-runtime/src/executor.rs` post-PR99 location)
   - `stop_execution` returns (`gateway-execution/src/lifecycle.rs:469`)
   Measure deltas across 10 stops on `/research`. Determine which
   segment dominates.

2. **Network-trace the LLM request.** Watch the HTTP/2 stream to the
   provider after click — does it really get dropped within 100 ms,
   or does `reqwest` keep reading the body for longer? Use
   `tcpdump`/`wireshark` or the daemon's existing tracing. If the
   upstream request lingers, the agent's local state was cancelled
   but tokens kept arriving from the LLM.

3. **Audit the batch writer.** Confirm the write queue drains before
   the `AgentStopped` event fires — or, if not, decide whether to
   force-flush on stop. Look at `BatchWriterHandle::shutdown` if it
   exists, or add one.

4. **Compare against quick-chat trace.** Run the same instrumentation
   on a quick-chat stop where the user reported "seamless." The
   diff should localize the research-specific delay.

## Mitigation ideas

In rough order of effort vs. impact:

- **A. Drop `rx` on `stop_observed`** (5 lines). Eliminates trailing
  tokens reaching the UI from the in-flight batch. Cheap test.

- **B. Replace `AtomicBool` poll with `watch::Receiver<bool>`**
  (~30 lines). Eliminates the 100 ms floor; recv loop wakes on the
  first changed signal. Touches `ExecutionHandle` API; some callers
  may need updating.

- **C. CancellationToken through `chat_stream`** (~80-150 lines).
  Architecturally correct; the upstream HTTP/2 stream is dropped by
  the provider's `reqwest` builder when the token cancels. Saves
  provider tokens on long stops. Touches every `LlmClient` impl in
  `runtime/agent-runtime/src/llm/`.

- **D. "Stop reason" on session** (~20 lines + schema migration). Lets
  the continuation watcher distinguish "child stopped cooperatively"
  from "child completed naturally" so root doesn't get re-entered for
  no reason. Useful regardless of stop UX.

## Acceptance criteria (when fixed)

- Click Stop on `/research` → UI reflects stopped state within ~100 ms
  visible-perception window (aspirational target: <50 ms).
- No trailing tokens stream after click.
- Behaviour matches quick-chat stop subjectively.

## Notes

- This is **not** a regression — pre-PR #99, research stop didn't work
  at all (waited 5-30 s). PR #99 took it from broken to "kind of works
  but not seamless." Calling that a win.
- Treat as latency / polish work, not correctness. Functional path is
  sound; the cooperative-stop architecture is the right one for the
  cooperative agent loop. CancellationToken (option C) would be the
  architectural finish.
