---
name: book-reader
description: >
  Read and memorize long-form documents (.txt, .md, .epub, .pdf) as
  vault-ready Obsidian folders. Use when the user wants a book identified,
  chunked, summarized, and stored so it can be recalled later by chapter,
  character, theme, event, or quote — via Obsidian graph view, a future
  obsidian_query tool, or the main knowledge graph.
metadata:
  version: "0.2.0"
---

# Book Reader

Read books the way a careful human reader does: identify the work, scan
its structure, read chapter by chapter, take notes, and leave the vault
with a fully linked record — chapters, entity pages, and a book index
that Obsidian renders as a graph out of the box.

Do not load the entire book into working context at once. Work progressively.

Output syntax for every `.md` file this skill writes is defined once in
[../_shared/obsidian_conventions.md](../_shared/obsidian_conventions.md).
Every section heading, frontmatter key, bullet shape, and wikilink rule
lives there — this skill does NOT redefine them.

## Use when

- The user asks to read, summarize, memorize, annotate, quote from, or
  catalog a book or long document
- The user wants a document added to a durable knowledge base
- The user asks what was previously read from a known title or source

## Core contract — exact inventory

Every successful ingestion produces **exactly** this folder, in the ward:

```
books/<slug>/
├── _index.md                                 # book MOC (frontmatter + chapter/character/theme/event wikilink lists)
├── chunks/ch-NN.md                           # one Markdown file per chapter, frontmatter + verbatim text
└── entities/<type>-<name>.md                 # one page per within-book entity (character, theme, event, place, concept)
```

And exactly one main-graph `ingest` call at the end — see
"Main-graph ingest" below.

**Do NOT produce:**
- `book.json` — its content belongs in `_index.md` frontmatter + body.
- `book.kg.json` or any other `*.kg.json` — the graph is the linked
  markdown. Wikilinks ARE the graph.
- Per-section graph JSON.
- Any file outside `books/<slug>/`.

If a chapter is too long for one Markdown file, split it sequentially
(`ch-03a.md`, `ch-03b.md`) — they share `chunk_num: 3`.

## Workflow

### 1. Check for prior ingestion

Look up the book by title-derived slug. If `books/<slug>/_index.md`
already exists, switch to retrieval behavior and do not re-ingest.

### 2. Identify the work

Extract title, author, publication info, language, source from the
document's OWN metadata — never from the filename:

- EPUB: Dublin Core in the OPF (`<dc:title>`, `<dc:creator>`, …). See
  `references/epub.md`.
- PDF: metadata dictionary (`/Title`, `/Author`); fall back to first-page
  scan. See `references/pdf.md`.
- TXT / MD: first ~2KB for a title line (leading `#`, ALL CAPS,
  Gutenberg header). See `references/txt.md`.

If no title is extractable, leave the field `null` and abort ingestion —
do NOT guess from the filename.

### 2a. Derive the slug

Generate a kebab-case slug from the real title:
- Lowercase, non-alphanumeric → `-`, collapse runs, strip leading/trailing `-`.
- Strip leading `the`/`a`/`an`.
- Cap at ~60 chars at a word boundary.

The slug is both the folder name (`books/<slug>/`) and the `<source-slug>`
used in frontmatter and entity slugs
(`character-<name>`, `theme-<name>`, etc.). Filenames like `pg30254.epub`
are NEVER the slug. If a folder for the same slug already exists, treat
the book as ingested (Step 1).

### 3. Find the reading skeleton

Split the document body into chapters (or a single implicit chapter if
no chapter structure exists). Preserve reading order and stable source
boundaries. See `references/chunking.md`.

### 4. Read and annotate progressively

Read one chapter at a time. Write each chapter as `chunks/ch-NN.md` per
the chunk shape in the shared conventions file. Requirements:

- Full frontmatter (source, chunk_num, chunk_title, line_start/end,
  summary, key_ideas, tags, mentions, quotes, questions).
- Body: `## Summary`, `## Full Text` (verbatim, with mentions rewritten
  to `[[wikilinks]]`), `## Quotes`, `## Questions`.
- Mentions rewriting follows the rules in
  `_shared/obsidian_conventions.md` (first-per-paragraph only, whole-word
  match, skip inside code/quotes/frontmatter/manual blocks).

### 5. Write entity pages

For every within-book entity surfaced across the chapters — character,
theme, event, place, concept, organization (fictional or in-story) —
write one `entities/<type>-<slug>.md` page per the entity page shape in
the shared conventions. Requirements:

- Full frontmatter (title, type, slug, source, aliases, tags).
- Body: one-sentence description, `## Relationships` (typed bullets
  shaped as `- <relation>: [[target-slug]]`), `## Mentioned in`
  (wikilinks to chapters with line numbers), `## Evidence` (quoted
  passages with `— [[ch-NN]] line <N>`).

The target slug of every relationship bullet MUST be an entity page this
skill also writes in the same run. If you reference a slug that doesn't
resolve to a written page, that's a broken link and the run fails
acceptance.

Minimum targets for a novel-length work: ≥ 8 character pages,
≥ 4 theme pages, ≥ 5 event pages, ≥ 10 typed relationship bullets across
those pages, each with populated evidence. Under that count, go back —
the vault isn't done until the graph reflects the book.

### 6. Write `_index.md`

After every chunk + entity page is written, assemble `books/<slug>/_index.md`
per the `_index.md` shape in the shared conventions:

- Frontmatter: title, type=book, slug, author, published, language,
  thesis, tags, aliases, date_read, source.
- Body: one-paragraph synopsis, `## Chapters` (wikilinks to ch-NN),
  `## Characters` (wikilinks to character pages),
  `## Themes` (wikilinks to theme pages),
  `## Key events` (wikilinks to event pages).
- End with an empty `<!-- manual --><!-- /manual -->` block.

### 7. Main-graph ingest

At the end of the run, call the `ingest` tool exactly once. The payload
carries **one book-summary entity plus one entity per cross-source real
entity** surfaced in the book.

**Within-source entities (fictional characters, in-book themes, in-book
events) do NOT go to main KG.** They live only in the vault. A
within-source entity is one whose identity exists only inside this book
(Elizabeth Bennet, Netherfield ball, Austen's in-book "pride" motif).

**Cross-source entities (real people, real organizations, real places,
public works, named concepts) DO go to main KG.** A cross-source entity
exists outside this book and could appear in other books, articles, or
research (Jane Austen the author, East India Company, London, transformer
architecture, Newtonian mechanics).

Rule of thumb: if the entity has a Wikipedia page or could plausibly have
one, it's cross-source.

Payload shape:

```json
{
  "entities": [
    {
      "id": "book:<slug>",
      "name": "<Title>",
      "type": "book",
      "properties": {
        "slug": "<slug>",
        "author": "<Author or null>",
        "published": "YYYY or null",
        "language": "en",
        "thesis": "one sentence",
        "tags": ["..."],
        "chapter_count": 34,
        "vault_path": "books/<slug>/_index.md",
        "chunk_dir": "books/<slug>/chunks/",
        "entities_dir": "books/<slug>/entities/",
        "character_count": 12,
        "theme_count": 5,
        "event_count": 8,
        "notable_quote": {"text": "...", "chunk_file": "chunks/ch-01.md", "line": 1}
      }
    },
    {
      "id": "person:jane-austen",
      "name": "Jane Austen",
      "type": "person",
      "properties": {
        "vault_path": "books/<slug>/entities/person-jane-austen.md",
        "role_in_book": "author",
        "evidence": [{"chunk_file": "chunks/ch-00.md", "line": 1}]
      }
    }
  ],
  "relationships": []
}
```

The main-graph entities are summary-level pointers — detail lives in the
vault. Relationships stay empty at the main-graph layer: cross-source
relationships across multiple books accumulate naturally as each book
adds its own `person:jane-austen` entity and properties merge.

### 8. Store memory facts

Save `memory_facts` for fast recall across sessions:

- **One** `domain` fact keyed `domain.<slug>.summary` — title, author,
  thesis, tag list. Default scope.
- Per-entity facts for the **most important** within-book entities
  (protagonists, major themes, climactic events) — scope `global`, keys
  `domain.<slug>.character.<kebab-name>`, `domain.<slug>.theme.<kebab-name>`,
  etc. One sentence each. Do NOT save every character — just the ones a
  user would plausibly ask about by name in a future session.

## Rules

1. Always check for existing ingestion first — look up by title-slug.
2. Filename is NEVER authoritative. Extract metadata from the document
   itself. `books/pg30254/` is wrong; `books/romance-of-lust/` is right.
3. Never load the whole book into working context.
4. Every chunk body includes verbatim text under `## Full Text` — no
   summary-only chapter files.
5. Every entity page must exist for every slug referenced in any
   relationship bullet, `## Characters`, `## Themes`, or `## Key events`
   list. No broken wikilinks.
6. Every section in the shared conventions uses its fixed heading
   vocabulary — do not invent new headings.
7. Typed relationships use the one bullet shape: `- <relation>: [[target-slug]]`.
   No Dataview syntax.
8. The `ingest` call is exactly once, at the end, with one book entity
   plus any cross-source real entities. Fictional characters stay in the
   vault only.
9. Prefer `null` over fabricated metadata.
10. If the document is image-only or OCR is required, hand off to an
    OCR-capable path first, then continue.

## Retrieval behavior

When the user asks about a previously read work, answer from the vault:

- Book list / "did we read X?" — scan `books/*/_index.md` frontmatter
  (title, aliases, slug).
- Chapter content — read `books/<slug>/chunks/ch-NN.md`.
- Character / theme / event — read
  `books/<slug>/entities/<type>-<slug>.md`.
- Cross-book entities — `graph_query` against the main KG (returns
  `vault_path` properties pointing into each book's vault).

Don't re-ingest. Don't re-chunk.
