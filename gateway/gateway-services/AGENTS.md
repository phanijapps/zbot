# gateway-services

File-backed configuration services with RwLock caching for agents, LLM providers, MCP servers, skills, settings, and agent delegation permissions.

## Build & Test

```bash
cargo test -p gateway-services    # 5 tests
```

## Key Types

| Type | Purpose |
|------|---------|
| `AgentService` | Agent config CRUD with RwLock cache |
| `ProviderService` | LLM provider management with RwLock cache |
| `McpService` | MCP server config management |
| `SkillService` | Skill discovery and loading |
| `SettingsService` | Application settings with RwLock cache |
| `AgentRegistry` | Agent delegation permission checking |
| `AgentConfig` / `Agent` | Agent configuration types |
| `Provider` / `ProviderTestResult` | Provider types |

## Common API Pattern

Each service follows the same pattern:

```rust
service.list() -> Vec<Config>
service.get(id) -> Option<Config>
service.create(request) -> Config
service.update(id, request) -> Config
service.delete(id)
service.reload()  // Invalidate RwLock cache
```

## File Structure

| File | Purpose |
|------|---------|
| `lib.rs` | Public exports |
| `agents.rs` | AgentService |
| `providers.rs` | ProviderService |
| `mcp.rs` | McpService |
| `skills.rs` | SkillService |
| `settings.rs` | SettingsService |
| `agent_registry.rs` | AgentRegistry (5 tests) |
