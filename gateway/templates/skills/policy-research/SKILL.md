---
name: policy-research
description: >
  Research regulatory / legal / public policy on a specific topic,
  jurisdiction, or agency action. Use when the user asks "what are the
  rules on X", "how is Y regulated in jurisdiction Z", "what's the
  status of <bill/regulation>", or wants the law-on-the-ground view
  before making a compliance or strategy call. Ingests agencies,
  officials, and concrete policies so the regulatory picture accumulates.
metadata:
  version: "0.1.0"
---

# Policy Research

Regulatory / legal / public-policy research on a specific topic,
jurisdiction, or agency action.

Structural contract: [`../_shared/research_archetype.md`](../_shared/research_archetype.md).
Output syntax: [`../_shared/obsidian_conventions.md`](../_shared/obsidian_conventions.md).

**Note:** outputs are informational, not legal advice. Artifacts must
flag when a question genuinely requires a licensed attorney or
compliance professional.

## Use when

- "What are the rules on X"
- "How is Y regulated in <jurisdiction>"
- "What's the status of <bill / regulation / executive order>"
- Pre-launch compliance scoping for a product / feature
- Tracking a specific regulatory development over time

## Subject slug

Kebab-case — pick the most stable anchor:
- Regulation / bill — `eu-ai-act`, `sec-rule-10b-51`,
  `california-sb-1047`
- Topic + jurisdiction — `data-privacy-california`,
  `ai-safety-federal-us`
- Agency action — `ftc-consent-decree-meta-2026`

## Typical artifacts

- `summary.md` — the state of the law / rule today, in plain language
- `timeline.md` — how it evolved (dated events, with citations)
- `obligations.md` — what entities are required to do, for whom
- `enforcement.md` — how it's enforced, penalty structure, track record
- `open-issues.md` — ambiguities, pending litigation, scheduled
  rulemakings
- `implications.md` — practical impact for the user's situation (with
  the "consult counsel" flag where applicable)

## Cross-source ingest profile

Ingested to main KG alongside the `research:policy-research:<subject>:<date-slug>`
summary entity:

- **Always** — `concept-<policy-slug>` for the regulation / rule /
  policy itself. Properties: `jurisdiction`, `effective_date`,
  `status` (`enacted` | `proposed` | `under-review` | `enjoined` |
  `superseded`), `vault_path`.
- **Per agency / authority** — `organization-<slug>` for each agency,
  regulator, or court materially involved (SEC, EU Commission, CJEU,
  FTC, CFPB).
- **Per material official** — `person-<slug>` for officials whose
  positions shape enforcement (agency chair, rapporteur, judge) —
  only when their identity meaningfully affects outcomes.
- **Per cited source** — `article-<slug>` or `work-<slug>` for the
  primary sources (the actual text of the rule, agency guidance,
  landmark rulings).

Relationships:
- `about: concept-<policy-slug>` from the session summary.
- `administered_by: organization-<slug>` from the policy to its
  enforcing agency.
- `jurisdiction: <jurisdiction-slug>` as a property on the policy
  concept (use a concept entity if the jurisdiction is itself worth
  tracking: `concept-eu-single-market`).
- `cites: <source-entity-id>` from the session summary to each primary
  source.

## Source fidelity

Policy research has to be exact. Every obligation or deadline statement
in `obligations.md` / `timeline.md` MUST cite a primary source
(`[[article-eu-ai-act-final-text]]`) — secondary reporting is
supporting context, not the claim itself. When the primary source is
paywalled or not reproducible in the vault, link the canonical URL in
the `article-<slug>` entity's `source_url` property.
