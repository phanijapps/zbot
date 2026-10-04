# rig-043-migration execution evidence

User authorization: 2026-10-03, “push the current branch changes to remote raise a PR and then pull changes from develop branch. Start a new branch and starte executing all specs”. This authorizes the four reviewed scopes and their existing plans. Both spec-stage reviews report Clean in ../rig-hooks-session-graph-review.md.

Implementation checkout: /home/videogamer/projects/agentzero-rig-hooks-session-graph
Branch: feat/rig-hooks-session-graph
Base target: origin/develop (user-selected); pull --ff-only reported already up to date.
Carried-forward published work: b867a18a, PR #274.

No acceptance criterion is marked complete by approval alone. Runtime/provider settings and live vault remain outside fixture execution.

## T1 — migration map and baseline (executed)

- Published branch snapshot b867a18a passed cargo check --workspace and npm run build.
- cargo test -p agent-runtime --lib -- --test-threads=1 passed: 456 tests, 2 ignored. No live provider request was invoked.
- cargo info acquired rig/rig-agent/rig-core/rig-rmcp 0.43.0; local release source verifies the API inventory in migration-map.md.
- Rust 1.95 minimum, explicit agent feature, batched tool-result commit and transcript-only final history are verified migration constraints.
- Existing fixtures are retained as construction oracles; no runtime dependency or behavior is changed by T1.

T2 is the next scheduled dependency/bridge work; T3 owns runner/hook/context/MCP migration and T4 proves integrated parity. No release acceptance criterion is complete merely because T1 passes.

## T2 — released bridge contract probe (started, incomplete)

Before modifying production adapters, a temporary standalone Rust crate at
`/tmp/zbot-rig043-probe` compiled against exact `rig = 0.43.0` with
`default-features = false` and `agent,rmcp`. It uses a deterministic in-process
Wire/Transport and performs no network/provider calls.

`cargo +1.97.0 test --manifest-path /tmp/zbot-rig043-probe/Cargo.toml`:
**3 passed, 0 failed**. These are SDK contract checks, not zbot runtime parity:

- Direct erased model call returns both supplied tool calls without execution,
  makes one transport call, and preserves absent usage counters as None.
- Agent response messages exclude supplied seed history. The host checkpoint
  assembler must retain its existing base/tail ownership instead of treating the
  SDK transcript as a complete conversation; the frozen host snapshot test is
  authoritative for that integration.
- A scoped private host value reaches DynamicTool dispatch without serialization;
  a dispatch hook prevents the later sibling from executing after the terminal
  callback, with literal tool-result text retained in the transcript.

The compiler corrected the private operation::completion::Finish path to the
public rig::operation::Finish re-export before the passing run.
`cargo +1.97.0 tree --manifest-path /tmp/zbot-rig043-probe/Cargo.toml -p rig-rmcp --depth 1`
resolves rig-rmcp 0.43.0 to rmcp 2.2.0. The existing host uses rmcp 1.7.0;
compatibility remains migration work, not a passed host MCP check.

No runtime manifest, lockfile or provider adapter has changed yet. T2 remains
the current scheduled wave; workspace check/clippy against the migrated release
and focused host regression suites have not passed because that port is pending.
Implementation branch feat/rig-hooks-session-graph is published with upstream
origin/feat/rig-hooks-session-graph. The original branch remains PR #274.

## Resume after CI recovery merge — 2026-10-03

PR #276 was confirmed merged at develop d514b428. The clean implementation
branch merged origin/develop without conflict; the canonical active Rig entry
has no findings. Cohort identity and approved plan hash match; wave T2 is
resumed without resetting the Rig workflow. The completed CI engine state is
carried forward after the user's merge confirmation.

Release source manifests require Rust 1.95.0. The installed Rust 1.97.0
compiler is selected in rust-toolchain.toml and the Docker and CI build entries
are aligned with it. The actual rust:1.97-slim-bookworm manifest and
dtolnay/rust-toolchain refs/heads/1.97.0 exist (read-only queries passed).

The release-component boundary tests first failed for rig-agent, rig-rmcp,
rig-cassette and rig-http escapes, then passed after extending the existing
guard. python -m unittest discover -s scripts -p test_rig_boundary_check.py:
5 tests passed. python scripts/rig_boundary_check.py: rig-boundary-clean.

Initial cargo +1.97.0 check -p agent-runtime after resolving the exact release
failed with 21 removed-API diagnostics, proving the planned adapter port is
required. The implementation unit keeps the existing LlmClient and ported
model/tool surfaces plus their coupled runner callbacks in the same working
layer; T3 will separately verify and refine runtime semantics before T4 parity.
Project-knowledge not requested. All earlier unrelated baseline findings remain
recorded; no provider or live vault settings are changed.

## Released port — verification in progress

The release bridge now compiles throughout the workspace. The first migrated
runtime test run passed 452 tests, failed four unchanged regressions and ignored
two opt-in tests. Those failures govern the fixes: denied calls must skip after
hooks; a per-run host scope must remain authoritative; checkpoint assembly must
respect its existing base/tail contract; and Stop observed after a tool result
must prevent the next sibling dispatch despite the SDK batching its public
results. No runtime parity pass is claimed from that run.

The SDK removes coupled model/tool/runner interfaces, so compilation requires
porting their signatures in one working unit. The necessary lifecycle mapping
exceeds the original 2000-line estimate. Implementation review will inspect two
bounded responsibility groups: released model/structured/client and private
tool-scope/results (about 1400 lines), then runner/lifecycle/context and MCP
fixture mappings (about 1000 lines), using the same complete working tree and
unchanged regression assertions. The SDK still owns dispatch and model turns.

Fresh dependency scans retain the baseline rustls 0.23.36 advisory
RUSTSEC-2026-0285 (cargo audit exit 1) and pre-existing distillation license
metadata failure (cargo deny exit 5). The scan adds no new vulnerability
advisory. These gates are not reported green or suppressed by the migration.

## T2 and T3 — migrated host parity verified

Cargo metadata --locked resolves rig, rig-core, rig-agent, rig-http,
rig-cassette and rig-rmcp to registry 0.43.0, with rmcp 2.2.0; no stale Rig Git
pin remains. Runtime release identities and the boundary guard both pass.

The four first-run failures were fixed against the frozen behavior assertions.
The final runtime library suite passes 457 tests, with the same two opt-in
provider tests ignored. The final adapter-only confirmation passes 105 tests.
Both cargo check --workspace and cargo clippy -p agent-runtime --all-targets
-- -D warnings pass with Rust 1.97.0, as do fmt --check and diff --check.
Logs: /tmp/port-runtime-tests-final.log, /tmp/port-adapter-final.log,
/tmp/port-workspace-final.log and /tmp/port-clippy-final.log.

The released callback mapping uses one private acknowledgment channel so the
host observes each tool result before the SDK can dispatch the next sibling.
Dropping the consumer releases pending acknowledgments. Rig owns the turn and
dispatch loop. Results retain per-call actions and terminal settlement; no
alternate executor or provider transport was introduced. Provider reasoning is
kept separate from single-completion intent output, and absent usage remains
unavailable instead of synthesizing counters.

Bundled fixes: Rust 1.97 adds a collapsible_match lint at
runtime/agent-tools/src/tools/multimodal.rs. The existing Object condition and
body were moved unchanged into a match guard to unblock the required scoped
clippy command. This is a compiler compatibility syntax change; no suppression
or additional behavior was added.

## T4 — gateway/provider verification in progress

The targeted gateway suites pass 52 tests, with one explicit live-provider
test ignored: rig_parity_tests, rig_tool_contracts, intent_output_tests,
lifecycle_tests, continuation_watcher_tests and session_invoker_tests. The
tool contract includes built-ins plus native MCP in one Rig run and real
compaction/recovery. Logs: /tmp/zbot-rig043-gateway-tests.log.

The existing opt-in intent test was invoked directly from its freshly built
test binary with isolated temporary storage and a 90-second subprocess bound.
Configured Z.AI glm-5.3 and Ollama glm-5.3-flash:cloud both pass the synthetic
research request at 5000 maximum output tokens (4.01 and 2.30 seconds). Only
provider credentials and configured model names were read; no settings were
written. Logs are secret-redacted and mode 0600 at
/tmp/zbot-rig043-live-provider-z.ai.log and
/tmp/zbot-rig043-live-provider-ollama.log.

The actual daemon builds successfully from the feature checkout (cargo build
-p daemon). Existing full-mode browser tests now use a fresh same-origin
loopback vault and cover both Chat and Research respond-only reload; the
Stop/continue fixture is isolated the same way. Browser and delegated Research
journey results are pending and are not counted as passes here.

### T4 final artifact observations

After the final Rust handoff, cargo build -p daemon passed again, and the
three isolated full-mode browser tests passed against that final binary:
Chat respond-only reload, Research respond-only reload, and Stop/continue
with a durable last-turn answer. Both respond-only paths render exactly one
answer and report zero fixture drift. Logs: /tmp/zbot-rig043-daemon-final.log
and /tmp/zbot-rig043-browser-final.log (3 passed, 2.2 minutes).

The browser Stop fixture documents a best-effort race. To prove actual
cancellation separately, /tmp/zbot-rig043-stop-smoke.py started the final
daemon on a fresh temporary vault and a local HTTP provider that deliberately
held its first streaming request. After observing the provider enter and the
session become running, the real cancel endpoint returned in 5 ms. Durable
API reads reported session crashed / root cancelled (the existing status
contract), cleared delegation bookkeeping and no false completion. A fresh
invoke on that same session completed and its respond-only terminal argument
was present in the stored message API. All child processes were torn down.
Log: /tmp/zbot-rig043-stop-smoke.log; vault: /tmp/zbot-rig043-stop-zhgx9b44/data.

All five opt-in full-runner golden tasks pass, including the strengthened
parallel join journey explicitly invoked in Research mode. Its durable
session mode remains research, both child completions precede root completion,
and the terminal answer is recovered from stored session messages. The other
golden paths retain their Chat mode. Log: /tmp/zbot-rig043-golden-final.log.

The combined strict agent-runtime / gateway-execution all-target clippy
passes, including test-stubs. The unchanged repository-wide doc lint remains
blocked solely by the registered exec-consolidation-waves Drafting vocabulary
issue; no migration metadata violation is reported. The Rig contract header
correctly declares no public API contract change; brief/contract checks from
authoring remain applicable because those public contracts are unchanged.

Final package coverage: cargo test -p agent-runtime passes 473 tests with two
opt-in tests ignored, including native MCP lifecycle/capability and procedure
integration suites; doctests have no cases. cargo test -p gateway-execution
--features test-stubs --lib passes all 557 tests, including common-builder and
runner/cancellation regressions. Logs: /tmp/zbot-rig043-runtime-package.log and
/tmp/zbot-rig043-gateway-lib.log.

## Acceptance evidence index

| Criterion | Observable proof |
| --- | --- |
| AC1 release identity | Locked Cargo metadata exact registry 0.43.0 closure; manifest/lock identity regression |
| AC2 sole runtime | Common-builder regressions in the 557-test gateway suite; root, continuation and delegated full-runner journeys; unchanged A2A runner ingress |
| AC3 providers | Single-completion typed-error/schema/multiple-call tests, 27 intent protocol regressions, bounded actual Ollama and Z.AI checks |
| AC4 tool policy | Frozen factory veto/chained-shaping/result suites, protected peer authority, skill/connector/native MCP gateway tool contracts |
| AC5 context | Frozen live-context steering/recall and snapshot tests; real gateway compaction/recovery |
| AC6 settlement | Frozen Stop/terminal sibling regressions; real pending-provider cancel/continue; Chat/Research respond-only browser reload; durable delegated answer |
| AC7 MCP | Runtime package native lifecycle/capability suites, owned-session drop/Stop/failure tests, and mixed built-in/MCP gateway execution |
| AC8 integrated parity | Final daemon browser3, full-runner golden5, targeted gateway52, full runtime473 and gateway557; both configured providers passed live intent |

Adversarial, security and whole-spec quality reviews each report Clean — ready to commit. Their reports are recorded alongside this evidence.
