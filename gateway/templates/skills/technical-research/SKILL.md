---
name: technical-research
description: >
  Deep dive on a technology — framework, library, protocol,
  architecture, standard, or technique. Use when the user asks "research
  X library", "how does Y protocol work", "evaluate Z architecture",
  "compare stacks A vs B", or wants a technical foundation before
  adopting or building. Ingests the technology as a work and its makers
  so the stack of tech you've researched accumulates.
metadata:
  version: "0.1.0"
---

# Technical Research

Technology deep-dive — a framework, library, protocol, architecture,
standard, or technique — at a point in time.

Structural contract: [`../_shared/research_archetype.md`](../_shared/research_archetype.md).
Output syntax: [`../_shared/obsidian_conventions.md`](../_shared/obsidian_conventions.md).

## Use when

- "Research X framework / library / protocol"
- "How does Y work" (for a specific technology, not a general concept)
- "Evaluate Z architecture for <use-case>"
- "Compare stacks A vs B" (technical rather than product comparison)
- Pre-adoption review before committing to a dependency
- Pre-implementation prior-art survey

## Subject slug

Kebab-case slug for the technology:
- Library / framework — `duckdb`, `pydantic-v2`, `effect-ts`, `axum`
- Protocol / standard — `http3`, `oauth2-device-flow`, `webrtc`
- Architecture / pattern — `event-sourcing`, `cqrs`, `raft-consensus`
- Technique — `flash-attention`, `copy-on-write`, `lockfree-queues`

## Typical artifacts

- `overview.md` — what it is, why it exists, the one-paragraph pitch
- `how-it-works.md` — mechanism, key concepts, mental model
- `tradeoffs.md` — strengths, weaknesses, known footguns
- `ecosystem.md` — maintainers, adoption, community, maturity, release
  cadence
- `comparison.md` — vs the 1-3 closest alternatives (when relevant)
- `recommendation.md` — adopt / avoid / watch, with conditions

## Cross-source ingest profile

Ingested to main KG alongside the `research:technical-research:<subject>:<date-slug>`
summary entity:

- **Always** — `work-<slug>` for the technology itself (library,
  framework, protocol, specified technique). Properties: `kind`
  (`library` | `framework` | `protocol` | `standard` | `architecture` |
  `technique`), `license`, `current_version`, `maintainer` (→
  `organization-<slug>` or `person-<slug>`), `vault_path`.
- **Per maintainer** — `organization-<slug>` or `person-<slug>` for the
  entity that owns / stewards it (Anthropic, Rust Foundation,
  individual maintainer).
- **Per compared alternative** — `work-<slug>` for the 1-3 closest
  alternatives discussed in `comparison.md`.
- **Per underpinning concept** — `concept-<slug>` for the theoretical
  ideas the technology rests on (`concept-consensus-algorithm`,
  `concept-copy-on-write`) when material.

Relationships:
- `about: work-<subject-slug>` from the session summary.
- `maintained_by: <maintainer-entity-id>` from the work to its
  maintainer.
- `alternative_to: work-<slug>` for each compared alternative.
- `implements: concept-<slug>` from the work to the concepts it
  realizes.
