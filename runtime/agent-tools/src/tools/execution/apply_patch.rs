// ============================================================================
// APPLY PATCH
// Codex-compatible patch format parser and applicator.
// Provides a first-class ApplyPatchTool — no shell interception needed.
// ============================================================================

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{Value, json};
use thiserror::Error;

use zero_core::{FileSystemContext, Tool, ToolContext, ToolPermissions, ZeroError};

// ============================================================================
// ERROR TYPES
// ============================================================================

#[derive(Debug, Error, PartialEq, Clone)]
pub enum PatchError {
    #[error("invalid patch: {0}")]
    InvalidPatch(String),
    #[error("invalid hunk at line {line_number}: {message}")]
    InvalidHunk { message: String, line_number: usize },
    #[error("apply error: {0}")]
    ApplyError(String),
    #[error("I/O error: {0}")]
    IoError(String),
}

// ============================================================================
// PATCH DATA TYPES
// ============================================================================

/// A single patch hunk — add, delete, or update a file.
#[derive(Debug, PartialEq, Clone)]
pub enum Hunk {
    AddFile {
        path: PathBuf,
        contents: String,
    },
    DeleteFile {
        path: PathBuf,
    },
    UpdateFile {
        path: PathBuf,
        move_path: Option<PathBuf>,
        chunks: Vec<UpdateFileChunk>,
    },
}

/// A contiguous change within an UpdateFile hunk.
#[derive(Debug, PartialEq, Clone)]
pub struct UpdateFileChunk {
    /// Optional context line (class/function header) used to locate the change.
    pub change_context: Option<String>,
    /// Lines to remove (context + deleted lines from the old file).
    pub old_lines: Vec<String>,
    /// Lines to insert (context + added lines for the new file).
    pub new_lines: Vec<String>,
    /// If true, old_lines should match at end of file.
    pub is_end_of_file: bool,
}

// ============================================================================
// MARKERS
// ============================================================================

const BEGIN_PATCH: &str = "*** Begin Patch";
const END_PATCH: &str = "*** End Patch";
const ADD_FILE: &str = "*** Add File: ";
const DELETE_FILE: &str = "*** Delete File: ";
const UPDATE_FILE: &str = "*** Update File: ";
const MOVE_TO: &str = "*** Move to: ";
const EOF_MARKER: &str = "*** End of File";
const CONTEXT_MARKER: &str = "@@ ";
const EMPTY_CONTEXT: &str = "@@";

/// Check if a line is a patch format marker (not file content).
fn is_patch_marker(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with("*** ") || trimmed == "@@" || trimmed.starts_with("@@ ")
}

// ============================================================================
// PARSER
// ============================================================================

/// Parse a patch string into a list of hunks.
/// Supports heredoc-wrapped patches (<<'EOF' ... EOF).
pub fn parse_patch(patch: &str) -> Result<Vec<Hunk>, PatchError> {
    let lines: Vec<&str> = patch.trim().lines().collect();
    let inner = match check_boundaries(&lines) {
        Ok(()) => &lines[..],
        Err(direct_err) => try_heredoc_unwrap(&lines).map_err(|_| direct_err)?,
    };

    let last = inner.len().saturating_sub(1);
    let mut remaining = &inner[1..last];
    let mut line_number = 2;
    let mut hunks = Vec::new();

    while !remaining.is_empty() {
        let (hunk, consumed) = parse_one_hunk(remaining, line_number)?;
        hunks.push(hunk);
        line_number += consumed;
        remaining = &remaining[consumed..];
    }

    Ok(hunks)
}

fn check_boundaries(lines: &[&str]) -> Result<(), PatchError> {
    let first = lines.first().map(|l| l.trim());
    let last = lines.last().map(|l| l.trim());
    // Tolerate LLMs prefixing *** End Patch with '+' (treats it as content in Add File blocks)
    let last_normalized = last.map(|l| l.strip_prefix('+').unwrap_or(l).trim_start());
    match (first, last_normalized) {
        (Some(f), Some(l)) if f == BEGIN_PATCH && (l == END_PATCH || l.starts_with(END_PATCH)) => {
            Ok(())
        }
        (Some(f), _) if f != BEGIN_PATCH => Err(PatchError::InvalidPatch(format!(
            "Patch must start with '*** Begin Patch'. Got: '{}'. \
             Format: apply_patch <<'EOF'\\n*** Begin Patch\\n*** Add File: path\\n+content\\n*** End Patch\\nEOF",
            f
        ))),
        _ => Err(PatchError::InvalidPatch(format!(
            "Patch must end with '*** End Patch'. Got: '{}'. \
             Format: apply_patch <<'EOF'\\n*** Begin Patch\\n*** Add File: path\\n+content\\n*** End Patch\\nEOF",
            last.unwrap_or("(empty)")
        ))),
    }
}

fn try_heredoc_unwrap<'a>(lines: &'a [&'a str]) -> Result<&'a [&'a str], PatchError> {
    if lines.len() >= 4 {
        let first = lines[0];
        let last = lines[lines.len() - 1];
        if (first == "<<EOF" || first == "<<'EOF'" || first == "<<\"EOF\"") && last.ends_with("EOF")
        {
            let inner = &lines[1..lines.len() - 1];
            check_boundaries(inner)?;
            return Ok(inner);
        }
    }
    Err(PatchError::InvalidPatch(
        "Patch must start with '*** Begin Patch'. \
         Format: apply_patch <<'EOF'\\n*** Begin Patch\\n*** Add File: path\\n+content\\n*** End Patch\\nEOF".into(),
    ))
}

fn parse_one_hunk(lines: &[&str], line_number: usize) -> Result<(Hunk, usize), PatchError> {
    let first = lines[0].trim();

    if let Some(path) = first.strip_prefix(ADD_FILE) {
        let mut contents = String::new();
        let mut consumed = 1;
        for line in &lines[1..] {
            if let Some(added) = line.strip_prefix('+') {
                // Standard: line has + prefix
                contents.push_str(added);
                contents.push('\n');
                consumed += 1;
            } else if !is_patch_marker(line) && !line.trim().is_empty() {
                // Lenient: auto-fix missing + prefix for content lines
                contents.push_str(line);
                contents.push('\n');
                consumed += 1;
            } else if line.trim().is_empty() {
                // Preserve blank lines in file content
                contents.push('\n');
                consumed += 1;
            } else {
                // Hit a patch marker — stop
                break;
            }
        }
        return Ok((
            Hunk::AddFile {
                path: PathBuf::from(path),
                contents,
            },
            consumed,
        ));
    }

    if let Some(path) = first.strip_prefix(DELETE_FILE) {
        return Ok((
            Hunk::DeleteFile {
                path: PathBuf::from(path),
            },
            1,
        ));
    }

    if let Some(path) = first.strip_prefix(UPDATE_FILE) {
        let mut remaining = &lines[1..];
        let mut consumed = 1;

        // Optional move
        let move_path = remaining
            .first()
            .and_then(|l| l.strip_prefix(MOVE_TO))
            .map(PathBuf::from);
        if move_path.is_some() {
            remaining = &remaining[1..];
            consumed += 1;
        }

        let mut chunks = Vec::new();
        while !remaining.is_empty() {
            // Skip blank lines between chunks
            if remaining[0].trim().is_empty() {
                consumed += 1;
                remaining = &remaining[1..];
                continue;
            }
            // Stop at next hunk header
            if remaining[0].starts_with("***") {
                break;
            }
            let (chunk, chunk_lines) =
                parse_chunk(remaining, line_number + consumed, chunks.is_empty())?;
            chunks.push(chunk);
            consumed += chunk_lines;
            remaining = &remaining[chunk_lines..];
        }

        if chunks.is_empty() {
            return Err(PatchError::InvalidHunk {
                message: format!("Update hunk for '{}' has no chunks", path),
                line_number,
            });
        }

        return Ok((
            Hunk::UpdateFile {
                path: PathBuf::from(path),
                move_path,
                chunks,
            },
            consumed,
        ));
    }

    Err(PatchError::InvalidHunk {
        message: format!(
            "Expected a file operation (*** Add File: / *** Update File: / *** Delete File:). \
             Got: '{}'. If adding a file, each content line must start with '+'.",
            first
        ),
        line_number,
    })
}

fn parse_chunk(
    lines: &[&str],
    line_number: usize,
    allow_missing_context: bool,
) -> Result<(UpdateFileChunk, usize), PatchError> {
    if lines.is_empty() {
        return Err(PatchError::InvalidHunk {
            message: "Empty update chunk".into(),
            line_number,
        });
    }

    let (context, start) = if lines[0] == EMPTY_CONTEXT {
        (None, 1)
    } else if let Some(ctx) = lines[0].strip_prefix(CONTEXT_MARKER) {
        (Some(ctx.to_string()), 1)
    } else if allow_missing_context {
        (None, 0)
    } else {
        return Err(PatchError::InvalidHunk {
            message: format!("Expected @@ context marker, got: '{}'", lines[0]),
            line_number,
        });
    };

    if start >= lines.len() {
        return Err(PatchError::InvalidHunk {
            message: "Update chunk has no diff lines".into(),
            line_number: line_number + 1,
        });
    }

    let mut chunk = UpdateFileChunk {
        change_context: context,
        old_lines: Vec::new(),
        new_lines: Vec::new(),
        is_end_of_file: false,
    };
    let mut parsed = 0;

    for line in &lines[start..] {
        if *line == EOF_MARKER {
            if parsed == 0 {
                return Err(PatchError::InvalidHunk {
                    message: "Update chunk has no diff lines".into(),
                    line_number: line_number + 1,
                });
            }
            chunk.is_end_of_file = true;
            parsed += 1;
            break;
        }

        match line.chars().next() {
            None => {
                // Empty line treated as context
                chunk.old_lines.push(String::new());
                chunk.new_lines.push(String::new());
            }
            Some(' ') => {
                chunk.old_lines.push(line[1..].to_string());
                chunk.new_lines.push(line[1..].to_string());
            }
            Some('+') => {
                chunk.new_lines.push(line[1..].to_string());
            }
            Some('-') => {
                chunk.old_lines.push(line[1..].to_string());
            }
            _ => {
                if parsed == 0 {
                    return Err(PatchError::InvalidHunk {
                        message: format!(
                            "Unexpected line in update chunk: '{}'. Lines must start with ' ', '+', or '-'",
                            line
                        ),
                        line_number: line_number + 1,
                    });
                }
                // Next hunk header
                break;
            }
        }
        parsed += 1;
    }

    Ok((chunk, parsed + start))
}

// ============================================================================
// SEEK SEQUENCE (fuzzy line matching)
// ============================================================================

/// Find `pattern` lines within `lines` starting at or after `start`.
/// Tries exact match, then trim-end, then trim-both, then Unicode-normalized.
fn seek_sequence(lines: &[String], pattern: &[String], start: usize, eof: bool) -> Option<usize> {
    if pattern.is_empty() {
        return Some(start);
    }
    if pattern.len() > lines.len() {
        return None;
    }

    let search_start = if eof && lines.len() >= pattern.len() {
        lines.len() - pattern.len()
    } else {
        start
    };

    // Exact match
    for i in search_start..=lines.len().saturating_sub(pattern.len()) {
        if lines[i..i + pattern.len()] == *pattern {
            return Some(i);
        }
    }

    // Trim-end match
    for i in search_start..=lines.len().saturating_sub(pattern.len()) {
        if pattern
            .iter()
            .enumerate()
            .all(|(j, p)| lines[i + j].trim_end() == p.trim_end())
        {
            return Some(i);
        }
    }

    // Trim-both match
    for i in search_start..=lines.len().saturating_sub(pattern.len()) {
        if pattern
            .iter()
            .enumerate()
            .all(|(j, p)| lines[i + j].trim() == p.trim())
        {
            return Some(i);
        }
    }

    // Unicode-normalized match
    for i in search_start..=lines.len().saturating_sub(pattern.len()) {
        if pattern
            .iter()
            .enumerate()
            .all(|(j, p)| normalize_unicode(&lines[i + j]) == normalize_unicode(p))
        {
            return Some(i);
        }
    }

    None
}

/// Normalize common Unicode punctuation to ASCII equivalents.
fn normalize_unicode(s: &str) -> String {
    s.trim()
        .chars()
        .map(|c| match c {
            '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}' | '\u{2015}'
            | '\u{2212}' => '-',
            '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}' => '\'',
            '\u{201C}' | '\u{201D}' | '\u{201E}' | '\u{201F}' => '"',
            '\u{00A0}' | '\u{2002}' | '\u{2003}' | '\u{2004}' | '\u{2005}' | '\u{2006}'
            | '\u{2007}' | '\u{2008}' | '\u{2009}' | '\u{200A}' | '\u{202F}' | '\u{205F}'
            | '\u{3000}' => ' ',
            other => other,
        })
        .collect()
}

// ============================================================================
// WARD ENFORCEMENT
// ============================================================================

/// Check if a cwd path is inside a wards/ directory (but not the scratch ward).
fn is_ward_context(cwd: &Path) -> bool {
    let cwd_str = cwd.to_string_lossy();
    // Normalize separators for cross-platform matching
    let normalized = cwd_str.replace('\\', "/");
    // Must contain /wards/ and NOT end with /wards/scratch (or be inside it)
    if let Some(pos) = normalized.rfind("/wards/") {
        let after_wards = &normalized[pos + 7..]; // after "/wards/"
        // scratch ward is exempt from root-file checks
        !after_wards.starts_with("scratch")
    } else {
        false
    }
}

/// Check if a file path has no directory component (lives in the ward root).
fn is_ward_root_file(path: &str) -> bool {
    let p = Path::new(path);
    match p.parent() {
        None => true,
        Some(parent) => parent == Path::new("") || parent == Path::new("."),
    }
}

/// Check if a filename is in the allow-list for ward root files.
fn is_allowed_root_file(filename: &str) -> bool {
    let name_lower = filename.to_lowercase();
    name_lower == "agents.md"
        || name_lower == "__init__.py"
        || name_lower == "requirements.txt"
        || name_lower == ".gitignore"
}

/// Check if a filename matches variant-file patterns (e.g. `_v2`, `_fixed`, `_3`).
fn is_variant_filename(filename: &str) -> bool {
    let stem = Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let stem_lower = stem.to_lowercase();

    // Check _v2, _v3, etc. (but allow _v1 as a legitimate version 1)
    if let Some(pos) = stem_lower.rfind("_v") {
        let after = &stem_lower[pos + 2..];
        if !after.is_empty()
            && after.chars().all(|c| c.is_ascii_digit())
            && after.parse::<u32>().map(|n| n >= 2).unwrap_or(false)
        {
            return true;
        }
    }

    // Check _fixed, _improved, _new, _backup, _copy, _old, _temp
    for suffix in &[
        "_fixed",
        "_improved",
        "_new",
        "_backup",
        "_copy",
        "_old",
        "_temp",
    ] {
        if stem_lower.ends_with(suffix) {
            return true;
        }
    }

    // Check _2, _3, _4 (numbered suffixes >= 2, at most 2 digits)
    if let Some(pos) = stem_lower.rfind('_') {
        let after = &stem_lower[pos + 1..];
        if after.len() <= 2
            && !after.is_empty()
            && after.chars().all(|c| c.is_ascii_digit())
            && after.parse::<u32>().map(|n| n >= 2).unwrap_or(false)
        {
            return true;
        }
    }

    false
}

// ============================================================================
// APPLICATOR
// ============================================================================

/// Result of applying a patch.
#[derive(Debug)]
pub struct PatchResult {
    pub added: Vec<PathBuf>,
    pub modified: Vec<PathBuf>,
    pub deleted: Vec<PathBuf>,
}

impl PatchResult {
    /// Format a summary like: "A path/new.rs\nM path/existing.rs\nD path/old.rs"
    pub fn summary(&self) -> String {
        let mut out = String::from("Patch applied successfully.\n");
        for p in &self.added {
            out.push_str(&format!("A {}\n", p.display()));
        }
        for p in &self.modified {
            out.push_str(&format!("M {}\n", p.display()));
        }
        for p in &self.deleted {
            out.push_str(&format!("D {}\n", p.display()));
        }
        out
    }
}

/// Apply parsed hunks to the filesystem. Paths are resolved relative to `cwd`.
pub fn apply_hunks(hunks: &[Hunk], cwd: &Path) -> Result<PatchResult, PatchError> {
    if hunks.is_empty() {
        return Err(PatchError::ApplyError("No hunks to apply".into()));
    }

    let ward_active = is_ward_context(cwd);

    let mut added = Vec::new();
    let mut modified = Vec::new();
    let mut deleted = Vec::new();

    for hunk in hunks {
        match hunk {
            Hunk::AddFile { path, contents } => {
                if ward_active {
                    let path_str = path.to_string_lossy();
                    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

                    // Block files in ward root (no directory component)
                    if is_ward_root_file(&path_str) && !is_allowed_root_file(filename) {
                        return Err(PatchError::ApplyError(format!(
                            "Error: Cannot create files in ward root. Put files in core/, stocks/{{ticker}}/, or output/. Got: {}",
                            filename
                        )));
                    }

                    // Block variant filenames
                    if is_variant_filename(filename) {
                        return Err(PatchError::ApplyError(format!(
                            "Error: Do not create variant files. Fix the original instead. Got: {}",
                            filename
                        )));
                    }
                }

                let full = cwd.join(path);
                if let Some(parent) = full.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| {
                        PatchError::IoError(format!(
                            "Failed to create directories for {}: {}",
                            full.display(),
                            e
                        ))
                    })?;
                }
                std::fs::write(&full, contents).map_err(|e| {
                    PatchError::IoError(format!("Failed to write {}: {}", full.display(), e))
                })?;
                added.push(path.clone());
            }
            Hunk::DeleteFile { path } => {
                // Variant check on delete (allow cleaning up, but still block variant names)
                if ward_active {
                    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if is_variant_filename(filename) {
                        return Err(PatchError::ApplyError(format!(
                            "Error: Do not create variant files. Fix the original instead. Got: {}",
                            filename
                        )));
                    }
                }

                let full = cwd.join(path);
                std::fs::remove_file(&full).map_err(|e| {
                    PatchError::IoError(format!("Failed to delete {}: {}", full.display(), e))
                })?;
                deleted.push(path.clone());
            }
            Hunk::UpdateFile {
                path,
                move_path,
                chunks,
            } => {
                // Variant check on update (not root check — updating root files is fine)
                if ward_active {
                    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if is_variant_filename(filename) {
                        return Err(PatchError::ApplyError(format!(
                            "Error: Do not create variant files. Fix the original instead. Got: {}",
                            filename
                        )));
                    }
                    // Also check the move destination if present
                    if let Some(dest) = move_path {
                        let dest_name = dest.file_name().and_then(|n| n.to_str()).unwrap_or("");
                        if is_variant_filename(dest_name) {
                            return Err(PatchError::ApplyError(format!(
                                "Error: Do not create variant files. Fix the original instead. Got: {}",
                                dest_name
                            )));
                        }
                    }
                }

                let full = cwd.join(path);
                let new_contents = derive_new_contents(&full, chunks)?;

                if let Some(dest) = move_path {
                    let dest_full = cwd.join(dest);
                    if let Some(parent) = dest_full.parent() {
                        std::fs::create_dir_all(parent).map_err(|e| {
                            PatchError::IoError(format!(
                                "Failed to create directories for {}: {}",
                                dest_full.display(),
                                e
                            ))
                        })?;
                    }
                    std::fs::write(&dest_full, &new_contents).map_err(|e| {
                        PatchError::IoError(format!(
                            "Failed to write {}: {}",
                            dest_full.display(),
                            e
                        ))
                    })?;
                    std::fs::remove_file(&full).map_err(|e| {
                        PatchError::IoError(format!(
                            "Failed to remove original {}: {}",
                            full.display(),
                            e
                        ))
                    })?;
                    modified.push(dest.clone());
                } else {
                    std::fs::write(&full, &new_contents).map_err(|e| {
                        PatchError::IoError(format!("Failed to write {}: {}", full.display(), e))
                    })?;
                    modified.push(path.clone());
                }
            }
        }
    }

    Ok(PatchResult {
        added,
        modified,
        deleted,
    })
}

/// Compute new file contents by applying chunks to the existing file.
fn derive_new_contents(path: &Path, chunks: &[UpdateFileChunk]) -> Result<String, PatchError> {
    let original = std::fs::read_to_string(path)
        .map_err(|e| PatchError::IoError(format!("Failed to read {}: {}", path.display(), e)))?;

    let mut lines: Vec<String> = original.split('\n').map(String::from).collect();
    // Drop trailing empty element from final newline
    if lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }

    let replacements = compute_replacements(&lines, path, chunks)?;
    let mut result = apply_replacements(lines, &replacements);

    // Ensure trailing newline
    if !result.last().is_some_and(String::is_empty) {
        result.push(String::new());
    }

    Ok(result.join("\n"))
}

/// Compute (start_index, old_len, new_lines) replacements from chunks.
fn compute_replacements(
    original: &[String],
    path: &Path,
    chunks: &[UpdateFileChunk],
) -> Result<Vec<(usize, usize, Vec<String>)>, PatchError> {
    let mut replacements = Vec::new();
    let mut line_idx: usize = 0;

    for chunk in chunks {
        // Locate context line
        if let Some(ctx) = &chunk.change_context {
            if let Some(idx) = seek_sequence(original, std::slice::from_ref(ctx), line_idx, false) {
                line_idx = idx + 1;
            } else {
                return Err(PatchError::ApplyError(format!(
                    "Context '{}' not found in {}",
                    ctx,
                    path.display()
                )));
            }
        }

        if chunk.old_lines.is_empty() {
            // Pure addition at end of file
            let insertion = if original.last().is_some_and(String::is_empty) {
                original.len() - 1
            } else {
                original.len()
            };
            replacements.push((insertion, 0, chunk.new_lines.clone()));
            continue;
        }

        // Find old_lines in file
        let mut pattern: &[String] = &chunk.old_lines;
        let mut found = seek_sequence(original, pattern, line_idx, chunk.is_end_of_file);
        let mut new_slice: &[String] = &chunk.new_lines;

        // Retry without trailing empty line
        if found.is_none() && pattern.last().is_some_and(String::is_empty) {
            pattern = &pattern[..pattern.len() - 1];
            if new_slice.last().is_some_and(String::is_empty) {
                new_slice = &new_slice[..new_slice.len() - 1];
            }
            found = seek_sequence(original, pattern, line_idx, chunk.is_end_of_file);
        }

        if let Some(start) = found {
            replacements.push((start, pattern.len(), new_slice.to_vec()));
            line_idx = start + pattern.len();
        } else {
            return Err(PatchError::ApplyError(format!(
                "Failed to find expected lines in {}:\n{}",
                path.display(),
                chunk.old_lines.join("\n"),
            )));
        }
    }

    replacements.sort_by_key(|(idx, _, _)| *idx);
    Ok(replacements)
}

/// Apply replacements in reverse order to preserve indices.
fn apply_replacements(
    mut lines: Vec<String>,
    replacements: &[(usize, usize, Vec<String>)],
) -> Vec<String> {
    for (start, old_len, new_segment) in replacements.iter().rev() {
        let start = *start;
        let old_len = *old_len;

        for _ in 0..old_len {
            if start < lines.len() {
                lines.remove(start);
            }
        }
        for (offset, new_line) in new_segment.iter().enumerate() {
            lines.insert(start + offset, new_line.clone());
        }
    }
    lines
}

// ============================================================================
// SHELL INTERCEPTOR (legacy, retained for tests)
// ============================================================================

/// Check if a shell command is an apply_patch invocation.
/// Returns the patch text if detected, None otherwise.
///
/// Note: No longer used at runtime — the first-class `ApplyPatchTool` handles
/// patch operations directly. Retained for backward-compatible tests.
#[allow(dead_code)]
pub fn detect_apply_patch(command: &str) -> Option<String> {
    let trimmed = command.trim();

    // Direct invocation: command starts with "apply_patch"
    if trimmed.starts_with("apply_patch") {
        // Extract everything after "apply_patch" as the patch argument
        let rest = trimmed.strip_prefix("apply_patch").unwrap_or("").trim();

        // Handle heredoc: apply_patch <<'EOF'\n...\nEOF
        if rest.starts_with("<<") {
            // The heredoc content is the patch
            // Find the delimiter (EOF, 'EOF', "EOF")
            let after_redirect = rest.strip_prefix("<<").unwrap_or("");
            let (delimiter, content_start) = extract_heredoc_delimiter(after_redirect);

            if let Some(end_pos) = content_start.rfind(&delimiter) {
                let patch_text = &content_start[..end_pos];
                return Some(patch_text.trim().to_string());
            }

            // If no matching delimiter, treat the rest as patch text
            return Some(content_start.trim().to_string());
        }

        // Direct argument
        if !rest.is_empty() {
            return Some(rest.to_string());
        }
    }

    // Also detect if command body contains *** Begin Patch (piped or embedded)
    if trimmed.contains(BEGIN_PATCH) && trimmed.contains(END_PATCH) {
        // Extract the patch between markers
        if let Some(start) = trimmed.find(BEGIN_PATCH) {
            if let Some(end) = trimmed.find(END_PATCH) {
                let patch = &trimmed[start..end + END_PATCH.len()];
                return Some(patch.to_string());
            }
        }
    }

    None
}

#[allow(dead_code)]
fn extract_heredoc_delimiter(s: &str) -> (String, &str) {
    let s = s.trim_start();

    // Handle quoted delimiters: 'EOF' or "EOF"
    if s.starts_with('\'') {
        if let Some(end) = s[1..].find('\'') {
            let delim = &s[1..end + 1];
            return (delim.to_string(), &s[end + 2..]);
        }
    }
    if s.starts_with('"') {
        if let Some(end) = s[1..].find('"') {
            let delim = &s[1..end + 1];
            return (delim.to_string(), &s[end + 2..]);
        }
    }

    // Unquoted: take until whitespace or newline
    let end = s.find(|c: char| c.is_whitespace()).unwrap_or(s.len());
    let delim = &s[..end];
    (delim.to_string(), &s[end..])
}

/// Intercept an apply_patch command within a shell invocation.
/// Returns Some(result_string) if handled, None if not an apply_patch command.
///
/// Note: No longer used at runtime — retained for backward-compatible tests.
#[allow(dead_code)]
pub fn intercept_apply_patch(command: &str, cwd: &Path) -> Option<Result<String, PatchError>> {
    let patch_text = detect_apply_patch(command)?;

    let hunks = match parse_patch(&patch_text) {
        Ok(h) => h,
        Err(e) => return Some(Err(e)),
    };

    match apply_hunks(&hunks, cwd) {
        Ok(result) => Some(Ok(result.summary())),
        Err(e) => Some(Err(e)),
    }
}

// ============================================================================
// APPLY PATCH TOOL (first-class tool, not via shell)
// ============================================================================

/// First-class tool for file creation, editing, and deletion via patch format.
/// Receives patch text directly as a JSON parameter — no shell, no heredoc, no quoting issues.
pub struct ApplyPatchTool {
    fs: Arc<dyn FileSystemContext>,
}

impl ApplyPatchTool {
    pub fn new(fs: Arc<dyn FileSystemContext>) -> Self {
        Self { fs }
    }
}

#[async_trait]
impl Tool for ApplyPatchTool {
    fn name(&self) -> &str {
        "apply_patch"
    }

    fn description(&self) -> &str {
        "Create, edit, or delete files. Patch format:\n\
         *** Begin Patch\n\
         *** Add File: path/file.py    (new file, lines start with +)\n\
         *** Update File: path/file.py (edit, hunks with @@, lines: ' ' context, '-' remove, '+' add)\n\
         *** Delete File: path/file.py (remove file)\n\
         *** End Patch\n\n\
         Every content line in Add File MUST start with '+'. Paths relative to ward."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "patch": {
                    "type": "string",
                    "description": "The patch content. Must start with '*** Begin Patch' and end with '*** End Patch'."
                }
            },
            "required": ["patch"]
        }))
    }

    fn permissions(&self) -> ToolPermissions {
        ToolPermissions::moderate(vec!["filesystem:write".into()])
    }

    async fn execute(&self, ctx: Arc<dyn ToolContext>, args: Value) -> zero_core::Result<Value> {
        let patch_text = args
            .get("patch")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ZeroError::Tool("Missing 'patch' parameter".to_string()))?;

        // Resolve CWD from ward context
        let cwd = resolve_ward_cwd(&self.fs, &ctx);

        // Parse the patch
        let hunks = match parse_patch(patch_text) {
            Ok(h) => h,
            Err(e) => {
                return Ok(json!({
                    "success": false,
                    "error": e.to_string(),
                }));
            }
        };

        if hunks.is_empty() {
            return Ok(json!({
                "success": false,
                "error": "Patch contains no file operations.",
            }));
        }

        // Apply the hunks
        match apply_hunks(&hunks, &cwd) {
            Ok(result) => Ok(json!({
                "success": true,
                "summary": result.summary(),
            })),
            Err(e) => Ok(json!({
                "success": false,
                "error": e.to_string(),
            })),
        }
    }
}

/// Resolve the working directory for patch operations.
/// Uses the active ward directory if available, otherwise falls back to home/Documents/zbot/wards/scratch.
pub fn resolve_ward_cwd(fs: &Arc<dyn FileSystemContext>, ctx: &Arc<dyn ToolContext>) -> PathBuf {
    // Try to get ward_id from context
    let ward_id = ctx
        .get_state("ward_id")
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_else(|| "scratch".to_string());

    // Try ward_dir from FileSystemContext
    if let Some(dir) = fs.ward_dir(&ward_id) {
        return dir;
    }

    // Fallback: use Documents/zbot/wards/{ward_id}
    if let Some(doc_dir) = dirs::document_dir().or_else(dirs::home_dir) {
        return doc_dir.join("zbot").join("wards").join(&ward_id);
    }

    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn wrap(body: &str) -> String {
        format!("*** Begin Patch\n{body}\n*** End Patch")
    }

    // ---- Parser tests ----

    #[test]
    fn test_parse_add_file() {
        let patch = wrap("*** Add File: foo.txt\n+hello\n+world");
        let hunks = parse_patch(&patch).unwrap();
        assert_eq!(hunks.len(), 1);
        assert_eq!(
            hunks[0],
            Hunk::AddFile {
                path: PathBuf::from("foo.txt"),
                contents: "hello\nworld\n".to_string()
            }
        );
    }

    #[test]
    fn test_parse_delete_file() {
        let patch = wrap("*** Delete File: old.txt");
        let hunks = parse_patch(&patch).unwrap();
        assert_eq!(
            hunks[0],
            Hunk::DeleteFile {
                path: PathBuf::from("old.txt")
            }
        );
    }

    #[test]
    fn test_parse_update_file() {
        let patch =
            wrap("*** Update File: main.rs\n@@ fn main\n context\n-old line\n+new line\n context2");
        let hunks = parse_patch(&patch).unwrap();
        match &hunks[0] {
            Hunk::UpdateFile { path, chunks, .. } => {
                assert_eq!(path, &PathBuf::from("main.rs"));
                assert_eq!(chunks.len(), 1);
                assert_eq!(chunks[0].change_context, Some("fn main".to_string()));
                assert_eq!(chunks[0].old_lines, vec!["context", "old line", "context2"]);
                assert_eq!(chunks[0].new_lines, vec!["context", "new line", "context2"]);
            }
            _ => panic!("Expected UpdateFile"),
        }
    }

    #[test]
    fn test_parse_multi_hunk() {
        let patch = wrap(
            "*** Add File: a.txt\n+line\n*** Delete File: b.txt\n*** Update File: c.txt\n@@\n-old\n+new",
        );
        let hunks = parse_patch(&patch).unwrap();
        assert_eq!(hunks.len(), 3);
    }

    #[test]
    fn test_parse_heredoc_wrapped() {
        let patch = "<<'EOF'\n*** Begin Patch\n*** Add File: test.txt\n+hi\n*** End Patch\nEOF";
        let hunks = parse_patch(patch).unwrap();
        assert_eq!(hunks.len(), 1);
    }

    #[test]
    fn test_parse_invalid_no_begin() {
        assert!(parse_patch("bad patch").is_err());
    }

    #[test]
    fn test_parse_empty_update_rejected() {
        let patch = wrap("*** Update File: empty.txt");
        assert!(parse_patch(&patch).is_err());
    }

    #[test]
    fn test_parse_update_without_context_marker() {
        // First chunk can omit @@ if it starts directly with diff lines
        let patch = "*** Begin Patch\n*** Update File: file.py\n import foo\n+bar\n*** End Patch";
        let hunks = parse_patch(patch).unwrap();
        match &hunks[0] {
            Hunk::UpdateFile { chunks, .. } => {
                assert_eq!(chunks[0].old_lines, vec!["import foo"]);
                assert_eq!(chunks[0].new_lines, vec!["import foo", "bar"]);
            }
            _ => panic!("Expected UpdateFile"),
        }
    }

    #[test]
    fn test_parse_end_of_file_marker() {
        let patch = wrap("*** Update File: f.txt\n@@\n+appended\n*** End of File");
        let hunks = parse_patch(&patch).unwrap();
        match &hunks[0] {
            Hunk::UpdateFile { chunks, .. } => {
                assert!(chunks[0].is_end_of_file);
            }
            _ => panic!("Expected UpdateFile"),
        }
    }

    #[test]
    fn test_parse_move_file() {
        let patch = wrap("*** Update File: old.rs\n*** Move to: new.rs\n@@\n-old\n+new");
        let hunks = parse_patch(&patch).unwrap();
        match &hunks[0] {
            Hunk::UpdateFile { move_path, .. } => {
                assert_eq!(move_path, &Some(PathBuf::from("new.rs")));
            }
            _ => panic!("Expected UpdateFile"),
        }
    }

    // ---- Applicator tests ----

    #[test]
    fn test_apply_add_file() {
        let dir = tempfile::tempdir().unwrap();
        let hunks = vec![Hunk::AddFile {
            path: PathBuf::from("new.txt"),
            contents: "hello\n".to_string(),
        }];
        let result = apply_hunks(&hunks, dir.path()).unwrap();
        assert_eq!(result.added.len(), 1);
        let content = fs::read_to_string(dir.path().join("new.txt")).unwrap();
        assert_eq!(content, "hello\n");
    }

    #[test]
    fn test_apply_delete_file() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("del.txt"), "x").unwrap();
        let hunks = vec![Hunk::DeleteFile {
            path: PathBuf::from("del.txt"),
        }];
        let result = apply_hunks(&hunks, dir.path()).unwrap();
        assert_eq!(result.deleted.len(), 1);
        assert!(!dir.path().join("del.txt").exists());
    }

    #[test]
    fn test_apply_update_file() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("upd.txt"), "foo\nbar\n").unwrap();
        let hunks = vec![Hunk::UpdateFile {
            path: PathBuf::from("upd.txt"),
            move_path: None,
            chunks: vec![UpdateFileChunk {
                change_context: None,
                old_lines: vec!["foo".to_string(), "bar".to_string()],
                new_lines: vec!["foo".to_string(), "baz".to_string()],
                is_end_of_file: false,
            }],
        }];
        let result = apply_hunks(&hunks, dir.path()).unwrap();
        assert_eq!(result.modified.len(), 1);
        let content = fs::read_to_string(dir.path().join("upd.txt")).unwrap();
        assert_eq!(content, "foo\nbaz\n");
    }

    #[test]
    fn test_apply_multiple_chunks() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("multi.txt"), "foo\nbar\nbaz\nqux\n").unwrap();
        let hunks = vec![Hunk::UpdateFile {
            path: PathBuf::from("multi.txt"),
            move_path: None,
            chunks: vec![
                UpdateFileChunk {
                    change_context: None,
                    old_lines: vec!["foo".to_string(), "bar".to_string()],
                    new_lines: vec!["foo".to_string(), "BAR".to_string()],
                    is_end_of_file: false,
                },
                UpdateFileChunk {
                    change_context: None,
                    old_lines: vec!["baz".to_string(), "qux".to_string()],
                    new_lines: vec!["baz".to_string(), "QUX".to_string()],
                    is_end_of_file: false,
                },
            ],
        }];
        let result = apply_hunks(&hunks, dir.path()).unwrap();
        assert_eq!(result.modified.len(), 1);
        let content = fs::read_to_string(dir.path().join("multi.txt")).unwrap();
        assert_eq!(content, "foo\nBAR\nbaz\nQUX\n");
    }

    #[test]
    fn test_apply_with_context_search() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("ctx.txt"),
            "fn main() {\n    let x = 1;\n    let y = 2;\n}\n",
        )
        .unwrap();
        let hunks = vec![Hunk::UpdateFile {
            path: PathBuf::from("ctx.txt"),
            move_path: None,
            chunks: vec![UpdateFileChunk {
                change_context: Some("fn main() {".to_string()),
                old_lines: vec!["    let x = 1;".to_string()],
                new_lines: vec!["    let x = 42;".to_string()],
                is_end_of_file: false,
            }],
        }];
        let result = apply_hunks(&hunks, dir.path()).unwrap();
        assert_eq!(result.modified.len(), 1);
        let content = fs::read_to_string(dir.path().join("ctx.txt")).unwrap();
        assert!(content.contains("let x = 42;"));
    }

    #[test]
    fn test_apply_move_file() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("src.txt"), "line\n").unwrap();
        let hunks = vec![Hunk::UpdateFile {
            path: PathBuf::from("src.txt"),
            move_path: Some(PathBuf::from("dst.txt")),
            chunks: vec![UpdateFileChunk {
                change_context: None,
                old_lines: vec!["line".to_string()],
                new_lines: vec!["line2".to_string()],
                is_end_of_file: false,
            }],
        }];
        let result = apply_hunks(&hunks, dir.path()).unwrap();
        assert_eq!(result.modified, vec![PathBuf::from("dst.txt")]);
        assert!(!dir.path().join("src.txt").exists());
        let content = fs::read_to_string(dir.path().join("dst.txt")).unwrap();
        assert_eq!(content, "line2\n");
    }

    // ---- Seek sequence tests ----

    #[test]
    fn test_seek_exact() {
        let lines: Vec<String> = vec!["foo", "bar", "baz"]
            .into_iter()
            .map(String::from)
            .collect();
        let pattern: Vec<String> = vec!["bar", "baz"].into_iter().map(String::from).collect();
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), Some(1));
    }

    #[test]
    fn test_seek_trim_whitespace() {
        let lines: Vec<String> = vec!["  foo  ", "  bar  "]
            .into_iter()
            .map(String::from)
            .collect();
        let pattern: Vec<String> = vec!["foo", "bar"].into_iter().map(String::from).collect();
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), Some(0));
    }

    #[test]
    fn test_seek_pattern_too_long() {
        let lines: Vec<String> = vec!["one"].into_iter().map(String::from).collect();
        let pattern: Vec<String> = vec!["too", "many"].into_iter().map(String::from).collect();
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), None);
    }

    // ---- Interceptor tests ----

    #[test]
    fn test_detect_apply_patch_direct() {
        let cmd =
            "apply_patch <<'EOF'\n*** Begin Patch\n*** Add File: x.txt\n+hi\n*** End Patch\nEOF";
        assert!(detect_apply_patch(cmd).is_some());
    }

    #[test]
    fn test_detect_apply_patch_embedded() {
        let cmd = "echo hello && apply_patch *** Begin Patch\n*** Add File: x\n+y\n*** End Patch";
        // Should detect embedded patch markers
        assert!(detect_apply_patch(cmd).is_some());
    }

    #[test]
    fn test_detect_not_apply_patch() {
        assert!(detect_apply_patch("ls -la").is_none());
        assert!(detect_apply_patch("git status").is_none());
    }

    #[test]
    fn test_intercept_full_flow() {
        let dir = tempfile::tempdir().unwrap();
        let cmd = format!(
            "apply_patch <<'EOF'\n*** Begin Patch\n*** Add File: hello.txt\n+hello world\n*** End Patch\nEOF"
        );
        let result = intercept_apply_patch(&cmd, dir.path());
        assert!(result.is_some());
        let output = result.unwrap().unwrap();
        assert!(output.contains("A hello.txt"));
        let content = fs::read_to_string(dir.path().join("hello.txt")).unwrap();
        assert_eq!(content, "hello world\n");
    }

    // ---- Ward enforcement: is_variant_filename ----

    #[test]
    fn test_variant_filename_v2() {
        assert!(is_variant_filename("analyzer_v2.py"));
        assert!(is_variant_filename("analyzer_v3.py"));
        assert!(is_variant_filename("report_v10.rs"));
    }

    #[test]
    fn test_variant_filename_suffixes() {
        assert!(is_variant_filename("main_fixed.py"));
        assert!(is_variant_filename("main_improved.rs"));
        assert!(is_variant_filename("script_new.py"));
        assert!(is_variant_filename("data_backup.json"));
        assert!(is_variant_filename("config_copy.toml"));
        assert!(is_variant_filename("handler_old.rs"));
        assert!(is_variant_filename("temp_temp.txt"));
    }

    #[test]
    fn test_variant_filename_numbered() {
        assert!(is_variant_filename("analyzer_2.py"));
        assert!(is_variant_filename("analyzer_3.py"));
        assert!(is_variant_filename("report_99.rs"));
    }

    #[test]
    fn test_variant_filename_not_variant() {
        assert!(!is_variant_filename("analyzer.py"));
        assert!(!is_variant_filename("main.rs"));
        assert!(!is_variant_filename("step_1.py")); // _1 is not >= 2
        assert!(!is_variant_filename("config_v1.toml")); // _v1 is version 1, could be legit
        // Note: _v1 is not blocked because it's version 1
    }

    // ---- Ward enforcement: is_ward_root_file ----

    #[test]
    fn test_ward_root_file() {
        assert!(is_ward_root_file("investigate.py"));
        assert!(is_ward_root_file("script.sh"));
        assert!(!is_ward_root_file("core/analyzer.py"));
        assert!(!is_ward_root_file("output/report.txt"));
    }

    #[test]
    fn test_allowed_root_files() {
        assert!(is_allowed_root_file("AGENTS.md"));
        assert!(is_allowed_root_file("agents.md"));
        assert!(is_allowed_root_file("__init__.py"));
        assert!(is_allowed_root_file("requirements.txt"));
        assert!(is_allowed_root_file(".gitignore"));
        assert!(!is_allowed_root_file("investigate.py"));
    }

    // ---- Ward enforcement: is_ward_context ----

    #[test]
    fn test_is_ward_context() {
        assert!(is_ward_context(Path::new("/home/user/zbot/wards/stocks")));
        assert!(is_ward_context(Path::new(
            "C:\\Users\\user\\Documents\\zbot\\wards\\myward"
        )));
        assert!(!is_ward_context(Path::new("/home/user/zbot/wards/scratch")));
        assert!(!is_ward_context(Path::new(
            "C:\\Users\\user\\Documents\\zbot\\wards\\scratch"
        )));
        assert!(!is_ward_context(Path::new("/home/user/projects/myapp")));
    }

    // ---- Ward enforcement: apply_hunks integration ----

    #[test]
    fn test_block_add_file_in_ward_root() {
        let dir = tempfile::tempdir().unwrap();
        // Create a ward-like path: .../wards/testward
        let ward_dir = dir.path().join("wards").join("testward");
        fs::create_dir_all(&ward_dir).unwrap();

        let hunks = vec![Hunk::AddFile {
            path: PathBuf::from("investigate.py"),
            contents: "print('hello')\n".to_string(),
        }];
        let result = apply_hunks(&hunks, &ward_dir);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Cannot create files in ward root"));
    }

    #[test]
    fn test_allow_add_file_in_ward_subdirectory() {
        let dir = tempfile::tempdir().unwrap();
        let ward_dir = dir.path().join("wards").join("testward");
        fs::create_dir_all(&ward_dir).unwrap();

        let hunks = vec![Hunk::AddFile {
            path: PathBuf::from("core/analyzer.py"),
            contents: "pass\n".to_string(),
        }];
        let result = apply_hunks(&hunks, &ward_dir);
        assert!(result.is_ok());
    }

    #[test]
    fn test_allow_agents_md_in_ward_root() {
        let dir = tempfile::tempdir().unwrap();
        let ward_dir = dir.path().join("wards").join("testward");
        fs::create_dir_all(&ward_dir).unwrap();

        let hunks = vec![Hunk::AddFile {
            path: PathBuf::from("AGENTS.md"),
            contents: "# Agent\n".to_string(),
        }];
        let result = apply_hunks(&hunks, &ward_dir);
        assert!(result.is_ok());
    }

    #[test]
    fn test_block_variant_add_file_in_ward() {
        let dir = tempfile::tempdir().unwrap();
        let ward_dir = dir.path().join("wards").join("testward");
        fs::create_dir_all(ward_dir.join("core")).unwrap();

        let hunks = vec![Hunk::AddFile {
            path: PathBuf::from("core/analyzer_v2.py"),
            contents: "pass\n".to_string(),
        }];
        let result = apply_hunks(&hunks, &ward_dir);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Do not create variant files"));
    }

    #[test]
    fn test_block_variant_update_file_in_ward() {
        let dir = tempfile::tempdir().unwrap();
        let ward_dir = dir.path().join("wards").join("testward");
        fs::create_dir_all(ward_dir.join("core")).unwrap();
        fs::write(ward_dir.join("core").join("analyzer_fixed.py"), "old\n").unwrap();

        let hunks = vec![Hunk::UpdateFile {
            path: PathBuf::from("core/analyzer_fixed.py"),
            move_path: None,
            chunks: vec![UpdateFileChunk {
                change_context: None,
                old_lines: vec!["old".to_string()],
                new_lines: vec!["new".to_string()],
                is_end_of_file: false,
            }],
        }];
        let result = apply_hunks(&hunks, &ward_dir);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Do not create variant files"));
    }

    #[test]
    fn test_no_ward_enforcement_outside_wards() {
        // When cwd is not inside wards/, no enforcement applies
        let dir = tempfile::tempdir().unwrap();

        let hunks = vec![Hunk::AddFile {
            path: PathBuf::from("investigate.py"),
            contents: "pass\n".to_string(),
        }];
        let result = apply_hunks(&hunks, dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_no_ward_enforcement_in_scratch() {
        let dir = tempfile::tempdir().unwrap();
        let scratch_dir = dir.path().join("wards").join("scratch");
        fs::create_dir_all(&scratch_dir).unwrap();

        let hunks = vec![Hunk::AddFile {
            path: PathBuf::from("investigate.py"),
            contents: "pass\n".to_string(),
        }];
        let result = apply_hunks(&hunks, &scratch_dir);
        assert!(result.is_ok());
    }

    #[test]
    fn test_allow_update_root_file_in_ward() {
        // Should be able to update files in ward root (for cleanup)
        let dir = tempfile::tempdir().unwrap();
        let ward_dir = dir.path().join("wards").join("testward");
        fs::create_dir_all(&ward_dir).unwrap();
        fs::write(ward_dir.join("existing.py"), "old\n").unwrap();

        let hunks = vec![Hunk::UpdateFile {
            path: PathBuf::from("existing.py"),
            move_path: None,
            chunks: vec![UpdateFileChunk {
                change_context: None,
                old_lines: vec!["old".to_string()],
                new_lines: vec!["new".to_string()],
                is_end_of_file: false,
            }],
        }];
        let result = apply_hunks(&hunks, &ward_dir);
        assert!(result.is_ok());
    }

    // ---- Lenient parsing tests ----

    #[test]
    fn test_add_file_lenient_no_plus_prefix() {
        let patch = "*** Begin Patch\n*** Add File: test.py\nimport os\nimport sys\nprint('hello')\n*** End Patch";
        let hunks = parse_patch(patch).unwrap();
        assert_eq!(hunks.len(), 1);
        match &hunks[0] {
            Hunk::AddFile { path, contents } => {
                assert_eq!(path, &PathBuf::from("test.py"));
                assert!(contents.contains("import os"));
                assert!(contents.contains("import sys"));
                assert!(contents.contains("print('hello')"));
            }
            _ => panic!("Expected AddFile"),
        }
    }

    #[test]
    fn test_add_file_lenient_mixed_plus_and_bare() {
        let patch = "*** Begin Patch\n*** Add File: test.py\n+import os\nimport sys\n+print('hello')\n*** End Patch";
        let hunks = parse_patch(patch).unwrap();
        match &hunks[0] {
            Hunk::AddFile { path: _, contents } => {
                assert!(contents.contains("import os"));
                assert!(contents.contains("import sys"));
                assert!(contents.contains("print('hello')"));
            }
            _ => panic!("Expected AddFile"),
        }
    }

    #[test]
    fn test_add_file_with_blank_lines() {
        let patch = "*** Begin Patch\n*** Add File: test.py\n+import os\n+\n+def main():\n+    pass\n*** End Patch";
        let hunks = parse_patch(patch).unwrap();
        match &hunks[0] {
            Hunk::AddFile { contents, .. } => {
                assert!(contents.contains("import os"));
                assert!(contents.contains("\n\n")); // blank line preserved
                assert!(contents.contains("def main():"));
            }
            _ => panic!("Expected AddFile"),
        }
    }

    #[test]
    fn test_error_message_includes_format_hint() {
        let patch = "wrong start\n*** End Patch";
        let err = parse_patch(patch).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("Begin Patch"),
            "Error should mention Begin Patch: {}",
            msg
        );
        assert!(
            msg.contains("Format:"),
            "Error should include format hint: {}",
            msg
        );
    }

    #[test]
    fn test_tolerates_plus_prefix_on_end_patch() {
        // GLM-5 and similar models prefix *** End Patch with '+' inside Add File blocks
        let patch = "*** Begin Patch\n*** Add File: script.py\n+print('hello')\n+*** End Patch";
        let hunks = parse_patch(patch).unwrap();
        assert_eq!(hunks.len(), 1);
        match &hunks[0] {
            Hunk::AddFile { path, contents } => {
                assert_eq!(path, &PathBuf::from("script.py"));
                assert_eq!(contents, "print('hello')\n");
            }
            _ => panic!("Expected AddFile"),
        }
    }

    #[test]
    fn test_error_propagates_real_cause() {
        // When both boundaries fail, error should describe the actual problem
        let patch = "*** Begin Patch\n*** Add File: f.py\n+code\nbad ending";
        let err = parse_patch(patch).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("end with"),
            "Error should mention end boundary: {}",
            msg
        );
    }
}
