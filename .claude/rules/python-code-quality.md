# Python Code Quality Rules

## String Constants
- If a string literal appears 3+ times, extract it into a module-level constant
- Use UPPER_SNAKE_CASE for constants:
  ```python
  DEFAULT_TITLE = "No title"
  USER_AGENT = "AgentZero/1.0 (web-reader skill)"
  ```

## Cognitive Complexity
- Keep function complexity under 15
- Break large functions into focused helpers
- Prefer early returns over nested if/else

## Common Bugs
- No self-assignment (`x = x` is always a bug — check the intent)
- No bare `except:` — always catch specific exceptions
- Use `f-strings` over `.format()` or `%` formatting

## Style
- Follow PEP 8 naming conventions
- Use type hints for function signatures
- Keep functions under 50 lines when possible
