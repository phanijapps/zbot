# zbot ContextEditingMiddleware: Architecture and Research Comparison

## Question

How does zbot's `ContextEditingMiddleware` control growing agent context, and how does that design compare with research-backed strategies for long-horizon LLM agents?

## Scope and evidence

The zbot analysis below comes from the indexed Engram code graph, anchored on `ContextEditingMiddleware::process`, `find_tool_results_to_clear_with_cascade`, `clear_tool_results`, `clear_tool_call_inputs`, and `build_runtime_middleware_pipeline`. It is an implementation map, not a runtime benchmark. External comparisons use the original research papers where available; preprints are labelled accordingly.

## What zbot does

### Role in the request pipeline

`ExecutorBuilder::build` installs a runtime middleware pipeline. When a model context window is known, the gateway enables context editing as a pre-processor before the plan block. Its operational policy differs by mode:

| Mode | Trigger | Recent tool results retained |
| --- | ---: | ---: |
| Chat | 80% of declared context window | 5 |
| Deep/non-chat | 70% of declared context window | 8 |

The generic `ContextEditingConfig` defaults to disabled, with a 10,000-token trigger, three retained results, `[cleared]` as the placeholder, skill-aware placeholders enabled, and cascade unload enabled. The gateway overrides the generic defaults when it knows the provider window. This makes the gateway—not the middleware’s default object—the normal policy authority.

### Decision and transformation path

```text
canonical conversation messages
        │
        ├─ estimate total tokens for the selected model
        ├─ compare against `trigger_tokens`
        │
        ▼ threshold exceeded
find eligible historical tool-result messages
        ├─ keep newest N (`keep_tool_results`)
        ├─ do not clear configured `exclude_tools`
        └─ follow skill/resource dependencies when cascade is on
        │
        ▼
replace selected result payloads with `placeholder`
        ├─ optionally clear associated tool-call inputs
        └─ record any unloaded skills in execution context
        │
        ▼
send the shortened canonical context to the next model call
```

The key point is that this is **selective context editing**, not conversation deletion. The persistent message/checkpoint surface remains available for replay and recovery. The runtime rewrites model-visible history to reduce token pressure.

### Selection rules and invariants

- It clears **tool results**, not ordinary old assistant prose. The project’s context-control tests specifically protect prose from being changed by the tool-result-clearing path.
- The most recent tool results are kept. The candidate-selection graph computes result indices, removes the final `N`, and then clears older eligible entries.
- `exclude_tools` gives a tool-level retention escape hatch. The indexed tests use it to preserve search results while clearing another tool’s output.
- `min_reclaim` and token estimation provide policy hooks to avoid an edit that saves too little context to matter. Its default is zero, so the actual behavior depends on the configured policy.
- `clear_tool_inputs` is optional. When enabled, zbot can also replace the corresponding tool-call arguments; when disabled, the call intent/shape remains visible even if its result has been cleared.
- Placeholder replacement preserves message position and the call/result relation. That is important because provider tool protocols expect a coherent transcript rather than a history with arbitrarily removed messages.

### Skill-aware cascade

zbot treats a loaded skill as an authority/context dependency, not just prose. The skill loader stores `skill:graph`, `skill:loaded_skills`, and `skill:current_skill` in tool context. When a historical `load_skill` result becomes eligible for clearing, `find_tool_results_to_clear_with_cascade` follows the associated resource calls. `clear_tool_results` can mark that skill unloaded and replace its dependent resource results as well.

This is a correctness feature. Keeping a skill resource after hiding the skill that scoped access to it would leave a confusing, potentially over-authoritative fragment of context. The inverse—retaining a load marker after discarding all its resources—would also misrepresent what the agent can rely on. An indexed end-to-end test, `real_compaction_and_recovery_preserve_skills_plan_and_effect_scope`, covers recovery with skills, a plan, and authority/effect scope present.

### What it does not do

`ContextEditingMiddleware` is not zbot’s durable semantic memory, retrieval, or generic transcript summarizer:

- Memory/knowledge-graph recall lives behind the store/recall path and is injected during bootstrap.
- Optional `SummarizationConfig` is separate. Its defaults are disabled, use a 16,000-token summary budget, and prefix summaries with `[Previous conversation summarized]`.
- Durable messages and checkpoints are owned by the conversation/execution-state persistence surfaces, not by this request-time middleware.

## Comparison with research and established strategies

### 1. Bounded, selective tool-result clearing — zbot’s current primary strategy

**Fit.** Strong as a first-line defense. Tool outputs are commonly high-volume, often redundant after an action is understood, and are safer to replace than user/assistant reasoning. zbot also keeps a recent working set and preserves transcript structure.

**Evidence.** Long-context models do not reliably exploit every token: performance can fall when relevant information lies in the middle of a long prompt, even for models advertised as long-context [Liu et al.](https://arxiv.org/abs/2307.03172). RULER finds that near-perfect simple “needle” retrieval does not predict robust performance on longer and more complex tasks; many evaluated models degrade substantially as length grows [Hsieh et al.](https://arxiv.org/abs/2404.06654). Anthropic’s production guidance independently identifies clearing old tool calls/results as one of the safest, lightest compaction forms [Anthropic](https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents). [moderate]

Downgrade: no peer review; vendor-blogged; indirectness (the Anthropic source is production guidance, while the papers establish the long-context problem rather than testing zbot’s policy).

**Assessment for zbot.** The basic choice is sound. The 70% deep-mode headroom is more conservative than chat’s 80%, which makes sense for tool-heavy work that may need several more calls before reaching a natural checkpoint. But fixed “keep last N results” is a recency heuristic, not a relevance policy: a critical old test failure, decision, or file location can be discarded while five recent trivial results remain.

### 2. LLM summarization / rolling compaction

**Fit.** zbot has the separate hook needed for this, but it should not replace selective clearing. Use it when the retained dialogue itself—not merely tool outputs—has important long-range state that no structured store represents.

**Evidence.** MemGPT proposes a virtual-memory model: move information between limited in-context memory and external tiers rather than treating the context window as the only memory [Packer et al.](https://arxiv.org/abs/2310.08560). Anthropic describes compaction as summarizing a near-full context while retaining decisions, unresolved bugs, and implementation detail; it advises first tuning for recall, then precision [Anthropic](https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents). However, work on revisitable memory identifies pruning and overwritten summaries as sources of lost latent evidence, particularly for non-linear/multi-hop reasoning [Shi et al.](https://arxiv.org/abs/2509.23040). [moderate]

Downgrade: no peer review (two cited research sources are preprints); heterogeneity (systems and workloads differ).

**Assessment for zbot.** Keep the current tool-result clearing before any summary. If summary is enabled, make it typed and auditable: preserve goals, decisions with evidence pointers, current plan/status, unresolved errors, changed files/artifacts, delegated work, and explicit uncertainty. Store source message IDs or durable references beside every summary item so an agent can reload evidence rather than treating the summary as ground truth.

### 3. Typed durable memory and just-in-time retrieval

**Fit.** This complements zbot especially well because zbot already has memory, knowledge graph, skills, plans, checkpoints, and recall adapters. The missing question is policy: which state is automatically reintroduced after compaction, and why?

**Evidence.** MemGPT’s tiered-memory framing supports moving durable information out of the prompt and retrieving it as needed [Packer et al.](https://arxiv.org/abs/2310.08560). LongMemEval finds a roughly 30% accuracy drop for commercial chat assistants and long-context models across sustained interaction, and reports benefits from session decomposition, fact-augmented indexing, and time-aware query expansion [Wu et al.](https://arxiv.org/abs/2410.10813). Anthropic recommends lightweight identifiers plus just-in-time retrieval instead of preloading all data [Anthropic](https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents). [moderate]

Downgrade: no peer review; vendor-blogged; indirectness (benchmarks are conversation/memory workloads, not zbot tool traces).

**Assessment for zbot.** Prefer a compact state manifest plus targeted reload over retaining raw tool payloads. Each cleared result should leave a machine-readable provenance reference—tool name, call ID, artifact URI/path, content hash, and short typed outcome. Before a later action that depends on it, the agent can fetch the authoritative result. This is stronger than keeping text merely because it is recent.

### 4. Task-state checkpoints and structured notes

**Fit.** zbot’s plan block, execution context, and recovery test already form a strong base. The advantage is that plans and authority scope are retained independently of conversational prose.

**Evidence.** Anthropic presents structured note-taking as a way to persist progress, dependencies, and task state outside the context window, and distinguishes it from compaction [Anthropic](https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents). LongMemEval’s results support separating and indexing facts rather than treating a history as one flat text blob [Wu et al.](https://arxiv.org/abs/2410.10813). MemoryAgentBench frames selective forgetting, accurate retrieval, test-time learning, and long-range understanding as distinct memory competencies; existing agents do not master all four [Hu et al.](https://arxiv.org/abs/2507.05257). [moderate]

Downgrade: no peer review (two preprints and a vendor source); indirectness.

**Assessment for zbot.** Treat these as non-compactable control-plane state: user intent/constraints, active plan, completed and pending steps, safety/effect scope, skill authority, unresolved failures, and durable artifact pointers. Keep them short, structured, versioned, and generated/updated at explicit milestones—not only when token pressure arrives.

### 5. Retrieval alone is not enough

**Fit.** Retrieval should be a recovery mechanism, not an excuse to aggressively erase evidence.

**Evidence.** LongMemEval shows targeted memory designs can improve retrieval and downstream QA [Wu et al.](https://arxiv.org/abs/2410.10813), but BRIGHT finds reasoning-intensive retrieval remains difficult even with a reduced search space [Su et al.](https://arxiv.org/pdf/2407.12883). Research on revisitable memory argues that one-way pruning loses evidence needed for later non-linear reasoning [Shi et al.](https://arxiv.org/abs/2509.23040). [moderate]

Downgrade: no peer review; heterogeneity; indirectness.

**Assessment for zbot.** Preserve lightweight provenance at clear time and test recall under realistic tasks. A retrieval failure after compaction should be observable and recoverable—not silently converted into a plausible hallucinated continuation.

## Recommended target policy for zbot

The research does not support one “best” compaction algorithm. It supports a layered policy:

1. **Retain a small, explicit working set.** Keep recent calls/results, current user turn, non-compactable control state, and the active plan.
2. **Clear raw tool payloads first.** Keep zbot’s structural placeholder, tool-call linkage, exclusion list, and skill-aware cascade.
3. **Write typed outcome/provenance records before clearing.** Do not ask a later model to reconstruct an old tool result from a vague summary.
4. **Retrieve just in time.** Reload source artifacts/results by identity when the next action needs them; favor authoritative storage over a generated summary.
5. **Summarize only residual narrative state.** Require references, preserve open questions and counterevidence, and allow a later agent to inspect the source history.
6. **Evaluate on zbot traces.** Measure task success, recovery correctness, recall precision/recall, context size/cost, compaction frequency, and unsafe/incorrect tool actions—not only whether a prompt fits.

## Concrete evaluation matrix

| Scenario | Current risk | Pass condition |
| --- | --- | --- |
| Old tool result contains the only failing-test output | Recency clearing removes the decisive diagnostic | Agent retrieves the result by provenance and fixes the right fault |
| Skill loaded, then its resource output is compacted | Dangling or over-authoritative resource context | Cascade keeps state coherent and reloading remains confined to the skill |
| Tool result is large but represented by a durable artifact | Raw transcript wastes tokens | Placeholder plus artifact pointer permits exact reload |
| Multi-turn plan after several compactions | Plan/authority drift | Checkpoint reconstruction preserves plan, scope, skills, and pending work |
| Middle-of-history decision becomes relevant later | Summary/retrieval misses it | Agent finds the cited decision source and reports uncertainty if not found |

## Known unknowns

- **Known-unknown:** How frequently zbot’s real workloads trigger compaction, how many tokens it reclaims, and which result classes are most often needed again. Would be closed by: production telemetry keyed by compaction event and an offline replay corpus.
- **Known-unknown:** Whether 80%/70% and retain-5/retain-8 are optimal by provider, model, and workload. Would be closed by: a trace-based ablation varying trigger, retained results, exclusions, and recovery method.
- **Known-unknown:** The exact fidelity of zbot’s optional summary model/prompt on real execution traces. Would be closed by: source-grounded factual-recall and recovery tests for summaries.
- **Unknowable:** A universally optimal policy. Different tasks trade latency, token cost, exact replay, and tolerance for stale context differently; no benchmark can collapse those product choices into one setting.

## Bottom line

zbot already implements a defensible first layer: selectively clear old tool output, retain a recent working set, preserve transcript shape, and treat skill resources as dependency-scoped. The highest-leverage improvement is not replacing it with opaque summarization. It is making every cleared payload **revisitable by typed provenance**, while preserving compact structured task state and measuring recovery on real zbot traces.
