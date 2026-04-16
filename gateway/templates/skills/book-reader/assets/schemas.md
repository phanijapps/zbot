# Output Schemas

Keep durable outputs limited to two file shapes.

## book.json

```json
{
  "book_id": "book-slug",
  "title": "...",
  "author": "...",
  "published": "YYYY or null",
  "language": "en or null",
  "source_url": "... or null",
  "raw_path": "...",
  "content_hash": "...",
  "thesis": "one sentence",
  "key_ideas": ["..."],
  "main_entities": ["..."],
  "notable_quotes": [
    {"text": "...", "chapter": 1, "line": 120}
  ],
  "tags": ["..."],
  "date_read": "ISO-8601",
  "chapters": [
    {"num": 1, "title": "...", "start": 1, "end": 500, "chunk": "chunks/ch-01.md"}
  ]
}
```

## book.kg.json

The structured per-book local knowledge graph. Stays on disk in `books/<slug>/book.kg.json` — not auto-ingested into the main graph. Rich enough to back a future Obsidian vault promotion and an optional `obsidian_query` tool. Shape aligns with the `ingest` tool so vault-promotion tools can consume it directly.

- Use stable slug IDs: `<type>:<kebab-name>` (e.g. `character:elizabeth-bennet`, `theme:victorian-hypocrisy`) — same id across books collapses to one node.
- Domain-specific metadata (aliases, chunk pointers, development arcs, confidence) lives in `properties`. Top-level keys for entities are `id`, `name`, `type`, `properties`; for relationships `type`, `from`, `to`, `properties`.
- Relationships MUST carry `properties.evidence` with at least one `{chunk_file, line}` pair. Empty evidence is a failure.

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
          {"chunk_file": "chunks/ch-01.json", "lines": [10, 120, 245]},
          {"chunk_file": "chunks/ch-05.json", "lines": [12, 340]}
        ]
      }
    },
    {
      "id": "theme:pride-and-prejudice",
      "name": "Pride and Prejudice",
      "type": "theme",
      "properties": {
        "description": "Mutual first-impression errors that the main romance must overcome",
        "first_appearance": {"chunk_file": "chunks/ch-02.json", "line": 5}
      }
    }
  ],
  "relationships": [
    {
      "type": "loves",
      "from": "character:elizabeth-bennet",
      "to": "character:fitzwilliam-darcy",
      "properties": {
        "direction": "mutual",
        "evidence": [
          {"chunk_file": "chunks/ch-16.json", "line": 234, "text": "..."},
          {"chunk_file": "chunks/ch-34.json", "line": 510, "text": "..."}
        ],
        "development": [
          {"chunk_file": "chunks/ch-05.json", "stage": "mutual dislike"},
          {"chunk_file": "chunks/ch-34.json", "stage": "proposal"}
        ],
        "confidence": 0.92
      }
    }
  ]
}
```

### Cross-book entities

Characters/themes/concepts appearing in multiple books should reuse the same id across their respective `book.kg.json` files. The graph merges properties (arrays concatenate, keys union), so `mentions_in` and `evidence` accumulate across sources over time.

## chunks/ch-*.md

```markdown
---
book_id: book-slug
chapter_num: 1
chapter_title: "..."
line_start: 1
line_end: 500
summary: "2–4 sentence summary"
key_ideas:
  - ...
  - ...
tags:
  - ...
mentions:
  people: []
  places: []
  concepts: []
quotes:
  - text: "..."
    line: 120
questions:
  - ...
---
# Chapter 1: ...

## Summary
2–4 sentence summary

## Key Ideas
- ...
- ...

## Full Text
verbatim text

## Quotes
> "...”
— line 120

## Questions
- ...
```
