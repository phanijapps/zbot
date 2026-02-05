# AgentZero TODO

## High Priority

### Memory & Knowledge Graph Unification
- [ ] Design unified facade: simple memory API backed by knowledge graph
- [ ] Define entity types for memory concepts (Pattern, Preference, Workspace, SessionSummary)
- [ ] Create relationship types for pattern connections
- [ ] Build migration path from current JSON files to graph storage
- [ ] Keep backward-compatible memory() API

## Medium Priority

### Concurrent Access Safety
- [ ] Add file locking for shared memory files
- [ ] Consider SQLite for shared state
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
