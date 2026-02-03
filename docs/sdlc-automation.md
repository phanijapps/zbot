# SDLC Automation with AgentZero

A comprehensive guide to automating Software Development Lifecycle tasks using AgentZero's connector architecture.

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Use Cases](#use-cases)
   - [PR Code Review](#1-pr-code-review)
   - [Issue Triage and Labeling](#2-automated-issue-triage-and-labeling)
   - [CI/CD Pipeline Failure Analysis](#3-cicd-pipeline-failure-analysis)
   - [Release Notes Generation](#4-release-notes-generation)
   - [Security Vulnerability Assessment](#5-security-vulnerability-assessment)
   - [Documentation Generation](#6-documentation-generation-from-code)
   - [Test Case Generation](#7-test-case-generation)
4. [Integration Patterns](#integration-patterns)
5. [Connector Configurations](#connector-configurations)

---

## Overview

### Why SDLC Automation with AI Agents?

Modern software development involves repetitive, time-consuming tasks that drain developer productivity:

- **Code reviews** require context switching and deep focus
- **Issue triage** demands pattern recognition across hundreds of tickets
- **Pipeline failures** need rapid root cause analysis
- **Documentation** falls behind because it's tedious
- **Security assessments** require specialized knowledge

AI agents excel at these tasks because they can:

1. **Process context at scale** - Read entire codebases, commit histories, and documentation
2. **Apply consistent standards** - Never forget coding guidelines or security policies
3. **Operate 24/7** - Review PRs at 3am, triage issues on weekends
4. **Learn patterns** - Improve recommendations based on team feedback
5. **Reduce toil** - Free developers to focus on creative problem-solving

### AgentZero's Approach

AgentZero provides a flexible connector architecture that integrates with your existing DevOps toolchain:

```
                    ┌─────────────────────────────────────────┐
                    │              DevOps Tools               │
                    │  GitHub  GitLab  Jira  Jenkins  Slack   │
                    └──────────────┬──────────────────────────┘
                                   │ Webhooks
                                   ▼
┌──────────────────────────────────────────────────────────────┐
│                         AgentZero                             │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────────┐  │
│  │    Cron     │    │   Gateway   │    │   Connectors    │  │
│  │  Scheduler  │───▶│   Submit    │───▶│  (HTTP/CLI)     │  │
│  └─────────────┘    └──────┬──────┘    └────────┬────────┘  │
│                            │                     │           │
│                     ┌──────▼──────┐             │           │
│                     │   Agent     │─────────────┘           │
│                     │  Execution  │                          │
│                     └─────────────┘                          │
└──────────────────────────────────────────────────────────────┘
                                   │
                                   ▼ Response Dispatch
                    ┌─────────────────────────────────────────┐
                    │           Your Services                  │
                    │  PR Comments  Slack  Email  Dashboards   │
                    └─────────────────────────────────────────┘
```

---

## Architecture

### Core Components

| Component | Purpose | Endpoint |
|-----------|---------|----------|
| **Gateway Submit** | Trigger agent execution | `POST /api/gateway/submit` |
| **Connectors** | Receive agent responses | HTTP webhook or CLI |
| **Cron Scheduler** | Scheduled triggers | 6-field cron expressions |
| **Status API** | Monitor execution | `GET /api/gateway/status/:session_id` |

### Submit Request Structure

```json
{
  "agent_id": "string",           // Agent to execute (e.g., "code-reviewer")
  "message": "string",            // Task description with context
  "source": "api",                // Trigger source: web|cli|api|cron|plugin
  "respond_to": ["connector-id"], // Where to send the response
  "thread_id": "string",          // For conversation threading
  "external_ref": "string",       // Correlation ID (PR number, issue ID)
  "metadata": {}                  // Custom data passed to agent
}
```

### Response Structure (Webhook Payload)

```json
{
  "context": {
    "session_id": "sess-abc123",
    "thread_id": "pr-456",
    "agent_id": "code-reviewer",
    "timestamp": "2026-02-03T10:30:00Z"
  },
  "capability": "respond",
  "payload": {
    "message": "Agent's response text",
    "execution_id": "exec-xyz789",
    "conversation_id": "conv-abc123"
  }
}
```

---

## Use Cases

### 1. PR Code Review

Automatically review pull requests for code quality, security issues, and best practices.

#### Trigger: GitHub Webhook

When a PR is opened or updated, GitHub sends a webhook to your integration service, which forwards to AgentZero.

```bash
# Trigger PR review
curl -X POST http://localhost:18791/api/gateway/submit \
  -H "Content-Type: application/json" \
  -d '{
    "agent_id": "code-reviewer",
    "message": "Review this pull request:\n\nRepository: octocat/hello-world\nPR #42: Add user authentication\nBranch: feature/auth -> main\n\nChanged files:\n- src/auth/login.ts (+156, -23)\n- src/auth/session.ts (+89, -0)\n- tests/auth.test.ts (+45, -0)\n\nDiff summary:\n```diff\n+export async function validateCredentials(email: string, password: string) {\n+  const user = await db.users.findByEmail(email);\n+  if (!user) return null;\n+  return bcrypt.compare(password, user.passwordHash);\n+}\n```\n\nFocus on: security, error handling, test coverage",
    "source": "api",
    "respond_to": ["github-pr-commenter"],
    "thread_id": "pr-42",
    "external_ref": "octocat/hello-world#42",
    "metadata": {
      "repo": "octocat/hello-world",
      "pr_number": 42,
      "author": "developer123"
    }
  }'
```

#### Response

```json
{
  "session_id": "sess-abc123",
  "execution_id": "exec-def456",
  "conversation_id": "conv-ghi789"
}
```

#### Webhook Payload (to github-pr-commenter)

```json
{
  "context": {
    "session_id": "sess-abc123",
    "thread_id": "pr-42",
    "agent_id": "code-reviewer",
    "timestamp": "2026-02-03T10:30:00Z"
  },
  "capability": "respond",
  "payload": {
    "message": "## Code Review Summary\n\n### Security\n- **CRITICAL**: Password comparison should use constant-time comparison to prevent timing attacks. Replace `bcrypt.compare` usage pattern.\n- Consider rate limiting on login attempts.\n\n### Code Quality\n- Good separation of concerns between login and session management.\n- Missing null check on `user.passwordHash` before comparison.\n\n### Test Coverage\n- Tests cover happy path but missing edge cases:\n  - Empty email/password\n  - SQL injection attempts\n  - Unicode handling\n\n**Recommendation**: Request changes for security fixes before merge.",
    "execution_id": "exec-def456",
    "conversation_id": "conv-ghi789"
  }
}
```

#### Cron: Scheduled Review Reminders

```bash
# Create cron job for stale PR reminders
curl -X POST http://localhost:18791/api/cron \
  -H "Content-Type: application/json" \
  -d '{
    "id": "stale-pr-reminder",
    "name": "Stale PR Reminder",
    "schedule": "0 0 9 * * 1-5",
    "agent_id": "pr-tracker",
    "message": "Check for PRs open more than 3 days without review. List them with authors and reviewers.",
    "respond_to": ["slack-dev-channel"],
    "timezone": "America/New_York"
  }'
```

---

### 2. Automated Issue Triage and Labeling

Classify incoming issues, assign priority, and suggest appropriate labels.

#### Trigger: Issue Created Webhook

```bash
curl -X POST http://localhost:18791/api/gateway/submit \
  -H "Content-Type: application/json" \
  -d '{
    "agent_id": "issue-triager",
    "message": "Triage this new issue:\n\nTitle: App crashes when uploading large files\n\nBody:\nWhen I try to upload a file larger than 50MB, the app crashes with no error message. This happens on both Chrome and Firefox.\n\nSteps to reproduce:\n1. Go to upload page\n2. Select a file > 50MB\n3. Click upload\n4. App freezes then crashes\n\nExpected: Upload should work or show error\nActual: Silent crash\n\nEnvironment: macOS 14.2, Chrome 120\n\nProvide:\n1. Suggested labels (comma-separated)\n2. Priority (P0-P4)\n3. Affected component\n4. Similar issues if any\n5. Initial triage notes",
    "source": "api",
    "respond_to": ["github-issue-labeler"],
    "external_ref": "octocat/app#789",
    "metadata": {
      "issue_number": 789,
      "repo": "octocat/app"
    }
  }'
```

#### Expected Response Payload

```json
{
  "context": {
    "session_id": "sess-triage-001",
    "thread_id": null,
    "agent_id": "issue-triager",
    "timestamp": "2026-02-03T14:22:00Z"
  },
  "capability": "respond",
  "payload": {
    "message": "## Issue Triage\n\n**Labels**: `bug`, `upload`, `crash`, `needs-investigation`\n\n**Priority**: P2 (High - affects core functionality but has workaround: smaller files)\n\n**Component**: Upload Service / File Handler\n\n**Similar Issues**:\n- #456 - Memory leak in file upload (closed, fixed in v2.1)\n- #623 - Upload timeout for large files (open)\n\n**Triage Notes**:\nThis appears to be a client-side memory issue when handling large file buffers. The silent crash suggests unhandled exception in the file reader. Recommend:\n1. Add file size validation before upload attempt\n2. Implement chunked upload for large files\n3. Add error boundary with user-friendly message\n\nAssign to: @frontend-team",
    "execution_id": "exec-triage-001",
    "conversation_id": "conv-triage-001"
  }
}
```

---

### 3. CI/CD Pipeline Failure Analysis

Analyze build failures and provide actionable debugging guidance.

#### Trigger: CI Failure Webhook (Jenkins/GitHub Actions)

```bash
curl -X POST http://localhost:18791/api/gateway/submit \
  -H "Content-Type: application/json" \
  -d '{
    "agent_id": "pipeline-analyzer",
    "message": "Analyze this CI pipeline failure:\n\nPipeline: main-build #1234\nStatus: FAILED\nDuration: 12m 34s\nTriggered by: Push to main (commit abc1234)\n\nFailed Stage: unit-tests\nFailed Job: test-api\n\nError Output:\n```\nRUNNING: npm test\n\n FAIL  src/api/users.test.ts\n  UserService\n    createUser\n      ✓ creates user with valid data (45ms)\n      ✕ throws on duplicate email (123ms)\n\n  ● UserService › createUser › throws on duplicate email\n\n    Expected: UniqueConstraintError\n    Received: undefined\n\n      45 |     await service.createUser(userData);\n      46 |     await service.createUser(userData); // duplicate\n    > 47 |     expect(error).toBeInstanceOf(UniqueConstraintError);\n\nTest Suites: 1 failed, 23 passed, 24 total\nTests:       1 failed, 156 passed, 157 total\n```\n\nRecent commits:\n- abc1234: Refactor user service to use new ORM\n- def5678: Add email validation\n\nProvide: root cause, fix suggestion, and prevention strategy",
    "source": "api",
    "respond_to": ["slack-ci-alerts", "github-commit-commenter"],
    "external_ref": "build-1234",
    "metadata": {
      "pipeline": "main-build",
      "build_number": 1234,
      "commit": "abc1234"
    }
  }'
```

#### Cron: Daily Pipeline Health Report

```bash
curl -X POST http://localhost:18791/api/cron \
  -H "Content-Type: application/json" \
  -d '{
    "id": "daily-ci-health",
    "name": "Daily CI Health Report",
    "schedule": "0 0 8 * * 1-5",
    "agent_id": "ci-reporter",
    "message": "Generate daily CI/CD health report:\n- Build success rate (last 24h)\n- Average build time\n- Most common failure reasons\n- Flaky test identification\n- Recommendations for improvement",
    "respond_to": ["slack-engineering", "email-tech-leads"],
    "timezone": "America/Los_Angeles"
  }'
```

---

### 4. Release Notes Generation

Automatically generate release notes from commits, PRs, and issues.

#### Trigger: Release Tag Created

```bash
curl -X POST http://localhost:18791/api/gateway/submit \
  -H "Content-Type: application/json" \
  -d '{
    "agent_id": "release-notes-generator",
    "message": "Generate release notes for version v2.5.0:\n\nPrevious version: v2.4.0\nRelease date: 2026-02-03\n\nMerged PRs since v2.4.0:\n- #234: Add dark mode support (feature)\n- #245: Fix memory leak in image processor (bug)\n- #251: Upgrade to React 19 (chore)\n- #256: Add French localization (i18n)\n- #261: Improve search performance by 40% (performance)\n- #267: Fix XSS vulnerability in comments (security)\n\nBreaking changes:\n- #251: Minimum Node version now 20.x\n\nGenerate:\n1. User-facing changelog (for website)\n2. Developer changelog (detailed)\n3. Migration guide if needed\n4. Highlight security fixes",
    "source": "api",
    "respond_to": ["github-release-updater", "slack-releases"],
    "external_ref": "v2.5.0",
    "metadata": {
      "version": "v2.5.0",
      "previous_version": "v2.4.0"
    }
  }'
```

#### Response Example

```json
{
  "context": {
    "session_id": "sess-release-001",
    "thread_id": null,
    "agent_id": "release-notes-generator",
    "timestamp": "2026-02-03T16:00:00Z"
  },
  "capability": "respond",
  "payload": {
    "message": "# Release v2.5.0\n\n## Highlights\n- Dark mode is here! Toggle in Settings > Appearance\n- Search is now 40% faster\n- French language support added\n\n## Security\n- Fixed XSS vulnerability in comment rendering (CVE-2026-1234)\n\n## Bug Fixes\n- Resolved memory leak when processing large images\n\n## Breaking Changes\n- **Node.js 20.x required** - Please upgrade before deploying\n\n## Migration Guide\n```bash\n# Update Node.js\nnvm install 20\nnvm use 20\n\n# Update dependencies\nnpm install\n```\n\n## Developer Changelog\nSee full PR list: [v2.4.0...v2.5.0](https://github.com/org/repo/compare/v2.4.0...v2.5.0)",
    "execution_id": "exec-release-001",
    "conversation_id": "conv-release-001"
  }
}
```

---

### 5. Security Vulnerability Assessment

Scan code changes for security vulnerabilities and provide remediation guidance.

#### Trigger: PR with Security-Sensitive Files

```bash
curl -X POST http://localhost:18791/api/gateway/submit \
  -H "Content-Type: application/json" \
  -d '{
    "agent_id": "security-reviewer",
    "message": "Security review for PR #312:\n\nChanged files:\n- src/auth/oauth.ts\n- src/api/middleware/auth.ts\n- src/utils/crypto.ts\n\nCode changes:\n```typescript\n// src/utils/crypto.ts\nexport function hashPassword(password: string): string {\n  return crypto.createHash(\"md5\").update(password).digest(\"hex\");\n}\n\nexport function generateToken(): string {\n  return Math.random().toString(36).substring(2);\n}\n```\n\nCheck for:\n1. OWASP Top 10 vulnerabilities\n2. Insecure cryptographic practices\n3. Authentication/authorization flaws\n4. Input validation issues\n5. Secrets exposure\n\nProvide severity rating (Critical/High/Medium/Low) for each finding.",
    "source": "api",
    "respond_to": ["github-security-review", "slack-security-team"],
    "external_ref": "security-review-pr-312",
    "metadata": {
      "pr_number": 312,
      "files": ["src/auth/oauth.ts", "src/api/middleware/auth.ts", "src/utils/crypto.ts"]
    }
  }'
```

#### Cron: Weekly Dependency Scan

```bash
curl -X POST http://localhost:18791/api/cron \
  -H "Content-Type: application/json" \
  -d '{
    "id": "weekly-security-scan",
    "name": "Weekly Security Scan",
    "schedule": "0 0 6 * * 1",
    "agent_id": "security-scanner",
    "message": "Run weekly security assessment:\n1. Check npm audit for vulnerabilities\n2. Review outdated dependencies\n3. Scan for hardcoded secrets\n4. Check for deprecated API usage\n5. Generate security posture report",
    "respond_to": ["email-security-team", "jira-security-backlog"],
    "timezone": "UTC"
  }'
```

---

### 6. Documentation Generation from Code

Generate and update documentation based on code changes.

#### Trigger: PR Merged to Main

```bash
curl -X POST http://localhost:18791/api/gateway/submit \
  -H "Content-Type: application/json" \
  -d '{
    "agent_id": "doc-generator",
    "message": "Generate documentation for new API endpoints:\n\nFile: src/api/routes/products.ts\n\n```typescript\n/**\n * Product API endpoints\n */\nimport { Router } from \"express\";\nimport { ProductService } from \"../services/product\";\n\nconst router = Router();\n\n// GET /api/products - List all products\nrouter.get(\"/\", async (req, res) => {\n  const { page = 1, limit = 20, category } = req.query;\n  const products = await ProductService.list({ page, limit, category });\n  res.json(products);\n});\n\n// GET /api/products/:id - Get product by ID\nrouter.get(\"/:id\", async (req, res) => {\n  const product = await ProductService.getById(req.params.id);\n  if (!product) return res.status(404).json({ error: \"Not found\" });\n  res.json(product);\n});\n\n// POST /api/products - Create new product (admin only)\nrouter.post(\"/\", requireAdmin, async (req, res) => {\n  const { name, price, category, description } = req.body;\n  const product = await ProductService.create({ name, price, category, description });\n  res.status(201).json(product);\n});\n```\n\nGenerate:\n1. OpenAPI/Swagger specification\n2. Markdown API reference\n3. Example curl commands\n4. TypeScript client types",
    "source": "api",
    "respond_to": ["github-docs-pr"],
    "external_ref": "docs-products-api",
    "metadata": {
      "source_file": "src/api/routes/products.ts"
    }
  }'
```

#### Cron: Weekly Documentation Audit

```bash
curl -X POST http://localhost:18791/api/cron \
  -H "Content-Type: application/json" \
  -d '{
    "id": "weekly-docs-audit",
    "name": "Weekly Documentation Audit",
    "schedule": "0 0 10 * * 5",
    "agent_id": "docs-auditor",
    "message": "Audit documentation freshness:\n1. Find undocumented public functions\n2. Identify stale documentation (code changed, docs not)\n3. Check for broken internal links\n4. Verify code examples still work\n5. Suggest documentation improvements",
    "respond_to": ["slack-docs-team"],
    "timezone": "America/New_York"
  }'
```

---

### 7. Test Case Generation

Generate test cases for new code or untested code paths.

#### Trigger: New Code Without Tests

```bash
curl -X POST http://localhost:18791/api/gateway/submit \
  -H "Content-Type: application/json" \
  -d '{
    "agent_id": "test-generator",
    "message": "Generate test cases for this new utility:\n\nFile: src/utils/validation.ts\n\n```typescript\nexport interface ValidationResult {\n  valid: boolean;\n  errors: string[];\n}\n\nexport function validateEmail(email: string): ValidationResult {\n  const errors: string[] = [];\n  \n  if (!email) {\n    errors.push(\"Email is required\");\n  } else if (!email.includes(\"@\")) {\n    errors.push(\"Email must contain @\");\n  } else if (email.length > 254) {\n    errors.push(\"Email too long\");\n  } else if (!/^[^\\s@]+@[^\\s@]+\\.[^\\s@]+$/.test(email)) {\n    errors.push(\"Invalid email format\");\n  }\n  \n  return { valid: errors.length === 0, errors };\n}\n\nexport function validatePassword(password: string): ValidationResult {\n  const errors: string[] = [];\n  \n  if (password.length < 8) errors.push(\"Password must be at least 8 characters\");\n  if (!/[A-Z]/.test(password)) errors.push(\"Password must contain uppercase\");\n  if (!/[a-z]/.test(password)) errors.push(\"Password must contain lowercase\");\n  if (!/[0-9]/.test(password)) errors.push(\"Password must contain number\");\n  \n  return { valid: errors.length === 0, errors };\n}\n```\n\nGenerate:\n1. Jest/Vitest test file\n2. Cover all branches\n3. Include edge cases\n4. Test error messages\n5. Add property-based tests if applicable",
    "source": "api",
    "respond_to": ["github-test-pr"],
    "external_ref": "tests-validation-utils",
    "metadata": {
      "source_file": "src/utils/validation.ts",
      "test_framework": "vitest"
    }
  }'
```

#### Cron: Coverage Gap Analysis

```bash
curl -X POST http://localhost:18791/api/cron \
  -H "Content-Type: application/json" \
  -d '{
    "id": "coverage-gap-analysis",
    "name": "Weekly Coverage Gap Analysis",
    "schedule": "0 0 9 * * 1",
    "agent_id": "coverage-analyzer",
    "message": "Analyze test coverage gaps:\n1. Identify files with < 80% coverage\n2. Find untested critical paths\n3. Suggest high-value test additions\n4. Prioritize by risk and complexity\n5. Generate ticket descriptions for test tasks",
    "respond_to": ["jira-test-backlog", "slack-qa-team"],
    "timezone": "UTC"
  }'
```

---

## Integration Patterns

### Pattern 1: Webhook Relay

External services send webhooks to your relay service, which enriches and forwards to AgentZero.

```
GitHub Webhook ──► Your Relay Service ──► AgentZero Submit
                      │
                      ├─ Fetch full diff
                      ├─ Get related issues
                      └─ Add org context
```

**Relay Service Example (Node.js/Express):**

```javascript
app.post('/webhooks/github', async (req, res) => {
  const { action, pull_request, repository } = req.body;

  if (action !== 'opened' && action !== 'synchronize') {
    return res.sendStatus(200);
  }

  // Enrich with additional context
  const diff = await github.getPullRequestDiff(pull_request.url);
  const files = await github.getPullRequestFiles(pull_request.url);

  // Submit to AgentZero
  const response = await fetch('http://localhost:18791/api/gateway/submit', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      agent_id: 'code-reviewer',
      message: `Review PR #${pull_request.number}: ${pull_request.title}\n\n${diff}`,
      source: 'api',
      respond_to: ['github-pr-commenter'],
      thread_id: `pr-${pull_request.number}`,
      external_ref: `${repository.full_name}#${pull_request.number}`,
      metadata: { files, author: pull_request.user.login }
    })
  });

  res.json(await response.json());
});
```

### Pattern 2: Scheduled Polling

Use cron jobs to periodically check for work and trigger agents.

```bash
# Poll for unreviewed PRs every hour
curl -X POST http://localhost:18791/api/cron \
  -H "Content-Type: application/json" \
  -d '{
    "id": "poll-unreviewed-prs",
    "name": "Poll Unreviewed PRs",
    "schedule": "0 0 * * * *",
    "agent_id": "pr-poller",
    "message": "Check GitHub for PRs without reviews in the last 4 hours. For each, trigger a review.",
    "respond_to": ["pr-review-dispatcher"]
  }'
```

### Pattern 3: Response Chaining

One agent's response triggers another agent via connector.

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  PR Reviewer    │────▶│  Response       │────▶│  Issue Creator  │
│  Agent          │     │  Router         │     │  Agent          │
└─────────────────┘     └─────────────────┘     └─────────────────┘
```

**Response Router Connector Example:**

```javascript
app.post('/connector/response-router', async (req, res) => {
  const { payload, context } = req.body;

  // Check if review found critical issues
  if (payload.message.includes('CRITICAL')) {
    await fetch('http://localhost:18791/api/gateway/submit', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        agent_id: 'issue-creator',
        message: `Create blocking issue from review:\n\n${payload.message}`,
        source: 'api',
        respond_to: ['github-issue-creator'],
        thread_id: context.thread_id
      })
    });
  }

  res.sendStatus(200);
});
```

### Pattern 4: Async Polling

For long-running tasks, submit and poll for completion.

```bash
# Submit task
SESSION=$(curl -s -X POST http://localhost:18791/api/gateway/submit \
  -H "Content-Type: application/json" \
  -d '{
    "agent_id": "codebase-analyzer",
    "message": "Analyze entire codebase for technical debt"
  }' | jq -r '.session_id')

# Poll for status
while true; do
  STATUS=$(curl -s "http://localhost:18791/api/gateway/status/$SESSION" | jq -r '.status')
  echo "Status: $STATUS"

  if [ "$STATUS" = "completed" ]; then
    echo "Done!"
    break
  fi

  sleep 5
done
```

---

## Connector Configurations

### GitHub PR Commenter

Posts agent responses as PR comments.

```bash
curl -X POST http://localhost:18791/api/connectors \
  -H "Content-Type: application/json" \
  -d '{
    "id": "github-pr-commenter",
    "name": "GitHub PR Commenter",
    "transport": {
      "type": "http",
      "callback_url": "https://your-service.com/connectors/github-pr-comment",
      "method": "POST",
      "headers": {
        "Authorization": "Bearer ${GITHUB_TOKEN}",
        "X-Connector-Secret": "${CONNECTOR_SECRET}"
      },
      "timeout_ms": 10000
    },
    "metadata": {
      "capabilities": [
        {
          "name": "post_comment",
          "description": "Post a comment on a GitHub PR"
        }
      ]
    }
  }'
```

### Slack Notifier

Sends messages to Slack channels.

```bash
curl -X POST http://localhost:18791/api/connectors \
  -H "Content-Type: application/json" \
  -d '{
    "id": "slack-dev-channel",
    "name": "Slack Dev Channel",
    "transport": {
      "type": "http",
      "callback_url": "https://your-service.com/connectors/slack",
      "method": "POST",
      "headers": {
        "Authorization": "Bearer ${SLACK_TOKEN}"
      }
    },
    "metadata": {
      "channel": "#dev-notifications",
      "capabilities": [
        {
          "name": "send_message",
          "description": "Send a message to Slack channel"
        }
      ]
    }
  }'
```

### Jira Issue Creator

Creates issues in Jira from agent responses.

```bash
curl -X POST http://localhost:18791/api/connectors \
  -H "Content-Type: application/json" \
  -d '{
    "id": "jira-backlog",
    "name": "Jira Backlog Creator",
    "transport": {
      "type": "http",
      "callback_url": "https://your-service.com/connectors/jira",
      "method": "POST",
      "headers": {
        "Authorization": "Basic ${JIRA_AUTH}"
      }
    },
    "metadata": {
      "project": "DEV",
      "issue_type": "Task",
      "capabilities": [
        {
          "name": "create_issue",
          "description": "Create a Jira issue"
        }
      ]
    }
  }'
```

### CLI Connector (Local Script)

Execute local scripts to process responses.

```bash
curl -X POST http://localhost:18791/api/connectors \
  -H "Content-Type: application/json" \
  -d '{
    "id": "local-notifier",
    "name": "Local Desktop Notifier",
    "transport": {
      "type": "cli",
      "command": "/usr/local/bin/notify-agent-response",
      "args": ["--format", "json"],
      "env": {
        "NOTIFICATION_SOUND": "true"
      }
    }
  }'
```

### Email Bridge

Send responses via email.

```bash
curl -X POST http://localhost:18791/api/connectors \
  -H "Content-Type: application/json" \
  -d '{
    "id": "email-tech-leads",
    "name": "Email Tech Leads",
    "transport": {
      "type": "http",
      "callback_url": "https://your-service.com/connectors/email",
      "method": "POST",
      "headers": {
        "Authorization": "Bearer ${EMAIL_API_KEY}"
      }
    },
    "metadata": {
      "recipients": ["tech-leads@company.com"],
      "subject_prefix": "[AgentZero]",
      "capabilities": [
        {
          "name": "send_email",
          "description": "Send email to tech leads"
        }
      ]
    }
  }'
```

---

## Quick Reference

### Submit API

```bash
POST http://localhost:18791/api/gateway/submit
```

### Status API

```bash
GET http://localhost:18791/api/gateway/status/:session_id
```

### Cancel API

```bash
POST http://localhost:18791/api/gateway/cancel/:session_id
```

### Cron Schedule Format

6-field format: `sec min hour day month weekday`

| Expression | Description |
|------------|-------------|
| `0 0 9 * * *` | Daily at 9:00 AM |
| `0 0 9 * * 1-5` | Weekdays at 9:00 AM |
| `0 */30 * * * *` | Every 30 minutes |
| `0 0 */4 * * *` | Every 4 hours |
| `0 0 6 * * 1` | Mondays at 6:00 AM |

### Response Routing

| `respond_to` Value | Behavior |
|-------------------|----------|
| `[]` or `null` | Response to web UI only |
| `["connector-id"]` | Dispatch to specified connector |
| `["a", "b"]` | Dispatch to multiple connectors |

---

## Next Steps

1. **Set up your first connector** - Start with Slack for visibility
2. **Create a code review agent** - Use the examples above as templates
3. **Configure webhooks** - Connect GitHub/GitLab to your relay service
4. **Add scheduled jobs** - Start with daily reports
5. **Monitor and iterate** - Check session logs and refine prompts
