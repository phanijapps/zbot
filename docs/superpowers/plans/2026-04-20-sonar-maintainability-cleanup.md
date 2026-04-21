# SonarQube Maintainability & Duplication Cleanup Plan

> **For agentic workers:** This is a strategic roadmap, not a single-PR implementation plan. Each phase should be executed on its own branch (or sequence of branches) off `feature/sonar` (or `main` once merged), with its own PR. Within a phase, prefer subagent-driven-development where steps are mechanical and well-specified.

**Goal:** Drive the SonarCloud Quality Gate from ERROR → PASSING by reducing maintainability issues from 151 → ~55 and duplicated-lines density from 4.2% → below the 3% new-code threshold.

**Architecture:** Phased, value-ordered sweep. Phase 0 unblocks CI (flaky test) so subsequent coverage numbers are meaningful. Phases 1-2 are mechanical sweeps that remove the tail fast. Phases 3-5 are the structural work — cross-crate defork, HTML partials, per-entity CRUD extraction, test-fixture crate. Phase 6 is opportunistic.

**Tech Stack:** Rust 2024 workspace; TypeScript 5.8 / React 19 / Vitest; Python 3.12 skills; Handlebars (or equivalent) for HTML report templates.

---

## Baseline (2026-04-20)

**Quality Gate:** ERROR. Failing conditions:
- `new_coverage = 0%` (expected threshold 80%) — unblocked by the CI fix landed in the previous PR; will fall out once that PR merges to `main` and the workflow runs.
- `new_duplicated_lines_density = 6.3%` (threshold 3%) — addressed by this plan.
- `new_security_hotspots_reviewed = 0%` — 7 hotspots need triage; out of scope here.
- `new_reliability_rating = 3` — 9 open bugs; out of scope here.

**Project totals:**
| Metric | Value |
|---|---|
| Lines of code | 133,723 |
| Code smells | 651 |
| Cognitive complexity (total) | 10,111 |
| Duplicated lines | 7,322 (4.2%) |
| Duplicated blocks | 431 across 88 files |
| Technical debt | 3,500 min (~58 hr) |

**Open HIGH/BLOCKER maintainability issues:** 151, distributed as:
| Bucket | Rules | Count |
|---|---|---|
| Cognitive complexity | `S3776` (rust/ts/py) | 102 (67%) |
| HTML template lint — `.dataset` | `javascript:S7761` | 33 (22%) |
| Wildcard imports | `rust:S2208` | 7 |
| `void` operator | `typescript:S3735` | 5 |
| Nesting depth >4 | `typescript:S2004` | 4 |

**Duplication clusters (64% of 7,322 dup lines concentrated here):**
| # | Cluster | Dup lines |
|---|---|---|
| 1 | Cross-crate fork: `framework/zero-middleware` ↔ `runtime/agent-runtime/src/middleware`; `framework/zero-llm/openai` ↔ `runtime/agent-runtime/src/llm/openai` | ~460 |
| 2 | HTML report templates (6 files in `gateway/templates/skills/html-report/`) | ~1,430 |
| 3 | `services/knowledge-graph/src/storage.rs` (697 dup lines, 24.7% density) | ~700 |
| 4 | MCP transports + `runner.rs` + `schema.rs` + `file.rs` | ~1,160 |
| 5 | Test-harness boilerplate across `gateway/tests/*`, `gateway-execution/tests/*`, `knowledge-graph/tests/*` | ~900 |

---

## Phase 0 · Unblock CI (flaky health-loop tests)

**Status:** Completed in this branch (commit TBD on `feature/sonar`).

**Problem:** `embedding_service::tests::health_loop_marks_ollama_unreachable_when_down` and `health_loop_pings_ollama_periodically_when_active` used fixed `tokio::time::sleep(300ms/400ms)` after starting a 50-ms-interval health loop. On self-hosted CI under load, the loop didn't complete enough ticks before the sleep expired, leaving stale health states and aborting the test run before Sonar could upload coverage.

**Fix:** Introduced `wait_until(deadline, check)` helper in the test module — polls every 20ms, returns true as soon as the condition holds, bails at deadline. Replaced both fixed sleeps with `wait_until(3s, ...)`. Tests still complete in <300ms on fast machines; they just no longer flake on slow ones.

**Why listed here:** until this passes, `cargo llvm-cov --workspace` exits non-zero in CI, the LCOV file isn't uploaded, and new-code coverage reports as 0%. That makes every subsequent phase's coverage metrics meaningless.

**Verification:** 25/25 embedding_service tests pass locally; `cargo fmt --all`, `cargo clippy -p gateway-services` clean.

---

## Phase 1 · Mechanical sweep (1 day, low risk)

**Goal:** Close the low-hanging tail — 49 issues that are structurally identical and need no architectural judgment.

### Task 1a · `.dataset` migration in HTML report templates (33 issues)

**Files:**
- `gateway/templates/skills/html-report/cri-template.html`
- `gateway/templates/skills/html-report/pnl-template.html`
- `gateway/templates/skills/html-report/portfolio-template.html`
- `gateway/templates/skills/html-report/risk-reversal-template.html`
- `gateway/templates/skills/html-report/stress-test-template.html`
- `gateway/templates/skills/html-report/template.html`
- `gateway/templates/skills/html-report/trade-specification-template.html`

**Rule:** `javascript:S7761` — Prefer `.dataset.x` over `getAttribute("data-x")`.

**Steps:**
- [ ] Grep for `getAttribute\("data-` in `gateway/templates/skills/html-report/`
- [ ] For each hit, replace `el.getAttribute("data-foo-bar")` → `el.dataset.fooBar`; setters likewise (`setAttribute("data-x", v)` → `el.dataset.x = v`)
- [ ] Render each template with a sample data payload and visually diff the output (open in browser)
- [ ] Commit one file at a time for easy bisect

### Task 1b · Wildcard import expansion (7 issues)

**Rule:** `rust:S2208`.

**Steps:**
- [ ] `cargo clippy --all-targets -- -D warnings 2>&1 | grep S2208` to list exact sites
- [ ] For each `use foo::*`, run `cargo expand` or inspect local usage, replace with explicit imports
- [ ] `cargo check --workspace` after each file

### Task 1c · `void` operator cleanup (5 issues)

**Rule:** `typescript:S3735`. Usually `void 0` (→ `undefined`) or `void expr` (→ drop the return value another way).

**Steps:**
- [ ] Find: `grep -rn "void " apps/ui/src/ --include='*.ts*'`
- [ ] Replace `void 0` with `undefined`; for `void promise` use a named `.catch()` or `.then(() => {})` block that documents intent
- [ ] `npm run build` + `npm test` after

### Task 1d · Nesting depth flattening (4 issues)

**Rule:** `typescript:S2004` — >4 levels of nested functions.

**Steps:**
- [ ] Fetch exact sites from SonarQube via `search_sonar_issues_in_projects` filter by rule
- [ ] For each, extract the innermost function to module scope (or nearest useful scope) — same pattern as `typescript-complexity.md` rules
- [ ] Add regression test if the closure captures mutable state

**Phase 1 exit criteria:** all 4 sub-tasks merged; SonarQube re-scan shows −49 HIGH/BLOCKER issues; no new issues introduced.

---

## Phase 2 · Collapse cross-crate forks (2-3 days, medium risk)

**Goal:** Remove ~600 duplicated lines that exist because historic refactors forked modules into `framework/` but never removed the `runtime/` originals.

### Task 2a · `zero-middleware` ↔ `agent-runtime/middleware` defork

**Files:**
- Canonical: `framework/zero-middleware/src/{config.rs, traits.rs}` (188 + 31 dup lines)
- Duplicate: `runtime/agent-runtime/src/middleware/{config.rs, traits.rs}` (194 + 31 dup lines)

**Steps:**
- [ ] `git log --oneline framework/zero-middleware/src/` and same for the runtime path — confirm which is newer / more used
- [ ] Diff the two pairs: `diff framework/zero-middleware/src/config.rs runtime/agent-runtime/src/middleware/config.rs` — enumerate real drift (not just re-ordered imports)
- [ ] If drift is cosmetic: replace the duplicate with `pub use framework_middleware::*;` (or equivalent re-export path) and delete the duplicate source
- [ ] If drift is semantic: port the unique logic to the canonical crate, then re-export
- [ ] `cargo check --workspace && cargo test --workspace` to catch breakage
- [ ] One commit per pair (config.rs then traits.rs)

### Task 2b · `zero-llm/openai` ↔ `agent-runtime/llm/openai` defork

**Files:**
- `framework/zero-llm/src/openai.rs` (53 dup, 9.8%)
- `runtime/agent-runtime/src/llm/openai.rs` (47 dup, 6.1%)

**Steps:**
- [ ] Same diff + canonical-selection process as 2a
- [ ] Extra check: any remaining `non_streaming.rs` (18 dup, 23.4%) should be folded into the canonical crate's `openai.rs` or moved alongside

### Task 2c · MCP transport abstraction — `http.rs` ↔ `sse.rs` (82% dup each, 132 lines)

**Files:**
- `runtime/agent-runtime/src/mcp/http.rs`
- `runtime/agent-runtime/src/mcp/sse.rs`
- Possibly `runtime/agent-runtime/src/mcp/stdio.rs` (226 dup, 55%)

**Strategy:** The duplication is the MCP JSON-RPC framing/dispatch. Extract to `mcp/transport.rs` as:

```rust
#[async_trait]
pub(crate) trait McpTransport: Send + Sync {
    async fn send(&self, msg: &McpMessage) -> Result<(), McpError>;
    async fn recv(&self) -> Result<McpMessage, McpError>;
}

pub(crate) async fn dispatch_loop<T: McpTransport>(
    t: &T,
    state: &mut McpState,
) -> Result<(), McpError> { /* extracted boilerplate */ }
```

**Steps:**
- [ ] Write the trait + extract the shared `dispatch_loop` (TDD: port one existing integration test first)
- [ ] Migrate `http.rs` to use the trait — run MCP integration tests
- [ ] Migrate `sse.rs` — run MCP integration tests
- [ ] Migrate `stdio.rs` — run MCP integration tests
- [ ] Final clippy + fmt

**Phase 2 exit criteria:** dup density on these 5-6 files drops below 5%; no behavior regressions in MCP/middleware integration tests.

---

## Phase 3 · HTML report templates — **re-scoped to config-only exclusion**

**Original goal:** Factor out ~1,400 duplicated lines across 7 `html-report/` templates into `_base.html` + `_table_section.html` partials.

**Why the original plan was wrong:** These templates are NOT server-side rendered pages — they are **self-contained HTML files distributed to end-users via the `html-report` skill**. `SKILL.md` documents the usage contract explicitly:

> "You only write the `<body>` content — no `<head>` needed! ... `template = read("/.pi/skills/html-report/template.html"); html = template.replace("{{BODY}}", body_content); write("reports/...html", html)`"

Each template is a one-file deliverable the skill invoker reads, substitutes `{{VAR}}` placeholders in, and writes to `reports/`. Breaking that into partials would:

1. Break the documented one-file-read UX
2. Require a build/assembly step users don't currently have
3. Couple the skill surface to an internal template engine — reversing the self-contained contract

The ~1,400 duplicated lines are **intentional**. Duplication-detection metric counts them as a maintenance bug, but they're a feature of the skill.

**Decision (2026-04-21):** Exclude `gateway/templates/**` from Sonar via `sonar.exclusions`. The entire `gateway/templates/` tree is distributed skill content (HTML reports, agent prompts, skill scripts, default configs) — none of it is first-party production code that should be graded against maintainability/duplication rules.

**Change applied:** `sonar-project.properties` line updated:
```
sonar.exclusions=...,gateway/templates/**
```

**Expected Sonar impact:**
- Duplicated-lines density drops by ~1,400 lines (the report-template cluster) plus any smaller duplication elsewhere under `gateway/templates/` → expect dup density <3%, the new-code quality gate threshold
- The 33 `javascript:S7761` issues Phase 1a fixed stay fixed in source even though the scanner now skips them — still good hygiene
- Analysis surface shrinks so coverage and complexity metrics become more meaningful (less noise from distributed content)

**If later we want structural deduplication (optional):** source-of-truth partials in `gateway/templates/skills/html-report/_partials/`, a Python build script that concatenates partials into the 7 distributed files, and a CI check verifying the generated output matches the committed files. User-facing skill contract preserved. That's a separate mini-project, not required for the quality gate.

**Phase 3 exit criteria (revised):** Sonar exclusion configured; plan doc records the decision; no source changes to the templates themselves.

---

## Phase 4 · Decompose the 5 "hot" Rust files (1 week, medium risk)

**Goal:** Attack the files where cognitive complexity AND duplication co-occur. ~900 dup lines + ~28 S3776 violations removed.

### Task 4a · `services/knowledge-graph/src/storage.rs` (697 dup, 6 S3776 hits)

**Hypothesis:** per-entity CRUD blocks (insert/update/delete/select for each of Entity, Event, Relation, Observation...) are pasted with minor parameter swaps.

**Steps:**
- [ ] Enumerate the duplicated blocks via `get_duplications` for this file
- [ ] Identify the common shape (likely: `fn insert_X(&self, x: X) -> Result<...>` + `fn fetch_X_by_id(&self, id: Uuid) -> Result<Option<X>>` + `fn delete_X(&self, id: Uuid)` per entity)
- [ ] Choose extraction style:
  - `EntityRepo<T>` trait with default methods, **or**
  - Declarative macro (`impl_entity_repo!(Entity, entities);`)
- [ ] Port one entity first as proof-of-concept; verify all existing tests green
- [ ] Port remaining entities; each in its own commit
- [ ] Expected reduction: 600+ dup lines, all 6 S3776 hits on this file

### Task 4b · `gateway/gateway-execution/src/runner.rs` (394 dup, 5 S3776 hits)

**Hypothesis:** mega-match on an event/command enum with per-variant logic inlined.

**Steps:**
- [ ] Identify the match — probably `match event { TokenEvent::X => {...}, ... }`
- [ ] Extract each arm to a named `handle_X(ctx: &mut Ctx, event: X)` in a sibling module
- [ ] The dispatcher becomes a flat `match event.kind() { ... => handle_foo(ctx, event) }` — same pattern as `.claude/rules/typescript-complexity.md` prescribes for TS switch/case
- [ ] Run `cargo test --package gateway-execution` — integration tests cover runner end-to-end

### Task 4c · `runtime/agent-runtime/src/executor.rs` (6 S3776 hits)

**Steps:**
- [ ] Same decomposition pattern as 4b
- [ ] Reference: the `resolve_thinking_flag` helper extracted on `enhancements` branch is the template for small-helper-per-concern

### Task 4d · `gateway/gateway-execution/src/distillation.rs` (6 S3776 hits)

**Steps:**
- [ ] Break `summarize_*` / `distill_*` functions into per-step private helpers
- [ ] Each helper should take the same `DistillCtx` struct (analogous to the TS `EventHandlerCtx` pattern in `.claude/rules/typescript-complexity.md`)

### Task 4e · `runtime/agent-tools/src/tools/execution/apply_patch.rs` (5 S3776 hits)

**Steps:**
- [ ] Split into three phases: `parse_patch(text) -> Patch`, `apply_patch(patch, fs) -> Result<Diff>`, `verify_diff(diff) -> Result<()>`
- [ ] Each phase becomes its own module under `apply_patch/`
- [ ] Existing integration tests stay; add unit tests for the parser alone
- [ ] Also targets `memory/MEMORY.md` notes on apply_patch mistakes — clearer code = fewer future mistakes

**Phase 4 exit criteria:** each hot file below the 15-cognitive-complexity threshold for every function; duplication density on storage.rs and runner.rs below 10%; all existing tests still green.

---

## Phase 5 · Shared test-fixture crate (2 days, low risk)

**Goal:** Eliminate ~700 duplicated test-setup lines across 8 test files and make future test authoring cheaper.

**Affected test files:**
- `gateway/tests/api_tests.rs` (187 dup, 22.2%)
- `gateway/tests/memory_unified_search.rs` (162 dup, 69.5%)
- `gateway/tests/ward_content_endpoint.rs` (121 dup, 50.2%)
- `gateway/gateway-execution/tests/{cold_boot,e2e_ward_pipeline_tests,session_state_tests}.rs`
- `gateway/gateway-services/tests/skill_ward_setup_tests.rs`
- `services/knowledge-graph/tests/resolver_scale.rs`

**Steps:**
- [ ] Create `gateway-test-fixtures/` crate, `[package.edition = "2024"]`, `[dev-dependencies]`-only
- [ ] Identify the shared setup patterns (TempDir → VaultPaths → DB → seed agents, skills, wards → return handle)
- [ ] Provide fixture builders:
  - `Fixture::new() -> Fixture` — temp dir + default paths
  - `Fixture::with_db() -> Fixture`
  - `Fixture::with_ward(name: &str) -> Fixture`
  - `Fixture::with_agent(agent_spec: AgentFixture)` etc.
- [ ] Migrate one test file to use the crate; run it
- [ ] Migrate remaining test files
- [ ] Expected: 700+ dup lines gone, 50+ fewer lines per new test file

**Phase 5 exit criteria:** no test file in the listed set exceeds 5% duplication density; `cargo test --workspace` green.

---

## Phase 6 · Long tail (opportunistic)

**Goal:** Remaining ~2,000 dup lines across ~40 files each under 50 lines. Do not batch-fix — pick up opportunistically when touching those files.

**Tracking:** every PR that modifies any file in Sonar's duplicated-files list should address its own duplication as part of the change.

---

## Expected impact after Phases 0-5

| Metric | Baseline | After | Delta |
|---|---|---|---|
| Maintainability issues (HIGH+) | 151 | ~55 | −96 |
| Duplicated lines | 7,322 | ~3,900 | −3,400 |
| Duplicated-lines density | 4.2% | ~2.2% | ✅ under 3% new-code gate |
| Duplicated files | 88 | ~40 | −48 |
| CI gate on new code | ERROR | OK (pending coverage land) | ✅ |

Note: Phase 3 counts ~1,400 of the −3,400 duplicated-lines delta as a Sonar-exclusion win (they stop being *measured*), not a source-level extraction. The remaining ~2,000-line reduction still comes from real code changes in Phases 2, 4, and 5.

---

## Execution cadence

Each phase = its own PR off `feature/sonar` (or off `main` once `feature/sonar` lands). Within a phase, prefer subagent-driven-development for mechanical tasks (Phases 1, 3) and write-plans-then-execute for architectural tasks (Phases 2, 4, 5).

**Suggested order by urgency:**
1. Phase 0 (done) — unblocks CI
2. Phase 1 — fast wins, demonstrable reduction, easy review
3. Phase 3 — biggest single duplication win
4. Phase 5 — unlocks cheaper test authoring going forward
5. Phase 2 — architectural clarity payoff
6. Phase 4 — largest effort, do last when momentum is high

---

## Out of scope (referenced for completeness)

- 9 open reliability bugs (`new_reliability_rating = 3`) — requires separate bug-triage pass
- 7 security hotspots (`new_security_hotspots_reviewed = 0%`) — triage via `change_security_hotspot_status` MCP tool, not code changes
- Coverage gap (`new_coverage = 0%`) — resolved by the CI fix on `enhancements` branch; will clear once that PR merges to main and the Sonar workflow runs
