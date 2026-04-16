# EPUB Guide

Use this file for EPUB books.

## Identification

Prefer embedded metadata over inferred guesses.

Use the package metadata and navigation structure to recover:
- title
- author
- language
- publisher
- publication date
- identifiers when available

If multiple values disagree, prefer package metadata first, then navigation labels, then visible title pages.

## Reading order

Respect the EPUB spine or equivalent canonical reading order.
Do not assume archive filename order is the book order.

## Body and chapter detection

Use navigation landmarks, table of contents entries, and document headings to form the chapter index.

Preferred signals:
- nav or toc entries
- spine item sequence
- heading elements inside chapter documents
- part or section labels

Merge tiny navigation fragments or ornamental pages into neighboring content when they are not meaningful reading units.
Keep cover pages, title pages, and copyright matter out of chapter chunks unless they contain useful book identity data.

## Notes

- Strip markup while preserving readable paragraph order.
- Preserve chapter titles as they appear to the reader.
- If the EPUB has no usable chapter structure, build chunks from the spine in reading order and treat them as implicit chapters.
