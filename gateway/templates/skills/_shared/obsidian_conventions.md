# Obsidian Conventions (shared)

Single-syntax reference for every producer skill that writes vault-ready
output (book-reader, article-reader, research-reader, …). Freeze these
rules here; link to this file from each skill so no skill invents its own
dialect.

Obsidian's graph view is built from wikilinks and frontmatter — there is
no separate graph JSON. If the markdown is linked correctly, the graph
exists for free.

## File kinds

A source folder (book, article, research topic) contains:

```
<source-slug>/
├── _index.md                       # MOC: frontmatter + wikilink index
├── chunks/ch-NN.md                 # verbatim body (chapters, sections)
└── entities/<type>-<slug>.md       # one page per within-source entity
```

`_index.md` is the entry point Obsidian uses for the folder.

## Frontmatter — every `.md` file MUST carry it

Keys are flat, lowercase, kebab-case values. Dates ISO-8601.

### `_index.md` (source root)

```yaml
---
title: "<Human-readable title>"
type: book | article | research
slug: <source-slug>
author: "<Author or null>"
published: "YYYY or null"
language: en
thesis: "one sentence"
tags: [<type>, <topic-tags>]
aliases: ["<alt title>", …]
date_read: 2026-04-16
source: "<ward-relative original path or URL>"
---
```

### Chunk (`chunks/ch-NN.md`)

```yaml
---
source: <source-slug>
chunk_num: N
chunk_title: "..."
line_start: 1
line_end: 500
summary: "2–4 sentences"
key_ideas: [...]
tags: [...]
mentions:
  people: [<entity-slug>, …]       # entity-slug == link target of the entity page
  places: [...]
  concepts: [...]
quotes:
  - text: "..."
    line: 120
questions: [...]
---
```

### Entity page (`entities/<type>-<slug>.md`)

```yaml
---
title: "<Display name>"
type: character | theme | event | place | concept | organization | person | work
slug: <type>-<slug>
source: <source-slug>
aliases: ["<surface form 1>", "<surface form 2>"]
tags: [<type>, <source-slug>, <optional-topic-tag>]
---
```

## Body sections — fixed vocabulary

Use these section headings exactly. Don't invent new ones. Omit a section
entirely if it has no content (don't leave an empty header).

### `_index.md` body

```markdown
# <Title>

> <one-paragraph synopsis>

## Chapters

- [[ch-01]] — <chapter title>
- [[ch-02]] — <chapter title>

## Characters

- [[character-elizabeth-bennet|Elizabeth Bennet]]
- [[character-fitzwilliam-darcy|Fitzwilliam Darcy]]

## Themes

- [[theme-pride]]
- [[theme-prejudice]]

## Key events

- [[event-netherfield-ball]]
- [[event-darcys-first-proposal]]

<!-- manual -->
<!-- User notes preserved across re-promotion. -->
<!-- /manual -->
```

### Chunk body (`chunks/ch-NN.md`)

```markdown
# Chapter N: <title>

## Summary

<2–4 sentences>

## Full Text

<verbatim text, with mentions of known entities rewritten to [[wikilinks]]>

## Quotes

> "<passage>"
— line 120

## Questions

- <question the text raises>
```

### Entity page body

```markdown
# <Display name>

<one-sentence description>

## Relationships

- loves: [[character-fitzwilliam-darcy]]
- sister of: [[character-jane-bennet]]
- daughter of: [[character-mr-bennet]]

## Mentioned in

- [[ch-01]] lines 10, 120
- [[ch-05]] lines 12, 340
- [[ch-34]] line 510

## Evidence

> "In vain have I struggled. It will not do."
— [[ch-34]] line 510

> "I could easily forgive his pride, if he had not mortified mine."
— [[ch-05]] line 342
```

## Typed relationships — the one bullet shape

Under `## Relationships`, every edge is one bullet:

```
- <relation>: [[<target-slug>]]
```

- `<relation>` is lowercase, space-separated (`loves`, `sister of`,
  `works at`, `enemy of`).
- `<target-slug>` is the slug of another entity page
  (`character-fitzwilliam-darcy`), optionally aliased with
  `[[slug|Display]]`.
- Do NOT use Dataview inline fields (`loves::`). Vanilla Obsidian only.

Parser: `^- (.+?): \[\[(.+?)(?:\|.+?)?\]\]$` → `{relation, target}`.

## Wikilink rewriting in chunk bodies

In the `## Full Text` of each chunk, rewrite prose mentions of known
entities to `[[slug|surface form]]`. Rules:

1. Rewrite only on **first mention per paragraph** — later mentions stay
   plain prose, keeps chapters readable.
2. Match whole words only (Unicode word boundary).
3. Do NOT rewrite inside: frontmatter, fenced code blocks, existing
   wikilinks/markdown links, quoted passages (`> …`), `<!-- manual -->`
   blocks.
4. Surface form preserved: `[[character-elizabeth-bennet|Elizabeth]]`
   renders as "Elizabeth" but links to the slug.

## Slug normalization

- Lowercase.
- Replace non-alphanumeric runs with single `-`.
- Strip leading articles (`the`, `a`, `an`) for stability across editions.
- Cap at 60 chars; truncate at a word boundary.
- Prefix with the type: `character-`, `theme-`, `event-`, `place-`,
  `concept-`, `organization-`, `person-`, `work-`, `book-`, `article-`.

Examples:
- `Elizabeth Bennet` (character) → `character-elizabeth-bennet`
- `The Romance of Lust` (book) → `book-romance-of-lust`
- `Victorian hypocrisy` (theme) → `theme-victorian-hypocrisy`
- `Steve Jobs` (real person) → `person-steve-jobs`

## Preserving user edits

Sections fenced with `<!-- manual -->` / `<!-- /manual -->` are
user-owned. On re-promotion, everything outside the fence is regenerated,
everything inside is copied forward unchanged. On first write, append an
empty manual block at the bottom of the file so the user has a place to
add notes.

## Within-source vs cross-source entities

Producer skills must decide, per entity, whether it belongs in the vault
only (within-source) or also in the main SQLite KG (cross-source):

| Entity | Vault page? | Main KG node? |
|---|---|---|
| Fictional character (Elizabeth Bennet) | yes | no |
| Theme unique to this source (Austen's "pride") | yes | no |
| Within-source event (Netherfield ball) | yes | no |
| Real person (Jane Austen, Steve Jobs) | yes | yes |
| Real organization (Apple, East India Company) | yes | yes |
| Real place (London, Pemberley is fictional, so no) | yes for real | yes for real |
| Public work, method, theory (transformer architecture) | yes | yes |
| Named concept that exists outside this source | yes | yes |

Rule of thumb: **if it exists outside this source, ingest to main KG.**
The main-KG node carries a `vault_path` property pointing at the entity's
vault page, and `evidence` pointing at the chunk(s) where it's mentioned.
One node per entity across all sources — IDs collapse (`person-steve-jobs`
is one node whether it came from a biography, a news article, or an
earnings call).
