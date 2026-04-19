# PDF Guide

Use this file for PDF books and long documents.

## First decision

Determine whether the PDF is text-extractable or image-only.

- If text can be extracted in reading order, continue.
- If the PDF is primarily scanned images without reliable text, route it through an OCR-capable path first, then continue with the OCR text as the source body.

## Identification

Recover metadata from the document itself where possible.

Look for:
- embedded document metadata
- title page
- author line
- running headers or footer clues
- table of contents
- publisher or imprint pages

Document metadata can be helpful but is not always trustworthy; verify against visible pages when possible.

## Body reconstruction

Reconstruct a plain-text reading body that removes repetitive headers, footers, and page numbers when they are not meaningful content.
Preserve paragraph order, headings, block quotes, and obvious section boundaries.

## Skeleton detection

Preferred chapter signals:
- table of contents entries
- large heading transitions
- numbered chapters or sections
- repeated running head changes
- appendix or part markers

If the PDF is an article, report, or essay with no chapters, create one implicit chapter or a small set of section-based chunks.

## Notes

- Be careful with two-column layouts, footnotes, and sidebars; preserve main reading order.
- Keep source-location fidelity so quotes can be recovered later.
- Remove only layout noise, not meaningful text.
