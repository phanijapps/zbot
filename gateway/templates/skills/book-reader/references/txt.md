# TXT and Markdown Guide

Use this file for plain text and markdown books.

## Identification

Determine title, author, language, and publication clues from the source itself, not just the filename.

Look first for:
- title lines near the top
- author bylines
- markdown heading structure
- front matter blocks
- Project Gutenberg header metadata when present

Treat boilerplate, scanner notes, repository wrappers, and filename-derived guesses as weak evidence.
If the title or author cannot be recovered confidently, set them to `null`.

## Body start

Separate front matter from the actual readable body.

Common signals:
- a title page followed by author information
- markdown heading hierarchy
- a clear table of contents
- Gutenberg start and end markers
- long licensing or repository notices before the work begins

## Skeleton detection

Prefer explicit structure:
- markdown headings
- chapter headings
- numbered sections
- part breaks
- scene separators if no chapters exist

If no natural structure exists, create one implicit chapter covering the full readable body.

## Notes

- Preserve heading text exactly when it defines chapter names.
- Ignore navigation chrome, generated anchors, and markdown artifacts that do not belong to the book.
- For markdown, distinguish content headings from site or repository headings.
