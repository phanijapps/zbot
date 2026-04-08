# Multimodal LLM Client Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the agent framework multimodal — extend core types with Image/File parts, add a ProviderEncoder trait for OpenAI-compatible encoding, flush base64 blobs to disk before DB persistence, and provide a `multimodal_analyze` tool as a universal vision fallback.

**Architecture:** Bottom-up layer implementation: zero-core types → zero-llm encoder → agent-runtime ChatMessage migration → agent-tools multimodal tool → gateway settings. Each layer builds on the previous. No UI changes.

**Tech Stack:** Rust (serde, serde_json, sha2, base64, tokio, async-trait), OpenAI-compatible API content array format.

**Spec:** `docs/superpowers/specs/2026-04-07-multimodal-llm-client-design.md`

---

## File Structure

| Action | File | Responsibility |
|--------|------|----------------|
| Modify | `framework/zero-core/src/types.rs` | Part enum: add Image, File, ContentSource, ImageDetail; remove Binary |
| Modify | `framework/zero-core/Cargo.toml` | Add `sha2`, `base64` dependencies |
| Create | `framework/zero-core/src/multimodal.rs` | ContentSource resolution, base64 flush, MIME helpers |
| Modify | `framework/zero-core/src/lib.rs` | Export multimodal module |
| Create | `framework/zero-llm/src/encoding.rs` | ProviderEncoder trait, EncodingError |
| Create | `framework/zero-llm/src/openai_encoder.rs` | OpenAiEncoder: encode Part→OpenAI content blocks |
| Modify | `framework/zero-llm/src/lib.rs` | Export encoding, openai_encoder modules |
| Modify | `runtime/agent-runtime/src/types/messages.rs` | ChatMessage.content: String → Vec<Part>, migration helpers |
| Modify | `runtime/agent-runtime/src/llm/openai.rs` | Use OpenAiEncoder for message building, backward compat |
| Modify | `runtime/agent-runtime/src/llm/client.rs` | Add `capabilities()` method to LlmClient trait |
| Create | `runtime/agent-tools/src/tools/multimodal.rs` | multimodal_analyze tool |
| Modify | `runtime/agent-tools/src/tools/mod.rs` | Register multimodal tool |
| Modify | `runtime/agent-tools/Cargo.toml` | Add `sha2`, `base64` dependencies |
| Modify | `gateway/gateway-services/src/settings.rs` | Add MultimodalConfig to AppSettings |
| Modify | `gateway/gateway-database/src/repository.rs` | Flush base64 before INSERT |

---

## Task 1: Core Types — ContentSource, ImageDetail, Part Enum

**Files:**
- Modify: `framework/zero-core/src/types.rs`
- Modify: `framework/zero-core/Cargo.toml`

- [ ] **Step 1: Write failing tests for new Part variants**

Add these tests at the bottom of the existing `#[cfg(test)] mod tests` in `framework/zero-core/src/types.rs`:

```rust
#[test]
fn test_part_image_base64() {
    let part = Part::Image {
        source: ContentSource::Base64("iVBOR...".to_string()),
        mime_type: "image/png".to_string(),
        detail: Some(ImageDetail::Auto),
    };
    match &part {
        Part::Image { source, mime_type, detail } => {
            assert!(matches!(source, ContentSource::Base64(_)));
            assert_eq!(mime_type, "image/png");
            assert!(matches!(detail, Some(ImageDetail::Auto)));
        }
        _ => panic!("Expected Image part"),
    }
}

#[test]
fn test_part_image_url() {
    let part = Part::Image {
        source: ContentSource::Url("https://example.com/img.png".to_string()),
        mime_type: "image/png".to_string(),
        detail: None,
    };
    let json = serde_json::to_string(&part).unwrap();
    assert!(json.contains("image"));
    assert!(json.contains("https://example.com/img.png"));
}

#[test]
fn test_part_image_fileref() {
    let part = Part::Image {
        source: ContentSource::FileRef("/tmp/attachments/abc123.png".to_string()),
        mime_type: "image/jpeg".to_string(),
        detail: Some(ImageDetail::High),
    };
    match &part {
        Part::Image { source, .. } => {
            assert!(matches!(source, ContentSource::FileRef(_)));
        }
        _ => panic!("Expected Image part"),
    }
}

#[test]
fn test_part_file() {
    let part = Part::File {
        source: ContentSource::Url("https://example.com/doc.pdf".to_string()),
        mime_type: "application/pdf".to_string(),
        filename: Some("report.pdf".to_string()),
    };
    match &part {
        Part::File { mime_type, filename, .. } => {
            assert_eq!(mime_type, "application/pdf");
            assert_eq!(filename.as_deref(), Some("report.pdf"));
        }
        _ => panic!("Expected File part"),
    }
}

#[test]
fn test_content_source_serialization_roundtrip() {
    let sources = vec![
        ContentSource::Url("https://example.com/img.png".to_string()),
        ContentSource::Base64("aGVsbG8=".to_string()),
        ContentSource::FileRef("/tmp/file.png".to_string()),
    ];
    for source in &sources {
        let json = serde_json::to_string(source).unwrap();
        let deserialized: ContentSource = serde_json::from_str(&json).unwrap();
        match (source, &deserialized) {
            (ContentSource::Url(a), ContentSource::Url(b)) => assert_eq!(a, b),
            (ContentSource::Base64(a), ContentSource::Base64(b)) => assert_eq!(a, b),
            (ContentSource::FileRef(a), ContentSource::FileRef(b)) => assert_eq!(a, b),
            _ => panic!("Roundtrip mismatch"),
        }
    }
}

#[test]
fn test_image_detail_serialization() {
    let details = vec![ImageDetail::Low, ImageDetail::High, ImageDetail::Auto];
    for detail in &details {
        let json = serde_json::to_string(detail).unwrap();
        let deserialized: ImageDetail = serde_json::from_str(&json).unwrap();
        assert_eq!(
            serde_json::to_string(detail).unwrap(),
            serde_json::to_string(&deserialized).unwrap()
        );
    }
}

#[test]
fn test_part_type_name() {
    assert_eq!(Part::text("hello").type_name(), "text");
    let img = Part::Image {
        source: ContentSource::Base64("x".to_string()),
        mime_type: "image/png".to_string(),
        detail: None,
    };
    assert_eq!(img.type_name(), "image");
    let file = Part::File {
        source: ContentSource::Url("https://x.com/f.pdf".to_string()),
        mime_type: "application/pdf".to_string(),
        filename: None,
    };
    assert_eq!(file.type_name(), "file");
}

#[test]
fn test_existing_text_and_function_still_work() {
    // Ensure existing behavior is unchanged
    let text = Part::text("hello");
    assert!(matches!(text, Part::Text { .. }));

    let fc = Part::function_call("search", serde_json::json!({"q": "test"}));
    assert!(matches!(fc, Part::FunctionCall { .. }));

    let content = Content::user("test");
    assert_eq!(content.text(), Some("test"));

    let json = serde_json::to_string(&content).unwrap();
    let roundtrip: Content = serde_json::from_str(&json).unwrap();
    assert_eq!(roundtrip.text(), Some("test"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd /home/videogamer/projects/agentzero && cargo test -p zero-core -- types::tests 2>&1 | tail -30`
Expected: Compilation errors — `ContentSource`, `ImageDetail`, `Part::Image`, `Part::File`, `type_name()` not defined.

- [ ] **Step 3: Implement the new types**

Replace the `Part` enum and add new types in `framework/zero-core/src/types.rs`. Keep all existing types and methods. Add after the `Content` impl block and before the existing `Part` enum:

```rust
/// Source of multimodal content (image, file).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum ContentSource {
    /// Remote URL or data: URI — stored as-is in DB
    #[serde(rename = "url")]
    Url(String),

    /// Raw base64 encoded bytes — ephemeral, never persisted to DB
    #[serde(rename = "base64")]
    Base64(String),

    /// Local file path — what DB stores after flushing Base64 to disk
    #[serde(rename = "file_ref")]
    FileRef(String),
}

/// Image detail level for vision models.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageDetail {
    /// 512px fixed — fast, fewer tokens
    Low,
    /// Full resolution with tiling
    High,
    /// Provider decides based on image size
    Auto,
}
```

Update the `Part` enum — remove `Binary`, add `Image` and `File`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Part {
    /// Plain text content
    #[serde(rename = "text")]
    Text { text: String },

    /// Function call (tool call)
    #[serde(rename = "function_call")]
    FunctionCall {
        name: String,
        args: Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
    },

    /// Function response (tool result)
    #[serde(rename = "function_response")]
    FunctionResponse {
        id: String,
        response: String,
    },

    /// Image content (PNG, JPEG, WebP, GIF)
    #[serde(rename = "image")]
    Image {
        source: ContentSource,
        mime_type: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<ImageDetail>,
    },

    /// File/document content (PDF, CSV, etc.)
    #[serde(rename = "file")]
    File {
        source: ContentSource,
        mime_type: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        filename: Option<String>,
    },
}
```

Add `type_name()` to the existing `impl Part` block:

```rust
impl Part {
    // ... existing methods (text, function_call, function_call_with_id) ...

    /// Get the type name of this part (for error messages and logging).
    pub fn type_name(&self) -> &'static str {
        match self {
            Part::Text { .. } => "text",
            Part::FunctionCall { .. } => "function_call",
            Part::FunctionResponse { .. } => "function_response",
            Part::Image { .. } => "image",
            Part::File { .. } => "file",
        }
    }

    /// Returns true if this part contains non-text content (image or file).
    pub fn is_multimodal(&self) -> bool {
        matches!(self, Part::Image { .. } | Part::File { .. })
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd /home/videogamer/projects/agentzero && cargo test -p zero-core -- types::tests 2>&1 | tail -30`
Expected: All tests pass including new and existing ones.

- [ ] **Step 5: Check workspace compiles**

Run: `cd /home/videogamer/projects/agentzero && cargo check --workspace 2>&1 | tail -30`
Expected: May show errors in downstream crates that pattern-match on `Part::Binary`. Note them for Task 2.

- [ ] **Step 6: Fix downstream Binary references**

Search for `Part::Binary` usage across the workspace:
Run: `cd /home/videogamer/projects/agentzero && grep -rn "Part::Binary\|Binary {" --include="*.rs" | grep -v target/ | grep -v "test"`

For each match, remove the `Binary` arm. If the match is in a wildcard pattern already, no change needed. If it's an explicit match, remove the arm.

Likely locations (from exploration):
- `framework/zero-llm/src/openai.rs` — the `extract_parts()` method may have a catch-all `_` arm already. Verify.

- [ ] **Step 7: Verify workspace compiles clean**

Run: `cd /home/videogamer/projects/agentzero && cargo check --workspace 2>&1 | tail -30`
Expected: Clean compile, no errors.

- [ ] **Step 8: Commit**

```bash
git add framework/zero-core/src/types.rs
git commit -m "feat(zero-core): add Image and File multimodal Part variants

Replace unused Binary variant with typed Image and File parts.
Add ContentSource (Url, Base64, FileRef), ImageDetail (Low, High, Auto),
and type_name()/is_multimodal() helpers."
```

---

## Task 2: Multimodal Helpers — Base64 Flush, MIME Utils

**Files:**
- Create: `framework/zero-core/src/multimodal.rs`
- Modify: `framework/zero-core/src/lib.rs`
- Modify: `framework/zero-core/Cargo.toml`

- [ ] **Step 1: Add dependencies to zero-core**

Add to `framework/zero-core/Cargo.toml` under `[dependencies]`:

```toml
sha2 = "0.10"
base64 = "0.22"
```

- [ ] **Step 2: Write failing tests for multimodal helpers**

Create `framework/zero-core/src/multimodal.rs` with tests only:

```rust
//! Multimodal content helpers — base64 flush, MIME utilities, source resolution.

use std::path::{Path, PathBuf};
use crate::types::{ContentSource, Part};

/// Flush base64 data from a Part to disk, returning a new Part with FileRef source.
/// Non-multimodal parts and parts with Url/FileRef sources are returned unchanged.
pub fn flush_part_to_disk(part: Part, attachments_dir: &Path) -> std::io::Result<Part> {
    todo!()
}

/// Flush all base64 data in a list of Parts to disk.
pub fn flush_parts_to_disk(parts: Vec<Part>, attachments_dir: &Path) -> std::io::Result<Vec<Part>> {
    todo!()
}

/// Resolve a ContentSource to base64 data for sending to an LLM.
/// - Base64: returned as-is
/// - Url: returned as-is (provider fetches)
/// - FileRef: read file from disk, encode as base64
pub fn rehydrate_source(source: &ContentSource) -> std::io::Result<ContentSource> {
    todo!()
}

/// Infer file extension from MIME type.
pub fn mime_to_extension(mime_type: &str) -> &str {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ContentSource, ImageDetail};
    use std::fs;

    #[test]
    fn test_mime_to_extension() {
        assert_eq!(mime_to_extension("image/png"), "png");
        assert_eq!(mime_to_extension("image/jpeg"), "jpg");
        assert_eq!(mime_to_extension("image/webp"), "webp");
        assert_eq!(mime_to_extension("image/gif"), "gif");
        assert_eq!(mime_to_extension("application/pdf"), "pdf");
        assert_eq!(mime_to_extension("text/csv"), "csv");
        assert_eq!(mime_to_extension("application/octet-stream"), "bin");
    }

    #[test]
    fn test_flush_text_part_unchanged() {
        let part = Part::text("hello");
        let dir = tempfile::tempdir().unwrap();
        let result = flush_part_to_disk(part.clone(), dir.path()).unwrap();
        assert!(matches!(result, Part::Text { .. }));
    }

    #[test]
    fn test_flush_image_base64_to_fileref() {
        let dir = tempfile::tempdir().unwrap();
        let data = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"fake png data");
        let part = Part::Image {
            source: ContentSource::Base64(data.clone()),
            mime_type: "image/png".to_string(),
            detail: Some(ImageDetail::Auto),
        };
        let flushed = flush_part_to_disk(part, dir.path()).unwrap();
        match &flushed {
            Part::Image { source: ContentSource::FileRef(path), mime_type, detail } => {
                assert!(Path::new(path).exists(), "File should exist on disk");
                assert_eq!(mime_type, "image/png");
                assert!(matches!(detail, Some(ImageDetail::Auto)));
                // Verify content
                let bytes = fs::read(path).unwrap();
                assert_eq!(bytes, b"fake png data");
            }
            _ => panic!("Expected Image with FileRef, got {:?}", flushed),
        }
    }

    #[test]
    fn test_flush_deduplicates_by_hash() {
        let dir = tempfile::tempdir().unwrap();
        let data = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"same content");
        let part1 = Part::Image {
            source: ContentSource::Base64(data.clone()),
            mime_type: "image/png".to_string(),
            detail: None,
        };
        let part2 = Part::Image {
            source: ContentSource::Base64(data.clone()),
            mime_type: "image/png".to_string(),
            detail: None,
        };
        let flushed1 = flush_part_to_disk(part1, dir.path()).unwrap();
        let flushed2 = flush_part_to_disk(part2, dir.path()).unwrap();
        // Both should point to the same file (content-addressed)
        match (&flushed1, &flushed2) {
            (Part::Image { source: ContentSource::FileRef(p1), .. },
             Part::Image { source: ContentSource::FileRef(p2), .. }) => {
                assert_eq!(p1, p2, "Same content should produce same file path");
            }
            _ => panic!("Expected FileRef parts"),
        }
    }

    #[test]
    fn test_flush_url_source_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let part = Part::Image {
            source: ContentSource::Url("https://example.com/img.png".to_string()),
            mime_type: "image/png".to_string(),
            detail: None,
        };
        let flushed = flush_part_to_disk(part, dir.path()).unwrap();
        match &flushed {
            Part::Image { source: ContentSource::Url(url), .. } => {
                assert_eq!(url, "https://example.com/img.png");
            }
            _ => panic!("URL source should be unchanged"),
        }
    }

    #[test]
    fn test_flush_file_base64_to_fileref() {
        let dir = tempfile::tempdir().unwrap();
        let data = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"fake pdf");
        let part = Part::File {
            source: ContentSource::Base64(data),
            mime_type: "application/pdf".to_string(),
            filename: Some("report.pdf".to_string()),
        };
        let flushed = flush_part_to_disk(part, dir.path()).unwrap();
        match &flushed {
            Part::File { source: ContentSource::FileRef(path), filename, .. } => {
                assert!(Path::new(path).exists());
                assert_eq!(filename.as_deref(), Some("report.pdf"));
            }
            _ => panic!("Expected File with FileRef"),
        }
    }

    #[test]
    fn test_rehydrate_base64_unchanged() {
        let source = ContentSource::Base64("aGVsbG8=".to_string());
        let result = rehydrate_source(&source).unwrap();
        assert!(matches!(result, ContentSource::Base64(ref s) if s == "aGVsbG8="));
    }

    #[test]
    fn test_rehydrate_url_unchanged() {
        let source = ContentSource::Url("https://example.com/img.png".to_string());
        let result = rehydrate_source(&source).unwrap();
        assert!(matches!(result, ContentSource::Url(_)));
    }

    #[test]
    fn test_rehydrate_fileref_to_base64() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.png");
        fs::write(&file_path, b"image bytes").unwrap();
        let source = ContentSource::FileRef(file_path.to_str().unwrap().to_string());
        let result = rehydrate_source(&source).unwrap();
        match result {
            ContentSource::Base64(data) => {
                use base64::Engine;
                let decoded = base64::engine::general_purpose::STANDARD.decode(&data).unwrap();
                assert_eq!(decoded, b"image bytes");
            }
            _ => panic!("Expected Base64 from rehydrated FileRef"),
        }
    }

    #[test]
    fn test_flush_parts_batch() {
        let dir = tempfile::tempdir().unwrap();
        let data = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"data");
        let parts = vec![
            Part::text("hello"),
            Part::Image {
                source: ContentSource::Base64(data),
                mime_type: "image/png".to_string(),
                detail: None,
            },
        ];
        let flushed = flush_parts_to_disk(parts, dir.path()).unwrap();
        assert_eq!(flushed.len(), 2);
        assert!(matches!(flushed[0], Part::Text { .. }));
        assert!(matches!(flushed[1], Part::Image { source: ContentSource::FileRef(_), .. }));
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cd /home/videogamer/projects/agentzero && cargo test -p zero-core -- multimodal::tests 2>&1 | tail -30`
Expected: All tests fail with `not yet implemented` (todo! panics).

- [ ] **Step 4: Implement the multimodal helpers**

Replace the `todo!()` bodies in `framework/zero-core/src/multimodal.rs`:

```rust
//! Multimodal content helpers — base64 flush, MIME utilities, source resolution.

use std::path::Path;

use base64::Engine;
use sha2::{Sha256, Digest};

use crate::types::{ContentSource, Part};

/// Infer file extension from MIME type.
pub fn mime_to_extension(mime_type: &str) -> &str {
    match mime_type {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "image/svg+xml" => "svg",
        "application/pdf" => "pdf",
        "text/csv" => "csv",
        "text/plain" => "txt",
        "text/html" => "html",
        "application/json" => "json",
        _ => "bin",
    }
}

/// Flush base64 data from a Part to disk, returning a new Part with FileRef source.
/// Non-multimodal parts and parts with Url/FileRef sources are returned unchanged.
pub fn flush_part_to_disk(part: Part, attachments_dir: &Path) -> std::io::Result<Part> {
    match part {
        Part::Image { source: ContentSource::Base64(ref data), ref mime_type, ref detail } => {
            let path = write_content_addressed(data, mime_type, attachments_dir)?;
            Ok(Part::Image {
                source: ContentSource::FileRef(path),
                mime_type: mime_type.clone(),
                detail: detail.clone(),
            })
        }
        Part::File { source: ContentSource::Base64(ref data), ref mime_type, ref filename } => {
            let path = write_content_addressed(data, mime_type, attachments_dir)?;
            Ok(Part::File {
                source: ContentSource::FileRef(path),
                mime_type: mime_type.clone(),
                filename: filename.clone(),
            })
        }
        // Everything else passes through unchanged
        other => Ok(other),
    }
}

/// Flush all base64 data in a list of Parts to disk.
pub fn flush_parts_to_disk(parts: Vec<Part>, attachments_dir: &Path) -> std::io::Result<Vec<Part>> {
    parts.into_iter().map(|p| flush_part_to_disk(p, attachments_dir)).collect()
}

/// Resolve a ContentSource to a form suitable for sending to an LLM.
/// - Base64: returned as-is
/// - Url: returned as-is (provider fetches)
/// - FileRef: read file from disk, encode as base64
pub fn rehydrate_source(source: &ContentSource) -> std::io::Result<ContentSource> {
    match source {
        ContentSource::FileRef(path) => {
            let bytes = std::fs::read(path)?;
            let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
            Ok(ContentSource::Base64(encoded))
        }
        other => Ok(other.clone()),
    }
}

/// Write base64 content to a content-addressed file (SHA-256 hash as filename).
/// Returns the absolute path to the written file.
/// If a file with the same hash already exists, skips writing (dedup).
fn write_content_addressed(
    base64_data: &str,
    mime_type: &str,
    attachments_dir: &Path,
) -> std::io::Result<String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(base64_data)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let hash = format!("{:x}", hasher.finalize());

    let ext = mime_to_extension(mime_type);
    let filename = format!("{}.{}", hash, ext);
    let file_path = attachments_dir.join(&filename);

    if !file_path.exists() {
        std::fs::create_dir_all(attachments_dir)?;
        std::fs::write(&file_path, &bytes)?;
    }

    Ok(file_path.to_string_lossy().to_string())
}
```

- [ ] **Step 5: Export the module**

Add to `framework/zero-core/src/lib.rs`:

```rust
pub mod multimodal;
```

Add `tempfile` to `framework/zero-core/Cargo.toml` under `[dev-dependencies]`:

```toml
[dev-dependencies]
tempfile = "3.10"
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cd /home/videogamer/projects/agentzero && cargo test -p zero-core -- multimodal::tests 2>&1 | tail -30`
Expected: All 10 tests pass.

- [ ] **Step 7: Verify workspace compiles**

Run: `cd /home/videogamer/projects/agentzero && cargo check --workspace 2>&1 | tail -30`
Expected: Clean.

- [ ] **Step 8: Commit**

```bash
git add framework/zero-core/
git commit -m "feat(zero-core): add multimodal helpers — base64 flush, rehydration, MIME utils

Content-addressed storage with SHA-256 dedup. flush_part_to_disk()
converts Base64 sources to FileRef for DB persistence.
rehydrate_source() reads FileRef back to Base64 for LLM calls."
```

---

## Task 3: ProviderEncoder Trait + OpenAI Encoder

**Files:**
- Create: `framework/zero-llm/src/encoding.rs`
- Create: `framework/zero-llm/src/openai_encoder.rs`
- Modify: `framework/zero-llm/src/lib.rs`

- [ ] **Step 1: Write failing tests for the encoder**

Create `framework/zero-llm/src/encoding.rs`:

```rust
//! Provider encoding trait — translates framework Part types to provider-specific wire format.

use zero_core::types::Part;

/// Error during content encoding.
#[derive(Debug, thiserror::Error)]
pub enum EncodingError {
    #[error("Unsupported content type '{part_type}' for model '{model}'")]
    UnsupportedContentType { part_type: String, model: String },

    #[error("Encoding failed: {reason}")]
    EncodingFailed { reason: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Trait for encoding framework Part types into provider-specific JSON.
pub trait ProviderEncoder {
    /// Encode parts into provider-specific JSON content value.
    /// Returns a plain string for text-only, or a JSON array for multimodal.
    fn encode_content(&self, parts: &[Part]) -> Result<serde_json::Value, EncodingError>;

    /// Check if this provider/model supports a specific part type.
    fn supports_part(&self, part: &Part) -> bool;

    /// Partition parts into (supported, unsupported).
    fn filter_unsupported<'a>(&self, parts: &'a [Part]) -> (Vec<&'a Part>, Vec<&'a Part>) {
        let mut supported = vec![];
        let mut unsupported = vec![];
        for part in parts {
            if self.supports_part(part) {
                supported.push(part);
            } else {
                unsupported.push(part);
            }
        }
        (supported, unsupported)
    }
}
```

Create `framework/zero-llm/src/openai_encoder.rs`:

```rust
//! OpenAI-compatible content encoder.
//!
//! Translates framework Part types into OpenAI API content blocks.
//! Handles backward compatibility: text-only → plain string, multimodal → content array.

use serde_json::{json, Value};

use zero_core::types::{ContentSource, ImageDetail, Part};
use zero_core::multimodal::rehydrate_source;

use crate::encoding::{EncodingError, ProviderEncoder};

/// Capabilities relevant to encoding decisions.
#[derive(Debug, Clone)]
pub struct EncoderCapabilities {
    pub vision: bool,
    pub tools: bool,
}

/// OpenAI-compatible content encoder.
pub struct OpenAiEncoder {
    capabilities: EncoderCapabilities,
    model_id: String,
}

impl OpenAiEncoder {
    pub fn new(capabilities: EncoderCapabilities, model_id: String) -> Self {
        Self { capabilities, model_id }
    }
}

impl ProviderEncoder for OpenAiEncoder {
    fn supports_part(&self, part: &Part) -> bool {
        match part {
            Part::Text { .. } => true,
            Part::Image { .. } => self.capabilities.vision,
            Part::File { .. } => self.capabilities.vision,
            Part::FunctionCall { .. } | Part::FunctionResponse { .. } => self.capabilities.tools,
        }
    }

    fn encode_content(&self, parts: &[Part]) -> Result<Value, EncodingError> {
        // Validate all parts are supported
        for part in parts {
            if !self.supports_part(part) {
                return Err(EncodingError::UnsupportedContentType {
                    part_type: part.type_name().to_string(),
                    model: self.model_id.clone(),
                });
            }
        }

        // Backward compat: if all parts are text, return a plain string
        let has_multimodal = parts.iter().any(|p| p.is_multimodal());
        if !has_multimodal {
            let text: String = parts.iter().filter_map(|p| match p {
                Part::Text { text } => Some(text.as_str()),
                _ => None,
            }).collect::<Vec<_>>().join("\n");
            return Ok(Value::String(text));
        }

        // Multimodal: encode as content array
        let mut blocks = Vec::new();
        for part in parts {
            blocks.push(encode_part(part)?);
        }
        Ok(Value::Array(blocks))
    }
}

/// Encode a single Part into an OpenAI content block.
fn encode_part(part: &Part) -> Result<Value, EncodingError> {
    match part {
        Part::Text { text } => Ok(json!({
            "type": "text",
            "text": text
        })),

        Part::Image { source, mime_type, detail } => {
            let resolved = rehydrate_source(source)?;
            let url = match &resolved {
                ContentSource::Base64(data) => format!("data:{};base64,{}", mime_type, data),
                ContentSource::Url(url) => url.clone(),
                ContentSource::FileRef(_) => unreachable!("rehydrate_source always resolves FileRef"),
            };
            let mut image_url = json!({ "url": url });
            if let Some(d) = detail {
                let detail_str = match d {
                    ImageDetail::Low => "low",
                    ImageDetail::High => "high",
                    ImageDetail::Auto => "auto",
                };
                image_url.as_object_mut().unwrap().insert("detail".to_string(), json!(detail_str));
            }
            Ok(json!({
                "type": "image_url",
                "image_url": image_url
            }))
        }

        Part::File { source, mime_type, .. } => {
            let resolved = rehydrate_source(source)?;
            let url = match &resolved {
                ContentSource::Base64(data) => format!("data:{};base64,{}", mime_type, data),
                ContentSource::Url(url) => url.clone(),
                ContentSource::FileRef(_) => unreachable!("rehydrate_source always resolves FileRef"),
            };
            Ok(json!({
                "type": "file",
                "file": { "url": url }
            }))
        }

        // FunctionCall/FunctionResponse should not appear in content blocks
        // (they're handled separately as tool_calls/tool results)
        Part::FunctionCall { .. } | Part::FunctionResponse { .. } => {
            Err(EncodingError::EncodingFailed {
                reason: "FunctionCall/FunctionResponse should not be in content blocks".to_string(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zero_core::types::ContentSource;

    fn vision_encoder() -> OpenAiEncoder {
        OpenAiEncoder::new(
            EncoderCapabilities { vision: true, tools: true },
            "gpt-4o".to_string(),
        )
    }

    fn text_only_encoder() -> OpenAiEncoder {
        OpenAiEncoder::new(
            EncoderCapabilities { vision: false, tools: true },
            "gpt-3.5-turbo".to_string(),
        )
    }

    #[test]
    fn test_text_only_returns_string() {
        let encoder = vision_encoder();
        let parts = vec![Part::text("hello world")];
        let result = encoder.encode_content(&parts).unwrap();
        assert_eq!(result, Value::String("hello world".to_string()));
    }

    #[test]
    fn test_text_only_multiple_parts_joined() {
        let encoder = vision_encoder();
        let parts = vec![Part::text("line 1"), Part::text("line 2")];
        let result = encoder.encode_content(&parts).unwrap();
        assert_eq!(result, Value::String("line 1\nline 2".to_string()));
    }

    #[test]
    fn test_image_base64_encodes_as_data_uri() {
        let encoder = vision_encoder();
        let parts = vec![
            Part::text("What is in this image?"),
            Part::Image {
                source: ContentSource::Base64("aGVsbG8=".to_string()),
                mime_type: "image/png".to_string(),
                detail: Some(ImageDetail::High),
            },
        ];
        let result = encoder.encode_content(&parts).unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["type"], "text");
        assert_eq!(arr[0]["text"], "What is in this image?");
        assert_eq!(arr[1]["type"], "image_url");
        assert_eq!(arr[1]["image_url"]["url"], "data:image/png;base64,aGVsbG8=");
        assert_eq!(arr[1]["image_url"]["detail"], "high");
    }

    #[test]
    fn test_image_url_passes_through() {
        let encoder = vision_encoder();
        let parts = vec![Part::Image {
            source: ContentSource::Url("https://example.com/img.png".to_string()),
            mime_type: "image/png".to_string(),
            detail: None,
        }];
        let result = encoder.encode_content(&parts).unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr[0]["image_url"]["url"], "https://example.com/img.png");
        // No detail field when None
        assert!(arr[0]["image_url"].get("detail").is_none());
    }

    #[test]
    fn test_file_encodes_correctly() {
        let encoder = vision_encoder();
        let parts = vec![Part::File {
            source: ContentSource::Base64("cGRmZGF0YQ==".to_string()),
            mime_type: "application/pdf".to_string(),
            filename: Some("report.pdf".to_string()),
        }];
        let result = encoder.encode_content(&parts).unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr[0]["type"], "file");
        assert_eq!(arr[0]["file"]["url"], "data:application/pdf;base64,cGRmZGF0YQ==");
    }

    #[test]
    fn test_text_only_model_rejects_image() {
        let encoder = text_only_encoder();
        let parts = vec![Part::Image {
            source: ContentSource::Base64("x".to_string()),
            mime_type: "image/png".to_string(),
            detail: None,
        }];
        let err = encoder.encode_content(&parts).unwrap_err();
        match err {
            EncodingError::UnsupportedContentType { part_type, model } => {
                assert_eq!(part_type, "image");
                assert_eq!(model, "gpt-3.5-turbo");
            }
            _ => panic!("Expected UnsupportedContentType"),
        }
    }

    #[test]
    fn test_text_only_model_accepts_text() {
        let encoder = text_only_encoder();
        let parts = vec![Part::text("hello")];
        let result = encoder.encode_content(&parts).unwrap();
        assert_eq!(result, Value::String("hello".to_string()));
    }

    #[test]
    fn test_supports_part() {
        let vision = vision_encoder();
        let text_only = text_only_encoder();

        let text = Part::text("hi");
        let image = Part::Image {
            source: ContentSource::Base64("x".to_string()),
            mime_type: "image/png".to_string(),
            detail: None,
        };

        assert!(vision.supports_part(&text));
        assert!(vision.supports_part(&image));
        assert!(text_only.supports_part(&text));
        assert!(!text_only.supports_part(&image));
    }

    #[test]
    fn test_filter_unsupported() {
        let encoder = text_only_encoder();
        let parts = vec![
            Part::text("hello"),
            Part::Image {
                source: ContentSource::Base64("x".to_string()),
                mime_type: "image/png".to_string(),
                detail: None,
            },
        ];
        let (supported, unsupported) = encoder.filter_unsupported(&parts);
        assert_eq!(supported.len(), 1);
        assert_eq!(unsupported.len(), 1);
        assert!(matches!(supported[0], Part::Text { .. }));
        assert!(matches!(unsupported[0], Part::Image { .. }));
    }

    #[test]
    fn test_image_fileref_rehydrated() {
        // Write a test file, create a FileRef, verify it gets encoded as data URI
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.png");
        std::fs::write(&file_path, b"fake png").unwrap();

        let encoder = vision_encoder();
        let parts = vec![Part::Image {
            source: ContentSource::FileRef(file_path.to_str().unwrap().to_string()),
            mime_type: "image/png".to_string(),
            detail: None,
        }];
        let result = encoder.encode_content(&parts).unwrap();
        let arr = result.as_array().unwrap();
        let url = arr[0]["image_url"]["url"].as_str().unwrap();
        assert!(url.starts_with("data:image/png;base64,"));
    }
}
```

- [ ] **Step 2: Export the modules**

Add to `framework/zero-llm/src/lib.rs`:

```rust
pub mod encoding;
pub mod openai_encoder;
```

And add to the pub use section:

```rust
pub use encoding::{ProviderEncoder, EncodingError};
pub use openai_encoder::{OpenAiEncoder, EncoderCapabilities};
```

Add `tempfile` dev-dependency to `framework/zero-llm/Cargo.toml`:

```toml
[dev-dependencies]
tempfile = "3.10"
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cd /home/videogamer/projects/agentzero && cargo test -p zero-llm -- openai_encoder::tests 2>&1 | tail -40`
Expected: All 10 tests pass.

- [ ] **Step 4: Verify workspace compiles**

Run: `cd /home/videogamer/projects/agentzero && cargo check --workspace 2>&1 | tail -30`
Expected: Clean.

- [ ] **Step 5: Commit**

```bash
git add framework/zero-llm/
git commit -m "feat(zero-llm): add ProviderEncoder trait and OpenAI encoder

ProviderEncoder trait with encode_content(), supports_part(), filter_unsupported().
OpenAiEncoder: text-only → plain string, multimodal → content array.
Capability-aware: rejects Image/File for non-vision models with clear error."
```

---

## Task 4: ChatMessage Migration — String → Vec<Part>

**Files:**
- Modify: `runtime/agent-runtime/src/types/messages.rs`

This is the most impactful change. `ChatMessage.content` changes from `String` to `Vec<Part>`. Custom serialization ensures backward compat with the OpenAI wire format.

- [ ] **Step 1: Write failing tests for the new ChatMessage**

Add to the existing tests in `runtime/agent-runtime/src/types/messages.rs`:

```rust
#[test]
fn test_message_text_helper() {
    let msg = ChatMessage::user("Hello".to_string());
    assert_eq!(msg.text_content(), "Hello");
    assert_eq!(msg.content.len(), 1);
    assert!(matches!(&msg.content[0], Part::Text { text } if text == "Hello"));
}

#[test]
fn test_message_multimodal_content() {
    use zero_core::types::{ContentSource, ImageDetail};
    let msg = ChatMessage {
        role: "user".to_string(),
        content: vec![
            Part::Text { text: "What is this?".to_string() },
            Part::Image {
                source: ContentSource::Base64("aGVsbG8=".to_string()),
                mime_type: "image/png".to_string(),
                detail: Some(ImageDetail::Auto),
            },
        ],
        tool_calls: None,
        tool_call_id: None,
    };
    assert_eq!(msg.text_content(), "What is this?");
    assert_eq!(msg.content.len(), 2);
    assert!(msg.has_multimodal_content());
}

#[test]
fn test_text_content_joins_multiple() {
    let msg = ChatMessage {
        role: "user".to_string(),
        content: vec![
            Part::Text { text: "line 1".to_string() },
            Part::Text { text: "line 2".to_string() },
        ],
        tool_calls: None,
        tool_call_id: None,
    };
    assert_eq!(msg.text_content(), "line 1\nline 2");
}

#[test]
fn test_serialization_text_only_is_string() {
    let msg = ChatMessage::user("Hello".to_string());
    let json = serde_json::to_value(&msg).unwrap();
    // Text-only messages should serialize content as a plain string
    assert_eq!(json["content"], "Hello");
    assert_eq!(json["role"], "user");
}

#[test]
fn test_serialization_multimodal_is_array() {
    use zero_core::types::ContentSource;
    let msg = ChatMessage {
        role: "user".to_string(),
        content: vec![
            Part::Text { text: "Describe".to_string() },
            Part::Image {
                source: ContentSource::Url("https://example.com/img.png".to_string()),
                mime_type: "image/png".to_string(),
                detail: None,
            },
        ],
        tool_calls: None,
        tool_call_id: None,
    };
    let json = serde_json::to_value(&msg).unwrap();
    // Multimodal messages serialize content as array
    assert!(json["content"].is_array());
}

#[test]
fn test_deserialization_from_string() {
    // Backward compat: deserialize old-format messages with string content
    let json = r#"{"role":"user","content":"Hello"}"#;
    let msg: ChatMessage = serde_json::from_str(json).unwrap();
    assert_eq!(msg.text_content(), "Hello");
    assert_eq!(msg.content.len(), 1);
}

#[test]
fn test_deserialization_from_array() {
    let json = r#"{"role":"user","content":[{"type":"text","text":"Hello"}]}"#;
    let msg: ChatMessage = serde_json::from_str(json).unwrap();
    assert_eq!(msg.text_content(), "Hello");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd /home/videogamer/projects/agentzero && cargo test -p agent-runtime -- types::messages::tests 2>&1 | tail -30`
Expected: Compilation errors — `text_content()`, `has_multimodal_content()` not defined, `content` is `String` not `Vec<Part>`.

- [ ] **Step 3: Implement the ChatMessage migration**

Rewrite `runtime/agent-runtime/src/types/messages.rs`:

```rust
// ============================================================================
// CHAT MESSAGE TYPES
// Core message types for LLM communication
// ============================================================================

use serde::{Deserialize, Serialize, Serializer, Deserializer};
use serde_json::Value;

use zero_core::types::Part;

/// A chat message in the conversation.
///
/// Content is stored as `Vec<Part>` internally to support multimodal messages.
/// Serialization is backward-compatible: text-only → plain string, multimodal → content array.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    /// Message role (system, user, assistant, tool)
    pub role: String,

    /// Message content — one or more Parts (text, image, file)
    pub content: Vec<Part>,

    /// Tool calls made by the assistant (optional)
    pub tool_calls: Option<Vec<ToolCall>>,

    /// ID of the tool call this message is responding to (optional)
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    /// Create a new user message (text-only convenience).
    #[must_use]
    pub fn user(content: String) -> Self {
        Self {
            role: "user".to_string(),
            content: vec![Part::Text { text: content }],
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// Create a new assistant message (text-only convenience).
    #[must_use]
    pub fn assistant(content: String) -> Self {
        Self {
            role: "assistant".to_string(),
            content: vec![Part::Text { text: content }],
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// Create a new system message (text-only convenience).
    #[must_use]
    pub fn system(content: String) -> Self {
        Self {
            role: "system".to_string(),
            content: vec![Part::Text { text: content }],
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// Create a new tool result message.
    #[must_use]
    pub fn tool_result(tool_call_id: String, content: String) -> Self {
        Self {
            role: "tool".to_string(),
            content: vec![Part::Text { text: content }],
            tool_calls: None,
            tool_call_id: Some(tool_call_id),
        }
    }

    /// Get all text content joined as a single string.
    /// Useful for backward compat and display.
    pub fn text_content(&self) -> String {
        self.content
            .iter()
            .filter_map(|p| match p {
                Part::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Returns true if this message contains any non-text parts.
    pub fn has_multimodal_content(&self) -> bool {
        self.content.iter().any(|p| p.is_multimodal())
    }
}

// Custom serialization: text-only → plain string content, multimodal → array
impl Serialize for ChatMessage {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;

        let field_count = 2
            + self.tool_calls.as_ref().map_or(0, |_| 1)
            + self.tool_call_id.as_ref().map_or(0, |_| 1);

        let mut s = serializer.serialize_struct("ChatMessage", field_count)?;
        s.serialize_field("role", &self.role)?;

        // Backward compat: text-only messages serialize content as plain string
        if !self.has_multimodal_content() {
            s.serialize_field("content", &self.text_content())?;
        } else {
            s.serialize_field("content", &self.content)?;
        }

        if let Some(ref tc) = self.tool_calls {
            s.serialize_field("tool_calls", tc)?;
        }
        if let Some(ref id) = self.tool_call_id {
            s.serialize_field("tool_call_id", id)?;
        }
        s.end()
    }
}

// Custom deserialization: accept both string and array content
impl<'de> Deserialize<'de> for ChatMessage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawMessage {
            role: String,
            content: Value,
            tool_calls: Option<Vec<ToolCall>>,
            tool_call_id: Option<String>,
        }

        let raw = RawMessage::deserialize(deserializer)?;

        let content = match raw.content {
            Value::String(text) => vec![Part::Text { text }],
            Value::Array(_) => {
                serde_json::from_value(raw.content)
                    .map_err(serde::de::Error::custom)?
            }
            Value::Null => vec![],
            other => {
                return Err(serde::de::Error::custom(format!(
                    "expected string or array for content, got {}",
                    other
                )));
            }
        };

        Ok(ChatMessage {
            role: raw.role,
            content,
            tool_calls: raw.tool_calls,
            tool_call_id: raw.tool_call_id,
        })
    }
}

/// A tool call made by the LLM
#[derive(Debug, Clone, Deserialize)]
pub struct ToolCall {
    /// Unique identifier for this tool call
    pub id: String,

    /// Name of the tool to call
    pub name: String,

    /// Arguments to pass to the tool (JSON object)
    pub arguments: Value,
}

// Custom serialization to match OpenAI's format
impl Serialize for ToolCall {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;

        // OpenAI format: { id, type: "function", function: { name, arguments: "..." } }
        let arguments_str = serde_json::to_string(&self.arguments)
            .map_err(serde::ser::Error::custom)?;

        let mut s = serializer.serialize_struct("ToolCall", 3)?;
        s.serialize_field("id", &self.id)?;
        s.serialize_field("type", "function")?;
        s.serialize_field("function", &ToolCallFunctionSerialization {
            name: &self.name,
            arguments: &arguments_str,
        })?;
        s.end()
    }
}

/// Helper struct for serializing the function part of a tool call
#[derive(Serialize)]
struct ToolCallFunctionSerialization<'a> {
    name: &'a str,
    arguments: &'a str,
}

impl ToolCall {
    /// Create a new tool call
    #[must_use]
    pub fn new(id: String, name: String, arguments: Value) -> Self {
        Self { id, name, arguments }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_creation() {
        let user_msg = ChatMessage::user("Hello".to_string());
        assert_eq!(user_msg.role, "user");
        assert_eq!(user_msg.text_content(), "Hello");
    }

    #[test]
    fn test_tool_call_creation() {
        let tool_call = ToolCall::new(
            "call_123".to_string(),
            "search".to_string(),
            serde_json::json!({"query": "test"}),
        );
        assert_eq!(tool_call.id, "call_123");
        assert_eq!(tool_call.name, "search");
    }

    #[test]
    fn test_message_text_helper() {
        let msg = ChatMessage::user("Hello".to_string());
        assert_eq!(msg.text_content(), "Hello");
        assert_eq!(msg.content.len(), 1);
        assert!(matches!(&msg.content[0], Part::Text { text } if text == "Hello"));
    }

    #[test]
    fn test_message_multimodal_content() {
        use zero_core::types::{ContentSource, ImageDetail};
        let msg = ChatMessage {
            role: "user".to_string(),
            content: vec![
                Part::Text { text: "What is this?".to_string() },
                Part::Image {
                    source: ContentSource::Base64("aGVsbG8=".to_string()),
                    mime_type: "image/png".to_string(),
                    detail: Some(ImageDetail::Auto),
                },
            ],
            tool_calls: None,
            tool_call_id: None,
        };
        assert_eq!(msg.text_content(), "What is this?");
        assert_eq!(msg.content.len(), 2);
        assert!(msg.has_multimodal_content());
    }

    #[test]
    fn test_text_content_joins_multiple() {
        let msg = ChatMessage {
            role: "user".to_string(),
            content: vec![
                Part::Text { text: "line 1".to_string() },
                Part::Text { text: "line 2".to_string() },
            ],
            tool_calls: None,
            tool_call_id: None,
        };
        assert_eq!(msg.text_content(), "line 1\nline 2");
    }

    #[test]
    fn test_serialization_text_only_is_string() {
        let msg = ChatMessage::user("Hello".to_string());
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["content"], "Hello");
        assert_eq!(json["role"], "user");
    }

    #[test]
    fn test_serialization_multimodal_is_array() {
        use zero_core::types::ContentSource;
        let msg = ChatMessage {
            role: "user".to_string(),
            content: vec![
                Part::Text { text: "Describe".to_string() },
                Part::Image {
                    source: ContentSource::Url("https://example.com/img.png".to_string()),
                    mime_type: "image/png".to_string(),
                    detail: None,
                },
            ],
            tool_calls: None,
            tool_call_id: None,
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert!(json["content"].is_array());
    }

    #[test]
    fn test_deserialization_from_string() {
        let json = r#"{"role":"user","content":"Hello"}"#;
        let msg: ChatMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.text_content(), "Hello");
        assert_eq!(msg.content.len(), 1);
    }

    #[test]
    fn test_deserialization_from_array() {
        let json = r#"{"role":"user","content":[{"type":"text","text":"Hello"}]}"#;
        let msg: ChatMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.text_content(), "Hello");
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd /home/videogamer/projects/agentzero && cargo test -p agent-runtime -- types::messages::tests 2>&1 | tail -30`
Expected: All tests pass.

- [ ] **Step 5: Fix compilation errors across workspace**

The `content: String` → `content: Vec<Part>` change will break callsites. Run:

Run: `cd /home/videogamer/projects/agentzero && cargo check --workspace 2>&1 | head -100`

Common fixes needed:
- `message.content` (String access) → `message.text_content()` (for display/logging/DB storage)
- `ChatMessage { content: text, .. }` → `ChatMessage::user(text)` or `ChatMessage::system(text)`
- Pattern matches on `content` field → use `text_content()`

Work through each error systematically. The factory methods (`user()`, `assistant()`, `system()`, `tool_result()`) already produce `Vec<Part>`, so most callsites just need the constructor change.

**Key files likely affected** (from the exploration):
- `runtime/agent-runtime/src/executor.rs` — message construction, content access
- `runtime/agent-runtime/src/llm/openai.rs` — `build_request_body()` uses `messages` directly via serde
- `gateway/gateway-database/src/repository.rs` — stores `content` as String in DB
- `gateway/gateway-execution/src/` — runner, batch writer, archiver
- `services/execution-state/src/` — message storage

For **DB persistence** sites (repository.rs, execution-state), use `message.text_content()` to store as String for now. The base64 flush happens before persistence, so `text_content()` is safe — multimodal parts will be FileRef references serialized as part of a JSON column in a later task if needed.

- [ ] **Step 6: Verify workspace compiles clean**

Run: `cd /home/videogamer/projects/agentzero && cargo check --workspace 2>&1 | tail -30`
Expected: Clean compile.

- [ ] **Step 7: Run full test suite**

Run: `cd /home/videogamer/projects/agentzero && cargo test --workspace 2>&1 | tail -40`
Expected: All tests pass.

- [ ] **Step 8: Commit**

```bash
git add runtime/agent-runtime/src/types/messages.rs
git add -u  # all fixup files
git commit -m "feat(agent-runtime): migrate ChatMessage.content from String to Vec<Part>

Custom serde: text-only → string (backward compat), multimodal → array.
Deserialization accepts both formats for reading old DB records.
All callsites migrated to use text_content() or factory methods."
```

---

## Task 5: Wire OpenAiEncoder into OpenAiClient

**Files:**
- Modify: `runtime/agent-runtime/src/llm/openai.rs`
- Modify: `runtime/agent-runtime/Cargo.toml` (add zero-llm dependency if not present)

- [ ] **Step 1: Check if agent-runtime depends on zero-llm**

Run: `grep zero-llm runtime/agent-runtime/Cargo.toml`

If not present, add:
```toml
zero-llm = { path = "../../framework/zero-llm" }
```

- [ ] **Step 2: Update build_request_body to use encoder**

In `runtime/agent-runtime/src/llm/openai.rs`, the `build_request_body` method currently serializes `messages` directly via `json!({ "messages": messages })`. Since ChatMessage now has custom serialization (text-only → string, multimodal → array), this already works correctly via serde.

However, we need to ensure `FileRef` sources are rehydrated before sending. Add a pre-processing step in `build_request_body`:

At the top of `openai.rs`, add imports:

```rust
use zero_core::types::{Part, ContentSource};
use zero_core::multimodal::rehydrate_source;
```

Add a helper method to `OpenAiClient`:

```rust
/// Rehydrate any FileRef sources in messages to Base64 before sending to the API.
/// This ensures the API receives actual content, not local file paths.
fn rehydrate_messages(messages: Vec<ChatMessage>) -> Vec<ChatMessage> {
    messages.into_iter().map(|mut msg| {
        msg.content = msg.content.into_iter().map(|part| {
            match &part {
                Part::Image { source: source @ ContentSource::FileRef(path), mime_type, detail } => {
                    match rehydrate_source(source) {
                        Ok(new_source) => Part::Image {
                            source: new_source,
                            mime_type: mime_type.clone(),
                            detail: detail.clone(),
                        },
                        Err(e) => {
                            tracing::warn!("Failed to rehydrate FileRef {}: {}", path, e);
                            part
                        }
                    }
                }
                Part::File { source: source @ ContentSource::FileRef(path), mime_type, filename } => {
                    match rehydrate_source(source) {
                        Ok(new_source) => Part::File {
                            source: new_source,
                            mime_type: mime_type.clone(),
                            filename: filename.clone(),
                        },
                        Err(e) => {
                            tracing::warn!("Failed to rehydrate FileRef {}: {}", path, e);
                            part
                        }
                    }
                }
                _ => part,
            }
        }).collect();
        msg
    }).collect()
}
```

Update `build_request_body` to call this:

```rust
fn build_request_body(
    &self,
    messages: Vec<ChatMessage>,
    tools: Option<Value>,
) -> Value {
    let messages = Self::rehydrate_messages(messages);
    // ... rest unchanged
}
```

- [ ] **Step 3: Run tests**

Run: `cd /home/videogamer/projects/agentzero && cargo test -p agent-runtime 2>&1 | tail -30`
Expected: All existing tests pass.

- [ ] **Step 4: Verify workspace compiles**

Run: `cd /home/videogamer/projects/agentzero && cargo check --workspace 2>&1 | tail -30`
Expected: Clean.

- [ ] **Step 5: Commit**

```bash
git add runtime/agent-runtime/
git commit -m "feat(agent-runtime): wire FileRef rehydration into OpenAiClient

Rehydrate FileRef sources to Base64 before sending to API.
ChatMessage custom serialization handles text-only vs multimodal encoding."
```

---

## Task 6: MultimodalConfig in Settings

**Files:**
- Modify: `gateway/gateway-services/src/settings.rs`

- [ ] **Step 1: Add MultimodalConfig struct**

In `gateway/gateway-services/src/settings.rs`, add after `DistillationConfig`:

```rust
/// Default multimodal model configuration.
/// Used by the multimodal_analyze tool as a universal vision fallback.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultimodalConfig {
    /// Provider ID for the vision model
    pub provider_id: Option<String>,
    /// Model ID (must have vision capability)
    pub model: Option<String>,
    /// Temperature for analysis calls (lower = more deterministic)
    #[serde(default = "default_multimodal_temperature")]
    pub temperature: f64,
    /// Max output tokens
    #[serde(default = "default_multimodal_max_tokens")]
    pub max_tokens: u32,
}

fn default_multimodal_temperature() -> f64 { 0.3 }
fn default_multimodal_max_tokens() -> u32 { 4096 }

impl Default for MultimodalConfig {
    fn default() -> Self {
        Self {
            provider_id: None,
            model: None,
            temperature: default_multimodal_temperature(),
            max_tokens: default_multimodal_max_tokens(),
        }
    }
}
```

Add `multimodal` field to `AppSettings`:

```rust
pub struct AppSettings {
    pub tools: ToolSettings,
    pub logs: LogSettings,
    pub execution: ExecutionSettings,
    #[serde(default)]
    pub multimodal: MultimodalConfig,
}
```

- [ ] **Step 2: Run tests**

Run: `cd /home/videogamer/projects/agentzero && cargo test -p gateway-services 2>&1 | tail -30`
Expected: All pass. The `#[serde(default)]` ensures existing settings.json files without `multimodal` key still deserialize.

- [ ] **Step 3: Verify workspace compiles**

Run: `cd /home/videogamer/projects/agentzero && cargo check --workspace 2>&1 | tail -30`
Expected: Clean.

- [ ] **Step 4: Commit**

```bash
git add gateway/gateway-services/src/settings.rs
git commit -m "feat(gateway): add MultimodalConfig to AppSettings

Default multimodal model config in settings.json for vision fallback.
Defaults: temperature 0.3, maxTokens 4096. Provider/model null until configured."
```

---

## Task 7: multimodal_analyze Tool

**Files:**
- Create: `runtime/agent-tools/src/tools/multimodal.rs`
- Modify: `runtime/agent-tools/src/tools/mod.rs`
- Modify: `runtime/agent-tools/Cargo.toml`

- [ ] **Step 1: Add dependencies**

Add to `runtime/agent-tools/Cargo.toml` under `[dependencies]`:

```toml
agent-runtime = { path = "../agent-runtime" }
zero-llm = { path = "../../framework/zero-llm" }
base64 = "0.22"
sha2 = "0.10"
```

- [ ] **Step 2: Create the multimodal tool**

Create `runtime/agent-tools/src/tools/multimodal.rs`:

```rust
// ============================================================================
// MULTIMODAL ANALYZE TOOL
// Universal vision fallback — any agent can process images/files via this tool.
// Uses the default multimodal model from settings.json.
// ============================================================================

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use zero_core::context::ToolContext;
use zero_core::tool::{Tool, ToolPermissions};
use zero_core::types::{ContentSource, ImageDetail, Part};

/// Tool that makes one-shot multimodal LLM calls using the default vision model.
pub struct MultimodalAnalyzeTool;

impl MultimodalAnalyzeTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for MultimodalAnalyzeTool {
    fn name(&self) -> &str {
        "multimodal_analyze"
    }

    fn description(&self) -> &str {
        "Analyze images, PDFs, or documents using a vision-capable model. \
         Send one or more content items with a prompt, get structured analysis back. \
         Use when you need to understand visual content but your current model doesn't support vision."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "required": ["content", "prompt"],
            "properties": {
                "content": {
                    "type": "array",
                    "description": "Content items to analyze",
                    "items": {
                        "type": "object",
                        "properties": {
                            "type": {
                                "type": "string",
                                "enum": ["image", "file"],
                                "description": "Content type"
                            },
                            "source": {
                                "type": "string",
                                "description": "File path, URL, or base64 data"
                            },
                            "detail": {
                                "type": "string",
                                "enum": ["low", "high", "auto"],
                                "description": "Image detail level (optional, default: auto)"
                            }
                        },
                        "required": ["type", "source"]
                    }
                },
                "prompt": {
                    "type": "string",
                    "description": "What to analyze or extract from the content"
                },
                "output_schema": {
                    "type": "object",
                    "description": "Optional JSON Schema for structured output"
                }
            }
        }))
    }

    fn permissions(&self) -> ToolPermissions {
        ToolPermissions::safe()
    }

    async fn execute(&self, ctx: Arc<dyn ToolContext>, args: Value) -> anyhow::Result<Value> {
        let content_items = args.get("content")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("'content' must be an array"))?;

        let prompt = args.get("prompt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("'prompt' is required"))?;

        let output_schema = args.get("output_schema").cloned();

        // Build Parts from input
        let mut parts: Vec<Part> = Vec::new();

        for item in content_items {
            let content_type = item.get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("image");

            let source_str = item.get("source")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Each content item must have a 'source'"))?;

            let source = resolve_source(source_str)?;

            match content_type {
                "image" => {
                    let detail = item.get("detail")
                        .and_then(|v| v.as_str())
                        .map(|d| match d {
                            "low" => ImageDetail::Low,
                            "high" => ImageDetail::High,
                            _ => ImageDetail::Auto,
                        });

                    let mime_type = infer_image_mime(source_str);
                    parts.push(Part::Image {
                        source,
                        mime_type,
                        detail,
                    });
                }
                "file" => {
                    let mime_type = infer_file_mime(source_str);
                    parts.push(Part::File {
                        source,
                        mime_type,
                        filename: None,
                    });
                }
                other => {
                    return Err(anyhow::anyhow!("Unknown content type: {}", other));
                }
            }
        }

        // Add the prompt as a text part
        parts.push(Part::Text { text: prompt.to_string() });

        // Read multimodal config from state (injected by gateway)
        let config_json = ctx.get_state("multimodal_config");
        if config_json.is_none() {
            return Ok(json!({
                "error": "No multimodal model configured. Add a vision-capable model to Settings > Multimodal."
            }));
        }

        let config = config_json.unwrap();
        let provider_id = config.get("providerId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("multimodal.providerId not configured"))?;
        let model = config.get("model")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("multimodal.model not configured"))?;

        // The actual LLM call will be made via a callback injected into the tool context.
        // For now, store the prepared request in state for the executor to pick up.
        // The executor has access to the LLM client and can make the one-shot call.
        let request = json!({
            "provider_id": provider_id,
            "model": model,
            "temperature": config.get("temperature").and_then(|v| v.as_f64()).unwrap_or(0.3),
            "max_tokens": config.get("maxTokens").and_then(|v| v.as_u64()).unwrap_or(4096),
            "parts": serde_json::to_value(&parts)?,
            "output_schema": output_schema,
        });

        // Set the multimodal request in state — the executor/gateway hooks will
        // detect this and make the actual LLM call, returning the result.
        ctx.set_state("multimodal_request".to_string(), request);

        // Signal that this tool needs a special execution path
        Ok(json!({
            "status": "multimodal_request_queued",
            "message": "Multimodal analysis request prepared. The executor will process this via the configured vision model."
        }))
    }
}

/// Resolve a source string to a ContentSource.
/// Handles: file:// paths, http(s):// URLs, absolute paths, base64 data.
fn resolve_source(source: &str) -> anyhow::Result<ContentSource> {
    if source.starts_with("data:") || source.starts_with("base64:") {
        // Strip prefix if present
        let data = source
            .strip_prefix("base64:")
            .or_else(|| {
                // data:image/png;base64,<data> → extract <data>
                source.find(";base64,").map(|pos| &source[pos + 8..])
            })
            .unwrap_or(source);
        Ok(ContentSource::Base64(data.to_string()))
    } else if source.starts_with("http://") || source.starts_with("https://") {
        Ok(ContentSource::Url(source.to_string()))
    } else {
        // File path — strip file:// prefix if present
        let path = source.strip_prefix("file://").unwrap_or(source);
        if !std::path::Path::new(path).exists() {
            return Err(anyhow::anyhow!("File not found: {}", path));
        }
        // Read and encode to base64
        let bytes = std::fs::read(path)?;
        let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);
        Ok(ContentSource::Base64(encoded))
    }
}

/// Infer MIME type from file extension for images.
fn infer_image_mime(source: &str) -> String {
    let lower = source.to_lowercase();
    if lower.ends_with(".png") { "image/png".to_string() }
    else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") { "image/jpeg".to_string() }
    else if lower.ends_with(".webp") { "image/webp".to_string() }
    else if lower.ends_with(".gif") { "image/gif".to_string() }
    else if lower.ends_with(".svg") { "image/svg+xml".to_string() }
    else { "image/png".to_string() } // default
}

/// Infer MIME type from file extension for documents.
fn infer_file_mime(source: &str) -> String {
    let lower = source.to_lowercase();
    if lower.ends_with(".pdf") { "application/pdf".to_string() }
    else if lower.ends_with(".csv") { "text/csv".to_string() }
    else if lower.ends_with(".txt") { "text/plain".to_string() }
    else if lower.ends_with(".html") || lower.ends_with(".htm") { "text/html".to_string() }
    else if lower.ends_with(".json") { "application/json".to_string() }
    else { "application/octet-stream".to_string() }
}
```

- [ ] **Step 3: Register the tool in mod.rs**

In `runtime/agent-tools/src/tools/mod.rs`, add import:

```rust
pub mod multimodal;
```

Add to the `optional_tools` function, inside the function body (after existing tools):

```rust
// Multimodal analysis (always available — it's a safety net)
tools.push(Arc::new(multimodal::MultimodalAnalyzeTool::new()));
```

- [ ] **Step 4: Run tests**

Run: `cd /home/videogamer/projects/agentzero && cargo check -p agent-tools 2>&1 | tail -30`
Expected: Clean compile.

Run: `cd /home/videogamer/projects/agentzero && cargo test -p agent-tools 2>&1 | tail -30`
Expected: All pass.

- [ ] **Step 5: Verify workspace compiles**

Run: `cd /home/videogamer/projects/agentzero && cargo check --workspace 2>&1 | tail -30`
Expected: Clean.

- [ ] **Step 6: Commit**

```bash
git add runtime/agent-tools/
git commit -m "feat(agent-tools): add multimodal_analyze tool

Universal vision fallback tool. Any agent can analyze images/files
via the configured vision model in settings.json. Resolves file paths,
URLs, and base64 sources. Queues request for executor to process."
```

---

## Task 8: Integration Test — End-to-End Multimodal Flow

**Files:**
- Create: `framework/zero-llm/tests/multimodal_integration.rs`

- [ ] **Step 1: Write integration test**

Create `framework/zero-llm/tests/multimodal_integration.rs`:

```rust
//! Integration test: multimodal message round-trip.
//! Create → flush to disk → store (text only) → rehydrate → encode for API.

use zero_core::types::{ContentSource, ImageDetail, Part};
use zero_core::multimodal::{flush_parts_to_disk, rehydrate_source};
use zero_llm::openai_encoder::{OpenAiEncoder, EncoderCapabilities};
use zero_llm::encoding::ProviderEncoder;

#[test]
fn test_full_multimodal_roundtrip() {
    let dir = tempfile::tempdir().unwrap();

    // 1. Create multimodal parts (as an agent would)
    let original_data = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        b"fake image bytes for testing",
    );
    let parts = vec![
        Part::Text { text: "What do you see?".to_string() },
        Part::Image {
            source: ContentSource::Base64(original_data.clone()),
            mime_type: "image/png".to_string(),
            detail: Some(ImageDetail::High),
        },
    ];

    // 2. Flush to disk (before DB persistence)
    let flushed = flush_parts_to_disk(parts, dir.path()).unwrap();
    assert!(matches!(&flushed[0], Part::Text { .. }));
    assert!(matches!(&flushed[1], Part::Image { source: ContentSource::FileRef(_), .. }));

    // 3. Verify no Base64 in flushed parts (safe for DB)
    for part in &flushed {
        match part {
            Part::Image { source: ContentSource::Base64(_), .. } => panic!("Base64 should be flushed"),
            Part::File { source: ContentSource::Base64(_), .. } => panic!("Base64 should be flushed"),
            _ => {}
        }
    }

    // 4. Encode for API (rehydration happens inside encoder)
    let encoder = OpenAiEncoder::new(
        EncoderCapabilities { vision: true, tools: true },
        "gpt-4o".to_string(),
    );
    let encoded = encoder.encode_content(&flushed).unwrap();

    // 5. Verify encoded format
    let arr = encoded.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["type"], "text");
    assert_eq!(arr[0]["text"], "What do you see?");
    assert_eq!(arr[1]["type"], "image_url");
    let url = arr[1]["image_url"]["url"].as_str().unwrap();
    assert!(url.starts_with("data:image/png;base64,"));
    assert_eq!(arr[1]["image_url"]["detail"], "high");

    // 6. Verify the base64 data survived the roundtrip
    let data_prefix = "data:image/png;base64,";
    let roundtripped_b64 = &url[data_prefix.len()..];
    assert_eq!(roundtripped_b64, original_data);
}

#[test]
fn test_text_only_backward_compat() {
    let encoder = OpenAiEncoder::new(
        EncoderCapabilities { vision: true, tools: true },
        "gpt-4o".to_string(),
    );
    let parts = vec![Part::Text { text: "Just text".to_string() }];
    let encoded = encoder.encode_content(&parts).unwrap();
    // Should be a plain string, not an array
    assert_eq!(encoded, serde_json::Value::String("Just text".to_string()));
}

#[test]
fn test_text_only_model_rejects_multimodal() {
    let encoder = OpenAiEncoder::new(
        EncoderCapabilities { vision: false, tools: true },
        "gpt-3.5-turbo".to_string(),
    );
    let parts = vec![
        Part::Text { text: "Describe this".to_string() },
        Part::Image {
            source: ContentSource::Base64("x".to_string()),
            mime_type: "image/png".to_string(),
            detail: None,
        },
    ];
    let err = encoder.encode_content(&parts).unwrap_err();
    assert!(err.to_string().contains("image"));
    assert!(err.to_string().contains("gpt-3.5-turbo"));
}
```

- [ ] **Step 2: Add dev dependencies**

Add to `framework/zero-llm/Cargo.toml` under `[dev-dependencies]`:

```toml
[dev-dependencies]
tempfile = "3.10"
base64 = "0.22"
```

- [ ] **Step 3: Run integration tests**

Run: `cd /home/videogamer/projects/agentzero && cargo test -p zero-llm --test multimodal_integration 2>&1 | tail -30`
Expected: All 3 tests pass.

- [ ] **Step 4: Run full workspace tests**

Run: `cd /home/videogamer/projects/agentzero && cargo test --workspace 2>&1 | tail -40`
Expected: All pass.

- [ ] **Step 5: Commit**

```bash
git add framework/zero-llm/tests/ framework/zero-llm/Cargo.toml
git commit -m "test(zero-llm): add multimodal integration tests

Full roundtrip: create → flush to disk → encode for API.
Backward compat: text-only → plain string.
Capability enforcement: text-only model rejects images."
```

---

## Task 9: Final Verification + Cleanup

- [ ] **Step 1: Full workspace build**

Run: `cd /home/videogamer/projects/agentzero && cargo build --workspace 2>&1 | tail -30`
Expected: Clean build.

- [ ] **Step 2: Full test suite**

Run: `cd /home/videogamer/projects/agentzero && cargo test --workspace 2>&1 | tail -40`
Expected: All pass.

- [ ] **Step 3: Clippy check**

Run: `cd /home/videogamer/projects/agentzero && cargo clippy --workspace 2>&1 | tail -30`
Fix any warnings in changed files.

- [ ] **Step 4: Commit any cleanup**

```bash
git add -u
git commit -m "chore: clippy fixes for multimodal implementation"
```
