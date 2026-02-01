# Test Automation Plan

> **Purpose**: Establish a comprehensive regression test suite for AgentZero
> **Status**: Planning Phase
> **Last Updated**: 2026-02-01

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Test Architecture](#test-architecture)
3. [Test Categories](#test-categories)
4. [Technology Stack](#technology-stack)
5. [Implementation Phases](#implementation-phases)
6. [Test Execution Strategy](#test-execution-strategy)
7. [CI/CD Integration](#cicd-integration)

---

## Executive Summary

This plan establishes a test automation framework for AgentZero covering:
- **Backend**: Rust unit tests, API integration tests
- **Frontend**: React component tests, UI integration tests
- **E2E**: Full-stack scenarios including long-running agent executions

### Key Metrics Goals
- Unit test coverage: 70%+
- API endpoint coverage: 100%
- Critical user flows: 100%
- Regression suite execution: < 15 minutes (excluding long-running tests)

---

## Test Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           TEST PYRAMID                                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│                         ▲                                                    │
│                        /█\                                                   │
│                       / █ \      E2E Tests (Playwright)                      │
│                      /  █  \     - Full agent workflows                      │
│                     /   █   \    - Multi-turn conversations                  │
│                    /────█────\   - Subagent delegation                       │
│                   /     █     \                                              │
│                  /      █      \  Integration Tests                          │
│                 /       █       \ - API endpoint tests                       │
│                /        █        \- WebSocket tests                          │
│               /─────────█─────────\- Database tests                          │
│              /          █          \                                         │
│             /           █           \ Unit Tests                             │
│            /            █            \- Rust: cargo test                     │
│           /             █             \- TS: Vitest                          │
│          /──────────────█──────────────\                                     │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Test Locations

```
agentzero/
├── services/
│   └── execution-state/
│       └── src/
│           └── *.rs              # Inline unit tests (#[cfg(test)])
│       └── tests/
│           └── integration.rs    # Integration tests
├── gateway/
│   └── src/
│       └── *.rs                  # Inline unit tests
│   └── tests/
│       └── api_tests.rs          # API integration tests
├── framework/
│   └── zero-*/
│       └── src/
│           └── *.rs              # Inline unit tests
├── apps/ui/
│   └── src/
│       └── **/*.test.ts          # Component unit tests
│       └── **/*.test.tsx         # React component tests
│   └── tests/
│       └── integration/          # Frontend integration tests
│       └── e2e/                  # Playwright E2E tests
└── memory-bank/test-cases/
    └── *.md                      # Test case documentation
```

---

## Test Categories

### 1. Unit Tests (Fast, Isolated)
**Execution Time**: < 1 second per test
**Run Frequency**: On every commit

| Category | Technology | Location |
|----------|------------|----------|
| Rust Core Logic | `cargo test` | `**/src/*.rs` with `#[cfg(test)]` |
| TypeScript Utils | Vitest | `apps/ui/src/**/*.test.ts` |
| React Components | Vitest + RTL | `apps/ui/src/**/*.test.tsx` |

### 2. Integration Tests (Medium Speed)
**Execution Time**: 1-30 seconds per test
**Run Frequency**: On PR, pre-merge

| Category | Technology | Location |
|----------|------------|----------|
| API Endpoints | Rust + reqwest | `gateway/tests/api_tests.rs` |
| Database Ops | Rust + SQLite in-memory | `services/*/tests/` |
| WebSocket | Rust + tokio-tungstenite | `gateway/tests/ws_tests.rs` |
| Frontend Integration | Vitest + MSW | `apps/ui/tests/integration/` |

### 3. E2E Tests (Slow, Full Stack)
**Execution Time**: 30 seconds - 5 minutes per test
**Run Frequency**: Nightly, pre-release

| Category | Technology | Location |
|----------|------------|----------|
| User Flows | Playwright | `apps/ui/tests/e2e/` |
| Agent Workflows | Playwright | `apps/ui/tests/e2e/agents/` |
| Long-Running Sessions | Custom harness | `tests/long-running/` |

---

## Technology Stack

### Backend (Rust)

```toml
# Cargo.toml dev-dependencies
[dev-dependencies]
tokio-test = "0.4"
tempfile = "3"
mockall = "0.12"        # Mocking framework
wiremock = "0.6"        # HTTP mocking
assert_matches = "1.5"  # Better assertions
serial_test = "3"       # Serial test execution
criterion = "0.5"       # Benchmarking
```

### Frontend (TypeScript/React)

```json
// package.json devDependencies
{
  "devDependencies": {
    "vitest": "^2.0.0",
    "@testing-library/react": "^16.0.0",
    "@testing-library/user-event": "^14.5.0",
    "@testing-library/jest-dom": "^6.5.0",
    "msw": "^2.4.0",
    "jsdom": "^25.0.0",
    "@playwright/test": "^1.48.0",
    "@types/jest": "^29.5.0"
  }
}
```

### E2E (Playwright)

```typescript
// playwright.config.ts
export default defineConfig({
  testDir: './tests/e2e',
  timeout: 60_000,
  retries: 2,
  use: {
    baseURL: 'http://localhost:5173',
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
  },
  webServer: {
    command: 'npm run dev',
    url: 'http://localhost:5173',
    reuseExistingServer: !process.env.CI,
  },
});
```

---

## Implementation Phases

### Phase 1: Foundation (Week 1-2)
**Goal**: Set up test infrastructure

- [ ] Install test dependencies (Rust + Frontend)
- [ ] Configure Vitest for frontend
- [ ] Configure Playwright for E2E
- [ ] Add test scripts to package.json
- [ ] Create test utility modules
- [ ] Set up CI test job

### Phase 2: Unit Tests (Week 3-4)
**Goal**: Cover core business logic

- [ ] execution-state service tests
- [ ] Gateway bus tests
- [ ] Type conversion tests
- [ ] Frontend utility function tests
- [ ] React component tests (isolated)

### Phase 3: Integration Tests (Week 5-6)
**Goal**: Test component interactions

- [ ] API endpoint tests
- [ ] WebSocket message tests
- [ ] Database operation tests
- [ ] Frontend API integration tests

### Phase 4: E2E Tests (Week 7-8)
**Goal**: Full user flow coverage

- [ ] Basic navigation tests
- [ ] Agent invocation tests
- [ ] Dashboard functionality tests
- [ ] Long-running workflow tests

---

## Test Execution Strategy

### Fast Suite (< 5 min)
```bash
# Run on every commit
cargo test --workspace --lib
cd apps/ui && npm test
```

### Full Suite (< 15 min)
```bash
# Run on PR
cargo test --workspace
cd apps/ui && npm test && npm run test:integration
```

### Complete Suite (< 60 min)
```bash
# Run nightly
cargo test --workspace
cd apps/ui && npm test && npm run test:integration && npm run test:e2e
```

### Long-Running Suite (1+ hours)
```bash
# Run weekly or pre-release
npm run test:e2e:long-running
```

---

## CI/CD Integration

### GitHub Actions Workflow

```yaml
# .github/workflows/test.yaml
name: Tests

on:
  push:
    branches: [main, v*]
  pull_request:
    branches: [main]

jobs:
  unit-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Rust tests
        run: cargo test --workspace --lib
      - name: Frontend tests
        run: |
          cd apps/ui
          npm ci
          npm test

  integration-tests:
    runs-on: ubuntu-latest
    needs: unit-tests
    steps:
      - uses: actions/checkout@v4
      - name: API tests
        run: cargo test --workspace
      - name: Frontend integration
        run: |
          cd apps/ui
          npm ci
          npm run test:integration

  e2e-tests:
    runs-on: ubuntu-latest
    needs: integration-tests
    steps:
      - uses: actions/checkout@v4
      - name: Install Playwright
        run: npx playwright install --with-deps
      - name: Run E2E tests
        run: |
          cd apps/ui
          npm ci
          npm run test:e2e
```

---

## Test Data Management

### Test Fixtures

```
memory-bank/test-cases/fixtures/
├── agents/
│   ├── root-agent.json
│   └── research-agent.json
├── sessions/
│   ├── single-turn.json
│   └── multi-turn.json
├── providers/
│   └── mock-provider.json
└── messages/
    ├── simple-query.json
    └── research-query.json
```

### Mock Services

| Service | Mock Strategy |
|---------|---------------|
| LLM Provider | Mock responses with canned data |
| MCP Servers | In-process mock servers |
| External APIs | MSW for frontend, wiremock for backend |

---

## Related Documents

- [Backend API Tests](./backend-api-tests.md)
- [Frontend UI Tests](./frontend-ui-tests.md)
- [E2E Scenarios](./e2e-scenarios.md)
- [Unit Test Coverage Plan](./unit-test-coverage.md)
