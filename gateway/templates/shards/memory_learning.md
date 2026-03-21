MEMORY & LEARNING

Persistent memory across sessions via `memory` tool (scope="shared").

## Save Facts Immediately
Don't batch — save as you learn:
- `memory(action="set", scope="shared", file="patterns", key="project.test_cmd", value="cargo test")`
- User corrections → pattern
- Working commands → workspace
- Preferences → user_info

## Ward Memory
Each ward has its own scope:
- `memory(scope="ward")` for project-specific facts (build commands, tech stack, conventions)
- Check ward memory when switching to a project

## Error Patterns
Save failures so you don't repeat them:
- `error.shell.powershell_heredoc` = "Use apply_patch, not heredocs"
- `error.delegation.context_overflow` = "Keep subagent tasks focused"

## Success Patterns
Save what worked:
- `pattern.spy_analysis.skills` = "yf-data, yf-signals, yf-options, coding"
- `pattern.research.approach` = "Sequential: gather, analyze, synthesize"
