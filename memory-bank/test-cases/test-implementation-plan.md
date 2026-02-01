# Test Implementation Plan

> **Reference:** `memory-bank/test-cases/` contains detailed test specifications.
> **Self-Correction:** Each part includes verification commands. If tests fail, fix and re-run before proceeding.

## How to Use This Plan

Each part is self-contained. Copy-paste the prompt for the part you want to execute.

**Critical Rule:** After each part, run the verification command. If it fails:
1. Read the error output
2. Fix the issue
3. Re-run verification
4. Only proceed when verification passes

---

## Quick Start Prompts

### Phase 1: Infrastructure Setup

**Part 1A - Backend Test Infrastructure:**
```
Read memory-bank/test-cases/test-implementation-plan.md and execute Part 1A: Backend Test Infrastructure.
Reference specs: memory-bank/test-cases/unit-test-coverage.md, memory-bank/test-cases/backend-api-tests.md
```

**Part 1B - Frontend Test Infrastructure:**
```
Read memory-bank/test-cases/test-implementation-plan.md and execute Part 1B: Frontend Test Infrastructure.
Reference specs: memory-bank/test-cases/unit-test-coverage.md, memory-bank/test-cases/frontend-ui-tests.md
```

**Part 1C - E2E Test Infrastructure:**
```
Read memory-bank/test-cases/test-implementation-plan.md and execute Part 1C: E2E Test Infrastructure.
Reference specs: memory-bank/test-cases/e2e-scenarios.md
```

### Phase 2: Unit Tests

**Part 2A - execution-state Unit Tests:**
```
Read memory-bank/test-cases/test-implementation-plan.md and execute Part 2A: execution-state Unit Tests.
Reference: memory-bank/test-cases/unit-test-coverage.md (execution-state section)
```

**Part 2B - Gateway Bus Unit Tests:**
```
Read memory-bank/test-cases/test-implementation-plan.md and execute Part 2B: Gateway Bus Unit Tests.
Reference: memory-bank/test-cases/unit-test-coverage.md (gateway section)
```

**Part 2C - Frontend Unit Tests:**
```
Read memory-bank/test-cases/test-implementation-plan.md and execute Part 2C: Frontend Unit Tests.
Reference: memory-bank/test-cases/unit-test-coverage.md (frontend section)
```

### Phase 3: Integration Tests

**Part 3A - Backend API Integration Tests:**
```
Read memory-bank/test-cases/test-implementation-plan.md and execute Part 3A: Backend API Integration Tests.
Reference: memory-bank/test-cases/backend-api-tests.md
```

**Part 3B - Frontend Integration Tests:**
```
Read memory-bank/test-cases/test-implementation-plan.md and execute Part 3B: Frontend Integration Tests.
Reference: memory-bank/test-cases/frontend-ui-tests.md
```

### Phase 4: E2E Tests

**Part 4A - E2E Quick Scenarios:**
```
Read memory-bank/test-cases/test-implementation-plan.md and execute Part 4A: E2E Quick Scenarios.
Reference: memory-bank/test-cases/e2e-scenarios.md (Quick Scenarios section)
```

**Part 4B - E2E Long-Running Scenarios:**
```
Read memory-bank/test-cases/test-implementation-plan.md and execute Part 4B: E2E Long-Running Scenarios.
Reference: memory-bank/test-cases/e2e-scenarios.md (Long-Running section)
```

### Phase 5: CI Integration

**Part 5A - GitHub Actions Setup:**
```
Read memory-bank/test-cases/test-implementation-plan.md and execute Part 5A: GitHub Actions Setup.
Reference: memory-bank/test-cases/test-automation-plan.md (CI/CD section)
```

---

## PHASE 1: Infrastructure Setup

### Part 1A: Backend Test Infrastructure

**Scope:** Set up Rust test dependencies and utilities

**Files to Create/Modify:**
- `services/execution-state/Cargo.toml` (dev-dependencies)
- `gateway/Cargo.toml` (dev-dependencies)
- `services/execution-state/src/test_utils.rs` (new)
- `gateway/src/test_utils.rs` (new)

**Changes:**

1. Add dev-dependencies to `services/execution-state/Cargo.toml`:
   ```toml
   [dev-dependencies]
   tokio-test = "0.4"
   tempfile = "3"
   assert_matches = "1.5"
   ```

2. Add dev-dependencies to `gateway/Cargo.toml`:
   ```toml
   [dev-dependencies]
   tokio-test = "0.4"
   tempfile = "3"
   assert_matches = "1.5"
   tower = { version = "0.4", features = ["util"] }
   axum-test = "15"
   ```

3. Create `services/execution-state/src/test_utils.rs`:
   ```rust
   //! Test utilities for execution-state tests.
   
   use crate::types::*;
   use tempfile::TempDir;
   
   /// Create a temporary database for testing.
   pub fn temp_db() -> (TempDir, String) {
       let dir = TempDir::new().unwrap();
       let path = dir.path().join("test.db");
       (dir, path.to_string_lossy().to_string())
   }
   
   /// Create a mock session for testing.
   pub fn mock_session(id: &str) -> Session {
       Session {
           id: id.to_string(),
           status: SessionStatus::Running,
           source: TriggerSource::Web,
           created_at: chrono::Utc::now(),
           updated_at: chrono::Utc::now(),
           completed_at: None,
           error_message: None,
       }
   }
   
   /// Create a mock execution for testing.
   pub fn mock_execution(id: &str, session_id: &str) -> AgentExecution {
       AgentExecution {
           id: id.to_string(),
           session_id: session_id.to_string(),
           agent_id: "root".to_string(),
           parent_execution_id: None,
           conversation_id: format!("conv-{}", id),
           status: ExecutionStatus::Running,
           turn_count: 0,
           started_at: chrono::Utc::now(),
           completed_at: None,
           error_message: None,
       }
   }
   ```

4. Add module to `services/execution-state/src/lib.rs`:
   ```rust
   #[cfg(test)]
   pub mod test_utils;
   ```

**Verification:**
```bash
cd services/execution-state && cargo check --tests
cd gateway && cargo check --tests
```

**Self-Correction:** If cargo check fails:
1. Read the compiler error
2. Fix missing imports or type mismatches
3. Re-run cargo check --tests
4. Repeat until it passes

**Done when:** Both crates compile with test configuration

---

### Part 1B: Frontend Test Infrastructure

**Scope:** Set up Vitest, Testing Library, and MSW

**Files to Create/Modify:**
- `apps/ui/package.json` (add devDependencies)
- `apps/ui/vitest.config.ts` (new)
- `apps/ui/src/test/setup.ts` (new)
- `apps/ui/src/test/mocks/handlers.ts` (new)
- `apps/ui/src/test/mocks/server.ts` (new)
- `apps/ui/src/test/utils.tsx` (new)

**Changes:**

1. Add to `apps/ui/package.json` devDependencies:
   ```json
   {
     "devDependencies": {
       "vitest": "^2.0.0",
       "@testing-library/react": "^16.0.0",
       "@testing-library/user-event": "^14.5.0",
       "@testing-library/jest-dom": "^6.5.0",
       "@testing-library/dom": "^10.0.0",
       "msw": "^2.4.0",
       "jsdom": "^25.0.0",
       "@vitest/coverage-v8": "^2.0.0"
     }
   }
   ```

2. Add scripts to `apps/ui/package.json`:
   ```json
   {
     "scripts": {
       "test": "vitest run",
       "test:watch": "vitest",
       "test:coverage": "vitest run --coverage",
       "test:ui": "vitest --ui"
     }
   }
   ```

3. Create `apps/ui/vitest.config.ts`:
   ```typescript
   import { defineConfig } from 'vitest/config';
   import react from '@vitejs/plugin-react';
   import path from 'path';

   export default defineConfig({
     plugins: [react()],
     test: {
       globals: true,
       environment: 'jsdom',
       setupFiles: ['./src/test/setup.ts'],
       include: ['src/**/*.{test,spec}.{ts,tsx}'],
       coverage: {
         provider: 'v8',
         reporter: ['text', 'html', 'lcov'],
         exclude: [
           'node_modules/',
           'src/test/',
           '**/*.d.ts',
           '**/*.config.*',
         ],
       },
     },
     resolve: {
       alias: {
         '@': path.resolve(__dirname, './src'),
       },
     },
   });
   ```

4. Create `apps/ui/src/test/setup.ts`:
   ```typescript
   import '@testing-library/jest-dom';
   import { cleanup } from '@testing-library/react';
   import { afterEach, beforeAll, afterAll } from 'vitest';
   import { server } from './mocks/server';

   // Start MSW server before all tests
   beforeAll(() => server.listen({ onUnhandledRequest: 'error' }));

   // Reset handlers after each test
   afterEach(() => {
     cleanup();
     server.resetHandlers();
   });

   // Close server after all tests
   afterAll(() => server.close());
   ```

5. Create `apps/ui/src/test/mocks/handlers.ts`:
   ```typescript
   import { http, HttpResponse } from 'msw';

   const API_BASE = 'http://localhost:18791';

   export const handlers = [
     // Dashboard stats
     http.get(`${API_BASE}/api/executions/stats/counts`, () => {
       return HttpResponse.json({
         sessions_running: 2,
         sessions_queued: 1,
         sessions_completed: 10,
         sessions_crashed: 0,
         sessions_cancelled: 0,
         executions_running: 3,
         executions_queued: 0,
         executions_completed: 15,
         executions_crashed: 1,
         sessions_by_source: { web: 8, cli: 3, api: 2 },
       });
     }),

     // Sessions list
     http.get(`${API_BASE}/api/executions/v2/sessions/full`, () => {
       return HttpResponse.json([
         {
           session: {
             id: 'sess-001',
             status: 'running',
             source: 'web',
             created_at: new Date().toISOString(),
             updated_at: new Date().toISOString(),
           },
           executions: [
             {
               id: 'exec-001',
               session_id: 'sess-001',
               agent_id: 'root',
               status: 'running',
               conversation_id: 'conv-001',
               turn_count: 5,
               started_at: new Date().toISOString(),
             },
           ],
           subagent_count: 0,
         },
       ]);
     }),

     // Gateway submit
     http.post(`${API_BASE}/api/gateway/submit`, async ({ request }) => {
       const body = await request.json() as Record<string, unknown>;
       return HttpResponse.json({
         session_id: 'sess-new',
         execution_id: 'exec-new',
         conversation_id: body.conversation_id || 'conv-new',
       });
     }),
   ];
   ```

6. Create `apps/ui/src/test/mocks/server.ts`:
   ```typescript
   import { setupServer } from 'msw/node';
   import { handlers } from './handlers';

   export const server = setupServer(...handlers);
   ```

7. Create `apps/ui/src/test/utils.tsx`:
   ```typescript
   import { ReactElement } from 'react';
   import { render, RenderOptions } from '@testing-library/react';
   import userEvent from '@testing-library/user-event';

   // Add providers here as needed (QueryClient, Theme, etc.)
   function AllProviders({ children }: { children: React.ReactNode }) {
     return <>{children}</>;
   }

   function customRender(
     ui: ReactElement,
     options?: Omit<RenderOptions, 'wrapper'>
   ) {
     return {
       user: userEvent.setup(),
       ...render(ui, { wrapper: AllProviders, ...options }),
     };
   }

   export * from '@testing-library/react';
   export { customRender as render };
   ```

**Verification:**
```bash
cd apps/ui && npm install && npm run test -- --run --passWithNoTests
```

**Self-Correction:** If npm install fails:
1. Check package.json for syntax errors
2. Try removing node_modules and package-lock.json
3. Re-run npm install

If test setup fails:
1. Check vitest.config.ts paths
2. Verify setup.ts imports
3. Check MSW version compatibility

**Done when:** `npm run test` runs without errors (even with no tests)

---

### Part 1C: E2E Test Infrastructure

**Scope:** Set up Playwright for E2E testing

**Files to Create/Modify:**
- `apps/ui/package.json` (add playwright)
- `apps/ui/playwright.config.ts` (new)
- `apps/ui/tests/e2e/.gitkeep` (new)
- `apps/ui/tests/e2e/fixtures.ts` (new)

**Changes:**

1. Add to `apps/ui/package.json` devDependencies:
   ```json
   {
     "devDependencies": {
       "@playwright/test": "^1.48.0"
     }
   }
   ```

2. Add scripts:
   ```json
   {
     "scripts": {
       "test:e2e": "playwright test",
       "test:e2e:ui": "playwright test --ui",
       "test:e2e:debug": "playwright test --debug"
     }
   }
   ```

3. Create `apps/ui/playwright.config.ts`:
   ```typescript
   import { defineConfig, devices } from '@playwright/test';

   export default defineConfig({
     testDir: './tests/e2e',
     fullyParallel: true,
     forbidOnly: !!process.env.CI,
     retries: process.env.CI ? 2 : 0,
     workers: process.env.CI ? 1 : undefined,
     reporter: [
       ['html', { outputFolder: 'playwright-report' }],
       ['list'],
     ],
     use: {
       baseURL: 'http://localhost:5173',
       trace: 'on-first-retry',
       screenshot: 'only-on-failure',
     },
     projects: [
       {
         name: 'chromium',
         use: { ...devices['Desktop Chrome'] },
       },
     ],
     webServer: {
       command: 'npm run dev',
       url: 'http://localhost:5173',
       reuseExistingServer: !process.env.CI,
       timeout: 120_000,
     },
   });
   ```

4. Create `apps/ui/tests/e2e/fixtures.ts`:
   ```typescript
   import { test as base, expect } from '@playwright/test';

   // Extend base test with custom fixtures
   export const test = base.extend<{
     dashboardPage: DashboardPage;
   }>({
     dashboardPage: async ({ page }, use) => {
       const dashboard = new DashboardPage(page);
       await use(dashboard);
     },
   });

   export { expect };

   // Page Object: Dashboard
   class DashboardPage {
     constructor(private page: import('@playwright/test').Page) {}

     async goto() {
       await this.page.goto('/ops');
     }

     async waitForLoad() {
       await this.page.waitForSelector('[data-testid="dashboard"]', {
         state: 'visible',
         timeout: 10_000,
       });
     }

     async getSessionCount() {
       const text = await this.page.textContent('[data-testid="session-count"]');
       return parseInt(text || '0', 10);
     }

     async filterBySource(source: string) {
       await this.page.click('[data-testid="source-filter"]');
       await this.page.click(`[data-testid="source-option-${source}"]`);
     }

     async getVisibleSessions() {
       return this.page.locator('[data-testid="session-card"]').all();
     }
   }
   ```

5. Create `apps/ui/tests/e2e/.gitkeep` (empty file)

**Verification:**
```bash
cd apps/ui && npm install && npx playwright install chromium
npx playwright test --list
```

**Self-Correction:** If playwright install fails:
1. Check system dependencies: `npx playwright install-deps`
2. Try installing specific browser: `npx playwright install chromium`

**Done when:** `npx playwright test --list` shows no errors

---

## PHASE 2: Unit Tests

### Part 2A: execution-state Unit Tests

**Scope:** Unit tests for types, repository, and service in execution-state

**Files to Create/Modify:**
- `services/execution-state/src/types.rs` (add tests module)
- `services/execution-state/src/repository.rs` (add tests module)

**Changes:**

1. Add tests to `services/execution-state/src/types.rs`:
   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;

       #[test]
       fn session_status_display() {
           assert_eq!(format!("{}", SessionStatus::Queued), "queued");
           assert_eq!(format!("{}", SessionStatus::Running), "running");
           assert_eq!(format!("{}", SessionStatus::Completed), "completed");
           assert_eq!(format!("{}", SessionStatus::Crashed), "crashed");
           assert_eq!(format!("{}", SessionStatus::Cancelled), "cancelled");
       }

       #[test]
       fn execution_status_display() {
           assert_eq!(format!("{}", ExecutionStatus::Queued), "queued");
           assert_eq!(format!("{}", ExecutionStatus::Running), "running");
           assert_eq!(format!("{}", ExecutionStatus::Completed), "completed");
           assert_eq!(format!("{}", ExecutionStatus::Crashed), "crashed");
       }

       #[test]
       fn trigger_source_serialization() {
           let sources = [
               (TriggerSource::Web, "\"web\""),
               (TriggerSource::Cli, "\"cli\""),
               (TriggerSource::Cron, "\"cron\""),
               (TriggerSource::Api, "\"api\""),
               (TriggerSource::Plugin, "\"plugin\""),
           ];
           
           for (source, expected) in sources {
               let json = serde_json::to_string(&source).unwrap();
               assert_eq!(json, expected, "Serialization failed for {:?}", source);
               
               let parsed: TriggerSource = serde_json::from_str(&json).unwrap();
               assert_eq!(parsed, source, "Deserialization failed for {}", expected);
           }
       }

       #[test]
       fn trigger_source_default() {
           assert_eq!(TriggerSource::default(), TriggerSource::Web);
       }

       #[test]
       fn dashboard_stats_default() {
           let stats = DashboardStats::default();
           assert_eq!(stats.sessions_running, 0);
           assert_eq!(stats.sessions_queued, 0);
           assert_eq!(stats.sessions_completed, 0);
           assert_eq!(stats.sessions_crashed, 0);
           assert_eq!(stats.sessions_cancelled, 0);
           assert_eq!(stats.executions_running, 0);
           assert_eq!(stats.executions_queued, 0);
           assert_eq!(stats.executions_completed, 0);
           assert_eq!(stats.executions_crashed, 0);
           assert!(stats.sessions_by_source.is_empty());
       }

       #[test]
       fn session_with_executions_subagent_count() {
           let session = Session {
               id: "sess-1".to_string(),
               status: SessionStatus::Running,
               source: TriggerSource::Web,
               created_at: chrono::Utc::now(),
               updated_at: chrono::Utc::now(),
               completed_at: None,
               error_message: None,
           };
           
           let executions = vec![
               AgentExecution {
                   id: "exec-1".to_string(),
                   session_id: "sess-1".to_string(),
                   agent_id: "root".to_string(),
                   parent_execution_id: None,
                   conversation_id: "conv-1".to_string(),
                   status: ExecutionStatus::Completed,
                   turn_count: 3,
                   started_at: chrono::Utc::now(),
                   completed_at: Some(chrono::Utc::now()),
                   error_message: None,
               },
               AgentExecution {
                   id: "exec-2".to_string(),
                   session_id: "sess-1".to_string(),
                   agent_id: "researcher".to_string(),
                   parent_execution_id: Some("exec-1".to_string()),
                   conversation_id: "conv-2".to_string(),
                   status: ExecutionStatus::Running,
                   turn_count: 1,
                   started_at: chrono::Utc::now(),
                   completed_at: None,
                   error_message: None,
               },
           ];
           
           let swe = SessionWithExecutions {
               session,
               executions,
               subagent_count: 1, // One subagent (researcher)
           };
           
           assert_eq!(swe.subagent_count, 1);
           assert_eq!(swe.executions.len(), 2);
       }
   }
   ```

2. Add tests to `services/execution-state/src/repository.rs` (at end of file):
   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;
       use tempfile::TempDir;

       fn setup_test_db() -> (TempDir, Repository) {
           let dir = TempDir::new().unwrap();
           let db_path = dir.path().join("test.db");
           let manager = DatabaseManager::new(db_path.to_str().unwrap()).unwrap();
           let repo = Repository::new(manager);
           (dir, repo)
       }

       #[tokio::test]
       async fn create_session_success() {
           let (_dir, repo) = setup_test_db();
           
           let session = repo
               .create_session(TriggerSource::Web)
               .await
               .unwrap();
           
           assert!(session.id.starts_with("sess-"));
           assert_eq!(session.status, SessionStatus::Running);
           assert_eq!(session.source, TriggerSource::Web);
       }

       #[tokio::test]
       async fn create_session_queued() {
           let (_dir, repo) = setup_test_db();
           
           let session = repo
               .create_session_queued(TriggerSource::Cron)
               .await
               .unwrap();
           
           assert_eq!(session.status, SessionStatus::Queued);
           assert_eq!(session.source, TriggerSource::Cron);
       }

       #[tokio::test]
       async fn get_session_not_found() {
           let (_dir, repo) = setup_test_db();
           
           let result = repo.get_session("nonexistent").await;
           assert!(result.is_err() || result.unwrap().is_none());
       }

       #[tokio::test]
       async fn update_session_status() {
           let (_dir, repo) = setup_test_db();
           
           let session = repo.create_session(TriggerSource::Web).await.unwrap();
           
           repo.update_session_status(&session.id, SessionStatus::Completed)
               .await
               .unwrap();
           
           let updated = repo.get_session(&session.id).await.unwrap().unwrap();
           assert_eq!(updated.status, SessionStatus::Completed);
           assert!(updated.completed_at.is_some());
       }

       #[tokio::test]
       async fn create_execution_success() {
           let (_dir, repo) = setup_test_db();
           
           let session = repo.create_session(TriggerSource::Web).await.unwrap();
           
           let execution = repo
               .create_execution(&session.id, "root", None, "conv-1")
               .await
               .unwrap();
           
           assert!(execution.id.starts_with("exec-"));
           assert_eq!(execution.session_id, session.id);
           assert_eq!(execution.agent_id, "root");
           assert_eq!(execution.status, ExecutionStatus::Running);
       }

       #[tokio::test]
       async fn get_dashboard_stats_empty() {
           let (_dir, repo) = setup_test_db();
           
           let stats = repo.get_dashboard_stats().await.unwrap();
           
           assert_eq!(stats.sessions_running, 0);
           assert_eq!(stats.sessions_queued, 0);
           assert_eq!(stats.executions_running, 0);
       }

       #[tokio::test]
       async fn get_dashboard_stats_with_data() {
           let (_dir, repo) = setup_test_db();
           
           // Create sessions with different sources
           let s1 = repo.create_session(TriggerSource::Web).await.unwrap();
           let s2 = repo.create_session(TriggerSource::Web).await.unwrap();
           let s3 = repo.create_session(TriggerSource::Cli).await.unwrap();
           
           // Complete one session
           repo.update_session_status(&s2.id, SessionStatus::Completed)
               .await
               .unwrap();
           
           // Create executions
           repo.create_execution(&s1.id, "root", None, "conv-1")
               .await
               .unwrap();
           repo.create_execution(&s3.id, "root", None, "conv-2")
               .await
               .unwrap();
           
           let stats = repo.get_dashboard_stats().await.unwrap();
           
           assert_eq!(stats.sessions_running, 2); // s1, s3
           assert_eq!(stats.sessions_completed, 1); // s2
           assert_eq!(stats.executions_running, 2);
           assert_eq!(*stats.sessions_by_source.get("web").unwrap_or(&0), 2);
           assert_eq!(*stats.sessions_by_source.get("cli").unwrap_or(&0), 1);
       }

       #[tokio::test]
       async fn get_sessions_with_executions() {
           let (_dir, repo) = setup_test_db();
           
           let session = repo.create_session(TriggerSource::Web).await.unwrap();
           repo.create_execution(&session.id, "root", None, "conv-1")
               .await
               .unwrap();
           let exec2 = repo
               .create_execution(&session.id, "researcher", Some("exec-1".into()), "conv-2")
               .await
               .unwrap();
           
           let sessions = repo.get_sessions_with_executions(None, None).await.unwrap();
           
           assert_eq!(sessions.len(), 1);
           assert_eq!(sessions[0].executions.len(), 2);
           assert_eq!(sessions[0].subagent_count, 1); // researcher is a subagent
       }
   }
   ```

**Verification:**
```bash
cd services/execution-state && cargo test -- --nocapture
```

**Self-Correction:** If tests fail:
1. Read the assertion error message
2. Check if the implementation matches the test expectation
3. Fix either the test or the implementation
4. Re-run tests

**Done when:** All tests pass with `cargo test`

---

### Part 2B: Gateway Bus Unit Tests

**Scope:** Unit tests for gateway bus types and error handling

**Files to Modify:**
- `gateway/src/bus/types.rs` (add/expand tests)
- `gateway/src/bus/http_bus.rs` (add tests)

**Changes:**

1. Expand tests in `gateway/src/bus/types.rs`:
   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;
       use execution_state::TriggerSource;

       #[test]
       fn session_request_new() {
           let req = SessionRequest::new("root", "Hello!");
           
           assert_eq!(req.agent_id, "root");
           assert_eq!(req.message, "Hello!");
           assert_eq!(req.source, TriggerSource::Web);
           assert!(req.session_id.is_none());
           assert!(req.priority.is_none());
           assert!(req.external_ref.is_none());
           assert!(req.metadata.is_none());
       }

       #[test]
       fn session_request_builder() {
           let req = SessionRequest::new("agent", "msg")
               .with_source(TriggerSource::Plugin)
               .with_session_id("sess-123")
               .with_priority(10)
               .with_external_ref("ext-ref")
               .with_metadata(serde_json::json!({"key": "value"}));
           
           assert_eq!(req.source, TriggerSource::Plugin);
           assert_eq!(req.session_id, Some("sess-123".to_string()));
           assert_eq!(req.priority, Some(10));
           assert_eq!(req.external_ref, Some("ext-ref".to_string()));
           assert!(req.metadata.is_some());
       }

       #[test]
       fn session_request_continue_session() {
           let req = SessionRequest::continue_session("sess-existing", "Follow up message");
           
           assert_eq!(req.session_id, Some("sess-existing".to_string()));
           assert_eq!(req.message, "Follow up message");
           assert_eq!(req.agent_id, "root");
       }

       #[test]
       fn session_handle_fields() {
           let handle = SessionHandle {
               session_id: "sess-1".to_string(),
               execution_id: "exec-1".to_string(),
               conversation_id: "conv-1".to_string(),
           };
           
           assert_eq!(handle.session_id, "sess-1");
           assert_eq!(handle.execution_id, "exec-1");
           assert_eq!(handle.conversation_id, "conv-1");
       }

       #[test]
       fn bus_error_session_not_found() {
           let err = BusError::SessionNotFound("sess-123".to_string());
           let msg = err.to_string();
           
           assert!(msg.contains("sess-123"));
           assert!(msg.to_lowercase().contains("not found") || msg.to_lowercase().contains("session"));
       }

       #[test]
       fn bus_error_execution_not_found() {
           let err = BusError::ExecutionNotFound("exec-456".to_string());
           let msg = err.to_string();
           
           assert!(msg.contains("exec-456"));
       }

       #[test]
       fn bus_error_invalid_state() {
           let err = BusError::InvalidState {
               current: "completed".to_string(),
               action: "pause".to_string(),
           };
           let msg = err.to_string();
           
           assert!(msg.contains("completed") || msg.contains("pause"));
       }

       #[test]
       fn session_request_json_deserialization() {
           let json = r#"{
               "agent_id": "root",
               "message": "Hello!",
               "source": "plugin",
               "priority": 5,
               "external_ref": "test-ref"
           }"#;
           
           let req: SessionRequest = serde_json::from_str(json).unwrap();
           
           assert_eq!(req.agent_id, "root");
           assert_eq!(req.message, "Hello!");
           assert_eq!(req.source, TriggerSource::Plugin);
           assert_eq!(req.priority, Some(5));
           assert_eq!(req.external_ref, Some("test-ref".to_string()));
       }

       #[test]
       fn session_request_minimal_json() {
           let json = r#"{
               "agent_id": "root",
               "message": "Hi"
           }"#;
           
           let req: SessionRequest = serde_json::from_str(json).unwrap();
           
           assert_eq!(req.agent_id, "root");
           assert_eq!(req.source, TriggerSource::Web); // default
       }

       #[test]
       fn session_handle_serialization() {
           let handle = SessionHandle {
               session_id: "sess-1".to_string(),
               execution_id: "exec-1".to_string(),
               conversation_id: "conv-1".to_string(),
           };
           
           let json = serde_json::to_string(&handle).unwrap();
           
           assert!(json.contains("sess-1"));
           assert!(json.contains("exec-1"));
           assert!(json.contains("conv-1"));
       }
   }
   ```

**Verification:**
```bash
cd gateway && cargo test bus:: -- --nocapture
```

**Self-Correction:** If tests fail, check:
1. Builder method implementations
2. Serde derive attributes
3. Default trait implementations

**Done when:** All bus tests pass

---

### Part 2C: Frontend Unit Tests

**Scope:** Unit tests for transport types and utility functions

**Files to Create:**
- `apps/ui/src/services/transport/types.test.ts`
- `apps/ui/src/services/transport/http.test.ts`
- `apps/ui/src/features/ops/components/SourceBadge.test.tsx`

**Changes:**

1. Create `apps/ui/src/services/transport/types.test.ts`:
   ```typescript
   import { describe, it, expect } from 'vitest';
   import type {
     Session,
     AgentExecution,
     SessionWithExecutions,
     DashboardStats,
     TriggerSource,
     SessionStatus,
     ExecutionStatus,
   } from './types';

   describe('Transport Types', () => {
     describe('Session', () => {
       it('has required fields', () => {
         const session: Session = {
           id: 'sess-123',
           status: 'running',
           source: 'web',
           created_at: '2026-01-01T00:00:00Z',
           updated_at: '2026-01-01T00:00:00Z',
         };
         
         expect(session.id).toBe('sess-123');
         expect(session.status).toBe('running');
         expect(session.source).toBe('web');
       });

       it('supports optional fields', () => {
         const session: Session = {
           id: 'sess-123',
           status: 'crashed',
           source: 'cli',
           created_at: '2026-01-01T00:00:00Z',
           updated_at: '2026-01-01T00:00:00Z',
           completed_at: '2026-01-01T01:00:00Z',
           error_message: 'Something went wrong',
         };
         
         expect(session.completed_at).toBeDefined();
         expect(session.error_message).toBe('Something went wrong');
       });
     });

     describe('AgentExecution', () => {
       it('has required fields', () => {
         const execution: AgentExecution = {
           id: 'exec-123',
           session_id: 'sess-123',
           agent_id: 'root',
           status: 'running',
           conversation_id: 'conv-123',
           turn_count: 5,
           started_at: '2026-01-01T00:00:00Z',
         };
         
         expect(execution.id).toBe('exec-123');
         expect(execution.agent_id).toBe('root');
         expect(execution.turn_count).toBe(5);
       });

       it('supports parent_execution_id for subagents', () => {
         const subagent: AgentExecution = {
           id: 'exec-456',
           session_id: 'sess-123',
           agent_id: 'researcher',
           parent_execution_id: 'exec-123',
           status: 'running',
           conversation_id: 'conv-456',
           turn_count: 2,
           started_at: '2026-01-01T00:00:00Z',
         };
         
         expect(subagent.parent_execution_id).toBe('exec-123');
       });
     });

     describe('TriggerSource', () => {
       it('supports all valid values', () => {
         const sources: TriggerSource[] = ['web', 'cli', 'cron', 'api', 'plugin'];
         expect(sources).toHaveLength(5);
       });
     });

     describe('SessionStatus', () => {
       it('supports all valid values', () => {
         const statuses: SessionStatus[] = [
           'queued',
           'running',
           'completed',
           'crashed',
           'cancelled',
         ];
         expect(statuses).toHaveLength(5);
       });
     });

     describe('DashboardStats', () => {
       it('has session and execution counts', () => {
         const stats: DashboardStats = {
           sessions_running: 2,
           sessions_queued: 1,
           sessions_completed: 10,
           sessions_crashed: 0,
           sessions_cancelled: 0,
           executions_running: 3,
           executions_queued: 0,
           executions_completed: 15,
           executions_crashed: 1,
           sessions_by_source: { web: 8, cli: 5 },
         };
         
         expect(stats.sessions_running).toBe(2);
         expect(stats.executions_running).toBe(3);
         expect(stats.sessions_by_source.web).toBe(8);
       });
     });
   });
   ```

2. Create `apps/ui/src/services/transport/http.test.ts`:
   ```typescript
   import { describe, it, expect, vi, beforeEach } from 'vitest';
   import { server } from '@/test/mocks/server';
   import { http, HttpResponse } from 'msw';

   // Import the actual functions (adjust path as needed)
   // import { getDashboardStats, listExecutionSessions } from './http';

   describe('HTTP Transport', () => {
     describe('getDashboardStats', () => {
       it('returns stats from API', async () => {
         // Using MSW handler defined in setup
         const response = await fetch('http://localhost:18791/api/executions/stats/counts');
         const stats = await response.json();
         
         expect(stats.sessions_running).toBe(2);
         expect(stats.sessions_queued).toBe(1);
         expect(stats.sessions_by_source).toHaveProperty('web');
       });

       it('handles API errors', async () => {
         server.use(
           http.get('http://localhost:18791/api/executions/stats/counts', () => {
             return new HttpResponse(null, { status: 500 });
           })
         );

         const response = await fetch('http://localhost:18791/api/executions/stats/counts');
         expect(response.ok).toBe(false);
         expect(response.status).toBe(500);
       });
     });

     describe('listExecutionSessions', () => {
       it('returns sessions array', async () => {
         const response = await fetch('http://localhost:18791/api/executions/v2/sessions/full');
         const sessions = await response.json();
         
         expect(Array.isArray(sessions)).toBe(true);
         expect(sessions.length).toBeGreaterThan(0);
         expect(sessions[0]).toHaveProperty('session');
         expect(sessions[0]).toHaveProperty('executions');
       });

       it('handles empty sessions', async () => {
         server.use(
           http.get('http://localhost:18791/api/executions/v2/sessions/full', () => {
             return HttpResponse.json([]);
           })
         );

         const response = await fetch('http://localhost:18791/api/executions/v2/sessions/full');
         const sessions = await response.json();
         
         expect(sessions).toEqual([]);
       });
     });

     describe('gateway submit', () => {
       it('creates new session', async () => {
         const response = await fetch('http://localhost:18791/api/gateway/submit', {
           method: 'POST',
           headers: { 'Content-Type': 'application/json' },
           body: JSON.stringify({
             agent_id: 'root',
             message: 'Hello!',
           }),
         });
         
         const handle = await response.json();
         
         expect(handle).toHaveProperty('session_id');
         expect(handle).toHaveProperty('execution_id');
         expect(handle).toHaveProperty('conversation_id');
       });
     });
   });
   ```

3. Create `apps/ui/src/features/ops/components/SourceBadge.test.tsx`:
   ```typescript
   import { describe, it, expect } from 'vitest';
   import { render, screen } from '@/test/utils';
   import { SourceBadge } from './SourceBadge';
   import type { TriggerSource } from '@/services/transport/types';

   describe('SourceBadge', () => {
     const sources: TriggerSource[] = ['web', 'cli', 'cron', 'api', 'plugin'];

     it.each(sources)('renders badge for source: %s', (source) => {
       render(<SourceBadge source={source} />);
       expect(screen.getByText(source)).toBeInTheDocument();
     });

     it('applies base styling', () => {
       const { container } = render(<SourceBadge source="web" />);
       const badge = container.firstChild;
       
       // Check it has some styling class
       expect(badge).toHaveClass('badge', 'px-2', 'py-1');
     });

     it('renders with custom className', () => {
       const { container } = render(
         <SourceBadge source="cli" className="custom-class" />
       );
       
       expect(container.firstChild).toHaveClass('custom-class');
     });
   });
   ```

**Verification:**
```bash
cd apps/ui && npm test
```

**Self-Correction:** If tests fail:
1. Check import paths match your project structure
2. Verify MSW handlers return expected data
3. Check component exists and exports correctly

**Done when:** All frontend unit tests pass

---

## PHASE 3: Integration Tests

### Part 3A: Backend API Integration Tests

**Scope:** Test API endpoints with real HTTP requests

**Files to Create:**
- `gateway/tests/api_tests.rs`

**Changes:**

1. Create `gateway/tests/api_tests.rs`:
   ```rust
   //! API integration tests for gateway endpoints.

   use axum::http::StatusCode;
   use axum_test::TestServer;
   use gateway::{create_http_router, AppState, GatewayConfig};
   use serde_json::json;
   use tempfile::TempDir;

   async fn setup_test_server() -> (TestServer, TempDir) {
       let dir = TempDir::new().unwrap();
       let config = GatewayConfig::default();
       
       // Create minimal AppState for testing
       // This may need adjustment based on your actual AppState construction
       let state = AppState::new_for_testing(dir.path()).await.unwrap();
       
       let router = create_http_router(config, state);
       let server = TestServer::new(router).unwrap();
       
       (server, dir)
   }

   #[tokio::test]
   async fn health_check() {
       let (server, _dir) = setup_test_server().await;
       
       let response = server.get("/api/health").await;
       
       response.assert_status_ok();
   }

   #[tokio::test]
   async fn stats_endpoint() {
       let (server, _dir) = setup_test_server().await;
       
       let response = server.get("/api/executions/stats/counts").await;
       
       response.assert_status_ok();
       
       let stats: serde_json::Value = response.json();
       assert!(stats.get("sessions_running").is_some());
       assert!(stats.get("executions_running").is_some());
   }

   #[tokio::test]
   async fn sessions_list_empty() {
       let (server, _dir) = setup_test_server().await;
       
       let response = server.get("/api/executions/v2/sessions/full").await;
       
       response.assert_status_ok();
       
       let sessions: Vec<serde_json::Value> = response.json();
       assert!(sessions.is_empty());
   }

   #[tokio::test]
   async fn gateway_submit_creates_session() {
       let (server, _dir) = setup_test_server().await;
       
       let response = server
           .post("/api/gateway/submit")
           .json(&json!({
               "agent_id": "root",
               "message": "Test message",
               "source": "api"
           }))
           .await;
       
       // May be 200 OK or 201 Created depending on implementation
       assert!(response.status_code().is_success());
       
       let handle: serde_json::Value = response.json();
       assert!(handle.get("session_id").is_some());
       assert!(handle.get("execution_id").is_some());
   }

   #[tokio::test]
   async fn gateway_status_not_found() {
       let (server, _dir) = setup_test_server().await;
       
       let response = server.get("/api/gateway/status/nonexistent").await;
       
       response.assert_status(StatusCode::NOT_FOUND);
   }

   #[tokio::test]
   async fn gateway_cancel_not_found() {
       let (server, _dir) = setup_test_server().await;
       
       let response = server.post("/api/gateway/cancel/nonexistent").await;
       
       response.assert_status(StatusCode::NOT_FOUND);
   }
   ```

**Verification:**
```bash
cd gateway && cargo test --test api_tests -- --nocapture
```

**Self-Correction:** If tests fail:
1. Check AppState construction for testing
2. Verify route paths match actual routes
3. Check response status codes match implementation

**Done when:** API integration tests pass

---

### Part 3B: Frontend Integration Tests

**Scope:** Test React components with API mocking

**Files to Create:**
- `apps/ui/tests/integration/dashboard.test.tsx`

**Changes:**

1. Create `apps/ui/tests/integration/dashboard.test.tsx`:
   ```typescript
   import { describe, it, expect, vi, beforeEach } from 'vitest';
   import { render, screen, waitFor } from '@/test/utils';
   import { server } from '@/test/mocks/server';
   import { http, HttpResponse } from 'msw';
   import { WebOpsDashboard } from '@/features/ops/WebOpsDashboard';

   describe('WebOpsDashboard Integration', () => {
     it('loads and displays sessions', async () => {
       render(<WebOpsDashboard />);
       
       // Wait for loading to complete
       await waitFor(() => {
         expect(screen.queryByText(/loading/i)).not.toBeInTheDocument();
       });
       
       // Should show session data from mock
       expect(screen.getByText(/sess-001/i)).toBeInTheDocument();
     });

     it('displays stats from API', async () => {
       render(<WebOpsDashboard />);
       
       await waitFor(() => {
         // Stats from mock handler
         expect(screen.getByText(/2/)).toBeInTheDocument(); // sessions_running
       });
     });

     it('handles API error gracefully', async () => {
       server.use(
         http.get('http://localhost:18791/api/executions/v2/sessions/full', () => {
           return new HttpResponse(null, { status: 500 });
         })
       );

       render(<WebOpsDashboard />);
       
       await waitFor(() => {
         expect(screen.getByText(/error/i)).toBeInTheDocument();
       });
     });

     it('filters sessions by source', async () => {
       server.use(
         http.get('http://localhost:18791/api/executions/v2/sessions/full', () => {
           return HttpResponse.json([
             {
               session: { id: 'sess-web', status: 'running', source: 'web', created_at: new Date().toISOString(), updated_at: new Date().toISOString() },
               executions: [],
               subagent_count: 0,
             },
             {
               session: { id: 'sess-cli', status: 'running', source: 'cli', created_at: new Date().toISOString(), updated_at: new Date().toISOString() },
               executions: [],
               subagent_count: 0,
             },
           ]);
         })
       );

       const { user } = render(<WebOpsDashboard />);
       
       await waitFor(() => {
         expect(screen.getByText(/sess-web/i)).toBeInTheDocument();
         expect(screen.getByText(/sess-cli/i)).toBeInTheDocument();
       });
       
       // Click filter (adjust selector based on actual component)
       const filter = screen.getByTestId('source-filter');
       await user.click(filter);
       
       const webOption = screen.getByTestId('source-option-web');
       await user.click(webOption);
       
       // Only web session should be visible
       expect(screen.getByText(/sess-web/i)).toBeInTheDocument();
       expect(screen.queryByText(/sess-cli/i)).not.toBeInTheDocument();
     });

     it('shows execution hierarchy within session', async () => {
       server.use(
         http.get('http://localhost:18791/api/executions/v2/sessions/full', () => {
           return HttpResponse.json([
             {
               session: { id: 'sess-1', status: 'running', source: 'web', created_at: new Date().toISOString(), updated_at: new Date().toISOString() },
               executions: [
                 { id: 'exec-root', session_id: 'sess-1', agent_id: 'root', status: 'completed', conversation_id: 'c1', turn_count: 3, started_at: new Date().toISOString() },
                 { id: 'exec-sub', session_id: 'sess-1', agent_id: 'researcher', parent_execution_id: 'exec-root', status: 'running', conversation_id: 'c2', turn_count: 1, started_at: new Date().toISOString() },
               ],
               subagent_count: 1,
             },
           ]);
         })
       );

       render(<WebOpsDashboard />);
       
       await waitFor(() => {
         expect(screen.getByText(/root/i)).toBeInTheDocument();
         expect(screen.getByText(/researcher/i)).toBeInTheDocument();
       });
     });

     it('auto-refreshes data', async () => {
       vi.useFakeTimers();
       
       let callCount = 0;
       server.use(
         http.get('http://localhost:18791/api/executions/v2/sessions/full', () => {
           callCount++;
           return HttpResponse.json([]);
         })
       );

       render(<WebOpsDashboard />);
       
       // Initial load
       await waitFor(() => expect(callCount).toBe(1));
       
       // Advance time past refresh interval (e.g., 5 seconds)
       vi.advanceTimersByTime(5000);
       
       await waitFor(() => expect(callCount).toBe(2));
       
       vi.useRealTimers();
     });
   });
   ```

**Verification:**
```bash
cd apps/ui && npm run test -- tests/integration/
```

**Self-Correction:** If tests fail:
1. Check component exists and renders correctly
2. Verify test IDs match actual component attributes
3. Adjust mock data to match expected component props

**Done when:** Frontend integration tests pass

---

## PHASE 4: E2E Tests

### Part 4A: E2E Quick Scenarios

**Scope:** Fast E2E tests for critical paths

**Files to Create:**
- `apps/ui/tests/e2e/dashboard.spec.ts`
- `apps/ui/tests/e2e/navigation.spec.ts`

**Changes:**

1. Create `apps/ui/tests/e2e/dashboard.spec.ts`:
   ```typescript
   import { test, expect } from './fixtures';

   test.describe('Dashboard', () => {
     test('loads successfully', async ({ page }) => {
       await page.goto('/ops');
       
       // Wait for dashboard to render
       await expect(page.locator('[data-testid="dashboard"]')).toBeVisible();
     });

     test('displays stats panel', async ({ page }) => {
       await page.goto('/ops');
       
       await expect(page.locator('[data-testid="stats-panel"]')).toBeVisible();
     });

     test('shows session list', async ({ page }) => {
       await page.goto('/ops');
       
       // May be empty or have sessions
       await expect(page.locator('[data-testid="session-list"]')).toBeVisible();
     });

     test('source filter works', async ({ page }) => {
       await page.goto('/ops');
       
       // Open filter dropdown
       await page.click('[data-testid="source-filter"]');
       
       // Select 'web' source
       await page.click('[data-testid="source-option-web"]');
       
       // Verify filter is applied (URL or UI state)
       await expect(page.locator('[data-testid="source-filter"]')).toContainText('web');
     });
   });

   test.describe('Dashboard - Real Agent', () => {
     test.skip('single turn conversation', async ({ page }) => {
       // This test requires a running backend with configured agent
       // Skip in CI unless backend is available
       
       await page.goto('/');
       
       // Navigate to chat
       await page.click('[data-testid="new-chat"]');
       
       // Type a simple message
       await page.fill('[data-testid="chat-input"]', 'What is 2 + 2?');
       await page.click('[data-testid="send-button"]');
       
       // Wait for response (up to 30 seconds)
       await expect(page.locator('[data-testid="assistant-message"]')).toBeVisible({
         timeout: 30_000,
       });
       
       // Verify response contains expected content
       const response = await page.textContent('[data-testid="assistant-message"]');
       expect(response).toContain('4');
     });
   });
   ```

2. Create `apps/ui/tests/e2e/navigation.spec.ts`:
   ```typescript
   import { test, expect } from '@playwright/test';

   test.describe('Navigation', () => {
     test('home page loads', async ({ page }) => {
       await page.goto('/');
       await expect(page).toHaveTitle(/AgentZero/i);
     });

     test('can navigate to dashboard', async ({ page }) => {
       await page.goto('/');
       
       // Click dashboard link
       await page.click('a[href="/ops"]');
       
       await expect(page).toHaveURL(/\/ops/);
     });

     test('can navigate to settings', async ({ page }) => {
       await page.goto('/');
       
       await page.click('a[href="/settings"]');
       
       await expect(page).toHaveURL(/\/settings/);
     });

     test('404 page for unknown routes', async ({ page }) => {
       await page.goto('/unknown-route-xyz');
       
       // Should show 404 or redirect to home
       await expect(page.locator('text=404').or(page.locator('text=not found'))).toBeVisible();
     });
   });
   ```

**Verification:**
```bash
cd apps/ui && npm run test:e2e -- --grep "Dashboard|Navigation"
```

**Self-Correction:** If tests fail:
1. Check if dev server is running
2. Verify selectors match actual DOM
3. Increase timeouts for slow operations

**Done when:** Quick E2E tests pass

---

### Part 4B: E2E Long-Running Scenarios

**Scope:** Tests for multi-turn conversations and subagent delegation

**Files to Create:**
- `apps/ui/tests/e2e/long-running/research.spec.ts`
- `apps/ui/tests/e2e/long-running/multi-turn.spec.ts`

**Changes:**

1. Create `apps/ui/tests/e2e/long-running/research.spec.ts`:
   ```typescript
   import { test, expect } from '@playwright/test';

   // Long-running tests - increase timeout
   test.setTimeout(300_000); // 5 minutes

   test.describe('Research Agent Scenarios', () => {
     test.skip(process.env.CI === 'true', 'Skip in CI - requires real LLM');

     test('research task triggers subagent', async ({ page }) => {
       await page.goto('/');
       
       // Start new chat
       await page.click('[data-testid="new-chat"]');
       
       // Send research prompt
       await page.fill(
         '[data-testid="chat-input"]',
         'Research the latest advancements in AI agents and summarize the top 3 developments.'
       );
       await page.click('[data-testid="send-button"]');
       
       // Wait for initial acknowledgment
       await expect(page.locator('[data-testid="assistant-message"]')).toBeVisible({
         timeout: 60_000,
       });
       
       // Open dashboard in new tab to monitor
       const dashboardPage = await page.context().newPage();
       await dashboardPage.goto('/ops');
       
       // Wait for session to appear
       await expect(dashboardPage.locator('[data-testid="session-card"]')).toBeVisible({
         timeout: 30_000,
       });
       
       // Wait for subagent indicator (research delegation)
       await expect(
         dashboardPage.locator('text=researcher').or(dashboardPage.locator('text=subagent'))
       ).toBeVisible({ timeout: 120_000 });
       
       // Back to chat - wait for final response
       await page.bringToFront();
       
       // Wait for completion indicator
       await expect(page.locator('[data-testid="chat-complete"]').or(
         page.locator('[data-testid="assistant-message"]:last-child')
       )).toBeVisible({ timeout: 180_000 });
       
       // Verify response has substantive content
       const messages = await page.locator('[data-testid="assistant-message"]').allTextContents();
       const fullResponse = messages.join(' ');
       
       expect(fullResponse.length).toBeGreaterThan(200);
       expect(fullResponse.toLowerCase()).toMatch(/ai|agent|research|development/);
     });

     test('handles research timeout gracefully', async ({ page }) => {
       await page.goto('/');
       await page.click('[data-testid="new-chat"]');
       
       // Complex research that might timeout
       await page.fill(
         '[data-testid="chat-input"]',
         'Analyze the complete history of computing from 1940 to present day.'
       );
       await page.click('[data-testid="send-button"]');
       
       // Should either complete or show meaningful progress/timeout message
       const result = await Promise.race([
         page.waitForSelector('[data-testid="chat-complete"]', { timeout: 180_000 }),
         page.waitForSelector('[data-testid="error-message"]', { timeout: 180_000 }),
         page.waitForSelector('text=taking longer than expected', { timeout: 180_000 }),
       ]);
       
       expect(result).toBeTruthy();
     });
   });
   ```

2. Create `apps/ui/tests/e2e/long-running/multi-turn.spec.ts`:
   ```typescript
   import { test, expect } from '@playwright/test';

   test.setTimeout(180_000); // 3 minutes

   test.describe('Multi-Turn Conversations', () => {
     test.skip(process.env.CI === 'true', 'Skip in CI - requires real LLM');

     test('maintains context across turns', async ({ page }) => {
       await page.goto('/');
       await page.click('[data-testid="new-chat"]');
       
       // Turn 1: Introduce a topic
       await page.fill('[data-testid="chat-input"]', 'My name is Alice and I like Python programming.');
       await page.click('[data-testid="send-button"]');
       
       await expect(page.locator('[data-testid="assistant-message"]')).toBeVisible({
         timeout: 30_000,
       });
       
       // Turn 2: Reference previous context
       await page.fill('[data-testid="chat-input"]', 'What is my name?');
       await page.click('[data-testid="send-button"]');
       
       // Wait for second response
       await page.waitForSelector('[data-testid="assistant-message"]:nth-child(2)', {
         timeout: 30_000,
       });
       
       const response2 = await page.textContent('[data-testid="assistant-message"]:last-child');
       expect(response2?.toLowerCase()).toContain('alice');
       
       // Turn 3: Reference another piece of context
       await page.fill('[data-testid="chat-input"]', 'What programming language did I mention?');
       await page.click('[data-testid="send-button"]');
       
       await page.waitForSelector('[data-testid="assistant-message"]:nth-child(3)', {
         timeout: 30_000,
       });
       
       const response3 = await page.textContent('[data-testid="assistant-message"]:last-child');
       expect(response3?.toLowerCase()).toContain('python');
     });

     test('dashboard shows turn count increasing', async ({ page, context }) => {
       // Start chat
       await page.goto('/');
       await page.click('[data-testid="new-chat"]');
       
       // Get session ID from URL or response
       await page.fill('[data-testid="chat-input"]', 'Hello, this is turn 1.');
       await page.click('[data-testid="send-button"]');
       
       await expect(page.locator('[data-testid="assistant-message"]')).toBeVisible({
         timeout: 30_000,
       });
       
       // Open dashboard
       const dashboard = await context.newPage();
       await dashboard.goto('/ops');
       
       // Find the session and check turn count
       const turnCount1 = await dashboard.textContent('[data-testid="turn-count"]');
       expect(parseInt(turnCount1 || '0')).toBeGreaterThanOrEqual(1);
       
       // Send another message
       await page.bringToFront();
       await page.fill('[data-testid="chat-input"]', 'This is turn 2.');
       await page.click('[data-testid="send-button"]');
       
       await page.waitForSelector('[data-testid="assistant-message"]:nth-child(2)', {
         timeout: 30_000,
       });
       
       // Check turn count increased
       await dashboard.bringToFront();
       await dashboard.reload();
       
       const turnCount2 = await dashboard.textContent('[data-testid="turn-count"]');
       expect(parseInt(turnCount2 || '0')).toBeGreaterThan(parseInt(turnCount1 || '0'));
     });
   });
   ```

**Verification:**
```bash
# Run with real backend (not in CI)
cd apps/ui && npm run test:e2e -- tests/e2e/long-running/
```

**Self-Correction:** If tests fail:
1. Ensure backend is running with real LLM provider configured
2. Increase timeouts for slower models
3. Check network connectivity to LLM API

**Done when:** Long-running tests pass with real backend

---

## PHASE 5: CI Integration

### Part 5A: GitHub Actions Setup

**Scope:** Configure CI pipeline for automated testing

**Files to Create:**
- `.github/workflows/test.yml`

**Changes:**

1. Create `.github/workflows/test.yml`:
   ```yaml
   name: Tests

   on:
     push:
       branches: [main, v*]
     pull_request:
       branches: [main]

   env:
     CARGO_TERM_COLOR: always

   jobs:
     # Fast unit tests - run first
     unit-tests:
       runs-on: ubuntu-latest
       steps:
         - uses: actions/checkout@v4
         
         - name: Setup Rust
           uses: dtolnay/rust-action@stable
         
         - name: Cache cargo
           uses: Swatinem/rust-cache@v2
         
         - name: Run Rust unit tests
           run: cargo test --workspace --lib
         
         - name: Setup Node
           uses: actions/setup-node@v4
           with:
             node-version: '20'
             cache: 'npm'
             cache-dependency-path: apps/ui/package-lock.json
         
         - name: Install frontend deps
           run: cd apps/ui && npm ci
         
         - name: Run frontend unit tests
           run: cd apps/ui && npm test

     # Integration tests - run after unit tests pass
     integration-tests:
       runs-on: ubuntu-latest
       needs: unit-tests
       steps:
         - uses: actions/checkout@v4
         
         - name: Setup Rust
           uses: dtolnay/rust-action@stable
         
         - name: Cache cargo
           uses: Swatinem/rust-cache@v2
         
         - name: Run Rust integration tests
           run: cargo test --workspace
         
         - name: Setup Node
           uses: actions/setup-node@v4
           with:
             node-version: '20'
             cache: 'npm'
             cache-dependency-path: apps/ui/package-lock.json
         
         - name: Install frontend deps
           run: cd apps/ui && npm ci
         
         - name: Run frontend integration tests
           run: cd apps/ui && npm run test -- tests/integration/

     # E2E tests - run after integration tests
     e2e-tests:
       runs-on: ubuntu-latest
       needs: integration-tests
       steps:
         - uses: actions/checkout@v4
         
         - name: Setup Node
           uses: actions/setup-node@v4
           with:
             node-version: '20'
             cache: 'npm'
             cache-dependency-path: apps/ui/package-lock.json
         
         - name: Install deps
           run: cd apps/ui && npm ci
         
         - name: Install Playwright
           run: cd apps/ui && npx playwright install --with-deps chromium
         
         - name: Build frontend
           run: cd apps/ui && npm run build
         
         - name: Run E2E tests
           run: cd apps/ui && npm run test:e2e
         
         - name: Upload test results
           if: failure()
           uses: actions/upload-artifact@v4
           with:
             name: playwright-report
             path: apps/ui/playwright-report/
             retention-days: 7

     # Coverage report - optional, runs in parallel
     coverage:
       runs-on: ubuntu-latest
       needs: unit-tests
       steps:
         - uses: actions/checkout@v4
         
         - name: Setup Rust
           uses: dtolnay/rust-action@stable
         
         - name: Install tarpaulin
           run: cargo install cargo-tarpaulin
         
         - name: Run coverage
           run: cargo tarpaulin --workspace --out Xml
         
         - name: Upload coverage
           uses: codecov/codecov-action@v4
           with:
             files: cobertura.xml
   ```

**Verification:**
```bash
# Test workflow syntax
act -l  # List jobs (requires 'act' installed)
# Or push to a test branch and verify in GitHub Actions UI
```

**Self-Correction:** If workflow fails:
1. Check job dependencies are correct
2. Verify all paths are relative to repo root
3. Check GitHub Actions runner has required tools

**Done when:** GitHub Actions workflow runs successfully on push

---

## Execution Order

```
Phase 1 (Infrastructure - do first):
  1A → 1B → 1C

Phase 2 (Unit Tests - do in parallel after Phase 1):
  2A ─┐
  2B ─┼─ (can run in parallel)
  2C ─┘

Phase 3 (Integration Tests - after Phase 2):
  3A → 3B

Phase 4 (E2E Tests - after Phase 3):
  4A → 4B

Phase 5 (CI - after Phase 4):
  5A
```

---

## Progress Tracking

Update this section as parts are completed:

| Part | Status | Notes |
|------|--------|-------|
| 1A | complete | Backend test infrastructure (dev-deps, test_utils modules) |
| 1B | complete | Frontend test infrastructure (Vitest, RTL, MSW, mock handlers) |
| 1C | complete | E2E test infrastructure (Playwright, page objects, smoke tests) |
| 2A | complete | execution-state unit tests (60 tests: types, repository, test_utils); fixed missing `source` column bug |
| 2B | complete | Gateway bus unit tests (49 tests: types, http_bus); fixed `source` field serde default |
| 2C | complete | Frontend unit tests (81 tests: types, http transport, SourceBadge component) |
| 3A | complete | Backend API integration tests (26 tests: health, stats, sessions, gateway bus, agents, conversations, skills, providers, MCPs, settings, error handling, CORS, content-type) |
| 3B | complete | Frontend integration tests (22 tests: dashboard loading, data display, empty states, error handling, source/status filtering, auto-refresh, source stats bar, session controls) |
| 4A | complete | E2E quick scenarios (37 tests: dashboard functionality, navigation, smoke tests, page objects, error handling, responsive viewports) |
| 4B | complete | E2E long-running scenarios (14 tests: research agent, multi-turn conversations, session persistence, session-debug tests for session management verification) |
| 5A | complete | GitHub Actions setup (test.yml with unit, integration, E2E, and coverage jobs; triggers DISABLED - manual workflow_dispatch only) |

---

## Self-Correction Protocol

When verification fails:

1. **Read the error** - Don't guess, read the actual error message
2. **Identify the cause** - Is it a typo? Missing import? Wrong path?
3. **Make minimal fix** - Don't refactor, just fix the specific issue
4. **Re-run verification** - Confirm the fix works
5. **Proceed only when green** - Never move to next part with failing tests

### Common Issues and Fixes

| Issue | Likely Cause | Fix |
|-------|--------------|-----|
| "module not found" | Wrong import path | Check relative paths, check exports |
| "test timeout" | Async not awaited | Add await, increase timeout |
| "element not found" | Wrong selector | Check data-testid attributes |
| "connection refused" | Server not running | Start dev server, check ports |
| "type mismatch" | API changed | Update types to match API |

---

## All Prompts (Copy-Paste Ready)

### Phase 1: Infrastructure
```
Read memory-bank/test-cases/test-implementation-plan.md and execute Part 1A: Backend Test Infrastructure.
```
```
Read memory-bank/test-cases/test-implementation-plan.md and execute Part 1B: Frontend Test Infrastructure.
```
```
Read memory-bank/test-cases/test-implementation-plan.md and execute Part 1C: E2E Test Infrastructure.
```

### Phase 2: Unit Tests
```
Read memory-bank/test-cases/test-implementation-plan.md and execute Part 2A: execution-state Unit Tests.
```
```
Read memory-bank/test-cases/test-implementation-plan.md and execute Part 2B: Gateway Bus Unit Tests.
```
```
Read memory-bank/test-cases/test-implementation-plan.md and execute Part 2C: Frontend Unit Tests.
```

### Phase 3: Integration Tests
```
Read memory-bank/test-cases/test-implementation-plan.md and execute Part 3A: Backend API Integration Tests.
```
```
Read memory-bank/test-cases/test-implementation-plan.md and execute Part 3B: Frontend Integration Tests.
```

### Phase 4: E2E Tests
```
Read memory-bank/test-cases/test-implementation-plan.md and execute Part 4A: E2E Quick Scenarios.
```
```
Read memory-bank/test-cases/test-implementation-plan.md and execute Part 4B: E2E Long-Running Scenarios.
```

### Phase 5: CI
```
Read memory-bank/test-cases/test-implementation-plan.md and execute Part 5A: GitHub Actions Setup.
```
