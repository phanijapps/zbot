# CLAUDE.md
This file guides Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview
Read `AGENTS.md` for overview.

## Tech Stack
Read "Technology Stack" section in `memory-bank/architecture.md`

## UI Architecture
Read `apps/ui/ARCHITECTURE.md` before embarking on UI changes or adding new ui components/styles.

## Architecture
Read `memory-bank/architecture.md`

## Design Decisions
Read `memory-bank/decisions.md` for technology choices and architecture decisions (why X over Y).

## Product Context
Read `memory-bank/product-context.md` for vision, principles, target users, and differentiators.

## Common Commands
Check Installation section in `AGENTS.md` at workspace level.

## Development Patterns
- Plans with concrete data models and file paths, not prose
- Layer-by-layer implementation: `framework/ → runtime/ → services/ → gateway/ → apps/`
- Test each phase: `cargo check --workspace` after Rust, `npm run build` after TypeScript
- Read before write: check existing patterns, avoid duplicating functionality
- Follow adjacent code patterns for error handling, naming, async

## Code Style
Review some crates
