---
name: stock-analysis
description: >
  Equity research on a public company — valuation, options, financials,
  fundamentals. Use when the user asks to analyze, value, or take a
  position on a stock, run DCF/multiples, review earnings, or
  investigate options flow. Produces a dated, vault-ready session folder
  and ingests the ticker's organization + key execs into the main
  knowledge graph so snapshots accumulate over time.
metadata:
  version: "0.1.0"
---

# Stock Analysis

Equity / options / fundamentals research on a public company at a point
in time.

Structural contract: [`../_shared/research_archetype.md`](../_shared/research_archetype.md).
Output syntax: [`../_shared/obsidian_conventions.md`](../_shared/obsidian_conventions.md).

## Use when

- The user asks to analyze, value, or take a view on a ticker
- "Run a DCF on X", "options flow on X", "is X a buy", "review X's Q3"
- Pre- or post-earnings review
- Position sizing / hedging decisions for a named equity

## Subject slug

The ticker, lowercased (`tsla`, `aapl`, `googl`, `brk-b`). If the user
refers to a private company or ADR without a common ticker, use the
company slug (`stripe`, `spacex`).

## Typical artifacts

Pick the subset that matches the session — don't force all of them:

- `valuation-summary.md` — DCF, multiples, fair-value range
- `options-analysis.md` — flow, skew, implied vol, strategy
- `financials-review.md` — income/balance/cash-flow walk
- `catalysts-and-risks.md` — forward catalysts, downside cases
- `peer-comparison.md` — relative valuation vs comparables
- `final-outcomes.md` — verdict, position sizing, stops/targets

## Cross-source ingest profile

Ingested to main KG alongside the `research:stock-analysis:<ticker>:<date-slug>`
summary entity:

- **Always** — `organization-<ticker-slug>` (Tesla Inc., Apple Inc.).
  Properties: `ticker`, `sector`, `industry`, `role_in_research: subject`.
  Evidence: `_index.md` + lead artifact.
- **When central** — `person-<exec-slug>` for CEO, CFO, or execs whose
  statements / track record materially shape the thesis.
- **When material** — `product-<slug>` or `concept-<slug>` for products
  or concepts load-bearing to the thesis (FSD, Optimus, Azure, Vision
  Pro).
- **Skip** — per-quarter metrics (17.3% gross margin, $2.1B revenue).
  Those are ephemeral; they live in the artifact files, not the main
  graph.

Relationships: `about: organization-<ticker-slug>` from the session
summary. Optional `mentions: person-<exec-slug>` for execs referenced.

## Date-slug conventions

- Earnings-anchored: `-q1`, `-q2`, `-q3`, `-q4`, `-fy<YY>`.
- Event-anchored: `-pre-earnings`, `-post-earnings`, `-cmd` (capital
  markets day), `-ir-call`.
- Intra-day: `-morning`, `-midday`, `-close`, `-after-hours`.
- Default: bare ISO date when the snapshot isn't anchored to an event.
