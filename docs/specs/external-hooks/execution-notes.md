# External hooks execution disposition

Scope: approved T1–T4 on feat/external-hooks, based on merged develop PR #277.
Full mode covers configuration/file I/O, subprocess execution, lifecycle wiring
and durable sanitized Activity. The original checkout and live vault are preserved.

Assumptions: parsing/process execution belongs in agent-runtime, VaultPaths and
root snapshot ownership remain in gateway-execution, and existing api-logs and
session details carry metadata. Scoped Rust tests, real Python/Node/native
subprocesses, full-daemon fixtures and rendered Activity checks prove delivery.
Provider settings, token budgets, authorization and the sole Rig executor remain
unchanged. No domain claim beyond the approved operator workflow requires grounding.

Declined: a plugin framework, remote hook service, implicit shell expansion,
Settings editor, automatic script discovery and per-token process spawning.

Prior spec-stage adversarial and secure-design reviews are Clean in
../rig-hooks-session-graph-review.md. The unchanged spec and plan were approved
by the user; plan hash is sealed and four sequential task waves are scheduled.
Project-knowledge not requested for review planning. No eligible authoring
observations were accumulated at the spec-approved or plan-locked gates.
Design-review/creative-direction and experience-reviewer are unavailable (named
skips); the existing session Activity tokens/layout are the visual reference.

Resolved: T1 bounded configuration/default/schema validation and pre-activation
gateway snapshot binding pass independent scoped gates (verification.md).
Resolved: T2 subprocess trust, bounds, cleanup and credential projection pass
real subprocess fixtures and independent scoped gates (verification.md).
Resolved: T3 root/delegated/continuation wiring, persisted ingress/context budget and protected composition pass independent construction gates.
Resolved: T4 durable metadata, starter/guide, actual daemon Chat/Research/Stop and rendered Activity proofs pass (verification.md).
Resolved: final adversarial, whole-spec quality, security and frontend reviews
are Clean. Every applicable finding was fixed and reverified; the authorized
host-shell exclusion was independently adjudicated. See review.md.

Review shape: dependency-ordered runtime configuration, subprocess protocol,
gateway/adapter lifecycle, and Activity projection. Review each against its
construction tests; the final whole-spec review covers their joined behavior.

T2 resolved: literal process invocation, strict response/projection, trust
validation, bounded pipes/deadline and Linux cleanup pass real fixtures and
independent gates. Rejected launcher bypass was reproduced and fixed. T3/T4
joined acceptance is now recorded below.

T3 review layers: invocation/per-run owner and raw-model wrapper; SDK callbacks
and host settlement; gateway accepted-message metadata, delegate identity and
continuation lookup. Each layer uses existing runtime/store/event seams. Root
execution IDs repeat across messages, so continuation lookup uses an explicit
invocation UUID. Resume metadata contains IDs/revision and one consumed-context byte counter, never a serialized
configuration snapshot or credentials. Construction and joined daemon/UI
evidence are recorded in verification.md.

Final review target includes the user-facing changelog entry. Tail is MIXED: the
runtime protocol, lifecycle propagation, and durable projection cannot be enabled
independently without exposing incomplete event/cleanup behavior. Four sealed
sequential task layers provide the review boundaries and construction proofs.
No plugin framework or separate supervisor is added to split this single feature.

Learning triage: browser cancellation must exercise the same identity spelling
and ingress guards as the shipped UI; direct process tests do not verify routing.
Capture provider discovery found no callable public project-knowledge seam:
project-knowledge unavailable. No fallback knowledge store was created.

## Review round 1 dispositions

- Served OpenAPI hook contract missing: apply. The live served endpoint is the
  caller's contract; worker owns the narrow OpenAPI mirror and endpoint regression.
- Completion metadata: resolve at the mandated finish gate. Implementing and
  unchecked ACs are intentional until every warranted reviewer is Clean, then
  Status becomes Shipped and each verified AC is checked. The assumption about
  unimplemented callback registration records the sealed authoring baseline;
  runtime/gateway registration is now proven by T3/T4 evidence. That historical
  statement is superseded by verification.md, not a missing runtime feature.
  No approved normative contract is rewritten or resealed to change history.

## Specialist round dispositions

- Invalid-tool blocks ignored (quality/security duplicate): apply. Every Rig
  rejection path must honor the configured terminal decision and mark settlement
  blocked, without granting a protected capability.
- Provider stream-decode fallback hidden from model attempt hooks: apply. Preserve
  the provider fallback while recording/budgeting each actual request.
- Populated-on-mount status regions: apply. Keep one empty status container
  mounted before loading/error text updates; existing visible copy stays intact.
- Same-UID unrestricted shell can edit hook files: resolved scope exclusion.
  Security reviewer independently confirms this is the existing authorized
  host-shell capability explicitly excluded by the sealed Protocol rules, not an
  in-scope blocker (/tmp/zbot-hooks-security-scope-disposition.md). Ownership
  checks reject untrusted entry points; they do not prove who wrote a same-UID
  file. No model/API hook registration or discovery is introduced. No unapproved
  host privilege, protected-path text filter or sandbox policy was added.

Final pre-stage tail measurement: 1,887 tracked additions, 93 deletions and
6,079 new-file lines (8,059 raw total); 5,669 new Rust/TypeScript/CSS behavior
and test lines alone. Review shape remains MIXED, with sealed dependency-ordered
T1 configuration, T2 subprocess, T3 lifecycle and T4 Activity boundaries.
This is one joined feature; the construction proofs permit independent review
of those layers without introducing a second executor.

## PR #278 CI merge follow-up

Plan: fix only GitHub-hosted Ubuntu test provisioning in .github/workflows/test.yml
for unit, integration and Rust coverage jobs. The upstream runner image's
install-nodejs.sh runs chmod -R 777 /usr/local/bin; hook program trust rejects
that writable executable. Preserve runtime checks and all existing assertions.
Verify with an isolated real Node permission probe (777 rejected/no spawn, then
go-w succeeds), the existing Node hook fixture, workflow parsing/order and diff
checks. Re-review the bounded change before push/merge. No production runtime
change, interpreter framework, test skip or host policy expansion.

Hypotheses: unsupported reaper would reject every hook, but Python/Go and other
process fixtures passed; Node JSON/argv semantics pass locally with the same
script; runner-provided writable Node is consistent with upstream installer and
the trust gate. The original CI run has 494 passing runtime tests and just the
Node observer failing. Security fails on unchanged rustls RUSTSEC-2026-0285,
already registered in backlog; this follow-up does not claim a clean scanner.

Source: https://github.com/actions/runner-images/blob/main/images/ubuntu/scripts/build/install-nodejs.sh

Resolved: real Node permission probe and original Node fixture pass; YAML ordering,
format and diff gates pass. CI follow-up adversarial, quality and security reviews
are Clean. The unchanged frontend retains its prior Clean review. No findings
remain in this bounded correction; hosted confirmation is the remaining merge check.
