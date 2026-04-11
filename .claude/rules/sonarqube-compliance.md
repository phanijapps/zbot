# SonarQube Compliance Rules

## Before Committing

### Rust
```bash
cargo fmt --all --check       # Formatting
cargo clippy --all-targets -- -D warnings  # Linting
cargo test --workspace        # Tests pass
```

### TypeScript
```bash
cd apps/ui
npm run lint                  # ESLint
npm run build                 # Type check + build
```

### Python
- No self-assignments
- String constants extracted (3+ occurrences)

## Quality Gates

### New Code Must Not Introduce
- Security vulnerabilities or hotspots
- Reliability bugs (S6848, S6853, S1082, S6443)
- Cognitive complexity > 15 in any function
- Functions nested > 4 levels deep

### Acceptable Suppressions (with comment)
- `#[allow(clippy::too_many_arguments)]` — for established function signatures
- `#[allow(clippy::missing_docs)]` — for internal types
- `// eslint-disable-next-line` — only with specific rule and explanation

### Not Acceptable
- Suppressing security rules
- Suppressing reliability rules without justification
- Adding `#![allow(...)]` at crate level without discussion

## SonarQube Issue Categories

| Priority | Fix Before Merge |
|----------|-----------------|
| Security Vulnerability | Always |
| Security Hotspot | Always |
| Reliability Bug (MAJOR+) | Always |
| Maintainability (complexity > 15) | For new code only |
| Style/Convention | Best effort |

## CI Pipeline
- `security.yaml` — runs on every push/PR: fmt, clippy, audit, npm audit, gitleaks
- `sonarqube.yml` — runs on push to main: coverage + full scan
- Both must pass before merge
