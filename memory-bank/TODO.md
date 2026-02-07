# AgentZero TODO

## High Priority

### Goal-Oriented Agent Execution
**Status**: ✅ COMPLETE

Agent was too passive - proposed plans instead of executing, lost context between turns.

**Phase 1 & 2 (No code, immediate)**:
- [x] Edit INSTRUCTIONS.md: Replace "propose plan before work" with action-oriented instructions
- [x] Edit algorithmic-art SKILL.md: Add "complete both steps in one response"

**Phase 3 & 4 (Code changes)**:
- [x] Persist tool calls to database (lifecycle.rs, runner.rs, stream.rs)
- [x] Load tool calls into context (repository.rs messages_to_chat_format)

**Testing completed** (sess-6d532fa6):
- [x] Test "use algo art skill to build me a heart" completes in one turn
- [x] Verify tool calls are saved to database (3 tool calls: list_skills, load_skill, write)
- [x] Verify tool calls appear in agent context on next turn

### Code Wards (Agent-Managed Project Containers)
**Status**: ✅ COMPLETE (Phases 1-3)
**Plan**: `memory-bank/plans/code-wards.md`

Named project directories (wards) that agents create, manage, and navigate autonomously.
Replaces per-session `code/{session_id}/` with persistent `wards/{ward_name}/`.
Shared Python venv across all wards. Per-ward node_modules (Node convention).

**Phases**:
- [x] Phase 1: Ward tool + FileSystem trait changes + ward_id persistence
- [x] Phase 2: Wire existing tools (shell, write/edit/read, memory)
- [x] Phase 3: System prompt + skill
- [ ] Phase 4: Creative hub (cross-ward discovery, pattern learning)

### Skill Loading & Unloading
**Status**: Needs planning

Dynamic skill lifecycle management during agent execution.

**Questions to resolve**:
- [ ] When to unload? (explicit, context limit, TTL, LRU?)
- [ ] Skill dependencies (skill A requires skill B loaded)
- [ ] Partial loading (load only relevant sections of large skills)
- [ ] Skill versioning (what if skill changes mid-session?)
- [ ] Conflict resolution (two skills with overlapping instructions)

**Implementation considerations**:
- [ ] Track loaded skills in execution context
- [ ] Token budget management (skills consume context)
- [ ] Skill priority/ordering in system prompt
- [ ] Unload triggers (manual, automatic, or both)
- [ ] Persistence across conversation turns

### Memory & Knowledge Graph Unification
- [ ] Design unified facade: simple memory API backed by knowledge graph
- [ ] Define entity types for memory concepts (Pattern, Preference, Workspace, SessionSummary)
- [ ] Create relationship types for pattern connections
- [ ] Build migration path from current JSON files to graph storage
- [ ] Keep backward-compatible memory() API

## Medium Priority

### Concurrent Access Safety
- [x] Add file locking for shared memory files (fs2 crate)
- [ ] Consider SQLite for shared state (future)
- [ ] Design inter-agent communication pattern

### Skills
- [ ] Create `knowledge-graph` skill explaining when/how to use graph tools
- [ ] Document skill creation patterns

## Low Priority

### Memory Enhancements
- [ ] Fuzzy search for memory tool
- [ ] Memory expiration/cleanup for old entries

## Completed (op_jaffa branch)

- [x] Filesystem-based system prompt (INSTRUCTIONS.md)
- [x] Shared memory system (4 files: user_info, workspace, patterns, session_summaries)
- [x] Workspace auto-inject into executor state
- [x] Sharded templates system
- [x] Auto-create INSTRUCTIONS.md on first run
- [x] Promote skills tools to core (list_skills, load_skill)
- [x] Promote search tools to core (grep, glob)
- [x] Add tooling_skills shard (skills-first approach)
- [x] Add memory_learning shard
- [x] Inject OS environment info
