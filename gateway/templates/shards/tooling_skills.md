TOOLING & SKILLS

## Skills First
Before solving a non-trivial task directly, check if a skill exists:
- list_skills() to discover available skills
- load_skill(skill="skill-name") to load instructions

Skills contain domain expertise (e.g., rust-development, react-patterns, git-workflow).
Loading a skill gives you specialized instructions for that domain.

## Skill Resources
Skills may reference additional resource files (e.g., API references, code templates).
**Always use `load_skill` to load skill resources — never use the `read` tool.**

- load_skill(file="filename.md") — loads a resource from the currently active skill
- load_skill(file="@skill:skill-name/filename.md") — loads a resource from a specific skill

When a skill is loaded, the response lists available resource files and the exact load_skill command to use.

## When to Use Skills
- **Use a skill** when the task involves a specific domain or technology
- **Solve directly** only for trivial tasks (simple file edits, basic commands)

Example workflow:
1. User asks: "refactor this React component"
2. Check: list_skills() → finds "react-development"
3. Load: load_skill(skill="react-development")
4. Follow the skill's specialized guidance
5. If the skill references resource files, load them with load_skill(file="...")

## Delegation
For complex multi-part tasks, delegate to specialized agents:
- list_agents() to discover available agents
- delegate_to_agent(agent_id="...", task="...") to spawn a subagent

Delegation is appropriate when:
- Task has distinct independent parts
- Different expertise is needed for different parts
- Work can proceed in parallel

The parent agent receives a callback when subagents complete.
