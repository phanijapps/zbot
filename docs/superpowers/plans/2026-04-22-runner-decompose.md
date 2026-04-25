# `runner.rs` God-Class Decomposition Plan

> **For agentic workers:** REQUIRED SUB-SKILL: use superpowers:subagent-driven-development to execute this plan task-by-task. Each phase has an explicit compile / test / clippy checkpoint that MUST pass before moving on. If any checkpoint fails and can't be recovered inside the same phase, stop and surface — do not paper over.

**Branch:** `feature/sonar-runner-decompose` — disposable. If Phase 5 (E2E) fails and we can't isolate the regression, the whole branch gets thrown away.

**Goal:** Take `gateway/gateway-execution/src/runner.rs` (2,761 lines, ~6% coverage, 6 `#[allow(clippy::too_many_arguments)]` confessions, 5 methods > 100 LOC) and decompose it into focused units without changing runtime behavior bit-for-bit.

**Correctness gate:** E2E. Unit tests at 6% coverage will not catch a hot-path regression here. Runner is the main execution loop — every session flows through it, so we validate with `e2e/scripts/boot-full-mode.sh simple-qa` (real daemon + scripted LLM) before merging.

---

## Why now

1. **Biggest footgun-per-line in the repo.** 1,945 lines in one `impl ExecutionRunner` block + a 566-line free function. A grumpy-psycho-neighbor adding code drops it into the nearest mega-method because that's where everything already lives. Mass just grows.
2. **Six `#[allow(clippy::too_many_arguments)]` confessions in one file** — the most concentrated anti-pattern cluster in the workspace (lifecycle.rs was second at 4, already fixed).
3. **Low coverage (6%)** isn't fixable by testing — the methods are too large and too intertwined. Simplification first, coverage as a byproduct.
4. **Every recent Sonar report cites runner.rs** as the top complexity + duplication hotspot in `gateway-execution`.

## Why this is risky (and how we manage it)

- **Hot path.** Every agent execution goes through this file. A subtle behavior change hurts every session.
- **Mitigation 1:** each extraction is behaviour-preserving. The function body after extraction should look structurally identical to before — we're pulling blocks out and calling them, not rewriting the logic.
- **Mitigation 2:** cargo check + cargo test + cargo clippy with `-D warnings` run after every single extraction. No commit without all three green.
- **Mitigation 3:** E2E gate at the end — Mode Full because Mode UI doesn't exercise the runner.
- **Mitigation 4:** disposable branch. If E2E fails and we can't isolate the regression, `git branch -D feature/sonar-runner-decompose` and we're back where we started.

---

## Map of the damage

### Functions with `#[allow(clippy::too_many_arguments)]`

| Line | Function | Size | Fix type |
|---|---|---|---|
| 126 | `ExecutionRunner::new` | ~35 LOC | `ExecutionRunnerConfig` (builder-pattern already exists via `with_connector_registry` — align) |
| 160 | `ExecutionRunner::with_connector_registry` | ~65 LOC | Same as `new` — share config struct |
| 328 | `spawn_with_notification` (nested inside `spawn_delegation_handler`) | small | `SpawnNotification` context struct |
| 884 | `ExecutionRunner::spawn_execution_task` | ~425 LOC | `ExecutionTaskContext` struct + handler extraction (see Phase 2) |
| 1626 | `ExecutionRunner::create_executor` | ~407 LOC | `ExecutorBuildCtx` + extract per-phase helpers |
| 2066 | `invoke_continuation` (free fn) | ~566 LOC | `ContinuationContext` + extract per-branch helpers |

### Mega-methods (>100 LOC)

| Method | Size | Kind |
|---|---|---|
| `invoke_continuation` (free fn) | 566 | LLM prompt build + retry loop + state mutation — can be split by phase |
| `create_executor` | 407 | Sequential build pipeline — ideal for per-phase helpers |
| `spawn_execution_task` (ExecutionRunner method, not the delegation one) | 425 | Main executor loop — mega-match on stream events |
| `spawn_delegation_handler` | 244 | Spawns the delegation-request consumer task |
| `invoke_with_callback` | 229 | Top-level invoke entrypoint |
| `spawn_continuation_handler` | 111 | Spawns the continuation-request consumer task |
| `resume_crashed_subagent` | 106 | Crashed-subagent recovery |

Four of the five longest functions live in `impl ExecutionRunner`. The fifth (`invoke_continuation`) is the free function at the bottom.

---

## Phase plan

Each phase has explicit entry checklist, exit criteria, and a rollback strategy. Phases are ordered by **risk ascending**: start with the cleanest wins, finish with the scariest.

### Phase 1 — Context structs for the 6 `#[allow]` sites

**Risk:** Low. Same pattern landed twice already (`spawn.rs`, `lifecycle.rs`). Mechanical.

**Steps:**
1. `ExecutionRunner::new` + `with_connector_registry` share 90% of their fields. Extract an `ExecutionRunnerConfig` struct with all constructor params; both constructors take the struct.
2. `spawn_with_notification` (nested fn): extract `SpawnNotificationCtx` struct.
3. `spawn_execution_task` (impl method): extract `ExecutionTaskCtx` struct.
4. `create_executor`: extract `ExecutorBuildCtx` struct.
5. `invoke_continuation` (free fn): extract `ContinuationCtx` struct.

**Exit criteria:**
- [ ] Zero `#[allow(clippy::too_many_arguments)]` in runner.rs
- [ ] `cargo check --workspace` green
- [ ] `cargo test --workspace` green
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` green
- [ ] One commit per context struct (so any individual one can be reverted)

**Rollback:** `git reset --hard HEAD~N` by commit, each struct is independent.

### Phase 2 — Decompose `spawn_execution_task` stream-event loop

**Risk:** Medium. Hot-path loop. Behaviour-preserving but easy to miss a fallthrough.

**What's inside (expected):** A `tokio::select!` or loop with a `match` on `StreamEvent` variants. Each arm touches multiple pieces of state. Same shape as the `events.rs::convert_stream_event` function we already tamed — but with side effects instead of pure mapping.

**Approach:**
1. Read the function fully; identify every `match event {...}` and `if let ... event ...` block.
2. For each event handled, extract a `handle_X_event(ctx: &mut HandlerCtx, event: X) -> Option<...>` helper.
3. Dispatcher becomes flat: one-line call per arm.
4. All side effects (batch-writer sends, metric updates, delegation registrations) stay in the handlers.

**Exit criteria:**
- [ ] `spawn_execution_task` body is < 150 LOC (was 425)
- [ ] All unit tests still green
- [ ] No new clippy warnings
- [ ] Commit with explicit statement of which handlers were extracted

**Rollback:** Revert the phase commit; cargo check must still pass on main.

### Phase 3 — Decompose `create_executor` into pipeline helpers

**Risk:** Medium. Sequential setup, but depends on ordering (model registry, rate limiter, middleware pipeline, MCP tools).

**Approach:** Read the function top-to-bottom; identify the implicit phases ("load agent and provider", "inject skills", "build middleware pipeline", "attach MCP tools", "wire rate limiter", "intent analysis", "return executor"). Each becomes a private helper on `ExecutionRunner` that takes `&mut ExecutorBuildCtx`.

**Exit criteria:**
- [ ] `create_executor` body is < 150 LOC (was 407)
- [ ] All unit tests still green

**Rollback:** Same as Phase 2.

### Phase 4 — Decompose `invoke_continuation`

**Risk:** High. 566 lines, many ownership transfers, complex retry logic. The most dangerous single change on this branch.

**Approach:**
1. Identify discrete phases: "build continuation prompt from subagent outputs", "call LLM", "parse response", "apply side effects". Extract each.
2. For the retry loop (if present): extract as a separate helper.
3. For the system-message construction from callback results: already partially extracted in `delegation/callback.rs`? Check before duplicating.

**Exit criteria:**
- [ ] `invoke_continuation` body is < 200 LOC
- [ ] All unit tests still green
- [ ] Visual inspection: extracted helpers have clear single responsibilities

**Rollback:** Phase 4 is its own commit series. If E2E fails in Phase 5, first suspect is Phase 4 — revert Phase 4 commits and re-run E2E to confirm.

### Phase 5 — E2E GATE

**This is the real correctness check.** Unit tests do not exercise the full runner loop — only E2E Mode Full does.

**Steps:**
1. Re-enable the e2e workflow (`workflow_dispatch` → trigger manually from GitHub UI) OR run locally.
2. Local run:
   ```
   cd /home/videogamer/projects/agentzero/e2e
   ./scripts/boot-full-mode.sh simple-qa &
   cd playwright
   npx playwright test full-mode/
   ```
3. Inspect `e2e/mock_llm` drift report: `{run_dir}/mock-llm-drift.json`. Non-zero drift = behaviour change in LLM call shape.
4. Inspect the session row in the test daemon's SQLite: status `completed`, correct root execution id, no orphaned pending_delegations.

**Exit criteria:**
- [ ] Mode Full simple-qa scenario completes with status `completed`
- [ ] Mock-LLM drift report is empty (zero unexpected calls)
- [ ] Playwright assertions all pass
- [ ] No new warnings in daemon stderr

**If Phase 5 fails:**
- Bisect: revert Phase 4 commits and re-run. If green, the regression is in Phase 4. Drill into which commit.
- If Phase 4 bisect is clean, revert Phase 3, and so on.
- If no single phase fixes it: throw the branch away (`git branch -D feature/sonar-runner-decompose`, `git push origin :feature/sonar-runner-decompose`).

### Phase 6 — Cleanup + PR

**Steps:**
1. Unused imports: `cargo clippy --fix --workspace --all-targets` cleanup pass.
2. Visibility tightening: private functions that are no longer used outside the module should lose their `pub`.
3. Add the 2026-04-22 date to this plan doc under "shipped on" if merged.
4. PR description: include the Phase 5 E2E report output verbatim.

---

## Non-goals

- **Coverage targets.** This branch is about shape, not tests. Coverage improvements are a follow-up.
- **Moving code out of runner.rs into new modules.** File shape stays the same — just smaller, flatter methods inside. A module split is a future PR against a stable runner.rs.
- **Changing public API.** `ExecutionRunner::new`, `invoke`, `invoke_with_callback`, `continue_execution`, `pause`, `resume`, etc. all keep their signatures (or, for `new`/`with_connector_registry`, gain a `Config` struct overload but the positional variant is retained if easy).

---

## Approval gate

Before executing Phase 1, the human owner must say "go plan" or equivalent. This is not blind work — every phase has a decision point.

**Planner sign-off checklist (I have checked):**
- [x] Identified every `#[allow(clippy::too_many_arguments)]` site in runner.rs
- [x] Identified every method > 100 LOC
- [x] Phases ordered by risk ascending
- [x] E2E gate named as the real correctness check
- [x] Rollback strategy per phase + branch-level
- [x] Explicit non-goals to prevent scope creep
