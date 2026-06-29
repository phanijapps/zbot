# Spec: Recall-Log Wiring

- **Status:** Implementing
- **Plan:** [`plan.md`](plan.md)
- **Shape:** service
- **Mode:** light (no new module/layer/dependency — wires an existing store into an existing path)

## Objective

Wire the existing `RecallLogStore` into the recall path so the system records **which facts it surfaced per session**. Today `recall_log` has 0 rows: `GatewayRecallLogStore` + `RecallLogRepository` exist and are exported, but nothing constructs them or calls `log_recall` — recall is unobservable ("you can't tell what facts the system relies on"). This is the observability precondition for diagnosing "hit and miss" recall and the basis for future predictive recall.

**Root cause (verified):** `MemoryRecall` holds no `RecallLogStore` field; the no-op trait default (`auxiliary.rs:58`) was the only path. The composition root never built `RecallLogRepository`/`GatewayRecallLogStore`.

## Boundaries

### Always do
- Best-effort logging only: `log_recalled` warns on per-item errors, never propagates — logging must not break recall.
- No-op when no store wired (tests/headless).
### Ask first
- Logging at the other 3 recall call sites (`runner/core.rs` continuation, `intent_analysis`, `delegation/spawn`) — deferred; bootstrap is the primary path.
### Never do
- Block recall on a logging failure; change `recall_unified`'s signature (logging is consumer-side).

## Testing Strategy
**TDD** — capturing-mock `RecallLogStore` unit test (asserts each recalled item.id is logged with the session id) + a no-op-when-unwired test. Cross-crate compile verified via `cargo check --workspace`.

## Acceptance Criteria
- [x] `MemoryRecall` holds an optional `RecallLogStore`, set via `set_recall_log_store`; `log_recalled(session_id, items)` logs each item id (best-effort).
- [x] Composition root (`gateway/src/state/mod.rs`) builds `RecallLogRepository` from `db_manager` (conversations.db), wraps in `GatewayRecallLogStore`, sets it on `MemoryRecall`.
- [x] `invoke_bootstrap` calls `recall.log_recalled(&session_id, &items)` after `recall_unified`.
- [x] `cargo test -p gateway-memory` green (new tests pass); `cargo test -p gateway-execution` green (468); `cargo check --workspace` clean; no new clippy warnings.

## Assumptions
- Technical: `db_manager` (conversations `DatabaseManager`) is in scope where `MemoryRecall` is built — verified (`ConversationRepository::new(db_manager.clone())` adjacent).
- Technical: `ScoredItem.id` is the durable fact/entity id to log (recall_log fact_key).
- Process: light-mode work-loop.
