---
name: book-reader
description: >
  Read and memorize long-form documents such as .txt, .md, .epub, and .pdf files.
  Use when the user wants a book or long document identified, chunked, summarized,
  indexed into graph memory, or recalled later by chapter, concept, quote, or entity.
---

# Book Reader

Read books the way a careful human reader does: identify the work, scan its structure,
read it section by section, take notes, and store it so it can be recalled later.

Do not load the entire book into working context at once. Work progressively.

## Use when

- The user asks to read, summarize, memorize, annotate, quote from, or catalog a book or long document
- The user wants a document added to a long-term knowledge base
- The user asks what was previously read from a known title or source

## Core contract — exact file + tool inventory

Every successful ingestion produces **exactly**:

1. `books/<slug>/book.json` — **one** JSON file, the book's metadata + chapter index. See `assets/schemas.md`.
2. `books/<slug>/chunks/ch-NN.md` — **one Markdown file per chapter**, with YAML frontmatter + verbatim text. Not JSON. Not per-section. One file per chapter.
3. `books/<slug>/book.kg.json` — **one** JSON file, the rich `{entities, relationships}` local knowledge graph for the book. Stays on disk for future promotion into the Obsidian vault and for the optional `obsidian_query` tool. It is **not** ingested into the main graph automatically — it is the per-book graph.
4. **One** `ingest` call at the end, with **exactly one** book-level entity for the main graph — rich properties pointing into the files and memory keys above. See "Main-graph ingest" below.
5. Memory facts for cross-book recall: **one** `domain` fact summarizing the book, plus per-character / per-theme / per-event facts with `scope=global` and hierarchical keys (`domain.<slug>.character.<kebab-name>`).

Anything outside this inventory is wrong. Specifically, do NOT produce:
- Per-chapter or per-section `.kg.json` files (e.g. `section_01.kg.json`, `chapter_XX.kg.json`). ONE `book.kg.json`, period.
- Secondary knowledge files like `insights.kg.json`, `themes.json`, `characters.json`. Everything goes in `book.kg.json`.
- A separate "distill" or "ward-distiller" step — the skill calls `ingest` inline at the end; there is no session-end sweep.
- A bulk `ingest` of every entity from `book.kg.json` into the main graph — only ONE book-level entity goes to the main graph. The detail entities stay in `book.kg.json` and in `memory_facts`.
- Any file outside `books/<slug>/` unless the runtime explicitly demands it.

If a chapter is too long for one Markdown file, split the Markdown (`ch-03a.md`, `ch-03b.md`) — **still one** `book.kg.json`.

## Workflow

### 1. Check for prior ingestion

Before reading, determine whether this exact book is already known by title, source, or content identity.
If it already exists, switch to retrieval behavior and do not ingest it again.

### 2. Identify the work

Determine the best available title, author, publication info, language, source, and canonical book id by reading **the document's own metadata**, not the filename.

- EPUB: parse Dublin Core fields (`<dc:title>`, `<dc:creator>`, `<dc:language>`, `<dc:source>`, `<dc:identifier>`) from the OPF inside the `.epub`. See `references/epub.md`.
- PDF: read the PDF metadata dictionary (`/Title`, `/Author`). If empty, fall back to the first page text. See `references/pdf.md`.
- TXT / MD: scan the first ~2KB for a title-like line (leading `#`, ALL CAPS title, Gutenberg header). See `references/txt.md`.

Filenames like `pg30254.epub`, `untitled.pdf`, or random hashes are **never** a source of truth for title or author. If the document has no extractable title, leave it `null` and abort ingestion — do NOT guess "The Gold-Bug" from a filename.

### 2a. Derive the book slug + folder name

Once you have the real title, generate a kebab-case slug from it:
- Lowercase, replace spaces and punctuation with `-`, collapse runs of `-`, strip leading/trailing `-`.
- Strip leading articles (`the`, `a`, `an`) to keep slugs stable across editions.
- Cap at ~60 chars; if longer, truncate at a word boundary.

Examples:
- `"The Romance of Lust"` → `romance-of-lust`
- `"Pride and Prejudice"` → `pride-and-prejudice`
- `"The Gold-Bug and Other Tales"` → `gold-bug-and-other-tales`

Use that slug as:
- The folder name: `books/<slug>/`
- The `book_id` field in all artifacts and entity ids (e.g. `book:<slug>`, `character:<slug>:elizabeth-bennet`)

The folder name is NEVER the input filename. `books/pg30254/` is wrong; `books/romance-of-lust/` is right. If a folder for the same slug already exists, treat the book as already ingested (Step 1).

If metadata is uncertain even after parsing the document, prefer `null` over guessing — and return the session with a note that the book could not be identified, rather than fabricating a title.

### 3. Find the reading skeleton

Locate the document body and split it into natural reading units, preferably chapters.
If the source has no explicit chapters, create a single implicit chapter covering the full body.

Chunking must preserve reading order and stable source boundaries.
See `references/chunking.md`.
Also see `assets/schemas.md` 

### 4. Read and annotate progressively

Read one chapter at a time. Write each chapter as `chunks/ch-NN.md` — **Markdown, not JSON**. Frontmatter carries the metadata, the body carries the verbatim chapter text.

For each chapter, the frontmatter must contain:
- `book_id: <slug>`
- `chapter_num: N`
- `chapter_title: "..."`
- `line_start` / `line_end`
- `summary` — 2-4 sentences
- `key_ideas: [...]`
- `tags: [...]`
- `mentions:` — `people: [...]`, `places: [...]`, `concepts: [...]` (these also become entities in `book.kg.json`)
- `quotes:` — `[{text, line}, ...]` (these also become quote entities in `book.kg.json`)
- `questions: [...]`

The body below the frontmatter MUST include the verbatim chapter text under a `## Full Text` section — no summary-only files.

Large chapters may be split into sequential Markdown files (`ch-03a.md`, `ch-03b.md`) — they stay linked to the same chapter_num. The graph payload (`book.kg.json`) stays as one file regardless.

### 5. Distill the whole book — produce `book.json` and `book.kg.json`

After all chunk Markdown files are written, read THEM (not the raw source) to build two outputs:

**`book.json`** — metadata + chapter index:
- thesis
- key ideas across the book
- main entities (names only, for the table of contents — full entity data lives in `book.kg.json`)
- notable quotes (with `chunk_file` + `line`)
- chapter index (`num`, `title`, `start`, `end`, `chunk`)
- tags

**`book.kg.json`** — the complete graph payload — see "Knowledge graph output" below. Single file covering every chapter. Do NOT emit one `.kg.json` per chapter.

### 6. Store for recall — three layers

Store the result in three layers:

1. **Chapter chunk files** (`chunks/ch-*.md`) — verbatim Markdown for re-reading and exact passage recovery.
2. **Per-book local graph** (`book.kg.json`) — rich `{entities, relationships}` covering every character, theme, event, quote, and location. This file stays on disk. It is **not** auto-ingested into the main graph. Future: it will be promoted into an Obsidian vault and queried via an optional `obsidian_query` tool.
3. **Main-graph ingest + memory facts** — at the very end of the skill run, you MUST:
   - Call `ingest` **once** with ONE book-level entity (`id: "book:<slug>"`, `type: "book"`) whose `properties` carry enough for future sessions to find everything: title, author, thesis, chapter_count, main character/theme/event name lists, `chunk_dir` path, `book_kg_path`, `memory_key_prefix`. See "Main-graph ingest" below for the exact shape.
   - Save **one** `domain` memory fact with the book summary (scope can be default; key `domain.<slug>.summary`).
   - Save per-character / per-theme / per-event memory facts with `scope=global` and hierarchical keys: `domain.<slug>.character.<kebab-name>`, `domain.<slug>.theme.<kebab-name>`, `domain.<slug>.event.<kebab-name>`. These are what `memory.recall` finds when the user later asks "who is Elizabeth Bennet" across sessions.

Why this split:
- Main graph stays small and cross-domain — one node per book, searchable by title/author/theme, not cluttered with 50 chapter nodes.
- `book.kg.json` stays rich and local — the full entity graph for the book, ready for vault promotion.
- `memory_facts` carry the character/theme detail globally — recall can surface "Charlie appears in Book X" across sessions without graph traversal.

## Main-graph ingest — the single book-level entity

At the end of the run, call the `ingest` tool exactly once. The payload has ONE entity and NO relationships (the rich relationships live in `book.kg.json`, not in the main graph):

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
        "language": "en or null",
        "thesis": "one sentence",
        "tags": ["..."],
        "chapter_count": 34,
        "chunk_dir": "books/<slug>/chunks/",
        "book_json_path": "books/<slug>/book.json",
        "book_kg_path": "books/<slug>/book.kg.json",
        "memory_key_prefix": "domain.<slug>.",
        "characters": ["Elizabeth Bennet", "Fitzwilliam Darcy", "..."],
        "themes": ["pride", "prejudice", "class", "..."],
        "key_events": ["Netherfield ball", "Darcy's first proposal", "..."],
        "notable_quote": {"text": "...", "chunk_file": "chunks/ch-01.md", "line": 1}
      }
    }
  ],
  "relationships": []
}
```

This is the ONLY thing that makes the book discoverable in the main graph. A future `graph_query(query="pride and prejudice")` returns this node and its properties — which then point the caller at `chunk_dir`, `book_kg_path`, and `memory_key_prefix` for deeper detail.

## Per-book local graph — `book.kg.json`

This file stays on disk in `books/<slug>/book.kg.json`. It is NOT auto-ingested. It will later be promoted into the Obsidian vault.

### Required richness — not just structure

A bland graph is a failure. "Book → has_chapter → Chapter 1" is structural padding. Every book MUST emit entities in these categories when present in the text:

- `character` / `person` — every named character; populate `aliases` with every surface form they're addressed by.
- `location` / `place` — settings, cities, houses.
- `event` — key plot events with a sentence description.
- `theme` — thematic threads (e.g. "Victorian hypocrisy", "coming of age").
- `concept` — ideas, symbols, motifs the book develops.
- `quote` — notable passages with exact `chunk_file` + `line`.
- `organization` — institutions, families-as-units, companies.

Structural entities (`book`, `chapter`, `volume`) are fine to include but must NOT be the majority.

### Evidence is mandatory

Every relationship MUST carry `properties.evidence` with at least one `{chunk_file, line}` pair. An empty `evidence: []` array is a failure — if you can't cite the passage, you haven't actually read enough to claim the relationship.

### Stable slug IDs

Use `<type>:<kebab-name>` so the same entity across books collapses in the graph:
- `character:elizabeth-bennet`
- `theme:victorian-hypocrisy`
- `location:pemberley`

### Shape (see `assets/schemas.md` for the full reference)

```json
{
  "book_id": "book-slug",
  "entities": [
    {
      "id": "character:elizabeth-bennet",
      "name": "Elizabeth Bennet",
      "type": "character",
      "properties": {
        "aliases": ["Lizzy", "Miss Bennet"],
        "description": "protagonist",
        "first_appearance": {"chunk_file": "chunks/ch-01.md", "line": 10},
        "mentions_in": [
          {"chunk_file": "chunks/ch-01.md", "lines": [10, 120]},
          {"chunk_file": "chunks/ch-05.md", "lines": [12, 340]}
        ]
      }
    }
  ],
  "relationships": [
    {
      "type": "loves",
      "from": "character:elizabeth-bennet",
      "to": "character:fitzwilliam-darcy",
      "properties": {
        "evidence": [
          {"chunk_file": "chunks/ch-34.md", "line": 510, "text": "..."}
        ],
        "confidence": 0.92,
        "development": [
          {"chunk_file": "chunks/ch-05.md", "stage": "initial dislike"},
          {"chunk_file": "chunks/ch-34.md", "stage": "proposal"}
        ]
      }
    }
  ]
}
```

### Minimum targets for a novel-length work

Roughly, for a novel, `book.kg.json` should contain: ≥ 8 character entities, ≥ 4 theme entities, ≥ 5 event entities, ≥ 10 relationships — each with populated evidence. If you finish reading and have fewer, go back and add them; the skill isn't done until `book.kg.json` reflects the book. (The main-graph `ingest` is separate and always exactly ONE book-level entity.)

## Required file shapes

### `book.json`

Contains top-level metadata, thesis, key ideas, major entities, notable quotes, tags, source identity, and chapter-to-chunk pointers.

### `chunks/ch-*.md`

Markdown with a YAML frontmatter header. The frontmatter carries structured fields (book_id, chapter_num, chapter_title, line_start, line_end, summary, key_ideas, tags, mentions, quotes, questions). The body carries the **verbatim** chapter text. See `assets/schemas.md` for the full template.

### `book.kg.json`

Structured `{entities, relationships}` payload, described in the "Knowledge graph output" section above. Required for the book to be queryable via `graph_query` in future sessions.

## Rules

1. Always check for existing ingestion first — look up by title-derived slug.
2. Never treat the filename as authoritative metadata. Extract title/author from the document's own metadata (EPUB OPF, PDF metadata dict, first-page scan). `books/<slug>/` uses the slugified title, not the input filename. If you catch yourself using `books/pg30254/` or `books/untitled/`, stop — go back to Step 2.
3. Never load the whole book into working context if progressive reading is possible.
4. Every quote must retain a recoverable source location.
5. Chunk files must contain verbatim text, not just summaries.
6. Book memory is one fact per book; character/theme/event detail belongs in `book.kg.json`, chapter prose belongs in chunk files. Do NOT use `memory.save_fact` as a substitute for emitting graph entities — that bypasses the graph and the next session can't query it.
7. Keep `SKILL.md` process-shaped; keep format mechanics in referenced files.
8. Prefer nulls over fabricated metadata.
9. Preserve reading order and source traceability.
10. If the document is image-only or OCR is required, hand off to an OCR-capable path first, then continue with this skill.

## Retrieval behavior

When the user asks about a previously read work, use stored memory and graph structure to answer from existing artifacts before considering re-ingestion.

Support these retrieval patterns:
- books already read
- whether a specific title or source was read
- what a given chapter says
- which chapters discuss a concept
- exact passage recovery
- cross-book entity lookup
