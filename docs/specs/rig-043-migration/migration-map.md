# Rig 0.43.0 migration map

Release oracle: published rig, rig-agent, rig-core and rig-rmcp 0.43.0 source
(downloaded by cargo info), revision 654567eb64274fca00cab86cdd32c86b9913769e.
The [versioned source](https://github.com/0xPlaygrounds/rig/tree/654567eb64274fca00cab86cdd32c86b9913769e)
is authoritative. No main-branch concept signature is treated as a release API.

Oracle tier: strong for Rust types; T1 verifies shipped source signatures and
the existing host regression baseline. The T2 compiler/runtime probe verifies
the released custom Wire/Transport, erased single call, scoped DynamicTool,
dispatch gate and response-history contract; see verification.md. Production
host parity is now verified by the runtime, gateway, browser and bounded live
provider results in [verification.md](verification.md). The dependency/compiler
notes below retain the original T1 baseline for comparison; the final workspace
selects Rust 1.97.0 and rmcp 2.2.0.

## Dependency and compiler contract

- rig facade 0.43.0 requires rust-version 1.95.0. Host stable is 1.94.1;
  installed 1.97.0 is available without installing another compiler.
- default-features=false requires explicit agent; rmcp enables the separate
  rig-rmcp component. AgentBuilder/Agent are exposed through that facade.
- Current docker/Dockerfile pins Rust 1.93 and must match the release’s minimum
  during T2. CI uses stable; its effective compiler must satisfy the minimum.
- The current SDK feature/transitive rmcp version must be checked rather than
  assumed equivalent. Existing host MCP ownership and actor filtering stay intact.
  The isolated 0.43.0 closure resolves rig-rmcp to rmcp 2.2.0; host 1.7.0 is
  not assumed type-compatible. The public decoder finish import is
  rig::operation::Finish; operation::completion is private.

## Adapter replacements

All direct production Rig imports remain in runtime/agent-runtime/src/rig_adapter.

| Owner | Existing API | Published replacement and preserved responsibility |
| --- | --- | --- |
| model.rs | CompletionModel/GetTokenUsage/RawStreamingChoice | rig::Model<Wire,Transport> erased to DynModel<operation::Completion>; host transport keeps LlmClient retries, rate limits and provider configuration. Wire payload is CompletionRequest; decoder writes text/reasoning/tool/final frames through Out. |
| model.rs | Generic CompletionResponse + mandatory usage | Nongeneric CompletionResponse; choice Vec; provider/raw fields. Usage counters are Option<u64>, retaining unavailable values rather than inventing provider counts. |
| engine.rs | Agent<M>, stream_chat(...).await | Nongeneric Agent; prompt(...).history(...).max_turns(...).tool_context(...).add_hook(...).tool_concurrency(1).stream(). Streaming start stays lazy. |
| tool.rs | ToolDyn/ToolCallExtensions | DynamicTool::new_with_context callback. A single scoped host carrier stores shared context/results; ToolContext scope skips serialization. ToolOutput::text retains literal shaped JSON strings. |
| tool_hook.rs | on_event/Flow::skip/rewrite_result | on_dispatch + DispatchAction::skip, on_outcome + rewrite_tool_result; family-filter completion/tool effects so callback families cannot duplicate host events. Protected veto and peer authority remain first. |
| tool_hook.rs | InvalidToolCall recovery | on_invalid_tool_call -> Some(InvalidToolCallAction::skip); denied names remain feedback, never executable repair. |
| engine.rs::TurnLimitHook | One-based CompletionCall | on_completion_call retains one-based turn; CompletionCallAction::Stop enforces existing tick-before-check policy, independent of changed native budget semantics. |
| context_policy.rs::ContextCapture | CompletionCall prompt/history snapshot | on_completion_call copies history and prompt with event.turn; no log reconstruction is needed before preparing the next provider request. |
| engine.rs/context_policy.rs | FinalResponse.history() | PromptResponse.messages() returns the run transcript, excluding supplied history. Pass the SDK transcript to the existing base/tail checkpoint assembler; do not prepend the seed again. |
| turn_events.rs | ToolCall id/call_id; old streamed choices | Unified CallId; to_string for host correlation, wire for provider identity. Map separate committed ToolCall once; text/reasoning use Item<StreamEvent>. |
| structured.rs/client.rs | CompletionClient/TypedPrompt/agent.completion | Direct single Rig model.call(CompletionRequest) for complete_once, with no agent loop/retry/tool execution. Typed agent prompt returns TypedPromptResponse.output. |
| capability/mcp tests | Old hook/flow/native ToolDyn probes | Port probes to released event actions/DynamicTool. Keep independent connections and close-on-stop evidence; facade rmcp path remains native-only. |
| rig_adapter.rs | rig/rig_core payload diagnostics | Include released rig_agent diagnostics in the protected payload policy; preserve secret-sentinel assertions. |

## Runtime semantic traps to test

1. Tool results are published after the entire batch settles, even at concurrency
   one. Breaking the host stream after a respond/delegate result is too late to
   prevent a sibling side effect. Gate subsequent host dispatch immediately when
   terminal/delegation action state is set; retain completed tool/action evidence.
2. Final transcript excludes seed input history. ContextPolicy already owns the
   base conversation and strips the absorbed prefix from the run transcript.
   Prepending the seed again duplicates history; retain the frozen snapshot
   regression as the oracle.
3. Streaming frames and committed tool calls have different boundaries. Exactly
   one host ToolCallStart is emitted per admitted call, with stable correlation.
4. Completion calls have optional provider usage; single intent completion still
   preserves all returned calls as data and original typed provider errors.
5. SDK request and error tracing belongs to another crate after the split; raw
   payload filtering must cover it before hooks can project sanitized data.

## Frozen construction baseline

cargo test -p agent-runtime --lib -- --test-threads=1:
**456 passed, 0 failed, 2 ignored**, 14.53 seconds, on published b867a18a.
The code bytes in this implementation checkout match that published baseline;
only workflow approval/evidence files have changed at this checkpoint.

Reuse factory/{context_tests,control_tests,live_context_tests,mcp_tests,
progress_tests,result_tests,snapshot_tests}.rs, model bridge tests, engine tests,
structured::completion_tests and MCP lifecycle/capability tests. These cover
provider transport, current schema/fenced JSON, admitted/denied tools, respond
arguments, sequential delegation, veto/rewrite, steering/recall/compaction,
Stop/drop, session resources and diagnostic redaction. Extend only concrete
new-release gaps; do not replace these regression assertions with SDK mocks.
