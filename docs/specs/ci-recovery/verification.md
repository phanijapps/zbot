# CI recovery evidence

User authorization: 2026-10-03, “fix them”, following the exact two-failure
diagnosis and async-trait 0.1.92 proposal. Checkout: agentzero-ci-recovery;
branch: fix/ci-stop-async-trait; base: develop fc41a64c.

## Reproduction and root cause

- npm test -- src/features/research-v2/ResearchPage.test.tsx -t 'shows Stop button only while running' reproduced the original assertion failure. A running fixture inherited sessionId=null from makeIdleResearch; ResearchHeader disables Stop without a session ID. The missing fixture identity is the earliest divergence, not async timing or a broken Stop handler.
- Security run 37168777960 reports 136 double_must_use diagnostics from async-trait expansion, in agent-primitives and zbot-stores-traits. The locked 0.1.89 macro inserts the offending attribute; published 0.1.92 removes it. [Upstream release](https://github.com/dtolnay/async-trait/releases/tag/0.1.92).
- Cargo's precise update also requires syn 3.0.6. Structural comparison of package records proves only async-trait 0.1.89 is removed, async-trait 0.1.92 and syn 3.0.6 are added, and every other package record is byte-value equivalent. No runtime, workflow or lint suppression is changed.

## Passing local gates

| Check | Result |
| --- | --- |
| cargo fmt --all --check | Pass |
| cargo clippy --all-targets -- -D warnings | Pass, 1m15s; Rust 1.94.1 |
| cargo check --workspace | Pass, 54.09s |
| cargo test -p agent-primitives -p zbot-stores-traits -p agent-runtime --lib -- --test-threads=1 | 515 passed; 2 existing ignored |
| npm test | 1,440 passed across 125 files |
| npm run build | Pass |
| python scripts/rig_boundary_check.py | rig-boundary-clean |

Rust checks reuse the existing build cache through CARGO_TARGET_DIR; source
edits stay in the isolated checkout. No provider call or live vault mutation is
part of these checks. The full frontend suite is repeated after the accessible
role assertion review correction; the count remains 1,440.

## Existing failures found by subsequent gates

- cargo audit --db /tmp/zbot-ci-recovery-advisory-db --json: RUSTSEC-2026-0285 affects unchanged rustls 0.23.36; additional unmaintained/unsound dependency warnings are reported. No advisory applies to the changed async-trait or syn packages. A fresh temporary DB avoids the broken shared local advisory cache without deleting it.
- cargo deny check: advisories and licenses fail for the unchanged rustls advisory and missing license metadata in services/distillation/Cargo.toml. Bans and sources pass.
- node scripts/npm-audit-high.mjs apps/ui: unchanged package-lock reports brace-expansion and nanoid advisories. No Node package is changed by this fix.
- lint-spec-status.py --root . --base-ref origin/develop: the already recorded exec-consolidation-waves/spec.md Status Drafting violation remains. No unrelated spec is relabeled.

These are baseline follow-ups, not ignored vulnerabilities or passing security
CI. The original two diagnosed failures are remediated; the entire security
workflow is not claimed green. Follow-up slugs are registered in workspace.toml.

## Review disposition

Full mode was selected when Cargo revealed the required additional syn major.
The initially prepared narrow patch is retained; the new full workflow is
sealed before final gate/review transitions. No other cohort is reset.

Applied: make the plan's package-comparison test include required syn3; add
explicit boundary rails; use an accessible role/name for the idle Stop check.
The temporary active/Approved membership is corrected to queued/Approved during
pre-execution review and becomes active/Implementing after sealing. Rejected
the obsolete plan Executing vocabulary: work-loop and its approve-plan validator
require Approved; final plan state is Done.

Project-knowledge not requested; no eligible scratch capture or knowledge diff.

Final adversarial review's documentation nit is applied: the shipped list now
uses a quarter-neutral heading, matching the October evidence. Only that heading
changed after the successful runtime gates; git diff --check passes.

Tail triage: 19 changed behavior/test lines; 17 changed generated lockfile lines.
The remaining material is bounded scope, workflow status and verification docs;
no large behavior transformation or PR stack is needed. Existing runtime gates
cover the unchanged Stop guard and compile all affected macro uses. This patch
ships no new user-invoked artifact requiring a new happy-path exercise.
Frontend/experience review is not triggered: production UI output is unchanged.
No new generalizable knowledge observation was admitted; the fixture and
upstream-patch details are incident evidence retained above.

Quality review clarified the MSRV evidence: async-trait 0.1.89 requires Rust
1.56; updated async-trait 0.1.92 and syn 3.0.6 require Rust 1.71. The passing
Rust 1.94.1 gate validates the updated packages. No compiler pin changes.

The factual MSRV wording correction changes the canonical spec hash without
changing the authorized objective, acceptance criteria or implementation. Only
this CI cohort baseline is re-pinned to the corrected evidence using its
existing run ID; the engine and all other cohorts remain intact. Its two
resolved nit fingerprints are retained here: shipped-quarter heading
12d5888c2602f8fc74226627bb06b1646030b18630d6cac82b1ffe48779f9a82;
MSRV wording 2831dd9f17191a7c4324d428ddaededee89d22051895bce053c03a9ded482141.
Final adversarial, security and whole-spec quality reviews each returned
Clean — ready to commit.
