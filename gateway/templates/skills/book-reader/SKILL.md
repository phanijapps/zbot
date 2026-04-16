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

## Core contract

For each book, create only these durable artifacts:

- One `<name_ofbook>/book.json` — metadata + chapter index
- One or more chapter chunk files in `<name_ofbook>/chunks/`
- One `<name_ofbook>/book.kg.json` — **the structured knowledge graph payload** — picked up by the ward-distiller skill at session end and ingested into `kg_entities` + `kg_relationships`
- One memory fact for the book

Do not create extra durable summary file types unless the runtime requires them transiently. Do NOT split knowledge across multiple `.kg.json` files (e.g., separate `insights.kg.json` for themes) — everything goes in one `book.kg.json`.

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

Read one chapter or chunk at a time.

For each chunk, preserve the verbatim text and add:
- a short summary
- key ideas
- notable quotes with source locations
- major people, places, and concepts
- open questions or unresolved themes
- topical tags

Large chapters may be split into multiple sequential chunk files, but they must remain linked to the same chapter.

### 5. Distill the whole book

After all chunks are complete, create a book-level synthesis using the chunk files rather than rereading the entire source.

Capture:
- thesis
- key ideas across the book
- main entities
- notable quotes
- chapter index
- tags

### 6. Store for recall

Store the result in three layers:
- files for verbatim re-reading (`chunks/ch-*.json`)
- the knowledge graph payload (`book.kg.json`) — see "Knowledge graph output" below for the REQUIRED shape
- one memory fact for fast recall

Graph memory should support:
- book lookup
- chapter lookup
- concept-to-chapter discovery
- entity mentions across books
- exact-passage recovery through chunk or source pointers

## Knowledge graph output — `book.kg.json`

This file is the ONLY thing that makes the book queryable across future sessions. The ward-distiller skill scans the ward, finds every `*.kg.json`, and calls the `ingest` tool on it — writing entities and relationships into the shared knowledge graph.

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
        "first_appearance": {"chunk_file": "chunks/ch-01.json", "line": 10},
        "mentions_in": [
          {"chunk_file": "chunks/ch-01.json", "lines": [10, 120]},
          {"chunk_file": "chunks/ch-05.json", "lines": [12, 340]}
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
          {"chunk_file": "chunks/ch-34.json", "line": 510, "text": "..."}
        ],
        "confidence": 0.92,
        "development": [
          {"chunk_file": "chunks/ch-05.json", "stage": "initial dislike"},
          {"chunk_file": "chunks/ch-34.json", "stage": "proposal"}
        ]
      }
    }
  ]
}
```

### Minimum targets for a novel-length work

Roughly, for a novel: ≥ 8 character entities, ≥ 4 theme entities, ≥ 5 event entities, ≥ 10 relationships — each with populated evidence. If you finish reading and have fewer, go back and add them; the skill isn't done until the graph reflects the book.

## Required file shapes

### `book.json`

Contains top-level metadata, thesis, key ideas, major entities, notable quotes, tags, source identity, and chapter-to-chunk pointers.

### `chunks/ch-*.json`

Contains the verbatim chunk text plus summary, ideas, quotes, mentions, questions, tags, and source span.

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
