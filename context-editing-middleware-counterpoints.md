# Counterpoints — context-editing-middleware-survey.md

## Finding: Long contexts make selective tool-result clearing a sound first-line strategy.

- **Counter-position:** The existence of long-context degradation does not establish that fixed recency-based clearing is the right remedy. Some modern long-context workloads perform better when the model receives enough raw context than when a retrieval or pruning system omits decisive evidence.
- **Counter-evidence:** A comparison of RAG and sufficiently resourced long-context models reports that long context outperformed RAG on average in its evaluated datasets, while RAG mainly retained a cost advantage [Li et al.](https://arxiv.org/abs/2407.16833). A later evaluation reaches a conditional result: long context generally wins on Wikipedia QA, summarization-based retrieval is comparable, and RAG has advantages for dialogue/general queries [Li et al.](https://arxiv.org/abs/2501.01880). LaRA likewise finds that the right choice depends on model, task, context length, and retrieved-chunk characteristics [Kuan Li et al.](https://arxiv.org/abs/2502.09977).
- **Verdict:** rating downgrade — `[moderate]` → `[low]`. Reason: `heterogeneity`; `indirectness`. The finding should be “sound as a conservative first lever to test,” not a proven default for every zbot workload.

## Finding: Typed durable provenance plus just-in-time retrieval is stronger than retaining raw results merely because they are recent.

- **Counter-position:** Provenance makes loss visible and reload possible, but it does not solve the hard part: identifying the right prior evidence and composing it correctly for a new task. Retrieval can turn a transcript problem into a recall-and-reasoning problem.
- **Counter-evidence:** BRIGHT reports low performance for reasoning-intensive retrieval, including a maximum recall@1 of 27.8 in its long-context retrieval setting [Su et al.](https://arxiv.org/pdf/2407.12883). RECON finds substantial limitations in reconstructing multi-hop evidence, tracking cascading invalidations, conflicts, and temporal constraints; its strongest non-oracle system reached 22.4% accuracy [Arya](https://arxiv.org/abs/2607.16716). The long-context-versus-RAG studies also show that direct context can win when the task requires broad or interdependent evidence [Li et al.](https://arxiv.org/abs/2407.16833).
- **Verdict:** do-not-resolve. Typed provenance/reload is preferable when an exact artifact can be named and fetched cheaply; retaining raw evidence can be preferable when the next reasoning step needs broad, uncertain, or multi-hop relationships that a retriever may fail to identify.

## Finding: Summarization should follow tool-result clearing and preserve typed, auditable state.

- **Counter-position:** A strict order is plausible design guidance, but it is not a universal research result. In some tasks, a high-fidelity summary of a verbose tool trace may be more useful than retaining a handful of raw recent results; in others, raw context is the only reliable evidence.
- **Counter-evidence:** The Long Context vs. RAG evaluation reports summarization-based retrieval comparable to long-context performance in parts of its benchmark [Li et al.](https://arxiv.org/abs/2501.01880). ReMemR1’s motivation supports the opposite concern—one-way pruning loses latent evidence—but it is a preprint on a different system and workload [Shi et al.](https://arxiv.org/abs/2509.23040). Anthropic’s guidance presents compaction and structured memory as complementary techniques, not a prescriptive middleware ordering [Anthropic](https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents).
- **Verdict:** do-not-resolve. Clear-first is attractive for protocol safety and faithful tool linkage; summary-first or summary-of-results can be better where semantic content, not protocol history, is the scarce resource. zbot needs trace-based ablation rather than a universal ordering claim.

## Finding: Plans, authority/effect scope, skill state, and artifact pointers should be non-compactable control-plane state.

- **Counter-position:** Making all such state permanently retained risks uncontrolled prompt growth and stale instructions. “Non-compactable” must mean independently versioned and replaceable, not endlessly accumulated.
- **Counter-evidence:** MemoryAgentBench identifies selective forgetting as a core competency alongside accurate retrieval, test-time learning, and long-range understanding, and finds that current memory systems do not master all of them [Hu et al.](https://arxiv.org/abs/2507.05257). LongMemEval explicitly includes knowledge updates and abstention among the important capabilities, which argues against treating all older control state as immutable [Wu et al.](https://arxiv.org/abs/2410.10813). MemGPT’s tiered-memory premise is movement between memory levels, not a permanently pinned prompt [Packer et al.](https://arxiv.org/abs/2310.08560).
- **Verdict:** rating downgrade — `[moderate]` → `[low]`. Reason: `heterogeneity`; `indirectness`. The safer formulation is: retain the *latest validated representation* of control-plane state, with supersession, expiry, and provenance.

## Moderator note

The strongest unresolved tension is not “compaction versus retrieval.” It is whether a later step needs a specific, addressable artifact (favor typed reload) or needs a diffuse web of historical relationships (favor broader retained/summarized context). That distinction should drive zbot’s policy and evaluation corpus.
