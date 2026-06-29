# Plan: Recall-Log Wiring

- **Spec:** [`spec.md`](spec.md)
- **Status:** Executed (T1–T4 done, gates green; other 3 call sites deferred)

## Approach
Consumer-side logging (no `recall_unified` signature change). `MemoryRecall` gains an optional `RecallLogStore` + a best-effort `log_recalled(session_id, items)` method. The composition root builds `RecallLogRepository` from the conversations `db_manager`, wraps it in `GatewayRecallLogStore`, and sets it on `MemoryRecall`. `invoke_bootstrap` calls `log_recalled` after each bootstrap `recall_unified`.

## Tasks
### T1: MemoryRecall field + setter + log_recalled method + tests
**Depends:** none — add `recall_log_store: Option<Arc<dyn RecallLogStore>>`; `set_recall_log_store`; `log_recalled` (iterate, best-effort `store.log_recall`). Capturing-mock test + no-op test. Done when: gateway-memory tests green.
### T2: invoke_bootstrap call site
**Depends:** T1 — `recall.log_recalled(&session_id, &items).await` after the bootstrap recall match arm (`session_id` from `PartialSetup`). Done when: gateway-execution tests green (468).
### T3: Composition root wiring
**Depends:** T1 — `state/mod.rs`: `RecallLogRepository::new(db_manager.clone())` → `GatewayRecallLogStore` → `recall.set_recall_log_store(...)`, before `memory_recall_inner` moves into `Arc`. Done when: `cargo check --workspace` clean.
### T4: Gates
**Done when:** workspace check clean; gateway-memory + gateway-execution tests green; no new clippy warnings (2 pre-existing warnings in unrelated `http/` files are not this slice's).

## Deferred
- `log_recalled` at the 3 non-bootstrap recall sites (continuation, intent_analysis, delegation) — bootstrap is the primary path; others when prioritized.

## Risks
- Logging latency on the recall path: mitigated by best-effort (warn-and-continue); `INSERT OR IGNORE` is cheap.
- `session_id` availability at other call sites if extended later — verify per site.

## Changelog
- 2026-06-29: initial plan (light mode); T1–T4 executed same day.
