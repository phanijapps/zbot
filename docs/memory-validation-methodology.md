# Memory Recall Validation Methodology

**Purpose:** Decide whether MMR (MEM-006), the cross-encoder reranker (MEM-007), or future reranking changes actually improve zbot's recall — instead of guessing from vibes.

The method is **predict-then-measure**: write down what we expect to happen before running, then compare against actuals. Predictions that hold = signal the change works. Predictions that miss = the knob is wrong or the change is wrong.

---

## The five phases

### Phase 1 — Build a test corpus

Pick 5-10 representative queries from past sessions. For each, capture three fields:

| Field | What goes here |
|-------|----------------|
| `query` | The verbatim user message that triggered recall |
| `session_id` | The session it came from (so we can pull context if needed) |
| `judgement` | One of: `recall_useful`, `recall_noise`, `recall_missed`, `recall_irrelevant` |

That's the corpus. Anchored in real work, not synthetic.

### Phase 2 — Predict, before running

For each query in the corpus, write **specific, falsifiable predictions** for each rerank config under test. Predictions go into a committed doc — no retconning after the fact.

A good prediction names:
- A specific fact ID or content phrase
- A specific rank position or rank delta
- A specific config it's predicting against (MMR on/off, rerank on/off)

A bad prediction is vague: "results will be better." That can't be falsified.

### Phase 3 — Run, capture actuals

You restart the daemon, fire each query, capture the recall output. Three options:

- **Lightweight (preferred):** add a `tracing::info!` block to `MemoryRecall::recall` that dumps top-K facts at each stage (post-rescore, post-MMR, post-rerank). Run with `RUST_LOG=gateway_memory::recall=info`. Logs become the actuals.
- **Heavier:** a `recall-eval` CLI binary that loads `knowledge.db` directly, runs queries against multiple configs, dumps JSON. Best for repeat tuning cycles.
- **Lightest:** eyeball the agent's response, infer which facts were used. Noisy but zero infra.

For first pass: lightweight. For ongoing tuning: build the CLI.

### Phase 4 — Compare

Take predictions + actuals, mark each prediction ✓ or ✗ with evidence:

```
Query 3: "explain the rescore pipeline"
  Prediction A: With MMR enabled, ≥2 Flux-LoRA correction duplicates drop to rank 6+
  Actual A:     Flux corrections at ranks 7 and 9 → ✓ MATCH
  
  Prediction B: Domain fact "rescore order" surfaces in top-3
  Actual B:     Domain fact at rank 5 → ✗ MISS
                Likely cause: category weight for `domain` (1.0) too low against
                competing corrections (1.5). Tune candidate: bump `domain` to 1.3
                OR introduce intent router to detect "explain X" → boost domain.
```

### Phase 5 — Decide

- **Most predictions hold + recall feels better:** ship as-is, move on
- **Predictions hold + recall feels the same:** the change was a no-op for this corpus. Either expand the corpus or revert
- **Predictions hold + recall feels worse:** the predictions were measuring the wrong thing. Redesign predictions, NOT the code
- **Predictions miss:** propose specific knob changes with rationale; re-run; iterate

The goal is to converge on a config that produces predictable, defensible recall behavior — not the "best" one in some abstract benchmark sense.

---

## Sample corpus + worked predictions

Here are three sample queries showing the pattern. Use this as a template when filling in your real corpus.

### Sample query 1 — noise case (MMR hypothesized to help)

```yaml
- query: "draft me three more slides for the math worksheet"
  session_id: sess-xxxx-1
  judgement: recall_noise
  context: |
    Recall surfaced 4+ corrections about Flux LoRA training and VRAM
    settings (from prior image-gen work) when the topic was math worksheets.
    None were relevant.
```

**Predictions:**

| # | Config | Prediction |
|---|--------|------------|
| 1.1 | MMR off, rerank off (baseline) | Top-10 contains ≥3 corrections sharing the `image-gen` theme |
| 1.2 | MMR on (λ=0.6), rerank off | Top-10 contains ≤1 `image-gen` correction; near-duplicates pushed below rank 8 |
| 1.3 | MMR on, rerank on (BGE-base) | Top-3 contains zero `image-gen` corrections; domain facts about "slides" or "worksheets" rank in top-5 |

**What we're testing:** MMR's diversity filter is supposed to suppress theme duplicates. Rerank is supposed to elevate content with high semantic overlap to "math worksheet". Prediction 1.1 establishes the baseline noise level. 1.2 isolates MMR's effect. 1.3 isolates the rerank's effect on top of MMR.

**Tune verdict if any fail:**
- If 1.2 fails (MMR didn't push duplicates down): lower λ to 0.4 (favor more diversity), or raise `candidate_pool` to 50 so MMR has more material to choose from
- If 1.3 fails (rerank didn't elevate the right content): check whether rerank ran (log line `fastembed reranker called`), whether `model_id` is correct, and whether `score_threshold` is dropping good results

### Sample query 2 — precision case (rerank hypothesized to help)

```yaml
- query: "what's the agent's policy on commit titles"
  session_id: sess-xxxx-2
  judgement: recall_missed
  context: |
    A schema fact existed for "use sentence case in commit titles" but
    didn't appear until rank 6. Several raw corrections about specific
    past commits crowded the top-5.
```

**Predictions:**

| # | Config | Prediction |
|---|--------|------------|
| 2.1 | MMR off, rerank off | Schema fact at rank 5 or worse; ≥3 raw `correction` facts above it |
| 2.2 | MMR on, rerank off | Schema rank unchanged (MMR doesn't reweight categories); maybe rank 4 if MMR diversified the corrections enough to surface it |
| 2.3 | MMR on, rerank on | Schema fact at rank 1 or 2 — cross-encoder should match "policy on commit titles" semantically with the schema content much more strongly than with individual corrections |

**What we're testing:** This is a "category" problem more than a "diversity" problem. MMR shouldn't help much (it preserves relevance order within categories). Rerank should help a lot — the schema's wording matches the query semantically; raw corrections describe specific past events.

**Tune verdict if any fail:**
- If 2.3 fails (schema doesn't rise to top-2): rerank may be over-weighting exact keyword matches. Try `model_id: "BAAI/bge-reranker-large"` (larger model, better semantic match). Or raise `score_threshold` to 0.5 to drop weak rerank scores.
- If 2.3 holds AND the agent actually responds with the schema rule (not the raw corrections): MEM-007 is paying off and we should leave it on.

### Sample query 3 — clean case (null hypothesis — no change expected)

```yaml
- query: "what's my preferred Python version"
  session_id: sess-xxxx-3
  judgement: recall_useful
  context: |
    A single `user` fact says "prefers Python 3.12". It already appears
    at rank 1. Nothing competes with it.
```

**Predictions:**

| # | Config | Prediction |
|---|--------|------------|
| 3.1 | MMR off, rerank off | `user` fact about Python 3.12 at rank 1 |
| 3.2 | MMR on, rerank off | `user` fact still at rank 1 (no diversity pressure — there's only one good answer) |
| 3.3 | MMR on, rerank on | `user` fact still at rank 1 (rerank confirms — no reason to change a perfect match) |

**What we're testing:** The change shouldn't regress queries that were already working. If MMR or rerank degrades a clean case, something is wrong with the implementation.

**Tune verdict if any fail:**
- If 3.2 or 3.3 doesn't return the `user` fact at rank 1: there's a regression. Roll back, debug.

---

## Acceptance criteria

A reranking change passes validation when **at least 70% of corpus predictions hold** (i.e., 7 of 10 if the corpus is N=10). Misses must be on the "didn't go far enough" side — never on regressions to baseline. If a previously-clean case (Sample 3 style) regresses, the change fails regardless of how the noise/precision cases performed.

---

## How to run a full cycle

1. **You provide:** 5-10 queries with session IDs + a one-line judgement per query
2. **I add:** a temporary `tracing::info!` block to `MemoryRecall::recall` (or build the eval CLI if we're going to do multiple cycles)
3. **I write:** predictions for each query in `docs/memory-validation-<date>.md`, committed
4. **You run:** restart daemon, fire each query, capture logs
5. **I compare:** prediction-by-prediction, ship-or-tune verdict per query, aggregate verdict for the change
6. **We decide:** ship / tune / roll back

Estimated time:
- Phase 1 (corpus): 15 min of your time
- Phase 2 (predictions): 1 hour of mine, before you run anything
- Phase 3 (logs): 15 min of yours
- Phase 4 (compare): 30 min of mine
- Phase 5 (tune iteration): 30 min per cycle if needed

A full first cycle: half-day of total wall time.

---

## What this method does NOT do

- **It is not a benchmark.** N=10 queries is a sniff test, not a leaderboard. We're checking for regressions and confirming specific knobs do specific things.
- **It does not measure end-user satisfaction.** A correct top-10 might still produce a bad response if the LLM ignores the surfaced facts. That's a different validation problem (one for the response-quality pass, not recall).
- **It does not replace production telemetry.** Long-term, you want metrics like "fraction of sessions where the user issued a correction within 3 turns of recall firing" — implicit signal that recall surfaced wrong content. That's MEM-009-and-beyond territory.

The method here is "is this change safe to ship" — fast, evidence-based, and committed before you run.
