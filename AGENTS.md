# Agent Zero

Desktop application for creating AI agents with visual workflow orchestration.

## Quick Reference

| Command | Purpose |
|---------|---------|
| `npm install` | Install dependencies |
| `npm run tauri dev` | Development mode |
| `npm run tauri build` | Production build |
| `cargo check --workspace` | Verify Rust code |
| `npx tsc --noEmit` | Verify TypeScript |

## Architecture

- **Frontend**: React 19 + TypeScript + Vite
- **Backend**: Tauri 2.x + Rust
- **Database**: SQLite
- **Workflow**: XY Flow (React Flow v12+)

See `memory-bank/` for detailed documentation:
- `product.md` - Product definition
- `architecture.md` - Technical architecture
- `technical_map.md` - Key modules, decisions, fixes

## Key Directories

```
src/features/workflow-ide/    # Visual workflow builder
src/features/agent-channels/  # Chat interface
crates/zero-*/                # Framework crates
application/*/                # Application crates
src-tauri/src/commands/       # Tauri IPC commands
```

## Conventions

1. Instructions in `AGENTS.md` files, not `config.yaml`
2. Orchestrator config at flow level, not as node
3. Frontend generates invocation IDs before backend calls
