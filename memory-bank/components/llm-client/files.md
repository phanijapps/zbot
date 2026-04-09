# LLM Client — File Reference

## Framework Layer (zero-core)

### Core Types
| File | What |
|------|------|
| `framework/zero-core/src/types.rs` | `Part` enum (Text, Image, File, FunctionCall, FunctionResponse), `ContentSource` (Url, Base64, FileRef), `ImageDetail` (Low, High, Auto), `Content` struct. Helper methods: `type_name()`, `is_multimodal()`, `Part::text()`, `Part::function_call()` |
| `framework/zero-core/src/multimodal.rs` | `flush_part_to_disk()`, `flush_parts_to_disk()`, `rehydrate_source()`, `mime_to_extension()`, internal `write_content_addressed()` (SHA-256 dedup) |
| `framework/zero-core/src/lib.rs` | Exports `pub mod multimodal` |
| `framework/zero-core/Cargo.toml` | Dependencies: `sha2`, `base64`. Dev: `tempfile` |

## Framework Layer (zero-llm)

### Provider Encoding
| File | What |
|------|------|
| `framework/zero-llm/src/encoding.rs` | `ProviderEncoder` trait (`encode_content`, `supports_part`, `filter_unsupported`), `EncodingError` enum |
| `framework/zero-llm/src/openai_encoder.rs` | `OpenAiEncoder` struct, `EncoderCapabilities` struct, `encode_part()` (Part → OpenAI JSON block). Text-only → plain string, multimodal → content array. FileRef rehydration via `rehydrate_source()` |
| `framework/zero-llm/src/lib.rs` | Exports `encoding`, `openai_encoder` modules. Re-exports `ProviderEncoder`, `EncodingError`, `OpenAiEncoder`, `EncoderCapabilities` |

### Integration Tests
| File | What |
|------|------|
| `framework/zero-llm/tests/multimodal_integration.rs` | End-to-end: create → flush → encode. Backward compat test. Capability rejection test |

## Runtime Layer (agent-runtime)

### Message Types
| File | What |
|------|------|
| `runtime/agent-runtime/src/types/messages.rs` | `ChatMessage` struct (content: `Vec<Part>`), custom `Serialize` (text-only → string, multimodal → array), custom `Deserialize` (accepts both), `text_content()`, `has_multimodal_content()`, factory methods (`user`, `assistant`, `system`, `tool_result`) |

### LLM Client
| File | What |
|------|------|
| `runtime/agent-runtime/src/llm/openai.rs` | `OpenAiClient` — `rehydrate_messages()` resolves FileRef → Base64 before API calls. Called in `build_request_body()` |
| `runtime/agent-runtime/src/llm/client.rs` | `LlmClient` trait — `chat()`, `chat_stream()` accept `Vec<ChatMessage>` |

## Runtime Layer (agent-tools)

### Multimodal Tool
| File | What |
|------|------|
| `runtime/agent-tools/src/tools/multimodal.rs` | `MultimodalAnalyzeTool` — direct HTTP call to vision model. `resolve_source()` (file/URL/base64), `infer_image_mime()`, `infer_file_mime()`. Reads `multimodal_config` from ToolContext state |
| `runtime/agent-tools/src/tools/mod.rs` | `pub use multimodal::MultimodalAnalyzeTool`, registered in `optional_tools()` |
| `runtime/agent-tools/src/lib.rs` | Re-exports `MultimodalAnalyzeTool` |

## Gateway Layer

### Settings & Config
| File | What |
|------|------|
| `gateway/gateway-services/src/settings.rs` | `MultimodalConfig` struct (providerId, model, temperature, maxTokens), `Default` impl, added to `ExecutionSettings` with `#[serde(default)]` |
| `gateway/src/http/settings.rs` | `UpdateExecutionSettingsRequest` includes `multimodal` field, `From` impl passes through |

### Executor Builder
| File | What |
|------|------|
| `gateway/gateway-execution/src/invoke/executor.rs` | Reads multimodal config from `settings.json`, resolves provider from `config/providers.json`, injects `multimodal_config` into executor initial state. Registers `MultimodalAnalyzeTool` for both root and subagent registries |

## UI Layer

### Settings Panel
| File | What |
|------|------|
| `apps/ui/src/features/settings/WebSettingsPanel.tsx` | Multimodal card in Settings > Advanced (Eye icon). Provider dropdown, model dropdown, temperature, max tokens |
| `apps/ui/src/services/transport/types.ts` | `MultimodalConfig` TypeScript interface, added to `ExecutionSettings` |

## Skill

| File | What |
|------|------|
| `gateway/templates/skills/eagle-eye/SKILL.md` | Eagle Eye visual intelligence skill — teaches agents to use `multimodal_analyze` for images, PDFs, charts, screenshots |
| `~/Documents/zbot/skills/eagle-eye/SKILL.md` | Runtime copy of the skill |

## Config Files (Runtime)

| File | What |
|------|------|
| `~/Documents/zbot/config/settings.json` | `execution.multimodal` — provider, model, temperature, maxTokens |
| `~/Documents/zbot/config/providers.json` | Provider entries with `id`, `baseUrl`, `apiKey` — resolved for multimodal tool |
