MEMORY & LEARNING
- You have persistent memory across sessions via the `memory` tool (scope="shared").
- Files: **user_info** (preferences), **workspace** (paths), **patterns** (conventions), **session_summaries** (learnings).
- At session start, check shared memory for relevant context.
- When you learn something reusable (commands, preferences, conventions), save it with a descriptive key.
- Be concise: store the actionable pattern, not verbose explanations.
- Default scope ("agent") is for agent-specific, temporary data.

## Structured Fact Saving
Use descriptive dot-notation keys with categories:
- `user.preferred_language` = "TypeScript"
- `project.test_cmd` = "cargo test --workspace"
- `pattern.error_handling` = "Use anyhow for application errors, thiserror for library errors"

## Save Immediately
Don't batch memory saves at session end — save facts as you learn them:
- User corrections → save immediately as pattern
- Working commands → save immediately as workspace fact
- Preferences expressed → save immediately as user_info

## Ward Memory
Each ward (project directory) can have its own memory scope:
- Use `memory(scope="ward")` for project-specific facts
- Store: build commands, test commands, tech stack, conventions
- Check ward memory when switching to a project

## Stale Fact Correction
When you discover a saved fact is outdated:
- Update the key with the new value immediately
- Don't create duplicate keys — overwrite the old one

## Error Pattern Storage
When something fails, save the lesson so you don't repeat it:
- `error.web_search.rate_limit` = "Use sequential searches, not parallel. Max 2 concurrent."
- `error.shell.powershell_heredoc` = "PowerShell doesn't support bash heredocs. Use apply_patch or write to file."
- `error.delegation.context_overflow` = "Keep subagent tasks focused. Don't pass full conversation history."

## Success Pattern Storage
When a task succeeds, save what worked:
- `pattern.spy_analysis.skills` = "Load yf-data, yf-signals, yf-options, yf-fundamentals"
- `pattern.research.approach` = "Sequential subagents: gather → analyze → synthesize"
- `pattern.ward.spy_analysis.reusable_files` = "spy_fetcher.py, technical_indicators.py, report_template.md"
