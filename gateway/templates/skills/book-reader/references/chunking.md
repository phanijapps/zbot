# Chunking Guide

Use this file for chunk sizing, source spans, and memory-safe reading.

## Goal

Turn a long book into stable, re-readable units that preserve order, traceability, and meaning.

## Default strategy

- Prefer chapter boundaries over fixed-size windows.
- If chapters are too large, split within the chapter into sequential subchunks.
- Preserve the original order exactly.
- Keep each chunk small enough to summarize and annotate reliably in one pass.

## Chunk policy

- First choice: one chunk per chapter.
- If a chapter is too large, split by strong internal boundaries in this order: section heading, scene break, paragraph block, then approximate size boundary.
- Avoid splitting in the middle of dialogue, lists, tables, or footnotes when possible.
- Maintain a continuous source span for every chunk.

## Required properties per chunk

Each chunk must preserve:

- `book_id`
- `chapter_num`
- `chapter_title`
- source span or line range
- verbatim text
- summary
- key ideas
- quotes with recoverable source locations
- mentions of people, places, and concepts
- tags

## Naming

- Single chunk chapter: `chunks/ch-01.md`
- Split chapter: `chunks/ch-01a.md`, `chunks/ch-01b.md`, `chunks/ch-01c.md` [If the chapter chunk is > 15KB.]

Use zero-padded chapter numbers for lexical sort stability.

## Book-level synthesis

Build the final book summary from the chunk files, not by rereading the full source.

## Guardrails

- Do not create overlapping chunks.
- Do not reorder material.
- Do not compress away the verbatim text.
- Do not create tiny fragments unless the source structure demands it.
- Prefer fewer, semantically coherent chunks over many shallow ones.
