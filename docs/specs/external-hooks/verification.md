# external-hooks execution evidence

User authorization: 2026-10-03, “push the current branch changes to remote raise a PR and then pull changes from develop branch. Start a new branch and starte executing all specs”. This authorizes the four reviewed scopes and their existing plans. Both spec-stage reviews report Clean in ../rig-hooks-session-graph-review.md.

Implementation checkout: /home/videogamer/projects/agentzero-rig-hooks-session-graph
Branch: feat/external-hooks
Base target: origin/develop (user-selected), merged Rig PR #277 at c7854d75.
Carried-forward local work: f36d9c04 closes the merged Rig workflow.

No acceptance criterion is marked complete by approval alone. Runtime/provider settings and live vault remain outside fixture execution.

## T1 — immutable configuration snapshot

2026-10-04: feat/external-hooks starts from merged develop c7854d75 and
carried workflow closure f36d9c04. Existing spec/plan approvals and two Clean
spec-stage reviews were reused unchanged; the plan is sealed.

Runtime stub red: five required config tests fail. Schema parity red: explicit
$schema:null accepted before correction. Gateway construction initially fails
to compile because PartialSetup has no snapshot field; fixture directory/mode
setup was corrected without weakening the production trust checks.

Parent independently verifies cargo test -p agent-runtime --lib
external_hooks::config::tests (6 passed), cargo test -p gateway-execution
--features test-stubs --lib runner::invoke_bootstrap::tests (14 passed),
cargo fmt --all -- --check and git diff --check (clean). Worker additionally
verifies VaultPaths (10 passed) and strict runtime/gateway all-target clippy.
All three declared JSON Schemas pass Draft202012Validator.check_schema;
positive/negative config fixtures verify event/failure/cwd restrictions.

One bounded reader loads before durable activation in both normal and persisted
root ingress. Missing config is empty, bad config cannot activate execution,
and the phase-one Arc retains its revision/content through file edits.
Actual child propagation belongs to T3; no subprocess/runtime invocation or
Activity completion is claimed by T1.

Logs: /tmp/zbot-hooks-t1-{red,schema-red,green-final,gateway-red,
gateway-green-final,vault-paths,clippy}.log. Existing libc 0.2 is reused as
a Unix-only direct dependency for effective-UID checks and later process
group cleanup; no additional process framework is introduced.

## T2 — bounded JSON subprocess execution

The initial red stub suite failed 13 protocol/projection/process assertions while
the six configuration tests remained green (`/tmp/zbot-hooks-t2-red.log`). A
subsequent negative control proved `env python3 SCRIPT` bypassed the file-form
check and spawned the isolated marker. Launcher/eval/package wrappers are now
rejected; `/tmp/zbot-hooks-t2-wrapper-red.log` preserves that failure.

The final scoped suite has 20 passing tests. It exercises actual Python, Node
.mjs and a locally compiled native program, literal argv, stdin EOF, vault cwd,
minimal PATH/LANG environment, nested and registered-value credential
projection reaching the script, exact 32 KiB dual-pipe success, overrun/nonzero/
malformed output, stdin backpressure, timeout, Stop and future abort. Strict
response parsing and the aggregate UTF-8 context byte budget are covered.

The parent independently reran `cargo test -p agent-runtime --lib external_hooks`
(20 passed; `/tmp/zbot-hooks-t2-parent-green.log`), formatting, diff checks and
sealed-plan validation. Runtime all-target strict clippy passed.

Process invocation is supported on verified Linux hosts. Each command starts
its own process group. Timeout/Stop kills the group and synchronously waits for
the direct child; future abort kills the group and schedules the owned child
wait. On this systemd PID-1 host, real fork fixtures observed parent and
descendant `/proc` disappearance after timeout, success and cancellation.
Descendants are adopted/reaped by host init; this does not promise containment
of operator programs that deliberately daemonize into another session/group.
Other hosts report unsupported invocation before spawn. No actual gateway
lifecycle, persistence or rendered-Activity acceptance is claimed at T2.

## T3 — lifecycle wiring and accepted-message identity

Root ingress carries one validated immutable snapshot and invocation UUID into
SDK runs, delegated children and continuations. Durable claims on the existing
session metadata prevent repeat ingress after restart, reject revision changes
and preserve the consumed 8 KiB UTF-8 context allowance. An interrupted unhooked
message cannot acquire newly enabled hooks on resume. Safe IDs/counters are
persisted; commands, hook context and secret matchers are not.

Red controls exposed missing SDK run/tool boundaries, nonobject tool admission,
fast-mode alias drift, model dispatch after Stop, and a yielded root remaining
Running after Stop. These pass against the existing Rig adapter and gateway.
Configured hooks cannot undo protected tool vetoes; invalid calls have no
before/after-tool effects. Invocation cancellation prevents child continuation.
The existing handle owns a single Stop settlement claim; session status enums
and no-hook control behavior are unchanged.

Independent parent commands, each exit 0:
- cargo test -p agent-runtime --lib external_hook: 30 tests, including actual SDK
  streaming/non-streaming, retry, terminal respond and process-abort controls.
- cargo test -p gateway-execution --features test-stubs --lib external_hooks_tests:
  8 tests, including new/precreated Chat/Research, immutable child lineage,
  persisted resume, ingress Stop and real root-to-child handoff Stop.
- cargo test -p gateway-execution --features test-stubs --lib invoke_bootstrap:
  14 tests; session_control: 8 tests.
- cargo test -p zbot-conversation --test session_meta: 3 tests.
- cargo fmt --all -- --check; git diff --check; sealed plan check-current.

Worker additional scoped gates: Rig factory 55, peer root lifecycle/K-0001 11;
strict all-target clippy for agent-runtime, gateway-execution, zbot-conversation
and a final gateway clippy after the Stop fix. Parent logs are
/tmp/zbot-hooks-t3-parent-{runtime,gateway,bootstrap,control,store}.log.
Stop red/green: /tmp/zbot-hooks-t3-yield-stop-{red,green}.log.

Supported process cleanup now requires verified Linux with systemd PID1;
unsupported hosts fail before command spawn. This is process-group cleanup,
not a sandbox or containment of deliberately daemonizing programs. Persisted
claims prevent automatic replay, not exactly-once external effects across
crashes. Durable Activity and actual daemon/browser joined proof remain T4.

## T4 — durable Activity and joined real-daemon proof

The fresh built zbotd and current UI pass all three isolated full-mode
Playwright journeys: Chat, Research delegation/continuation, and browser Stop
(3 passed, /tmp/zbot-hooks-t4-e2e-green.log). Fixtures use temporary vaults and
a loopback mock provider; live settings and provider token budgets are untouched.
Python, Node .mjs and a compiled Go executable receive actual boundaries. Chat
records eight hook events; Research records twenty over three SDK runs. Each
accepted message has one ingress pair. Reload preserves Activity identity and
terminal answers without executing hooks again; all fixtures report zero drift.
Private context, stderr, reason and command paths stay out of the projection.

The first browser Stop proof exposed a preliminary cancellation validator
rejecting the existing sess-chat-UUID spelling. The narrow fix accepts both
existing canonical spellings while retaining actor/conversation ownership.
Three cancellation controls pass. The final browser proof observes both the
command and its forked child disappear from /proc before cleanup observation;
the same Activity ID/time settles Cancelled and remains settled after reload.

Independent parent gates: workspace cargo check, cargo fmt --all -- --check,
git diff --check, full UI npm run lint, api-logs tests (13), session-details
HTTP tests (15), and the sealed plan check-current all pass. Worker gates also
pass: Activity projector (5), starter files (2), cancellation (3), actual Go
execution (1), UI scoped tests (34), tsc and strict all-target clippy for runtime,
gateway-execution, gateway and api-logs. Logs use /tmp/zbot-hooks-t4-* plus
/tmp/zbot-hooks-workspace-check.log. Earlier T1–T3 gates remain applicable.

Rendered evidence, keyboard focus, axe and scoped HTML validation are recorded
in frontend-evidence.md. No full-workspace test, whole-shell HTML, full WCAG
2.2 AA or CWV pass is claimed. Supported process cleanup remains Linux with
systemd PID1; intentionally escaping programs are outside the cleanup promise.
Final independent adversarial, quality, security and frontend reviews are Clean
(review.md); all eleven acceptance criteria have recorded construction and
joined-artifact evidence.

Known global documentation skips: lint-spec-status reports the unchanged
exec-consolidation-waves Drafting status; lint-traceability reports four unchanged
intent/ward producer pointers. Their files are outside this diff and both failures
are registered in workspace backlog. No global documentation pass is claimed.

Review round 1 OpenAPI correction: actual served YAML/JSON now publish the
canonical hook Activity surface in OpenAPI 3.0.3 form, including nullable fields
and a hook/non-hook oneOf. Endpoint parity and real ingress-response regression
were red before the mirror and green afterward (16 session-details tests).
Logs: /tmp/zbot-hooks-openapi-{red,green,clippy}.log. Runtime/process code and
the previously passing joined Playwright journeys are unchanged by this fix.

Specialist frontend correction: one persistent empty status container receives
loading/error changes instead of creating populated live regions. aria-busy is
on the Activity list, outside the announcement ancestor, so loading text is not
held until completion. The prior component fails the region-identity regression;
the corrected loading→content→cached error→Retry path preserves the same node.
All 34 scoped UI tests pass (/tmp/zbot-hooks-live-region-{red,green}.log).

Specialist runtime corrections: invalid-call explicit/failure-policy vetoes now
return terminal Rig actions and mark run settlement blocked. Twelve actual Rig
cases cover unknown/non-object/protected-policy rejection, including pending
sibling effects; four direct malformed-SDK callback controls check no recovery
or outcome storage after a block. Continue/failure-continue behavior is retained.

A broken real HTTP stream previously issued two requests inside one model hook
pair. The streaming seam now separates the actual failed request and fallback,
with two distinct pairs and the shared context budget. Block/Stop controls
prevent the fallback. Unhooked fallback, malformed SSE and partial text/tool
no-fallback behavior are retained without a new executor or observer framework.
Worker full agent-runtime suite: 495 unit and 16 integration tests pass, two
pre-existing tests ignored; strict runtime Clippy/fmt/diff pass. Logs:
/tmp/zbot-hooks-finalfix-*.log. Final independent parent rerun: workspace cargo check, full UI lint and tsc,
37 runtime hook controls, daemon build, and all three joined Playwright journeys
pass. Chat, delegated Research and browser Stop use the freshly built daemon;
zero fixture drift, actual descendant disappearance and durable reload confirmed.
Logs: /tmp/zbot-hooks-final-{workspace-check,ui-lint,ui-type,parent-runtime,daemon-build,e2e}.log.
Final rendered recapture/keyboard inspection, axe (zero violations, fifteen
passing rules per route), and Activity subtree HTML validation pass.
Final independent re-reviews are Clean (review.md).

PR #278 merge-time CI correction: the hosted Ubuntu image installs a world-writable
/usr/local/bin/node. The hook trust gate correctly rejects it. An isolated real
Node binary with mode 0777 is rejected before its script writes a marker; changing
only that binary to 0755 runs the same script successfully. The diagnostic fixture
was removed after the probe. Existing Node argv/JSON hook regression also passes.
Logs: /tmp/zbot-hooks-pr278-{permissions-probe,node-fixture}.log.

The CI-only fix removes group/world write from the image's Node binary before
unit, integration and coverage invocation; workflow YAML/order and diff checks
pass. Runtime permissions, test assertions and production code are unchanged.
The original security job failed on the recorded rustls advisory. Bounded independent adversarial, quality and security follow-up reviews are
Clean. Hosted rerun remains pending before merge.
