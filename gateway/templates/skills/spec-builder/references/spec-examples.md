# spec-builder — worked examples

One example per classification. Load the relevant section when you are unsure of the spec template shape.

---

## skill_match

**Prompt:** "Search the web for today's weather in Tokyo and report it."

**Returned spec:**

````
=== FILE: spec.md ===
# Spec: Report today's weather in Tokyo

**Established:** 2026-04-18
**Classification:** skill_match

## Inputs snapshot
- prompt: provided
- intent: provided
- context: missing
- state: missing
- suggested_skills: provided

## Problem statement
> Search the web for today's weather in Tokyo and report it.

## Intent
Return the current weather conditions in Tokyo to the user in plain text.

## Context summary
- none — no context provided

## Suggested skills (from orchestrator)
- web_search: handles the whole ask (query → parse → summarize)

## Additional skills required
- none

## Raw tools required
- none — prefer skills

## Capability gaps
- none

## Assumptions
- "today" means the planner agent's current date in Tokyo's timezone.
- User wants temperature, conditions, and maybe precipitation — not a full forecast.

## Success criteria
- The reply names Tokyo's current temperature and conditions for today.
- The source is a publicly accessible weather service.

## Approach
One skill invocation. The web_search skill retrieves current weather for Tokyo and formats the response. No decomposition needed.

## Phases
- Phase A — Query weather — web_search

## Phase dependency graph

```
A
```

## Pre-mortem
1. Weather service rate-limits — mitigation: web_search falls back to a second source.
2. Ambiguous "today" across timezones — mitigation: Assumption above pins the interpretation.

## Steps — PENDING (plan-composer fills this)
````

---

## research

**Prompt:** "Research the top 5 graph databases and recommend one for our workload."

**Returned spec:**

````
=== FILE: spec.md ===
# Spec: Recommend a graph database for our workload

**Established:** 2026-04-18
**Classification:** research

## Inputs snapshot
- prompt: provided
- intent: provided
- context: provided
- state: missing
- suggested_skills: provided

## Problem statement
> Research the top 5 graph databases and recommend one for our workload.

## Intent
Produce a recommendation backed by a comparison of the five leading graph database options.

## Context summary
- Workload is read-heavy with ~10M nodes and ~50M edges.
- Existing stack runs on AWS; self-hosted is acceptable.
- Budget is modest; managed offerings preferred but not required.

## Suggested skills (from orchestrator)
- web_search: find current leaders and feature comparisons
- summarize_sources: synthesize findings into a comparison

## Additional skills required
- none

## Raw tools required
- none — prefer skills

## Capability gaps
- none

## Assumptions
- "Top 5" means widely-used and actively-maintained, not a specific benchmark ranking.
- Recommendation must fit the context's AWS / read-heavy / modest-budget constraints.

## Success criteria
- A shortlist of exactly 5 databases with source URLs.
- A comparison matrix covering: read performance, ops model, license, pricing, community size.
- A single recommendation with a one-paragraph justification tied to the context.

## Approach
Gather candidates and current feature data with web_search (parallel-safe on the 5 candidates). Synthesize into a comparison matrix. Write a recommendation memo tied to the workload constraints.

## Phases
- Phase A — Gather candidate data — web_search (one pass per candidate, parallel-safe)
- Phase B — Build comparison — summarize_sources
- Phase C — Write recommendation — summarize_sources

## Phase dependency graph

```
A ──▶ B ──▶ C
```

## Pre-mortem
1. Comparison matrix becomes a feature dump with no pointed conclusion — mitigation: Phase C is explicit about tying the recommendation to workload constraints.
2. Sources are stale or marketing-heavy — mitigation: Phase A prefers recent independent sources and cites each row in the matrix.

## Steps — PENDING (plan-composer fills this)
````

---

## build

**Prompt:** "Generate a 10-page competitive analysis report on our Tier-2 SaaS competitors."

**Returned spec:**

````
=== FILE: spec.md ===
# Spec: 10-page competitive analysis of Tier-2 SaaS competitors

**Established:** 2026-04-18
**Classification:** build

## Inputs snapshot
- prompt: provided
- intent: provided
- context: provided
- state: missing
- suggested_skills: provided

## Problem statement
> Generate a 10-page competitive analysis report on our Tier-2 SaaS competitors.

## Intent
Deliver a written competitive analysis document covering the Tier-2 competitors named in context, approximately 10 pages long.

## Context summary
- Tier-2 competitors: CompA, CompB, CompC, CompD.
- Audience: product leadership.
- Prior analyses exist for Tier-1 competitors and set the expected format.

## Suggested skills (from orchestrator)
- web_search: fetch product, pricing, and market-position data
- summarize_sources: compress findings per competitor
- long_form_writer: produce the final report

## Additional skills required
- none

## Raw tools required
- none — prefer skills

## Capability gaps
- none

## Assumptions
- "10 pages" means roughly 3000-4000 words in markdown; exact pagination depends on rendering.
- Tier-2 competitors are those named in context, not a fresh market scan.

## Success criteria
- One markdown report of approximately 10 pages (3000-4000 words).
- Each competitor has dedicated sections: offering, pricing, positioning, strengths, weaknesses.
- Closing section with cross-competitor takeaways and strategic implications.
- Every factual claim has a citation.

## Approach
Fetch per-competitor data (parallel). Compress each competitor's findings. Write the full report tying comparisons together. No format-conversion step — prompt asks only for the report.

## Phases
- Phase A — Fetch per-competitor data (parallel) — web_search
- Phase B — Compress each competitor's findings — summarize_sources
- Phase C — Write integrated report — long_form_writer

## Phase dependency graph

```
A ──▶ B ──▶ C
```

## Pre-mortem
1. Report reads as four separate profiles with no cross-competitor insight — mitigation: Phase C explicitly requires cross-competitor takeaways.
2. Citation quality drifts — mitigation: Phase B preserves source URLs; Phase C must retain them.

## Steps — PENDING (plan-composer fills this)
````

---

## delta

**Scenario:** The `research` example above has already been planned. The user follows up with: "Actually focus just on open-source options."

**Returned spec (full; delta updates):**

````
=== FILE: spec.md ===
# Spec: Recommend an open-source graph database for our workload

**Established:** 2026-04-18
**Classification:** delta

## Inputs snapshot
- prompt: provided
- intent: provided
- context: provided
- state: provided (prior spec: "Recommend a graph database for our workload")
- suggested_skills: provided

## Problem statement
> Actually focus just on open-source options.

## Intent
Produce the recommendation using only open-source graph databases; update the prior spec in place.

## Context summary
- Baseline spec: "Recommend a graph database for our workload" (established 2026-04-18).
- Workload, AWS / read-heavy / modest-budget constraints carried over.
- Scope narrowed to open-source candidates only.

## Suggested skills (from orchestrator)
- web_search: find current open-source leaders
- summarize_sources: rebuild comparison focused on OSS options

## Additional skills required
- none

## Raw tools required
- none

## Capability gaps
- none

## Assumptions
- "Open-source" includes permissive and copyleft — user can narrow further if needed.
- Managed-service column in the comparison shifts to "hosted-OSS option available?".

## Success criteria
- Shortlist of up to 5 actively-maintained open-source graph databases.
- Comparison matrix restructured: license, hosted-OSS option, read performance, ops model, community size.
- Single recommendation for the open-source subset.

## Approach
Re-run candidate gather with an OSS-only filter. Rebuild the comparison matrix with updated columns. Rewrite the recommendation to the new scope.

## Phases
- Phase A — Gather OSS candidates — web_search
- Phase B — Rebuild comparison — summarize_sources
- Phase C — Write recommendation — summarize_sources

## Phase dependency graph

```
A ──▶ B ──▶ C
```

## Pre-mortem
1. OSS projects with thin enterprise backing mis-scored on reliability — mitigation: Phase B's ops-model column flags this explicitly.
2. Hosted-OSS offerings (e.g., managed Neo4j Community) confused with proprietary — mitigation: matrix column is "hosted-OSS option available?" not "managed service".

## Steps — PENDING (plan-composer fills this — only changed phases re-decomposed)
````
