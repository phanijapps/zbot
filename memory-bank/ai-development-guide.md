# AI-Assisted Development Guide for AgentZero

This document captures effective patterns, prompts, and techniques that work well when using AI assistants (like Claude Code) on this project.

---

## 1. Planning Documents That Work

### Effective Plan Structure

The most successful plans in this project follow this pattern:

```markdown
# Feature Name Implementation Plan

## Status: [IN PROGRESS | COMPLETE]

## Overview
[1-3 sentences describing what and why]

## Data Model
[Show the actual data structures - diagrams, schemas, types]

## Task Breakdown

### Phase 1: [Layer Name]
| Task | Description | File |
|------|-------------|------|
| #1   | Specific change | exact/file/path.rs |

### Phase 2: [Next Layer]
...

## Dependencies
[ASCII diagram showing task order]

## Expected Behavior
[Table of scenarios and outcomes]
```

### Why This Works

1. **Status tracking**: Top-level status makes it easy to resume
2. **Concrete data models**: Showing actual schemas/types grounds the implementation
3. **File-level specificity**: Tasks reference exact files, not vague areas
4. **Phase grouping**: Groups tasks by architectural layer (backend → frontend)
5. **Dependency graph**: ASCII diagrams clarify execution order
6. **Behavior tables**: Clear success criteria prevent scope creep

### Real Example: Scoped Event Emission

The `scoped-event-emission.md` plan was highly effective because it:

- Started with verified data model (queried from actual DB)
- Included concrete filter logic in pseudocode
- Tracked bug fixes with root cause analysis
- Updated status as work progressed

---

## 2. Bug Fix Documentation

### Pattern: Root Cause First

When documenting bug fixes, this structure works:

```markdown
## Bug Fix: [Short Title]

**Issue**: [What user observed]

**Root Cause**: [Technical explanation with code snippet]
```rust
// BEFORE (broken)
code_that_was_wrong();

// AFTER (fixed)
code_that_works();
```

**Files Changed**:
- `path/to/file.rs:line_range` - description

**Why It Failed**: [Deeper explanation of the failure mode]
```

### Why This Works

- Future developers understand the "why" not just "what"
- Code snippets make the fix reproducible
- Line numbers enable quick navigation
- "Why It Failed" prevents regression

---

## 3. Effective Prompts

### For Feature Implementation

**Good prompt:**
> "Implement connectors feature. Start by reading the plan at `memory-bank/plans/connectors-cron-logging.md`, then implement Phase 1 tasks."

**Why it works:** Points to existing context, gives clear scope.

**Less effective:**
> "Add a way for agents to send messages to external services"

**Why it's worse:** Requires AI to rediscover design decisions.

### For Bug Investigation

**Good prompt:**
> "Events aren't reaching the UI. Backend logs show events publishing. Check the scope filtering in `gateway/src/websocket/subscriptions.rs` and how frontend subscribes in `apps/ui/src/services/transport/http.ts`."

**Why it works:** Narrows search space, provides two anchor points.

**Less effective:**
> "WebSocket events are broken, fix it"

**Why it's worse:** Too vague, could investigate wrong area.

### For Code Review

**Good prompt:**
> "Review the changes in `gateway/src/connectors/` against the design in `memory-bank/plans/connectors-cron-logging.md`. Check for: error handling, missing edge cases, deviation from spec."

**Why it works:** Provides review criteria, points to spec.

### For Refactoring

**Good prompt:**
> "The event handling in `handler.rs` has grown complex. Extract scope filtering into a separate module following the pattern in `subscriptions.rs`. Keep backward compatibility."

**Why it works:** Identifies the problem, suggests pattern, sets constraint.

---

## 4. Level of Detail in Plans

### Too Little Detail (Fails)

```markdown
## Tasks
1. Add connector support
2. Add cron support
3. Add UI
```

**Problem:** No file paths, no data models, no phases.

### Too Much Detail (Slows Down)

```markdown
## Task 1.1.1
Add `id: String` field to ConnectorConfig struct in line 45 of config.rs

## Task 1.1.2
Add `name: String` field to ConnectorConfig struct in line 46 of config.rs
...
```

**Problem:** Micromanages, doesn't allow AI judgment on implementation.

### Right Level of Detail

```markdown
## Phase 1: Connector Infrastructure

| Task | Description | File |
|------|-------------|------|
| #1 | `ConnectorConfig` and `ConnectorTransport` types | gateway/src/connectors/config.rs |
| #2 | `ConnectorRegistry` with CRUD operations | gateway/src/connectors/mod.rs |
| #3 | Persistence to `connectors.json` | gateway/src/connectors/service.rs |
| #4 | HTTP endpoints for connector management | gateway/src/http/connectors.rs |
```

**Why it works:**
- Groups related work
- Specifies what to create, not how
- File paths guide but don't micromanage
- Assumes competence on implementation details

---

## 5. Architecture Documentation

### What to Include

1. **ASCII diagrams** - Visual overview (see `architecture.md`)
2. **Layer dependencies** - What imports what
3. **Data flows** - Request → Response paths
4. **Core traits** - Key abstractions with signatures
5. **Design decisions** - "Why X instead of Y?"

### What to Skip

- Comprehensive API docs (use code comments)
- Tutorial-style walkthroughs
- Implementation details that change frequently

---

## 6. Techniques That Work

### 1. Read Before Write

Always read relevant files before making changes:
- Check existing patterns
- Understand current state
- Avoid duplicating functionality

### 2. Layer-by-Layer Implementation

Follow the dependency graph:
```
framework/ → runtime/ → services/ → gateway/ → apps/
```

Backend before frontend. Core before edges.

### 3. Test After Each Phase

Don't batch all testing at the end. Verify each phase:
- `cargo check --workspace` after Rust changes
- `npm run build` after TypeScript changes
- Manual testing for integration points

### 4. Update Plans as You Go

Plans are living documents:
- Mark tasks complete with ✅
- Add bug fixes discovered during implementation
- Update status at top of file

### 5. Use Memory Bank for Context

Store key decisions in `memory-bank/`:
- Architecture decisions
- Data models
- Implementation plans
- Test scenarios

This survives across sessions better than conversation history.

---

## 7. Anti-Patterns to Avoid

### 1. Starting Without Reading

**Bad:** Jump straight into coding
**Good:** Read AGENTS.md, architecture.md, relevant plans first

### 2. Solving Problems Not Asked

**Bad:** "While I'm here, let me also refactor this..."
**Good:** Stay focused on the specific task

### 3. Ignoring Existing Patterns

**Bad:** Invent new error handling approach
**Good:** Follow patterns in adjacent files

### 4. Plans Without Data Models

**Bad:** Describe features in prose only
**Good:** Show actual types, schemas, diagrams

### 5. Skipping Root Cause Analysis

**Bad:** "Fixed by changing X to Y"
**Good:** "Root cause was Z, fixed by changing X to Y because..."

---

## 8. Session Management

### Starting a Session

1. Read `CLAUDE.md` (happens automatically)
2. Check `memory-bank/` for relevant plans
3. Review recent commits: `git log --oneline -10`
4. Understand current state before changing anything

### Continuing Work

If resuming a plan:
1. Read the plan file
2. Check status markers
3. Find first incomplete task
4. Continue from there

### Ending a Session

1. Update plan status
2. Commit with descriptive message
3. Note any blockers or next steps in plan

---

## 9. Prompt Templates

### Feature Implementation
```
Implement [feature name].

Context:
- Plan: memory-bank/plans/[plan-name].md
- Related: [existing similar code paths]

Start with Phase [N] tasks. Run `cargo check` after each task.
```

### Bug Investigation
```
[Symptom description]

Suspected area: [file or module]
Related: [other files that interact]

Investigate root cause, propose fix with explanation.
```

### Code Review
```
Review [file/PR] against:
- Design: [plan or spec reference]
- Patterns: [similar code for comparison]

Check for: [specific concerns]
```

### Refactoring
```
Refactor [target code] to [goal].

Constraints:
- Maintain backward compatibility
- Follow pattern in [example file]
- Keep [specific behavior]
```

---

## 10. Project-Specific Tips

### Rust Backend

- Use `cargo check --workspace` frequently (faster than build)
- Follow existing error handling with `anyhow`
- Match async patterns from adjacent code
- Check trait bounds when adding generics

### TypeScript Frontend

- Components go in `apps/ui/src/features/` or `components/`
- Services in `apps/ui/src/services/`
- Follow existing naming conventions
- Use existing hooks before creating new ones

### Testing

- Backend: Add tests in same file as implementation
- Frontend: Colocate tests with components
- E2E: `apps/ui/tests/e2e/`

### Debugging

- Backend: `RUST_LOG=debug cargo run`
- Frontend: Browser DevTools + React DevTools
- WebSocket: Check network tab for message flow

---

## Summary

**What works:**
- Plans with concrete data models and file paths
- Phase-based implementation following dependency order
- Bug documentation with root cause analysis
- Prompts that provide context and narrow scope
- Reading before writing, testing after each phase

**What doesn't work:**
- Vague plans without specifics
- Starting to code without understanding context
- Over-engineering or solving unasked problems
- Skipping incremental verification
