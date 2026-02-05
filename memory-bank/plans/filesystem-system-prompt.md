# Filesystem-Based System Prompt

## Status: COMPLETE

## Overview

Enable the base system prompt to be loaded from `~/Documents/agentzero/INSTRUCTIONS.md` instead of being embedded at compile time. This allows runtime customization without recompiling.

## Current State

```
gateway/src/templates.rs:
  - default_system_prompt() -> String
  - Loads from embedded rust_embed template
  - No filesystem access

gateway/templates/system_prompt.md:
  - Embedded at compile time via rust_embed
  - Cannot be modified without recompiling
```

**Call Sites:**
1. `gateway/src/execution/invoke/setup.rs:172` - `AgentLoader::load_or_create_root()`
2. `gateway/src/execution/runner.rs:341` - creates `AgentLoader`
3. `gateway/src/execution/runner.rs:813` - continuation handler creates `AgentLoader`
4. `gateway/src/execution/delegation/spawn.rs:116` - delegation creates `AgentLoader`

## Target State

```
~/Documents/agentzero/
├── INSTRUCTIONS.md          # User-editable system prompt (Jaffa template)
├── agents/
├── agents_data/
```

**Behavior:**
1. Check if `{data_dir}/INSTRUCTIONS.md` exists
2. If yes → load from filesystem
3. If no → fall back to embedded template

## Task Breakdown

### Phase 1: Modify templates.rs

| Task | Description | File |
|------|-------------|------|
| #1.1 | Add `load_system_prompt(data_dir: &Path) -> String` function | `gateway/src/templates.rs` |
| #1.2 | Keep `default_system_prompt()` as fallback (no breaking change) | `gateway/src/templates.rs` |
| #1.3 | Update existing test | `gateway/src/templates.rs` |
| #1.4 | Add test: filesystem prompt loads when file exists | `gateway/src/templates.rs` |
| #1.5 | Add test: fallback to embedded when file missing | `gateway/src/templates.rs` |

### Phase 2: Modify AgentLoader

| Task | Description | File |
|------|-------------|------|
| #2.1 | Add `config_dir: PathBuf` field to `AgentLoader` | `gateway/src/execution/invoke/setup.rs` |
| #2.2 | Update `AgentLoader::new()` signature to accept `config_dir` | `gateway/src/execution/invoke/setup.rs` |
| #2.3 | Update `load_or_create_root()` to use `load_system_prompt()` | `gateway/src/execution/invoke/setup.rs` |

### Phase 3: Update Call Sites

| Task | Description | File |
|------|-------------|------|
| #3.1 | Update `ExecutionRunner::invoke_agent()` | `gateway/src/execution/runner.rs:341` |
| #3.2 | Update `invoke_continuation()` | `gateway/src/execution/runner.rs:813` |
| #3.3 | Update `spawn_delegated_agent()` | `gateway/src/execution/delegation/spawn.rs:116` |

### Phase 4: Create Default INSTRUCTIONS.md

| Task | Description | File |
|------|-------------|------|
| #4.1 | Copy Jaffa template to gateway templates | `gateway/templates/system_prompt.md` |
| #4.2 | Document in README/AGENTS.md | Documentation |

## Dependencies

```
Phase 1 ──► Phase 2 ──► Phase 3
                          │
                          ▼
                       Phase 4
```

## Code Changes

### templates.rs (Phase 1)

```rust
use std::path::Path;

/// Load system prompt from filesystem, falling back to embedded.
///
/// Checks for `INSTRUCTIONS.md` in the data directory first.
/// If not found, returns the embedded default template.
pub fn load_system_prompt(data_dir: &Path) -> String {
    let instructions_path = data_dir.join("INSTRUCTIONS.md");

    if instructions_path.exists() {
        match std::fs::read_to_string(&instructions_path) {
            Ok(content) if !content.trim().is_empty() => {
                tracing::info!("Loaded system prompt from {:?}", instructions_path);
                return content;
            }
            Ok(_) => {
                tracing::warn!("INSTRUCTIONS.md is empty, using embedded default");
            }
            Err(e) => {
                tracing::warn!("Failed to read INSTRUCTIONS.md: {}, using embedded default", e);
            }
        }
    }

    default_system_prompt()
}

/// Get the embedded default system prompt.
///
/// This is the fallback when no filesystem override exists.
pub fn default_system_prompt() -> String {
    Templates::get("system_prompt.md")
        .map(|file| String::from_utf8_lossy(&file.data).to_string())
        .unwrap_or_else(|| "You are a helpful AI assistant.".to_string())
}
```

### setup.rs (Phase 2)

```rust
pub struct AgentLoader<'a> {
    agent_service: &'a AgentService,
    provider_resolver: ProviderResolver<'a>,
    config_dir: PathBuf,  // NEW
}

impl<'a> AgentLoader<'a> {
    pub fn new(
        agent_service: &'a AgentService,
        provider_service: &'a ProviderService,
        config_dir: PathBuf,  // NEW
    ) -> Self {
        Self {
            agent_service,
            provider_resolver: ProviderResolver::new(provider_service),
            config_dir,
        }
    }

    pub async fn load_or_create_root(...) -> Result<...> {
        // ... existing code ...
        Err(_) if agent_id == "root" => {
            // ...
            let agent = Agent {
                // ...
                instructions: crate::templates::load_system_prompt(&self.config_dir),  // CHANGED
                // ...
            };
        }
    }
}
```

### runner.rs (Phase 3)

```rust
// Line ~341
let agent_loader = AgentLoader::new(
    &self.agent_service,
    &self.provider_service,
    self.config_dir.clone(),  // NEW
);

// Line ~813
let agent_loader = AgentLoader::new(
    &agent_service,
    &provider_service,
    config_dir.clone(),  // NEW
);
```

### spawn.rs (Phase 3)

```rust
// Line ~116
let agent_loader = AgentLoader::new(
    &agent_service,
    &provider_service,
    config_dir.clone(),  // NEW
);
```

## Test Plan

### Unit Tests (templates.rs)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default_system_prompt_contains_expected_content() {
        let prompt = default_system_prompt();
        // Update assertion to match new Jaffa template content
        assert!(prompt.contains("Jaffa") || prompt.contains("Tool Call Guidelines"));
    }

    #[test]
    fn test_load_system_prompt_from_filesystem() {
        let dir = TempDir::new().unwrap();
        let instructions_path = dir.path().join("INSTRUCTIONS.md");
        std::fs::write(&instructions_path, "Custom system prompt content").unwrap();

        let prompt = load_system_prompt(dir.path());
        assert_eq!(prompt, "Custom system prompt content");
    }

    #[test]
    fn test_load_system_prompt_falls_back_when_missing() {
        let dir = TempDir::new().unwrap();
        // No INSTRUCTIONS.md file

        let prompt = load_system_prompt(dir.path());
        // Should return embedded default
        assert!(!prompt.is_empty());
        assert!(prompt.contains("Tool Call Guidelines") || prompt.contains("Jaffa"));
    }

    #[test]
    fn test_load_system_prompt_falls_back_when_empty() {
        let dir = TempDir::new().unwrap();
        let instructions_path = dir.path().join("INSTRUCTIONS.md");
        std::fs::write(&instructions_path, "   \n  ").unwrap();  // whitespace only

        let prompt = load_system_prompt(dir.path());
        // Should return embedded default, not empty string
        assert!(!prompt.trim().is_empty());
    }
}
```

### Integration Tests

The existing `gateway/tests/api_tests.rs` tests use `TempDir` and don't rely on specific system prompt content, so they should continue to work.

**Manual verification:**
1. Start daemon without `INSTRUCTIONS.md` → uses embedded
2. Create `~/Documents/agentzero/INSTRUCTIONS.md` with custom content
3. Restart daemon → loads custom content
4. Delete file → restart → falls back to embedded

## Verification Commands

```bash
# Run unit tests
cargo test -p gateway --lib -- templates

# Run all gateway tests
cargo test -p gateway

# Run integration tests
cargo test -p gateway --test api_tests

# Check compilation
cargo check --workspace
```

## Rollback

If issues arise:
1. Revert `templates.rs` changes
2. Revert `setup.rs` signature change
3. Revert call site changes
4. All changes are additive/non-breaking until Phase 3

## Success Criteria

- [ ] `cargo test -p gateway` passes
- [ ] `cargo test -p gateway --test api_tests` passes
- [ ] Daemon starts with no `INSTRUCTIONS.md` (uses embedded)
- [ ] Daemon loads custom `INSTRUCTIONS.md` when present
- [ ] Daemon logs which source was used
