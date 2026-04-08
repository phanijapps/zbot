# LLM Client — Error Handling & Resilience

## Error Points and Fallbacks

### 1. Unsupported Content Type (Capability Check)

**Trigger**: `OpenAiEncoder.encode_content()` receives Image/File parts but model lacks `vision` capability.

**Behavior**: Returns `EncodingError::UnsupportedContentType { part_type: "image", model: "gpt-3.5-turbo" }`. Error propagates to the agent as an execution error. Planning agent sees it and delegates to a vision-capable model.

**No silent dropping.** The framework never strips content quietly.

**Location**: `framework/zero-llm/src/openai_encoder.rs`, `encode_content()` validation loop.

---

### 2. No Multimodal Config (Tool Fallback)

**Trigger**: `multimodal_analyze` tool calls `ctx.get_state("multimodal_config")` and gets `None`.

**Behavior**: Returns clear error JSON: `{ "error": "No multimodal model configured. Add a vision-capable model to Settings > Advanced > Multimodal." }`

**Causes**:
- User hasn't configured a multimodal provider/model in settings
- Provider ID in settings doesn't match any entry in `config/providers.json`
- The `config/providers.json` file path was wrong (was `providers.json`, fixed to `config/providers.json`)

**Location**: `runtime/agent-tools/src/tools/multimodal.rs`, top of `execute()`.

---

### 3. Provider Credentials Not Found

**Trigger**: `ExecutorBuilder.build()` reads `config/providers.json` but can't find a provider matching the configured `providerId`.

**Behavior**: `multimodal_config` state is NOT injected — silently skipped. The tool then hits error point #2.

**Diagnosis**: Check that `settings.json > execution.multimodal.providerId` matches an `id` field in `config/providers.json`.

**Location**: `gateway/gateway-execution/src/invoke/executor.rs`, provider lookup block.

---

### 4. File Not Found (Source Resolution)

**Trigger**: `resolve_source("/path/to/image.png")` in the multimodal tool, file doesn't exist.

**Behavior**: Returns `ZeroError::Tool("File not found: /path/to/image.png")`. Agent sees the error and can adjust.

**Location**: `runtime/agent-tools/src/tools/multimodal.rs`, `resolve_source()`.

---

### 5. Base64 Decode Failure (Flush)

**Trigger**: `flush_part_to_disk()` receives malformed base64 data.

**Behavior**: Returns `std::io::Error(InvalidData)`. Caller decides — typically the message is rejected before DB persistence.

**Location**: `framework/zero-core/src/multimodal.rs`, `write_content_addressed()`.

---

### 6. FileRef Rehydration Failure

**Trigger**: `rehydrate_source()` can't read the file at the FileRef path (deleted, permissions, etc.).

**Behavior in OpenAiClient**: Warning logged, original part kept as-is. The API call may fail with the provider if the content is garbled.

**Behavior in OpenAiEncoder**: Returns `EncodingError::Io(...)`.

**Behavior in multimodal tool**: Returns `ZeroError::Tool("Failed to resolve image: ...")`.

**Location**: `framework/zero-core/src/multimodal.rs` and `runtime/agent-runtime/src/llm/openai.rs`.

---

### 7. API Call Failure (multimodal_analyze)

**Trigger**: The vision model API returns non-2xx status.

**Behavior**: Returns `ZeroError::Tool("Multimodal API error (status): error_body")`. Agent sees the full error including status code and response body.

**Common causes**:
- Model doesn't support the content format (older models)
- Rate limiting (429)
- Invalid API key
- Model not available on the provider

**Location**: `runtime/agent-tools/src/tools/multimodal.rs`, HTTP response check.

---

### 8. Backward Compat Deserialization

**Trigger**: Old JSON with `"content": "string"` loaded from DB.

**Behavior**: Custom deserializer converts to `vec![Part::Text { text: "string" }]`. Transparent.

**Trigger**: Null content `"content": null`.

**Behavior**: Converted to empty `vec![]`.

**Trigger**: Unexpected content type (number, object).

**Behavior**: Returns `serde::de::Error("expected string or array for content, got ...")`.

**Location**: `runtime/agent-runtime/src/types/messages.rs`, custom `Deserialize` impl.

## Resilience Summary

| Failure | Impact | Recovery |
|---------|--------|----------|
| Non-vision model gets image | Error to agent | Agent delegates to vision model |
| No multimodal config | Tool returns error JSON | User configures in Settings > Advanced |
| Provider ID mismatch | Config not injected | Check settings vs providers.json |
| Source file missing | Tool error | Agent adjusts path or reports |
| Base64 malformed | Flush fails | Message rejected |
| FileRef file deleted | Rehydration fails | Re-flush from original source |
| Vision API down | HTTP error | Agent retries or falls back |
| Old DB format | Auto-converted | Transparent |
