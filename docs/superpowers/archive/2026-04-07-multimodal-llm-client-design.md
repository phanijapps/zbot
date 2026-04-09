# Multimodal LLM Client — Design Spec

**Date:** 2026-04-07
**Status:** Draft
**Scope:** Backend only — framework message types, LLM client encoding, persistence. No UI changes.

## Problem

The agent layer currently handles text-only messages. The `Part` enum in `zero-core` has an unused `Binary` variant, the model registry tracks `vision` capabilities, and the OpenAI client hardcodes `supports_vision() -> false`. There is no end-to-end path for multimodal content (images, PDFs, documents) to flow through the system to LLM providers.

This blocks:
- PDF processing (text + images across pages) via doc-shard agents
- Visual webpage understanding via browsing agents
- Agent→agent multimodal communication (e.g., chart-builder → visual-reviewer)
- Any skill or subagent that produces or consumes non-text content

## Design Principles

1. **Framework as backbone** — carries content, enforces capability checks, no processing logic
2. **Intelligence in agents** — planning agent handles routing, sharding, delegation for multimodal tasks
3. **Content-type-agnostic, capability-aware** — the framework doesn't care what content means, but does prevent sending images to text-only models
4. **No silent degradation** — unsupported content returns clear errors; the agent decides what to do
5. **No DB bloat** — base64 blobs are flushed to disk, only file references are persisted

## Approach: Hybrid (Extend Part Enum + Provider Encoder Trait)

Extend the existing `Part` enum with `Image` and `File` variants for minimal core change. Introduce a `ProviderEncoder` trait in the LLM client layer to isolate provider-specific wire format encoding. Today only OpenAI-compatible format; the trait makes adding providers trivial later.

## Layer 1: Core Types (`zero-core`)

**File:** `framework/zero-core/src/types.rs`

### Part Enum (updated)

```rust
pub enum Part {
    // Existing
    Text { text: String },
    FunctionCall { name: String, args: String, id: String },
    FunctionResponse { id: String, response: String },

    // New multimodal variants (replaces unused Binary)
    Image {
        source: ContentSource,
        mime_type: String,          // "image/png", "image/jpeg", "image/webp", "image/gif"
        detail: Option<ImageDetail>,
    },
    File {
        source: ContentSource,
        mime_type: String,          // "application/pdf", "text/csv", etc.
        filename: Option<String>,   // original filename for display/context
    },
}
```

### ContentSource

```rust
pub enum ContentSource {
    /// Remote or data: URL — stored as-is in DB
    Url(String),

    /// Raw base64 encoded bytes — ephemeral, never persisted to DB
    Base64(String),

    /// Local file path — what DB stores after flushing Base64 to disk
    FileRef(String),
}
```

### ImageDetail

```rust
pub enum ImageDetail {
    Low,    // 512px fixed — fast, fewer tokens
    High,   // full resolution with tiling
    Auto,   // provider decides based on image size
}
```

### Removed

- `Part::Binary { mime_type, data: Vec<u8> }` — removed. Was unused. Replaced by typed `Image` and `File` variants with proper metadata.

## Layer 2: Provider Encoder Trait (`zero-llm`)

**File:** `framework/zero-llm/src/encoding.rs` (new)
**File:** `framework/zero-llm/src/openai.rs` (updated)

### Trait

```rust
pub trait ProviderEncoder {
    /// Encode parts into provider-specific JSON content array
    fn encode_content(&self, parts: &[Part]) -> Result<serde_json::Value, EncodingError>;

    /// Check if this provider/model supports a specific part type
    fn supports_part(&self, part: &Part) -> bool;

    /// Partition parts into supported and unsupported
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

pub enum EncodingError {
    UnsupportedContentType { part_type: String, model: String },
    EncodingFailed { reason: String },
}
```

### OpenAI Encoder

```rust
pub struct OpenAiEncoder {
    capabilities: ModelCapabilities,
    model_id: String,
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

    fn encode_content(&self, parts: &[Part]) -> Result<serde_json::Value, EncodingError> {
        // Check all parts are supported first
        for part in parts {
            if !self.supports_part(part) {
                return Err(EncodingError::UnsupportedContentType {
                    part_type: part.type_name().to_string(),
                    model: self.model_id.clone(),
                });
            }
        }
        // Encode to OpenAI content array format
        // ... (implementation details in plan)
    }
}
```

### OpenAI Encoding Map

| Part Variant | OpenAI Content Block |
|---|---|
| `Part::Text { text }` | `{ "type": "text", "text": "..." }` |
| `Part::Image { source: Base64(data), mime_type, detail }` | `{ "type": "image_url", "image_url": { "url": "data:{mime};base64,{data}", "detail": "{detail}" } }` |
| `Part::Image { source: Url(url), detail }` | `{ "type": "image_url", "image_url": { "url": "{url}", "detail": "{detail}" } }` |
| `Part::Image { source: FileRef(path), ... }` | Read file → encode as Base64 variant above |
| `Part::File { source: Base64(data), mime_type }` | `{ "type": "file", "file": { "url": "data:{mime};base64,{data}" } }` |
| `Part::File { source: Url(url) }` | `{ "type": "file", "file": { "url": "{url}" } }` |
| `Part::File { source: FileRef(path), ... }` | Read file → encode as Base64 variant above |

### Backward Compatibility

When a message contains only `Part::Text`, the `content` field is encoded as a plain `String` (not a content array). This ensures backward compatibility with providers or models that don't support the content array format.

When a message contains any non-text parts, `content` is encoded as a JSON array of content blocks.

## Layer 3: Content Persistence

### Base64 Flush Strategy

Before any `ChatMessage` is written to DB:

1. Walk all `Part`s in the message
2. For each `Part::Image` or `Part::File` with `source: Base64(data)`:
   a. Compute content hash (SHA-256 of the raw bytes)
   b. Write to `agent_data/{session_id}/attachments/{hash}.{ext}`
   c. Replace `source` with `FileRef("agent_data/{session_id}/attachments/{hash}.{ext}")`
3. `Part::Text`, `Url`, and `FileRef` sources pass through unchanged

### Rehydration

When a message with `FileRef` sources needs to go to an LLM:

1. `ProviderEncoder` encounters `FileRef(path)`
2. Reads file from disk
3. Encodes as base64 for the API call
4. The `FileRef` in the stored message is not modified

This means disk is the source of truth for blob content. DB stores only references.

### Deduplication

Content-addressed storage (`{hash}.{ext}`) means the same image sent twice only stores once. The hash-based naming handles this automatically.

## Layer 4: Integration — Agent Runtime

**File:** `runtime/agent-runtime/src/types/messages.rs`

### ChatMessage Update

```rust
pub struct ChatMessage {
    pub role: String,
    pub content: Vec<Part>,          // Was: String — now multimodal
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_call_id: Option<String>,
}
```

### Executor Changes

**File:** `runtime/agent-runtime/src/executor.rs`

1. When building LLM request, create `OpenAiEncoder` with model's capabilities from registry
2. Pass `message.content` (Vec<Part>) through `encoder.encode_content()`
3. If `EncodingError::UnsupportedContentType` returned, propagate as execution error back to the agent
4. Before persisting any message to DB, run the base64 flush

### Model Capability Lookup

The executor already has access to the model registry via services. When creating the encoder:

```rust
let capabilities = model_service.get_capabilities(&model_id);
let encoder = OpenAiEncoder::new(capabilities, model_id.clone());
```

## What Changes

| Layer | File(s) | Change |
|---|---|---|
| `zero-core` | `framework/zero-core/src/types.rs` | Update `Part` enum, add `ContentSource`, `ImageDetail` |
| `zero-llm` | `framework/zero-llm/src/encoding.rs` (new) | `ProviderEncoder` trait, `EncodingError` |
| `zero-llm` | `framework/zero-llm/src/openai.rs` | `OpenAiEncoder` impl, update message building |
| `agent-runtime` | `runtime/agent-runtime/src/types/messages.rs` | `ChatMessage.content` → `Vec<Part>` |
| `agent-runtime` | `runtime/agent-runtime/src/executor.rs` | Wire encoder into LLM call path, base64 flush before DB write |
| `agent-tools` | `runtime/agent-tools/src/tools/multimodal.rs` (new) | `multimodal_analyze` tool implementation |
| `gateway` | Settings schema | `multimodal` config block (provider, model, temperature, maxTokens) |

## Migration: ChatMessage Content Field

Changing `ChatMessage.content` from `String` to `Vec<Part>` is a breaking change. All existing code that constructs or reads `ChatMessage` needs updating. To ease migration:

```rust
impl ChatMessage {
    /// Convenience constructor for text-only messages (most common case)
    pub fn text(role: &str, text: impl Into<String>) -> Self {
        Self {
            role: role.to_string(),
            content: vec![Part::Text { text: text.into() }],
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// Get text content as a single string (for backward compat in consumers that only need text)
    pub fn text_content(&self) -> String {
        self.content.iter().filter_map(|p| match p {
            Part::Text { text } => Some(text.as_str()),
            _ => None,
        }).collect::<Vec<_>>().join("\n")
    }
}
```

This lets existing callsites migrate from `ChatMessage { content: "hello".into(), .. }` to `ChatMessage::text("user", "hello")` with minimal churn, while new multimodal callsites construct `Vec<Part>` directly.

## What Does NOT Change

- **Gateway WebSocket protocol** — UI changes are a separate activity
- **Agent tools** — existing tools produce text; they'll produce `Part`s when individual tools are updated
- **Delegation/subagent spawning** — already passes messages; multimodal parts flow through automatically
- **Skills and planning agent** — consume these types when ready
- **Model registry** — already has `vision` capability; no schema changes needed

## Layer 5: Default Multimodal Config + `multimodal_analyze` Tool

### Settings Configuration

A default multimodal model in `settings.json` ensures the system always has a vision-capable path, even without specialized agents:

```json
{
  "multimodal": {
    "provider": "openai",
    "model": "gpt-4o",
    "temperature": 0.3,
    "maxTokens": 4096
  }
}
```

This is the "always available" vision model. Skills, tools, and agents reference it by convention rather than hardcoding model IDs.

### `multimodal_analyze` Tool

A framework-level tool available to all agents. Any agent — even one running on a text-only model — can process visual content by calling this tool.

**Tool Definition:**

```rust
multimodal_analyze {
    /// Content items to analyze — images, files, or a mix
    content: Vec<MultimodalInput>,

    /// Natural language prompt describing what to extract or analyze
    prompt: String,

    /// Optional JSON Schema — when provided, the response is structured JSON
    output_schema: Option<serde_json::Value>,
}

enum MultimodalInput {
    /// Image from URL, file path, or base64
    Image { source: String, detail: Option<String> },

    /// Document file (PDF, etc.) from URL or file path
    File { source: String },
}
```

**Example tool calls:**

```json
// Extract tables from a PDF
{
  "name": "multimodal_analyze",
  "arguments": {
    "content": [
      { "file": { "source": "file:///workspace/report.pdf" } }
    ],
    "prompt": "Extract all tables with their headers and row data",
    "output_schema": {
      "type": "object",
      "properties": {
        "tables": {
          "type": "array",
          "items": {
            "type": "object",
            "properties": {
              "title": { "type": "string" },
              "headers": { "type": "array", "items": { "type": "string" } },
              "rows": { "type": "array", "items": { "type": "array", "items": { "type": "string" } } }
            }
          }
        }
      }
    }
  }
}

// Describe a screenshot
{
  "name": "multimodal_analyze",
  "arguments": {
    "content": [
      { "image": { "source": "file:///tmp/screenshot.png", "detail": "high" } }
    ],
    "prompt": "Describe the UI layout, identify interactive elements, and note any visual issues"
  }
}

// Compare two images
{
  "name": "multimodal_analyze",
  "arguments": {
    "content": [
      { "image": { "source": "file:///workspace/before.png" } },
      { "image": { "source": "file:///workspace/after.png" } }
    ],
    "prompt": "What changed between these two screenshots?"
  }
}
```

**Internal execution flow:**

1. Read `settings.multimodal` for provider/model config
2. Resolve `source` strings: `file://` paths → read from disk → `ContentSource::Base64`, URLs → `ContentSource::Url`
3. Build `Vec<Part>` from inputs + `Part::Text` from prompt
4. Create `OpenAiEncoder` with the multimodal model's capabilities
5. Make a one-shot LLM call (not a conversation — single request/response)
6. If `output_schema` provided, pass as `response_format` for structured output
7. Return response text (or parsed JSON) as the tool result

**Why a tool, not a subagent:**

- **Zero overhead** — no session, no execution context, no ward injection. Just a direct LLM call.
- **Synchronous** — result comes back in the same turn, agent continues reasoning immediately.
- **Universal** — available to every agent by default. No delegation, no capability matching.
- **Composable** — an agent can call it multiple times in one turn (e.g., analyze page 1, then page 2).

### Multimodal Processing Paths

There are three ways multimodal content gets processed, from most natural to most fallback:

**Path 1: Native multimodal agent (primary)**
The agent itself runs on a vision-capable model (e.g., a subagent spawned with GPT-4o). Multimodal `Part`s flow directly in `ChatMessage.content` — the agent sees images/files natively alongside text. No tool call, no delegation. This is the most natural and efficient path.

- Root agent on a vision model → processes directly
- Subagent spawned with a vision model → receives multimodal content in its task context
- Planning agent delegates *with* the image content inline → subagent sees it immediately

**Path 2: Specialized agent/skill delegation**
A specialized agent (doc-shard, vision-analyzer) exists with domain knowledge for the content type. The planning agent delegates the multimodal content to it. The specialist runs on a vision model and returns structured results.

- Best for complex tasks: multi-page PDFs, visual QA workflows, comparative analysis
- The specialist adds domain intelligence beyond raw vision capability

**Path 3: `multimodal_analyze` tool (safety net)**
The agent is on a text-only model and no specialist is available. It calls `multimodal_analyze` as a tool — a one-shot LLM call to the default vision model from settings. Results come back as text/JSON in the same turn.

- Universal fallback — available to every agent regardless of its own model
- Zero overhead — no session, no execution context, just a direct call
- Handles the "fresh install, no specialists configured" case

**Path 4: No vision capability at all**
No multimodal config in settings, no vision-capable agents. The system returns a clear error: "No multimodal model configured. Add a vision-capable model to Settings > Multimodal."

The planning agent learns these paths. The framework provides the pipes (Layer 1-4) and the safety net (Layer 5); agent intelligence decides which path to use.

## Error Flow

1. Agent (or subagent/skill) builds a message with `Part::Image` content
2. Executor passes it to `OpenAiEncoder::encode_content()`
3. If model lacks vision: `EncodingError::UnsupportedContentType { part_type: "Image", model: "gpt-3.5-turbo" }`
4. Executor surfaces this as an execution error in the conversation
5. Planning agent sees the error, delegates to a vision-capable model/subagent

No silent dropping. No auto-fallback. Clear error, agent adapts.

## Testing Strategy

- **Unit tests** for `OpenAiEncoder`: encode each Part variant, verify JSON output matches OpenAI spec
- **Unit tests** for capability checks: text-only model rejects Image parts, vision model accepts all
- **Unit tests** for base64 flush: verify Base64 → FileRef conversion, deduplication via hash
- **Integration test**: round-trip — create multimodal ChatMessage, persist to DB (verify no base64), rehydrate for LLM call (verify base64 restored)
- **Backward compat test**: text-only messages still encode as plain string, not content array
- **Unit tests** for `multimodal_analyze` tool: verify source resolution (file://, URL, base64), prompt construction, output_schema passthrough
- **Integration test** for `multimodal_analyze`: end-to-end call with mock provider, verify correct OpenAI content blocks sent
- **Settings test**: missing multimodal config returns clear error, valid config resolves to correct provider/model
