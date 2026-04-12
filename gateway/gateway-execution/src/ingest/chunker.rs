//! Paragraph-aware text chunker.
//!
//! Splits prose into overlapping windows of target token count, preferring
//! paragraph boundaries (`\n\n`), falling back to sentence terminators
//! (`. ? !` followed by whitespace), falling back to character count.
//! Token estimation is char-count / 4 (GPT-4-family rule of thumb).

#[derive(Debug, Clone)]
pub struct ChunkOptions {
    pub target_tokens: usize,
    pub overlap_tokens: usize,
}

impl Default for ChunkOptions {
    fn default() -> Self {
        Self {
            target_tokens: 1000,
            overlap_tokens: 100,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    pub index: usize,
    pub text: String,
    pub char_start: usize,
    pub char_end: usize,
}

/// Estimate token count from char count. 4 chars/token is the GPT-4 rule of thumb.
pub fn estimate_tokens(text: &str) -> usize {
    text.len() / 4
}

/// Split `text` into overlapping chunks respecting paragraph boundaries.
pub fn chunk_text(text: &str, opts: ChunkOptions) -> Vec<Chunk> {
    if text.is_empty() {
        return Vec::new();
    }

    let target_chars = opts.target_tokens.saturating_mul(4);
    let overlap_chars = opts.overlap_tokens.saturating_mul(4);

    let mut chunks = Vec::new();
    let total = text.len();
    let mut cursor = 0usize;
    let mut index = 0usize;

    while cursor < total {
        let ideal_end = (cursor + target_chars).min(total);
        let end = if ideal_end < total {
            find_preferred_split(text, cursor, ideal_end)
        } else {
            ideal_end
        };

        let chunk_text_str = text[cursor..end].trim().to_string();
        if !chunk_text_str.is_empty() {
            chunks.push(Chunk {
                index,
                text: chunk_text_str,
                char_start: cursor,
                char_end: end,
            });
            index += 1;
        }

        if end >= total {
            break;
        }
        let next_cursor = end.saturating_sub(overlap_chars);
        cursor = if next_cursor <= cursor {
            end
        } else {
            next_cursor
        };
    }

    chunks
}

/// Find the best split point in `text[min..max]`:
/// 1. Latest `\n\n` >= min
/// 2. Otherwise latest sentence terminator (. ? !) followed by whitespace
/// 3. Otherwise `max` (hard cut)
fn find_preferred_split(text: &str, min: usize, max: usize) -> usize {
    let slice = &text[min..max];
    if let Some(idx) = slice.rfind("\n\n") {
        return min + idx + 2;
    }
    let bytes = slice.as_bytes();
    // Iterate from end; find last (terminator, whitespace) pair.
    let mut last: Option<usize> = None;
    for (i, _) in slice.char_indices() {
        let b = bytes[i];
        if (b == b'.' || b == b'?' || b == b'!') && i + 1 < bytes.len() {
            let next = bytes[i + 1];
            if next == b' ' || next == b'\n' {
                last = Some(i);
            }
        }
    }
    match last {
        Some(i) => min + i + 2,
        None => max,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_text_produces_no_chunks() {
        assert!(chunk_text("", ChunkOptions::default()).is_empty());
    }

    #[test]
    fn short_text_fits_in_one_chunk() {
        let text = "Just a brief sentence.";
        let chunks = chunk_text(text, ChunkOptions::default());
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, text);
        assert_eq!(chunks[0].index, 0);
    }

    #[test]
    fn paragraph_boundary_preferred() {
        let text =
            "Paragraph one here and it is quite long enough to exceed.\n\nParagraph two follows.";
        let opts = ChunkOptions {
            target_tokens: 15, // ~60 chars
            overlap_tokens: 0,
        };
        let chunks = chunk_text(text, opts);
        assert!(chunks.len() >= 2);
        assert!(chunks[0].text.contains("Paragraph one"));
        assert!(chunks[1].text.contains("Paragraph two"));
    }

    #[test]
    fn sentence_boundary_fallback() {
        let text =
            "First sentence. Second sentence. Third sentence. Fourth sentence. Fifth sentence.";
        let opts = ChunkOptions {
            target_tokens: 8, // ~32 chars
            overlap_tokens: 0,
        };
        let chunks = chunk_text(text, opts);
        assert!(chunks.len() >= 2);
        // Every chunk but possibly the last should end at a sentence boundary.
        for c in &chunks[..chunks.len() - 1] {
            assert!(
                c.text.ends_with('.') || c.text.ends_with('?') || c.text.ends_with('!'),
                "chunk does not end at sentence boundary: {}",
                c.text
            );
        }
    }

    #[test]
    fn chunks_overlap_when_configured() {
        let text = "The quick brown fox jumps over the lazy dog. ".repeat(20);
        let opts = ChunkOptions {
            target_tokens: 30,  // ~120 chars
            overlap_tokens: 10, // ~40 chars
        };
        let chunks = chunk_text(&text, opts);
        assert!(chunks.len() >= 2);
        // Overlap: chunk[0].char_end > chunk[1].char_start

        assert!(
            chunks[0].char_end > chunks[1].char_start,
            "expected overlap: end {} > start {}",
            chunks[0].char_end,
            chunks[1].char_start
        );
    }

    #[test]
    fn indices_are_sequential_from_zero() {
        let text = "alpha. beta. gamma. delta. epsilon. zeta. eta. theta. ".repeat(5);
        let opts = ChunkOptions {
            target_tokens: 15,
            overlap_tokens: 0,
        };
        let chunks = chunk_text(&text, opts);
        for (i, c) in chunks.iter().enumerate() {
            assert_eq!(c.index, i);
        }
    }

    #[test]
    fn estimate_tokens_is_chars_over_four() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("abcdefghijklmnop"), 4);
    }
}
