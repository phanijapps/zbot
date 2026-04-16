---
name: market-research
description: >
  Industry / market research — size, growth, segmentation, trends,
  major players, structural dynamics. Use when the user asks "size the
  X market", "TAM for Y", "trends in Z industry", or wants a
  macro-to-micro view of a sector. Ingests the industry concept and
  major players so market snapshots accumulate over time.
metadata:
  version: "0.1.0"
---

# Market Research

Size, shape, and track a market or industry. Macro view — not a single
product or company.

Structural contract: [`../_shared/research_archetype.md`](../_shared/research_archetype.md).
Output syntax: [`../_shared/obsidian_conventions.md`](../_shared/obsidian_conventions.md).

## Use when

- "How big is the X market" / "TAM for Y"
- "Trends in the Z industry"
- "Size the opportunity for X"
- Pre-strategy / pre-investment industry briefing
- Segmentation analysis (who buys what, why)

## Subject slug

Kebab-case industry / market slug:
- `ev-passenger-vehicles`
- `ai-coding-assistants`
- `cloud-infrastructure`
- `electronic-health-records`
- `direct-to-consumer-fitness`

## Typical artifacts

- `market-size.md` — TAM / SAM / SOM with assumptions shown, or the
  best available sizing estimate with source
- `segmentation.md` — how the market breaks down: by customer, by
  use-case, by geography, by price band
- `growth-drivers.md` — what's expanding the market, what's shrinking it
- `players-and-share.md` — who's in the market and rough share split
- `trends-and-outlook.md` — 1-3-5 year forward view
- `structural-dynamics.md` — margin structure, moats, regulation, unit
  economics

## Cross-source ingest profile

Ingested to main KG alongside the `research:market-research:<subject>:<date-slug>`
summary entity:

- **Always** — `concept-<industry-slug>` for the market itself.
  Properties: `market_size_usd`, `size_source`, `cagr`, `date_sized`,
  `vault_path`.
- **Per major player** — `organization-<slug>` for each company
  materially shaping the market (top ~5-10 by share or strategic
  relevance). Do NOT enumerate every niche player.
- **Per segment** — `concept-<segment-slug>` for material segments
  (`concept-enterprise-saas`, `concept-consumer-mobility`) when the
  segmentation is structural rather than ad-hoc.

Relationships:
- `about: concept-<industry-slug>` from the session summary.
- `participant_in: concept-<industry-slug>` from each major-player
  organization to the market concept.
- `segment_of: concept-<industry-slug>` from each segment concept to
  its parent market.
