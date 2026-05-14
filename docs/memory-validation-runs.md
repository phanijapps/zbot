# Memory Recall Validation — Run Plan

**Purpose:** A test corpus of 10 diverse queries that exercise different parts of the memory pipeline (always-inject corrections, recall + rescore, MMR diversity, intent routing, cross-encoder reranking, KG retention, ward isolation).

**Companion docs:**
- `docs/memory-validation-methodology.md` — the predict-then-measure methodology
- `docs/memory-explained.md` — the system reference

**Baseline runs** (already executed, scored ✓ in earlier verification):
- `sess-013c81f4-39b0-4917-8827-8ceed77ac6b3` — AAPL peer valuation (9.5 min, completed clean)
- `sess-c3f20160-d29f-4399-ba3d-dea3c3a46a30` — ACN peer valuation (9.3 min, 10/10 predictions held, **proven primitive reuse** via `core/yf_helpers.py` + `core/valuation_analysis.py`)

---

## How to use this doc

1. Pick a run from one of the four tiers below.
2. Open a fresh session in the indicated ward (or let the agent decide).
3. Paste the query verbatim.
4. When the session completes, paste the session id back and we score it against the predictions.
5. After every ~5 runs, pause for ≥1 hour to let the sleep cycle (CorrectionsAbstractor + Synthesizer) abstract patterns from the new data.

**Total agent runtime across all 10:** ~90–120 min.

---

## Tier 1 — Reuse tests (validate primitive registry + recall)

Same ward as AAPL/ACN. Tests whether prior work compounds.

### Run 1 — MSFT peer valuation

```
Analyze MSFT and compare it with its peers and tell me if it is overvalued or not with html slide deck and charts
```

**Ward:** `financial-analysis`
**Predicted intent:** procedural or no-confident-match (defaults)
**Memory dependency:** Should fully reuse `core/yf_helpers.py` + `core/valuation_analysis.py` from AAPL/ACN runs.

**Predictions:**
- Wall time **< 9.3 min** (ACN baseline) — compounding effect of the third reuse
- Builder Step 2 imports: `from core.yf_helpers import fetch_ticker_info, extract_valuation_metrics`
- Builder Step 3 imports: `from core.valuation_analysis import compute_peer_stats, build_chart_configs`
- Peers researched fresh (MSFT's are Big Tech: AAPL, GOOG, META, AMZN — not the same as ACN's IT Services peers)
- Zero "Task too large" failures
- Verdict produced (overvalued / fair / undervalued)

**Diagnostic if any prediction misses:**
- Slow run + no `from core.*` imports → recall isn't surfacing the primitive registry; need to bump category weight for `primitive` or fix ward affinity
- Same-style failures as pre-fix sessions → check whether daemon picked up the latest build

### Run 2 — TSLA historical valuation (new shape)

```
Compare TSLA's valuation across 2020–2025 and identify when it was most overvalued vs its peers
```

**Ward:** `financial-analysis`
**Predicted intent:** procedural / domain-question
**Memory dependency:** Reuse `core/yf_helpers.py` for data fetching, BUT requires writing a NEW primitive for temporal/historical analysis (snapshot funcs in `core/valuation_analysis.py` aren't shaped for this).

**Predictions:**
- `yf_helpers` reused; agent fetches historical price + metrics ranges
- Planner identifies this as a "delta" classification (new shape) not "build" (greenfield)
- A new primitive (likely `core/historical_analysis.py`) gets created AND registered in `memory-bank/core_docs.md`
- Wall time longer than Run 1 (5-15% premium for new primitive design)
- The "never rely on LLM training data" correction is critical — TSLA peers in 2020 differ from 2025; agent must fetch real data

**Diagnostic if any prediction misses:**
- Agent re-implements the snapshot pattern with hardcoded date filtering → it didn't recognize the shape difference; planner-agent needs help recognizing delta vs build
- Agent fabricates 2020 metrics from training data → "never rely on LLM training" correction wasn't surfaced or followed

---

## Tier 2 — Cross-ward isolation (validate ward affinity)

Different wards. Tests that financial corrections DON'T leak into unrelated work.

### Run 3 — Chain rule lesson plan ⬅ **STARTED HERE**

```
Create a lesson plan teaching the chain rule of calculus to high school students with interactive HTML examples and animations
```

**Ward:** `homework-help` (or fresh)
**Predicted intent:** procedural / how-to
**Memory dependency:** Cold start for the topic. Should reuse the slide-deck construction PATTERN (different from `financial-analysis`'s specific primitives) but no math-specific primitives exist yet.

**Predictions:**
- Recall surfaces general workflow corrections (planner-agent, research-agent, etc.) but NOT financial corrections
- If recall returns any `yfinance`, `peer stats`, `valuation` content → **ward affinity is leaking** (a real bug, log it)
- Planner-agent creates a fresh spec in `wards/homework-help/...` or wherever this ward lives
- Builder constructs an HTML deck with interactive elements (likely Chart.js or vanilla JS)
- Math notation rendered correctly (MathJax/KaTeX) OR rendered as styled HTML
- The "Each delegation task must produce ONE output file" correction shapes the build into discrete steps

**Diagnostic if any prediction misses:**
- Financial corrections appear in recall → ward affinity broken in the always-inject path → file MEM-009 (ward filter on corrections)
- Math notation breaks → that's a skill gap, not a memory issue

### Run 4 — WW1 deck with primary sources

```
Build a slide deck explaining the causes of WW1 with primary source citations, suitable for AP World History
```

**Ward:** `homework-help` (or `history-research`)
**Predicted intent:** procedural / domain-question
**Memory dependency:** Should reuse general slide-builder pattern. Heavy research-agent path.

**Predictions:**
- `research-agent` invoked at least twice (one per major historical claim)
- Citations include real URLs from real sources (NOT placeholder "TODO: cite this")
- The "never rely on LLM training data" correction enforces the citation discipline
- Slide deck consistent style with prior decks (5-10 slides, navigation, ADHD-friendly visuals)

**Diagnostic if any prediction misses:**
- Citations fabricated → "never rely on training data" correction wasn't surfaced/followed → memory issue OR LLM ignored the fact
- No `research-agent` invocation → skill discovery failure → unrelated to memory

---

## Tier 3 — Intent variety (validate intent router)

Each query has a distinct intent. Tests the kNN classifier + per-intent profiles.

### Run 5 — Rust CLI tool (code-help intent)

```
Write a Rust CLI tool that watches a directory and emits SHA256 of new files to stdout. Cross-platform (Linux + macOS).
```

**Ward:** any (probably `code` or root)
**Predicted intent:** `code-help`
**Memory dependency:** Cold for this specific tool. Should surface generic Rust patterns from prior code work (CLI argument parsing conventions, tokio patterns if observed in memory).

**Predictions:**
- Intent classifier picks `code-help` → profile bumps `pattern` weight to 1.4, max_hops to 3
- Recall surfaces Rust-related `pattern` facts if any exist
- Builder produces single-file CLI or small module
- Cross-platform notes (`notify` crate or similar) included

**Diagnostic if any prediction misses:**
- Intent classified as something else (factoid? procedural?) → check confidence_threshold setting; might need more exemplars in the code-help bucket
- No Rust-specific facts surfaced → either no such facts exist in memory yet (cold start, expected) OR they exist but recall didn't match

### Run 6 — Factoid lookup

```
What model did I set the orchestrator to use, and why did I pick that one?
```

**Ward:** any
**Predicted intent:** `factoid-lookup`
**Memory dependency:** Should retrieve the orchestrator setting from prior sessions/configs. The "why" part requires user-fact memory.

**Predictions:**
- Intent classifier picks `factoid-lookup` → profile bumps `domain` to 1.5, drops `correction` to 0.8, disables graph traversal
- Agent answers directly from memory (current setting in `settings.json`: `glm-5-turbo` via `provider-z.ai`)
- "Why" requires a `user` category fact about model choice — likely absent → agent should say "you haven't told me why"
- Response is SHORT (factoid-style) — not a 2-page essay
- Always-inject corrections appear but don't dominate (since profile suppresses them)

**Diagnostic if any prediction misses:**
- Agent guesses or generates plausible-sounding answer instead of saying "I don't know why" → "never rely on training data" correction failed to constrain
- Response is verbose with lots of unrelated corrections → intent router didn't fire OR `factoid-lookup` exemplars don't match this query

### Run 7 — Correction recall

```
Did I tell you not to use raw curl for web scraping? If so, what's the alternative?
```

**Ward:** any
**Predicted intent:** `correction-recall`
**Memory dependency:** Direct test of correction retrieval.

**Predictions:**
- Intent classifier picks `correction-recall` → profile cranks `correction` to 2.5, larger top-K
- Recall surfaces the existing correction: *"Use duckduckgo-search and light-panda-browser skills for web research. Never use raw shell curl or wget for web scraping..."*
- Agent answers: yes you did, use duckduckgo-search / light-panda-browser
- Response cites the specific tools

**Diagnostic if any prediction misses:**
- Agent says "no" or fabricates an answer → correction didn't surface in recall (despite being conf=1.00 in memory) → recall pipeline bug
- Agent answers correctly but doesn't name the tools → recall surfaced something tangential

### Run 8 — Procedural / setup walkthrough

```
Walk me through how to set up a new agent in this codebase from scratch — config files, agent prompt, ward, and a test session.
```

**Ward:** any (codebase-aware)
**Predicted intent:** `procedural`
**Memory dependency:** Heavy on `memory-bank/` docs + agent registry. Should reuse patterns from prior agent setups.

**Predictions:**
- Intent classifier picks `procedural` → profile boosts `strategy` + `pattern`
- Agent surfaces existing agent setup patterns from `memory-bank/agents.md` or similar
- Multi-step response with concrete file paths
- Includes a "test it" step at the end (matches the procedural taxonomy in the exemplar bank)

**Diagnostic if any prediction misses:**
- Vague generic answer → intent classifier didn't route, OR no procedural facts exist in memory
- Real file paths but wrong/stale → KG entities about agents/wards are out of date

---

## Tier 4 — Noise + diversity tests (validate MMR + sleep cycle)

Run these AFTER Tier 1–3 have accumulated corrections + schemas. The CorrectionsAbstractor cycle should have run at least once.

### Run 9 — Multi-theme query (MMR stress test)

```
Build a quick visualization showing CPU and memory usage of the zbot daemon over the last hour. Should refresh live.
```

**Ward:** any
**Predicted intent:** procedural / code-help
**Memory dependency:** Touches multiple themes — "visualization" (slide builder), "monitoring" (likely new), "live refresh" (UI pattern).

**Predictions:**
- Multi-facet recall — at least 3 distinct categories in top-10 (not all `pattern` or all `correction`)
- MMR diversifies the top-K — if MMR is OFF you'd see clustering; with it ON you should see breadth
- Agent likely uses `tracing` logs + a small Python/Rust HTML page polling
- Does NOT default to producing a financial-style HTML deck (that would indicate template-lock-in)

**Diagnostic if any prediction misses:**
- Top-K all in one category → MMR isn't running, OR `mmr.lambda` is too high (favors relevance too much) → drop `lambda` from 0.6 to 0.4
- Agent produces a financial-deck-styled output → over-fitting to past patterns; needs intent routing to detect "this is monitoring, not finance"

### Run 10 — Self-referential summary

```
I'm going to give a 10-minute talk on "what I built this month with zbot." Help me outline the most interesting wins from my memory.
```

**Ward:** any (probably root)
**Predicted intent:** uncertain — might route to `correction-recall`, `procedural`, or default
**Memory dependency:** **Maximum** — requires correct recall of past sessions, schemas, recurring patterns, completed features.

**Predictions:**
- Agent surfaces real wins from prior sessions (AAPL/ACN/MSFT decks, math lessons, code tools, agent setups)
- NOT generic platitudes ("you built things")
- Cites specific accomplishments with timestamps or session refs
- Likely references handoff facts from prior session summaries

**Diagnostic if any prediction misses:**
- Generic / vague summary → handoff facts aren't surfacing OR not enough sessions have produced rich handoffs yet
- Specific but inaccurate facts → recall is mixing facts from different sessions → may need stricter ward filtering

---

## Run order recommendation

| Order | Run | Reason for placement |
|---|---|---|
| 1 | Run 1 (MSFT) | Lowest-variance test — should "just work", gives baseline confidence |
| 2 | Run 5 (Rust CLI) | First code-help — fresh skill area, no reuse expected |
| 3 | Run 3 (chain rule) ⬅ STARTED HERE | First cross-ward — checks isolation |
| 4 | Run 7 (correction recall) | Direct intent test — fastest signal whether intent routing matters |
| 5 | Run 6 (factoid lookup) | Same — short answer, easy to score |
| --- | *Pause ≥ 1 hour for sleep cycle* | Lets CorrectionsAbstractor + Synthesizer process accumulated data |
| 6 | Run 2 (TSLA historical) | Re-tests financial ward after more memory state has accumulated |
| 7 | Run 4 (WW1) | Research-heavy, tests web research path |
| 8 | Run 8 (agent setup procedural) | Tests procedural intent on real codebase knowledge |
| 9 | Run 9 (zbot monitor) | Multi-theme query — MMR validation |
| 10 | Run 10 (self-summary) | Stress test — end-to-end memory recall |

**You started with Run 3 — that's fine.** Run order matters only for the sleep-cycle pause (Runs 1-5 first to accumulate facts, then pause, then Runs 6-10). Run 3 is in the first group either way.

---

## What to capture per run

For each completed run, paste back:
1. Session id
2. (Optional) your subjective verdict: useful / noisy / missed something / surprised

I'll then pull the database and produce:
- ✓ / ✗ per prediction with evidence
- Wall time vs baseline
- Reuse evidence (specific import lines or file references)
- Token cost
- Any failures (cascading or self-recovered)
- 1-2 sentence final deliverable quality

After 5 runs we'll have enough data to decide: ship the rerank stack as-is, tune a knob, or file a specific gap as MEM-009/010.

---

## Housekeeping

**Before Run 6 (after the 1-hour pause):** restart the daemon **once** with `rerank.enabled: true` in `~/Documents/zbot/config/settings.json`. The cross-encoder will trigger its model download on first recall (~280MB, one-time). That lets Runs 6-10 exercise the full rerank stack including the cross-encoder.

**If a run takes much longer than expected** (>15 min): check for cascading retries. With the 4000-char fix in place, this should be rare, but if it happens, send the session id immediately so we can diagnose.

**If a run completes but feels off** (the response was wrong in some way you can articulate): note WHY in your reply. Subjective signal is valuable when the numbers say "predictions held."
