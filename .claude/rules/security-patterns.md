# Security Pattern Rules

## Regex Safety (ReDoS Prevention)
- Never use unbounded quantifiers with overlapping character classes:
  ```
  BAD:  /([^)]+)/     — backtracking risk
  GOOD: /([\d.e+-]+)/ — bounded, no overlap with delimiter
  GOOD: /([^)]+?)/    — non-greedy (acceptable but not ideal)
  ```
- For URL/path cleaning, prefer string methods over regex:
  ```typescript
  // BAD — regex
  url.replace(/\/+$/, "")

  // GOOD — no regex
  while (url.endsWith("/")) url = url.slice(0, -1);
  ```

## Random Number Generation
- `Math.random()` is acceptable ONLY for visual/UI purposes (layout randomization, colors)
- For anything security-related, use `crypto.getRandomValues()` or `crypto.randomUUID()`
- When deterministic randomness is needed, use a seeded approach: `(index * prime) % range`

## Network Security
- All `curl` commands MUST use `--proto '=https'` to prevent HTTP downgrade on redirects
- Never disable TLS verification (`--insecure`, `-k`)
- API keys must come from environment variables or config files, never hardcoded

## Secrets
- Never commit API keys, tokens, or passwords
- Use `.env` files (gitignored) for local secrets
- Use GitHub Secrets for CI/CD tokens
- `gitleaks` runs in CI — any committed secret will fail the build

## Dependencies
- Run `npm audit --audit-level=high` before merging
- Run `cargo audit` before merging
- Address HIGH and CRITICAL vulnerabilities immediately
