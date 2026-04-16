---
name: competitive-analysis
description: >
  Map the competitive landscape for a company, product, or market
  segment — who competes, on what axes, with what positioning and
  differentiation. Use when the user asks "who competes with X", "what
  is X's moat", "map the competitors in Y market", or wants a
  positioning breakdown. Ingests the full competitor set so the map
  accumulates across sessions.
metadata:
  version: "0.1.0"
---

# Competitive Analysis

Map the competitive landscape for a company, product, or market
segment.

Structural contract: [`../_shared/research_archetype.md`](../_shared/research_archetype.md).
Output syntax: [`../_shared/obsidian_conventions.md`](../_shared/obsidian_conventions.md).

## Use when

- "Who competes with X"
- "Map the <market> landscape"
- "What is X's moat / differentiation / pricing power"
- M&A landscape, share-shift analysis
- Pre-investment / pre-launch competitive sanity checks

## Subject slug

- **Company-anchored** — `<company-slug>-vs-competitors`
  (`stripe-vs-competitors`)
- **Market-anchored** — `<market-slug>`
  (`ai-code-assistants`, `observability-platforms`, `cloud-hyperscalers`)

## Typical artifacts

- `landscape-map.md` — the competitor set with one-line framing per
  player; include a 2×2 positioning diagram description if axes are
  clear
- `positioning-axes.md` — the axes that matter (price, breadth, focus,
  ecosystem, etc.) and where each player sits
- `moats-and-differentiation.md` — per-player moat analysis
- `share-dynamics.md` — who's gaining / losing share and why (if data
  exists)
- `strategic-implications.md` — the "so what" for the user's decision

## Cross-source ingest profile

Ingested to main KG alongside the `research:competitive-analysis:<subject>:<date-slug>`
summary entity:

- **Always** — `organization-<slug>` for every company in the
  landscape (at minimum the 3-8 most material players — do not name
  every minor competitor).
- **When the segment is product-defined** — `product-<slug>` for each
  product in the comparison.
- **When the segment is concept-defined** — `concept-<slug>` for the
  market / category frame (`concept-ai-code-assistant`,
  `concept-cloud-hyperscaler`).

Relationships:
- `about: <subject-entity-id>` from the session summary.
- `competes_with: organization-<slug>` — pairwise edges between every
  two companies in the map. Use the session summary entity as the
  originator (keeps edges attributable to THIS snapshot) or emit
  company-to-company edges directly with `source_session` in the edge
  properties.
- `differentiated_by: concept-<axis-slug>` — the axes that matter.
