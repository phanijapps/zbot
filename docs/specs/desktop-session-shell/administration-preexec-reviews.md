# Administration amendment pre-execution reviews

Scope: existing Agents, Settings and Integrations presentation under AC11/T4–T5. Run `147871c4-2894-4cc2-9795-8bcfc168867e`; no production implementation changes. User approved scope and tracking-only reset; build-strategy approval is separate.

## Secure-design review

Reviewer: `/root/preexec_security`, read-only. Target: amended spec/plan and administration retrofit/disposition, existing route/credential contracts; scoped access-control, secrets/crypto and injection depth was supplied in the brief. No new authorization, credential-storage, API, runtime or network behavior is planned. QA is isolated and synthetic.

Verbatim verdict:

> Clean — ready to commit.

This is design review only, not implementation acceptance, a security scan or permission to commit. Re-review the implementation at its warranted gates.

Re-review after materializing the construction tests, adding real commissioning preservation coverage and splitting T4/T5 returned the same clean verdict. No production or credential-handling implementation changed during this review.

## Adversarial spec/plan review

Reviewer: `/root/preexec_adversarial`, read-only. First-pass findings and dispositions:

- Blocker: AC11 route/query red artifacts deferred beyond PLAN. Applied: materialized App/DesktopRail/tab-return tests and strict existing helper contract tests, plus shared TabBar/Slideover/ModalOverlay keyboard stubs. All compile before production edits; none are skipped or expected-failure annotated.
- Blocker: no guard-preservation artifact for moved routes. Applied: App.extra uses the real CommissioningGuard (only CommissioningScreen is a page double); each pending-installation admin route must redirect before rendering its page. These three preservation tests pass now and would fail if route migration dropped the guard.
- Concern: administration slice not dependency-sized. Applied: split T4 guarded route/return/tab navigation from T5 presentation/editor reflow/shared keyboard/browser evidence. T5 depends on T4, T4 on T3; inspector/rollout retain their original obligations as T6/T7. Explicit pre-production stop/split threshold forbids a silent over-2,000-line task.

Red validation: 15 expected missing-behavior failures, 115 passing cases across 9 suites, 4.76s. Failures are 6 missing admin chrome/return routes, 3 missing rail return links, 3 dropped page tab-return values and 3 missing shared keyboard contracts. Strict helper and real guard cases pass. Typecheck passes; lint has zero errors and the same 20 existing warnings. One earlier test-only timing failure was corrected by awaiting the real guard before the existing version badge assertion; no production logic changed. Test stderr includes existing Three.js duplicate import and a red Agent tab-case async act warning; not treated as migration acceptance.

## Named skips and verification limits

Second-pass blocker: desktop rail presence alone did not rule out retained legacy primary chrome. Applied explicit absence assertions for `Primary` and `Mobile primary` navigation and the `z-Bot home` topbar link on each administration route, including remount. Focused rerun: six expected red cases and 17 passing cases; the three chrome cases now fail specifically on retained Primary navigation. Typecheck and diff checks pass again.

Final adversarial verdict, after the assertion fix:

> Clean — ready to commit.

Secure-design reviewer also confirmed the final test-only target clean. This closes pre-execution review only: no production implementation acceptance or commit authorization. User scope approval is recorded; separate build-strategy approval remains pending.

Design-review, experience-reviewer and frontend-reviewer unavailable. The approved desktop mockup supplies direction; main-agent rendered inspection will be provisional. No project-knowledge enquiry/capture requested. Existing administration/nav/App baseline: 213 tests / 19 suites passed in 4.56s. Diff whitespace check passes. The unrelated invalid `exec-consolidation-waves` status remains a registered pre-existing doc-lint skip; the existing goal-artifacts backlink warning remains visible. Migration tests, production UI build/rendered evidence, implementation security/adversarial/quality review and original outstanding spec criteria are not complete.
