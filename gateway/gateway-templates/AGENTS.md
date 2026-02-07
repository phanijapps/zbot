# gateway-templates

System prompt assembly from embedded templates and shard injection into custom INSTRUCTIONS.md.

## Build & Test

```bash
cargo test -p gateway-templates    # 10 tests
```

## Key Types

| Type | Purpose |
|------|---------|
| `Templates` | Rust-embed struct for compile-time embedded template files |

## Public API

| Function | Purpose |
|----------|---------|
| `load_system_prompt()` | Load or create INSTRUCTIONS.md, append shards |
| `default_system_prompt()` | Fallback embedded prompt |
| `load_required_shards()` | Load all required shard files |
| `load_shard()` | Load a single template shard |

## Embedded Templates

```
templates/
├── instructions_starter.md        # Default INSTRUCTIONS.md content
└── shards/
    ├── tooling_skills.md           # Tools + skills + ward instructions
    └── memory_learning.md          # Memory system usage
```

Shards are appended to the user's custom INSTRUCTIONS.md at prompt assembly time.

## File Structure

| File | Purpose |
|------|---------|
| `lib.rs` | All functionality (10 tests) |
