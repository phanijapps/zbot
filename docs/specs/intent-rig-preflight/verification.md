# Verification: Rig intent preflight recovery

Current code uses one real Rig Agent completion per existing attempt and explicitly requests low reasoning effort for GLM-5.3 on direct z.ai and Ollama. The configured z.ai model and 5000-token output setting are unchanged. Native JSON and Ollama fenced JSON both pass host validation.

## Final functional evidence

- Original checkout: `cargo check --workspace` and all **27 intent HTTP regressions** passed after the final prompt correction. The opt-in provider test remains ignored by default.
- Runtime library: **455 passed, 2 ignored**. Workspace check, scoped runtime/execution clippy with warnings denied, formatting and diff checks passed in the isolated checkout.
- Final direct z.ai probe: exact saved current-affairs request, current vault rubric, populated catalog including 48 skills, 44 agents (including synthesized ward agents), 37 wards, actual configured endpoint, `glm-5.3`, **5000 output tokens**. Compiled public analyzer returned a **validated Graph decision in 8.79 seconds**; build time excluded. No endpoint proxy.
- Final real Ollama fallback probe: same exact input/rubric/expanded catalog, `glm-5.3-flash:cloud`, 5000 tokens. First missing tool submission was controlled locally; subsequent fenced responses came from configured Ollama. Validated Graph decision in **12.53 seconds**, including one resource correction. Modes tools → fenced_json → fenced_json, two real provider requests. The proxy did not inject reasoning hints.
- Actual daemon: dev watcher rebuilt zbotd (PID 1982015), and a real WebSocket invocation exercised indexing, memory lookup and provider completion. Intent returned a **validated Simple decision, no fallback, in 20.88 seconds** including a resource correction. Overall invocation time to that event was 41.26 seconds. The temporary diagnostic's extra Graph assertion exited nonzero because the valid result was Simple; this is not presented as a Graph-planning success. Standalone exact-input Graph verification above passed. Full delegated research was not run.
- Every diagnostic session was scoped-cancelled immediately at intent completion; both daemon diagnostics have cancelled root executions, no child executions, zero root/delegated execution tokens and no pending continuation. Preflight/model/embedding calls still occurred; zero execution tokens is not a claim that the diagnostic cost nothing.

Live probes keep credentials in process memory and do not persist raw prompts/model responses in this report. Standalone probes use empty memory/procedure fixtures; the actual daemon check is the evidence for real retrieval. Model classifications may vary; these results do not guarantee every provider call meets the deadline or every decision selects Graph.

## Failures retained

The original user session `sess-77f2b099-b699-53d6-abb0-2a6ca88ac4b4` received no intent completion before its shared 45-second deadline. Baseline exact-input direct z.ai analyzer failed `deadline_exceeded` at 45.01 seconds. An earlier fixed probe with seven configured agents passed Graph at 14.12 seconds; the final 44-agent probe above supersedes it.

Ollama without an explicit effort hint failed at 45.01 seconds. A repeat observed HTTP 200 after 35.26 seconds, finish_reason length and no content/tool calls; the corrective request then exceeded the remaining deadline. Temporary low-effort injection subsequently produced responses in roughly 2–3 seconds, with one invalid-shape run and one corrected successful run. Production low effort then passed the seven-agent fallback probe in 2.07 seconds. These early probes omitted synthesized ward agents; the expanded final probe is listed separately above.

The first actual-daemon probe, `sess-90d73bf9-c1e1-59c9-9f3a-6207b575ea95`, failed at 19.12 seconds: unknown_resource on the first decision, invalid_json on its correction. The vault rubric mandates unavailable coding and permits root in recommendation arrays. No raw response was retained, so the rejected ID is unknown. The final fix explicitly subordinates unavailable rubric recommendations to catalog IDs and repeats the existing transport instruction on every corrective prompt. The second daemon session, `sess-75429707-e295-55e8-a66e-e51fc6995b96`, returned the validated Simple decision recorded above.

## Regression and review evidence

- z.ai low-effort/5000 HTTP assertion confirmed red before production change: absent hint versus low; decision/token checks already passed.
- Ollama low-effort/fenced fallback assertion confirmed the same red before adding the Ollama condition.
- Native resource-correction contract regression confirmed red because the latest correction omitted the bare-JSON instruction; final test passes.
- Negative HTTP cases prove no effort hint for z.ai GLM-4.7, Ollama GPT-OSS and other-provider GLM-5.3.
- Rig tests verify one completion, no executable intent tools/hidden retries, all tool arguments retained and typed capability errors preserved.
- Provider-body error canary is absent from public fallback diagnostics and captured tracing events. Arbitrary provider parameters cannot override model/messages/tools/token controls.

Initial adversarial/security/quality implementation reviews were clean after negative coverage and registration fixes. Final narrowed adversarial, security and quality reviews of the two prompt wording changes are clean; functional checks and local application are complete. The cohort scope amendment re-pinned approved hashes with zero retries at that time, retaining the earlier receipt; the engine run was preserved.

## Preservation and remaining scope

The original user session remains stopped: root cancelled, both historical children completed, pending delegations/continuation zero, cumulative input frozen at 1,460,972 and output at 28,982. Persisted session status remains crashed, not cancelled.

Only five source/test files and three new verification/spec documents were applied, with additive README/workspace registrations. Final hash receipt confirms all 127 unrelated previously edited/untracked files unchanged, all five source/test files equal to the reviewed candidate, and README/workspace changes additive. All three affected sessions remain stopped with no pending continuation.

Separate research search pacing, result-offload recovery and cumulative cost remain outside this intent fix. The 5000 setting is an output budget; it does not cap cumulative session input across repeated prompts. No deadline/token/delegation policy or provider credentials changed.

Named pre-existing limitations: origin/main freshness warning; global status lint rejects unrelated exec-consolidation-waves Drafting; project-knowledge capture blocked by staged_dual_writer. No unrelated repair or migration was performed. UI review is not applicable to this backend change.
