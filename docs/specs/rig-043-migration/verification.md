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
- Agent response messages exclude supplied seed history. The host must prepend
  that history when assembling checkpoints.
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
