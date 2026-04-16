---
name: product-research
description: >
  Evaluate and compare products — consumer tech, software, gear,
  services. Use when the user asks "which X should I buy", "compare
  A vs B", "research the best X for Y", or wants a buying decision
  backed by specs, reviews, and trade-offs. Ingests product entities
  and their makers so comparisons accumulate.
metadata:
  version: "0.1.0"
---

# Product Research

Evaluate or compare products to support a buying / adoption decision.

Structural contract: [`../_shared/research_archetype.md`](../_shared/research_archetype.md).
Output syntax: [`../_shared/obsidian_conventions.md`](../_shared/obsidian_conventions.md).

## Use when

- "Which X should I buy"
- "Compare A vs B" / "A vs B vs C"
- "Research the best X for Y use-case"
- Shortlist narrowing before a purchase
- Spec sheets / benchmarks for a product category

## Subject slug

Kebab-case. Two forms:
- **Single product** — `<product-slug>` (`macbook-pro-m5`,
  `framework-laptop-16`).
- **Comparison / category** — `<category-slug>` when the session
  evaluates multiple products in a category (`ultralight-laptops`,
  `mechanical-keyboards`, `ev-suvs-2026`).

## Typical artifacts

- `shortlist.md` — the 2-5 products considered, with one-line framing
- `spec-comparison.md` — feature/spec matrix
- `reviews-summary.md` — distilled third-party reviews with source links
- `trade-offs.md` — the honest pros/cons that matter for the use-case
- `recommendation.md` — the pick and why

## Cross-source ingest profile

Ingested to main KG alongside the `research:product-research:<subject>:<date-slug>`
summary entity:

- **Per product in shortlist** — `product-<slug>` entity. Properties:
  `maker` (→ `organization-<slug>`), `category`, `price_band`,
  `vault_path`.
- **Per maker** — `organization-<slug>` for every company whose product
  is in the shortlist (Apple, Framework Computer, Logitech).
- **Per benchmark source** — `work-<benchmark-slug>` for recognized
  benchmarks cited (PassMark, Cinebench, DXOMARK) when they materially
  drive the comparison.
- **Skip** — specific SKUs / model variants beyond what's in the
  shortlist, and reviewer-specific scores that aren't reusable.

Relationships:
- `about: product-<primary-slug>` OR `about: concept-<category-slug>`
  from the session summary.
- `evaluates: product-<slug>` for each product in the shortlist.
- `recommends: product-<slug>` for the chosen one (one per session).
