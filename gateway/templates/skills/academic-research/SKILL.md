---
name: academic-research
description: >
  Literature review on an academic or technical research topic — seminal
  papers, key authors, consensus and open questions. Use when the user
  asks "what does the literature say about X", "research papers on Y",
  "summarize the academic work on Z", or wants a lit-review foundation
  before writing or deciding. Ingests papers as works and authors as
  persons so citations accumulate across sessions.
metadata:
  version: "0.1.0"
---

# Academic Research

Literature review on a research topic — survey the papers, identify the
key authors and schools, name the open questions.

Structural contract: [`../_shared/research_archetype.md`](../_shared/research_archetype.md).
Output syntax: [`../_shared/obsidian_conventions.md`](../_shared/obsidian_conventions.md).

## Use when

- "What does the academic literature say about X"
- "Summarize the research on Y"
- "Key papers on Z"
- Pre-writing a paper / blog / thesis on a topic
- Pre-implementing a technique to understand prior art

## Subject slug

Kebab-case topic slug:
- A concept (`retrieval-augmented-generation`, `graph-neural-networks`)
- A technique (`mixture-of-experts`, `diffusion-sampling`)
- A field (`mechanistic-interpretability`, `fluid-mechanics`)

## Typical artifacts

- `survey.md` — the big-picture narrative: how the field got here, what
  the current consensus is
- `key-papers.md` — 5-15 landmark papers with one-paragraph each
- `schools-and-authors.md` — research groups and individuals driving
  the field, their positions
- `open-questions.md` — what's unresolved, what's contested
- `bibliography.md` — full citation list (can be BibTeX-style or plain)

## Cross-source ingest profile

Ingested to main KG alongside the `research:academic-research:<subject>:<date-slug>`
summary entity:

- **Per landmark paper** — `work-<paper-slug>` entity. Properties:
  `title`, `authors` (list of `person-<slug>` ids), `year`, `venue`,
  `url` or `arxiv_id`, `vault_path`.
- **Per key author** — `person-<slug>` entity. Properties:
  `affiliations` (list of `organization-<slug>`), `role: researcher`.
- **Per research group / institution** — `organization-<slug>` for
  material labs (OpenAI, DeepMind, MIT CSAIL, Stanford NLP).
- **Per core concept/technique** — `concept-<slug>` for the subject and
  its major sub-techniques.

Relationships:
- `about: concept-<subject-slug>` from the session summary.
- `cites: work-<paper-slug>` from the session summary to each paper.
- `authored_by: person-<slug>` from each paper to its authors.
- `affiliated_with: organization-<slug>` from each author to their
  institution.

## Citation format

In `survey.md` and `key-papers.md`, cite papers inline as wikilinks:

```markdown
[[work-attention-is-all-you-need|Vaswani et al. 2017]] introduced
the transformer architecture.
```

Citations accumulate across sessions on the same or related topics,
building a shared bibliography in the main KG.
