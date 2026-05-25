//! Markdown → styled-line rendering.
//!
//! Converts a markdown string into a list of `Line`s where each line is a
//! sequence of styled `Span`s. The UI layer then maps each `Span` to an
//! iocraft `Text` widget inside a `View(flex_direction: Row)`.
//!
//! Scope (v1):
//! - Paragraphs (soft-wrap = single line)
//! - **Bold** → bright + bold weight
//! - `Inline code` → cyan
//! - Code blocks (```lang … ```) → distinguished block, monospace
//! - `- item` / `* item` → bulleted with `•` prefix
//! - `# heading` / `## heading` / `### heading` → bold + colored
//!
//! Anything more exotic (links, tables, blockquotes, nested lists) falls
//! back to plain text. We optimise for "what the assistant actually emits."

use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};

#[derive(Debug, Clone)]
pub struct Line {
    pub kind: LineKind,
    pub spans: Vec<Span>,
}

#[derive(Debug, Clone)]
pub enum LineKind {
    Plain,
    Bullet,
    Heading {
        /// Heading level (1-6). Currently all levels render identically;
        /// kept for future per-level styling.
        #[allow(dead_code)]
        level: u8,
    },
    CodeBlock,
    Blank,
}

#[derive(Debug, Clone)]
pub struct Span {
    pub text: String,
    pub style: SpanStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanStyle {
    Plain,
    Bold,
    Code,
}

impl Line {
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            kind: LineKind::Plain,
            spans: vec![Span { text: text.into(), style: SpanStyle::Plain }],
        }
    }

    pub fn blank() -> Self {
        Self { kind: LineKind::Blank, spans: vec![] }
    }
}

/// Parse a markdown document into a flat list of rendered lines.
pub fn parse_markdown(md: &str) -> Vec<Line> {
    let parser = Parser::new(md);

    let mut lines: Vec<Line> = Vec::new();
    let mut current_spans: Vec<Span> = Vec::new();
    let mut current_kind: LineKind = LineKind::Plain;
    let mut active_style: SpanStyle = SpanStyle::Plain;
    let mut in_code_block: bool = false;

    let flush_into = |lines: &mut Vec<Line>,
                      spans: &mut Vec<Span>,
                      kind: &mut LineKind| {
        if !spans.is_empty() {
            lines.push(Line {
                kind: kind.clone(),
                spans: std::mem::take(spans),
            });
        }
        *kind = LineKind::Plain;
    };

    for event in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                flush_into(&mut lines, &mut current_spans, &mut current_kind);
                current_kind = LineKind::Heading { level: level as u8 };
                active_style = SpanStyle::Bold;
            }
            Event::End(TagEnd::Heading(_)) => {
                flush_into(&mut lines, &mut current_spans, &mut current_kind);
                active_style = SpanStyle::Plain;
                lines.push(Line::blank());
            }
            Event::Start(Tag::Paragraph) => {
                // Already in Plain by default.
            }
            Event::End(TagEnd::Paragraph) => {
                flush_into(&mut lines, &mut current_spans, &mut current_kind);
                lines.push(Line::blank());
            }
            Event::Start(Tag::List(_)) => {
                // List opens an implicit block; items will set Bullet kind.
            }
            Event::End(TagEnd::List(_)) => {
                lines.push(Line::blank());
            }
            Event::Start(Tag::Item) => {
                flush_into(&mut lines, &mut current_spans, &mut current_kind);
                current_kind = LineKind::Bullet;
            }
            Event::End(TagEnd::Item) => {
                flush_into(&mut lines, &mut current_spans, &mut current_kind);
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                flush_into(&mut lines, &mut current_spans, &mut current_kind);
                in_code_block = true;
                // We don't render the language tag in v1.
                let _ = kind; // keep `CodeBlockKind::Fenced(name)` available for future use
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                lines.push(Line::blank());
            }
            Event::Start(Tag::Strong) | Event::Start(Tag::Emphasis) => {
                active_style = SpanStyle::Bold;
            }
            Event::End(TagEnd::Strong) | Event::End(TagEnd::Emphasis) => {
                active_style = SpanStyle::Plain;
            }
            Event::Code(s) => {
                current_spans.push(Span {
                    text: s.to_string(),
                    style: SpanStyle::Code,
                });
            }
            Event::Text(s) => {
                if in_code_block {
                    // Code blocks may contain multiple lines internally.
                    for (i, l) in s.lines().enumerate() {
                        if i > 0 {
                            flush_into(&mut lines, &mut current_spans, &mut current_kind);
                        }
                        current_kind = LineKind::CodeBlock;
                        current_spans.push(Span {
                            text: l.to_string(),
                            style: SpanStyle::Code,
                        });
                    }
                } else {
                    current_spans.push(Span {
                        text: s.to_string(),
                        style: active_style,
                    });
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if in_code_block || matches!(current_kind, LineKind::Bullet) {
                    flush_into(&mut lines, &mut current_spans, &mut current_kind);
                } else {
                    current_spans.push(Span {
                        text: " ".to_string(),
                        style: active_style,
                    });
                }
            }
            // Tables, links, footnotes, html, rule, etc. — fall through as plain
            // text or ignore. v1 doesn't try to render them.
            _ => {}
        }
    }

    flush_into(&mut lines, &mut current_spans, &mut current_kind);

    // Trim trailing blank lines.
    while matches!(lines.last(), Some(l) if matches!(l.kind, LineKind::Blank)) {
        lines.pop();
    }

    lines
}

/// Plain text fallback (no markdown parsing). Used while a response is still
/// streaming — incomplete markdown could mis-parse.
pub fn plain_lines(text: &str) -> Vec<Line> {
    if text.is_empty() {
        return Vec::new();
    }
    text.lines().map(Line::plain).collect()
}

// =========================================================================
// Drop the CodeBlockKind import warning when unused
// =========================================================================
const _: fn() = || {
    let _ = CodeBlockKind::Fenced;
};

#[cfg(test)]
mod tests {
    use super::*;

    fn collect_text(lines: &[Line]) -> Vec<String> {
        lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.text.as_str()).collect::<String>())
            .collect()
    }

    #[test]
    fn plain_paragraph() {
        let out = parse_markdown("hello world");
        let text = collect_text(&out);
        assert_eq!(text, vec!["hello world"]);
    }

    #[test]
    fn bold_span_marked() {
        let out = parse_markdown("hello **bold** world");
        assert_eq!(out.len(), 1);
        let styles: Vec<_> = out[0].spans.iter().map(|s| s.style).collect();
        assert!(styles.contains(&SpanStyle::Bold));
        assert!(styles.contains(&SpanStyle::Plain));
    }

    #[test]
    fn inline_code_marked() {
        let out = parse_markdown("call `foo()` to start");
        assert!(out.iter().any(|l| {
            l.spans.iter().any(|s| s.style == SpanStyle::Code && s.text == "foo()")
        }));
    }

    #[test]
    fn code_block_lines() {
        let md = "```\nlet x = 1;\nlet y = 2;\n```";
        let out = parse_markdown(md);
        let code_lines: Vec<_> = out
            .iter()
            .filter(|l| matches!(l.kind, LineKind::CodeBlock))
            .collect();
        assert_eq!(code_lines.len(), 2);
    }

    #[test]
    fn bullet_list() {
        let md = "- alpha\n- beta\n- gamma";
        let out = parse_markdown(md);
        let bullets: Vec<_> = out
            .iter()
            .filter(|l| matches!(l.kind, LineKind::Bullet))
            .collect();
        assert_eq!(bullets.len(), 3);
    }

    #[test]
    fn heading_level() {
        let out = parse_markdown("# Title\n\n## Sub");
        let levels: Vec<u8> = out
            .iter()
            .filter_map(|l| match l.kind {
                LineKind::Heading { level } => Some(level),
                _ => None,
            })
            .collect();
        assert_eq!(levels, vec![1, 2]);
    }

    #[test]
    fn plain_lines_no_markdown() {
        let out = plain_lines("line one\nline two");
        assert_eq!(out.len(), 2);
        assert!(matches!(out[0].kind, LineKind::Plain));
    }
}
