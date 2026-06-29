# Memory Data Quality Assessment

**Date:** 2026-06-29 · **DB:** `~/Documents/zbot/data/knowledge.db` · **Method:** first-hand sample reading + quantitative quality checks (1,217 facts, 509 entities, 828 relationships).

## Verdict (one line each)

- **`memory_facts`: MIXED — ~70% genuine durable knowledge, ~30% extractor noise / duplication / raw residue; confidence calibration has collapsed (everything reads as "certain").**
- **Knowledge graph: MEDIUM — semantically richer than the facts (calibrated confidence, specific typed edges), but ~10–15% junk entities, flat at the relationship level, 37% orphans, 18% vague edges, and a type-casing bug.**

The signal is real and valuable in both — the problem is **dilution by extractor artifacts**, not absence of knowledge. This directly feeds the "hit and miss" recall problem: noisy facts/entities crowd out good ones, and the `min_score=0.3` floor is partly compensating for noise it can't distinguish.

---

## memory_facts (1,217)

### Strengths
- The **knowledge categories are genuinely good**: `correction`, `instruction`, `strategy`, `pattern`, `user`, `agent`, `skill` hold specific, reusable, accurate content — e.g. *"Brave Search Free plan: 1 req/s, 2000/month quota; parallel searches trigger 429 → space sequentially"*, *"Shell redirects for writing files are blocked; use write_file"*, *"use 'value streams'/'capabilities' not 'repos'"*. Real, well-formed.
- Bi-temporal working (49 superseded); provenance columns (`source_summary`, `source_episode_id`) populated.

### Problems (with evidence)
1. **Confidence is uncalibrated.** mean **0.92**, p50 **0.95**, **311 facts at exactly 1.0**, only **3 below 0.6**. When everything is "confident," confidence carries no ranking/trust signal. *(Contrast: KG confidence mean 0.75 — calibrated.)*
2. **Extractor noise — code symbols stored as facts.** `primitive` category: `TagChecker.handle_endtag(self, tag)`, `fetch_ticker(tkr)`, `json_safe(value)`, and `main()` appears **13×**. `ward` category stores directory names: `.venv`, `.node_env`, `system-maintenance`. These aren't facts.
3. **Duplication (~12%).** **49 duplicate groups covering 141 facts** — code tokens (`main()`×13, `fetch_ticker`×3) and raw episode-summary JSON blobs repeated.
4. **Raw residue as facts.** `ctx` category stores verbatim `{"summary":"..."}` JSON and `--- execution_id: … ---` handoff dumps. Not well-formed knowledge.
5. **Length extremes.** **172 facts >500 chars** (the `FACT_TOO_LONG` failures), max **10,871 chars** (a giant dump); **28 facts <20 chars** (junk tokens).
6. **Taxonomy inconsistency.** `category` × `epistemic_class` misaligned: `correction` is a category (50 rows) but the `correction` epistemic_class has **1** row (corrections are split current=18 / procedural=17); `domain` is scattered across archival=263 / current=190 / convention=40. The two axes aren't applied consistently.
7. **Write-once, never reinforced.** **1,069 / 1,217 (88%)** have `mention_count=1`. Most facts are stored once and never re-encountered/verified.

---

## Knowledge graph (509 entities, 828 relationships)

### Strengths
- **Relationship semantics are specific and mostly meaningful**: `part_of`(131), `uses`(126), `analyzedby`(89), `created`(43), `peerof`(26), `preceded_by`(16), `author_of`(16), `cites`(9). Samples are legitimate: `AGENTS.md --part_of--> z-Bot`, `CoALA --analyzedby--> agent-memory-framework`, `plan-composer --after--> spec-builder`.
- **Confidence is calibrated** (unlike facts): mean **0.75**, range **0.16–0.80**, **none ≥0.9**.
- Domain-appropriate typing for the financial ward (ticker entities: BP, XOM, CVX, …).
- Hierarchy partially built on **entities** (layers 0–4: 374/64/45/22/4).

### Problems (with evidence)
1. **Code/path/token garbage as entities.** `/*`, `/>`, `/api`, `/api/health`, `/api/curator/cleanup`, `/tmp/zbot-*`, `./AGENTS.md`, `./housekeeping.sh\`` (with a trailing backtick), full attachment file paths — all typed `file`. ~10–15% of entities are URL/path/code-syntax leakage.
2. **Relationship hierarchy is flat.** **All 828 relationships are layer 0; 0 are inter-cluster.** Entities were layered (0–4) but relationships weren't → inter-cluster recall cannot work (confirms gap D3).
3. **37% orphan entities** (186/509 have no relationships) — significant isolation.
4. **18% vague edges** — `related_to`(82) + `mentions`(69) + associated = **151**. e.g. `Microsoft --related_to--> ORCL` is low-signal/spurious.
5. **Type-casing bug.** `Concept`(135) vs `concept`(77) — **every** type is split by case (212 "concepts" fragmented). Extractor doesn't normalize `entity_type`.
6. **Entity-resolution gap.** **18 name-duplicate groups** (same name → multiple entity ids despite the alias table).
7. **20 `unknown`-type** entities (unclassified).

---

## Root cause

Both stores share one upstream cause: **the extraction layer (distillation + knowledge-graph extractor) is too permissive** — it ingests code symbols, file paths, URLs, raw JSON, and directory names as first-class facts/entities. The noise is generated at write time; downstream (recall, ranking, `min_score`) is left to compensate. This matches the project principle *fix the source, not the symptom* ([[feedback_fix_source_not_symptom]]).

Secondary: weak dedup/normalization at write, and a confidence-assignment rule on facts that defaults high.

---

## Recommended quality fixes (additive — all land in the consolidation crate / Spec 4)

| # | Fix | Target | Effect |
|---|---|---|---|
| Q1 | **Extraction filters** — reject/ down-rank code symbols (regex `^[A-Za-z_]\w*\([^)]*\)$`), file paths, URLs, dir names, raw JSON, and <N-char tokens at write time. | KG extractor (`services/knowledge-graph`), distillation fact writer | Removes ~10–15% KG junk + `primitive`/`ward` fact noise |
| Q2 | **Dedup at write** — normalized-content key for facts (already have `normalized_hash` on entities; extend to facts); tighten entity resolution. | fact store + KG resolver | Kills the 49 dup groups / 141 facts + 18 name dup groups |
| Q3 | **Recalibrate fact confidence** — lower default for write-once/unverified facts toward the KG's ~0.75 shape; reserve ≥0.9 for multi-source/reinforced. | fact writer | Restores confidence as a ranking signal |
| Q4 | **Normalize `entity_type` casing** at extraction. | KG extractor | Collapses Concept/concept split |
| Q5 | **Wire inter-cluster relationship synthesis** (D3) + **prune/link orphans**. | hierarchy_builder, consolidation | Fixes flat-relationship hierarchy; reduces 37% orphans |
| Q6 | **Taxonomy discipline** — pick one axis (category OR epistemic_class) as primary; map the other deterministically. | fact writer | Removes category×epistemic inconsistency |

These are Consolidation-pipeline concerns → they belong in **Phase 2 (`zbot-consolidation`)** of the modularization plan and **Spec 4 (reconcile stores)** of the behavior specs. They are the *quality* counterpart to the structural work: modularize the pipeline (structure) + tighten extraction (quality).

---

*Cross-references: `2026-06-29-memory-layer-data-dictionary.md` · `2026-06-28-memory-application-gap-analysis.md` · `2026-06-29-memory-layer-modularization-plan.md` · [[project_memory_application_gaps]] · [[project_memory_modularization]]*
