# Orchestrator Configuration

## Goal

Make the root agent's provider, model, temperature, max tokens, and thinking mode configurable via Settings > Advanced tab, stored in settings.json.

## Current State

The root agent is created dynamically in `setup.rs:load_or_create_root()` with hardcoded values:
- `temperature: 0.7`
- `max_tokens: 8192`
- `thinking_enabled: false`
- Provider/model: whatever the default provider offers

No config.yaml on disk. No UI to change these.

## Design

### Storage: settings.json

```json
{
  "execution": {
    "orchestrator": {
      "providerId": null,
      "model": null,
      "temperature": 0.7,
      "maxTokens": 16384,
      "thinkingEnabled": true
    }
  }
}
```

- `providerId: null` → use default provider (from providers.json isDefault flag)
- `model: null` → use provider's default model
- `thinkingEnabled: true` → orchestrator reasons before delegating (default ON)
- `maxTokens: 16384` → higher than old default (8192) to accommodate thinking output

### Rust: OrchestratorConfig

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrchestratorConfig {
    #[serde(default)]
    pub provider_id: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default = "default_temperature")]
    pub temperature: f64,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default = "default_true")]
    pub thinking_enabled: bool,
}
```

Defaults: temperature=0.7, max_tokens=16384, thinking_enabled=true.

Added to `ExecutionSettings`:
```rust
pub struct ExecutionSettings {
    pub max_parallel_agents: u32,
    pub setup_complete: bool,
    pub agent_name: Option<String>,
    pub subagent_non_streaming: bool,
    pub orchestrator: OrchestratorConfig,  // NEW
}
```

### load_or_create_root changes

In `gateway/gateway-execution/src/invoke/setup.rs`, the auto-creation path reads from settings:

```rust
Err(_) if agent_id == "root" => {
    // Read orchestrator config from settings
    let orch = settings.get_execution_settings()
        .map(|s| s.orchestrator)
        .unwrap_or_default();

    // Resolve provider: orchestrator config → default provider
    let provider = match &orch.provider_id {
        Some(id) if !id.is_empty() => self.provider_resolver.get(id)?,
        _ => self.provider_resolver.get_default()?,
    };

    // Resolve model: orchestrator config → provider default
    let model = orch.model
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| provider.default_model().to_string());

    let agent = Agent {
        id: "root".to_string(),
        temperature: orch.temperature,
        max_tokens: orch.max_tokens,
        thinking_enabled: orch.thinking_enabled,
        model,
        provider_id: provider.id.clone().unwrap_or_default(),
        // ... rest unchanged
    };
}
```

**Requirement**: `load_or_create_root` needs access to `SettingsService`. Currently it only has `AgentService` and `ProviderService` via `AgentLoader`. The `AgentLoader` struct needs a `settings` field added, or the settings are passed as a parameter.

### API

No new endpoints. `OrchestratorConfig` serializes as part of `ExecutionSettings` via existing:
- `GET /api/settings/execution` → returns orchestrator config
- `PUT /api/settings/execution` → updates orchestrator config

The `UpdateExecutionSettingsRequest` gets the same `orchestrator` field.

### UI: Settings > Advanced Tab

New "Orchestrator" card below the existing "Execution" card:

- **Provider** dropdown — verified providers + "Default" option (null). Populated from `listProviders()`.
- **Model** dropdown — models from selected provider. Updates when provider changes. "Default" option (null).
- **Temperature** — number input, 0-2, step 0.1, default 0.7
- **Max Output Tokens** — number input, min 1024, default 16384
- **Thinking** — toggle, default ON

Save calls `updateExecutionSettings()` with the full execution settings including orchestrator.

### What stays hardcoded

| Setting | Value | Reason |
|---------|-------|--------|
| `single_action_mode` | true | Architectural — root delegates one at a time |
| `agent_type` | "orchestrator" | Identity |
| `instructions` | SOUL.md + templates | Managed via config files, not UI |
| `mcps` | empty | Root doesn't use MCP servers directly |
| `skills` | empty | Root doesn't load skills directly |

### Thinking Mode Details

When `thinkingEnabled: true`:
- OpenAI-compatible `{"thinking": {"type": "enabled"}}` added to API request
- Model reasons internally before producing tool calls
- Reasoning content streamed via `StreamChunk::Reasoning`
- Max tokens should be ≥16384 to accommodate reasoning + response
- Already validated against model registry capabilities (if model doesn't support thinking, silently disabled with warning log)

### Files to Modify

| File | Change |
|------|--------|
| `gateway/gateway-services/src/settings.rs` | Add `OrchestratorConfig` struct + field on `ExecutionSettings` |
| `gateway/src/http/settings.rs` | Add orchestrator to `UpdateExecutionSettingsRequest` + `From` impl |
| `gateway/gateway-execution/src/invoke/setup.rs` | Read from settings in `load_or_create_root` |
| `apps/ui/src/services/transport/types.ts` | Add `OrchestratorConfig` to `ExecutionSettings` |
| `apps/ui/src/features/settings/WebSettingsPanel.tsx` | Orchestrator card in Advanced tab |
| `memory-bank/architecture.md` | Document orchestrator config |
| `memory-bank/decisions.md` | Document design decisions |
