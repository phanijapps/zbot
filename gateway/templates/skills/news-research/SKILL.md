---
name: news-research
description: >
  News synthesis on a topic at a point in time — catch-up digests,
  event tracking, breaking-story analysis. Use when the user asks "what
  happened with X", "news on X", "catch me up on Y", or wants a timeline
  of recent events. Cites sources, ingests mentioned entities and source
  articles into the main knowledge graph.
metadata:
  version: "0.1.0"
---

# News Research

Synthesize recent news on a topic into a dated, cited session record.

Structural contract: [`../_shared/research_archetype.md`](../_shared/research_archetype.md).
Output syntax: [`../_shared/obsidian_conventions.md`](../_shared/obsidian_conventions.md).

## Use when

- "What happened with X this week?"
- "Catch me up on Y"
- "News on X" / "latest on X"
- Event tracking (election, conflict, earnings cycle, launch, scandal)
- Building a timeline of a developing story

## Subject slug

The topic the news is about — kebab-case:
- A ticker / company (`tsla`, `apple`)
- A person (`elon-musk`, `powell`)
- An event or story (`fed-march-meeting`, `openai-dev-day`)
- A geography/policy frame (`eu-ai-act`, `middle-east`)

## Typical artifacts

- `summary.md` — the single-paragraph "what's going on" digest
- `timeline.md` — dated bullet list of events with source citations
- `sources.md` — annotated list of the articles/posts read
- `implications.md` — what this means, for whom, what to watch next

## Cross-source ingest profile

Ingested to main KG alongside the `research:news-research:<subject>:<date-slug>`
summary entity:

- **Always** — the subject entity itself, as its natural type:
  - `organization-<slug>` if the subject is a company
  - `person-<slug>` if the subject is an individual
  - `event-<slug>` if the subject is a recurring or named event
  - `concept-<slug>` for topical frames (AI regulation, geopolitics)
- **Per story** — people and organizations that materially drive the
  story (not every name mentioned — only those with agency in the
  narrative).
- **Per source** — one `article-<slug>` entity per distinct article
  cited, with `source_url` + `publisher` properties. News coverage
  accumulates across sessions on the same subject.

Relationships:
- `about: <subject-entity-id>` from the session summary.
- `cites: article-<slug>` from the session summary to each cited article.
- `mentions: <entity-id>` for people/orgs mentioned but not primary.

## Date-slug conventions

- Default: bare ISO date — news sessions are usually one-per-day.
- Event-anchored: `-breaking`, `-<event-name>` when the session tracks
  a specific story (`-fomc-day`, `-launch-day`).
- Catch-up depth: `-weekly`, `-monthly` for digest-style sessions.

## Source citation rule

Every factual claim in `summary.md` / `timeline.md` / `implications.md`
that came from a news article MUST carry an inline citation pointing at
an `article-<slug>` entry in `sources.md`:

```markdown
Tesla delivered 430k vehicles in Q1 [[source-tesla-q1-delivery-press-release]].
```

No uncited claims. If a claim is inference from multiple sources, cite
the strongest one and mark the claim as synthesis in the prose.
