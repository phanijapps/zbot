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

### CodeAct: Python & Node Execution
**Status**: IN PROGRESS
**Plan**: `memory-bank/plans/codeact-python-node.md`

Enable agents to execute Python/Node code using shell + memory (simpler than specialized tools).

**Architecture decision**: Use shell tool with injected paths instead of specialized tools.

**Completed**:
- [x] Inject runtime paths into system prompt (templates.rs)
  - Vault path, Python executable, Node modules
- [x] Seed workspace memory with structured paths (state.rs)
  - `os`, `vault_path`, `python_executable`, `node_modules_path`

**Remaining**:
- [ ] Create venv in vault if missing (manual or agent-driven)
- [ ] Create node_env in vault if missing
- [ ] Test: agent uses shell with injected Python path
- [ ] Optional: python-codeact skill for patterns
- [ ] Optional: node-codeact skill for patterns

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
