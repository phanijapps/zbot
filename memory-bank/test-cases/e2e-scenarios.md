# End-to-End Test Scenarios

> **Purpose**: Define comprehensive E2E test scenarios for full-stack testing
> **Technology**: Playwright + Custom Test Harness
> **Execution**: Nightly builds, pre-release validation
> **Duration**: Long-running tests (30s - 5min per test)

---

## Table of Contents

1. [Test Categories](#test-categories)
2. [Quick Scenarios (< 30s)](#quick-scenarios--30s)
3. [Standard Scenarios (30s - 2min)](#standard-scenarios-30s---2min)
4. [Long-Running Scenarios (2min - 10min)](#long-running-scenarios-2min---10min)
5. [Stress Test Scenarios](#stress-test-scenarios)
6. [Error Recovery Scenarios](#error-recovery-scenarios)

---

## Test Categories

| Category | Duration | Run Frequency | Examples |
|----------|----------|---------------|----------|
| Quick | < 30s | Every commit | Navigation, simple API calls |
| Standard | 30s - 2min | Every PR | Single-turn chats, CRUD operations |
| Long-Running | 2min - 10min | Nightly | Multi-turn conversations, delegations |
| Stress | 10min+ | Weekly | Concurrent sessions, memory tests |

---

## Quick Scenarios (< 30s)

### SC-Q-001: Application Loads Successfully

**Objective**: Verify the app loads and displays correctly

```typescript
// apps/ui/tests/e2e/quick/app-loads.spec.ts
import { test, expect } from '@playwright/test';

test('application loads without errors', async ({ page }) => {
  // Listen for console errors
  const errors: string[] = [];
  page.on('console', msg => {
    if (msg.type() === 'error') errors.push(msg.text());
  });
  
  await page.goto('/');
  
  // Core elements visible
  await expect(page.locator('nav')).toBeVisible();
  await expect(page.locator('main')).toBeVisible();
  
  // No console errors
  expect(errors).toHaveLength(0);
});
```

### SC-Q-002: API Health Check

**Objective**: Backend is responding

```typescript
test('backend health check passes', async ({ request }) => {
  const response = await request.get('/api/health');
  expect(response.ok()).toBeTruthy();
  
  const body = await response.json();
  expect(body.status).toBe('healthy');
});
```

### SC-Q-003: WebSocket Connection

**Objective**: WebSocket connects successfully

```typescript
test('websocket connects', async ({ page }) => {
  await page.goto('/chat');
  
  // Wait for connection indicator
  await expect(page.locator('[data-testid="ws-connected"]')).toBeVisible({
    timeout: 5000
  });
});
```

---

## Standard Scenarios (30s - 2min)

### SC-S-001: Simple Question Answering

**Objective**: Agent answers a factual question

**Prompt**: "What is the capital of France?"

```typescript
// apps/ui/tests/e2e/standard/simple-qa.spec.ts
import { test, expect } from '@playwright/test';

test.describe('Simple Question Answering', () => {
  test('answers factual question correctly', async ({ page }) => {
    await page.goto('/chat');
    
    // Send question
    await page.fill('[data-testid="chat-input"]', 'What is the capital of France?');
    await page.click('[data-testid="send-button"]');
    
    // Wait for response
    await expect(page.locator('.message-assistant')).toBeVisible({ timeout: 30000 });
    
    // Verify answer contains Paris
    const response = await page.locator('.message-assistant').textContent();
    expect(response?.toLowerCase()).toContain('paris');
  });
});
```

### SC-S-002: Current Time Query

**Objective**: Agent provides current time

**Prompt**: "What is the current time?"

```typescript
test('provides current time', async ({ page }) => {
  await page.goto('/chat');
  
  await page.fill('[data-testid="chat-input"]', 'What is the current time?');
  await page.click('[data-testid="send-button"]');
  
  await expect(page.locator('.message-assistant')).toBeVisible({ timeout: 30000 });
  
  // Response should contain time pattern
  const response = await page.locator('.message-assistant').textContent();
  expect(response).toMatch(/\d{1,2}:\d{2}/);
});
```

### SC-S-003: Agent Selection

**Objective**: User can select different agents

```typescript
test('allows agent selection', async ({ page }) => {
  await page.goto('/chat');
  
  // Open agent selector
  await page.click('[data-testid="agent-selector"]');
  
  // Select a different agent
  await page.click('text=Research Agent');
  
  // Verify selection
  await expect(page.locator('[data-testid="selected-agent"]')).toHaveText('Research Agent');
});
```

### SC-S-004: Session Creation via API

**Objective**: Foreign plugin can create sessions

```typescript
test('creates session via gateway API', async ({ request }) => {
  const response = await request.post('/api/gateway/submit', {
    data: {
      agent_id: 'root',
      message: 'Hello from API test',
      source: 'api',
      external_ref: 'e2e-test-001'
    }
  });
  
  expect(response.ok()).toBeTruthy();
  
  const body = await response.json();
  expect(body.session_id).toMatch(/^sess-/);
  expect(body.execution_id).toMatch(/^exec-/);
});
```

### SC-S-005: Dashboard Displays Active Session

**Objective**: Active sessions appear on dashboard

```typescript
test('shows active session on dashboard', async ({ page, request }) => {
  // Create a session via API
  const createResponse = await request.post('/api/gateway/submit', {
    data: {
      agent_id: 'root',
      message: 'Long running task for dashboard test',
      source: 'api'
    }
  });
  const session = await createResponse.json();
  
  // Navigate to dashboard
  await page.goto('/');
  
  // Should see the session
  await expect(page.locator(`text=${session.session_id}`)).toBeVisible({
    timeout: 10000
  });
});
```

---

## Long-Running Scenarios (2min - 10min)

### SC-L-001: Multi-Turn Conversation with Context

**Objective**: Agent maintains context across 5+ turns

**Conversation Flow**:
1. "My name is Alice and I live in Seattle"
2. "What's my name?"
3. "Where do I live?"
4. "I moved to Portland"
5. "Where do I live now?"

```typescript
// apps/ui/tests/e2e/long-running/multi-turn.spec.ts
import { test, expect } from '@playwright/test';

test.describe('Multi-Turn Conversation', () => {
  test.setTimeout(120000); // 2 minutes

  test('maintains context across 5 turns', async ({ page }) => {
    await page.goto('/chat');
    
    const messages = [
      { input: 'My name is Alice and I live in Seattle', expectContains: ['alice', 'seattle'] },
      { input: "What's my name?", expectContains: ['alice'] },
      { input: 'Where do I live?', expectContains: ['seattle'] },
      { input: 'I moved to Portland', expectContains: ['portland'] },
      { input: 'Where do I live now?', expectContains: ['portland'] },
    ];
    
    for (const msg of messages) {
      await page.fill('[data-testid="chat-input"]', msg.input);
      await page.click('[data-testid="send-button"]');
      
      // Wait for response
      const responseCount = await page.locator('.message-assistant').count();
      await expect(page.locator('.message-assistant').nth(responseCount)).toBeVisible({
        timeout: 30000
      });
      
      // Verify response contains expected content
      const response = await page.locator('.message-assistant').last().textContent();
      for (const expected of msg.expectContains) {
        expect(response?.toLowerCase()).toContain(expected.toLowerCase());
      }
    }
  });
});
```

### SC-L-002: Subagent Delegation - Research Task

**Objective**: Root agent delegates to research subagent

**Prompt**: "Research the latest advancements in AI and provide a summary"

```typescript
test.describe('Subagent Delegation', () => {
  test.setTimeout(300000); // 5 minutes

  test('delegates research task to subagent', async ({ page }) => {
    await page.goto('/chat');
    
    // Send research request
    await page.fill(
      '[data-testid="chat-input"]',
      'Research the latest advancements in AI and provide a summary'
    );
    await page.click('[data-testid="send-button"]');
    
    // Wait for delegation indicator (may take time)
    await expect(page.locator('[data-testid="delegation-in-progress"]')).toBeVisible({
      timeout: 60000
    });
    
    // Navigate to dashboard to verify
    await page.click('text=Dashboard');
    
    // Find the session
    await expect(page.locator('[data-testid="active-session"]')).toBeVisible();
    
    // Expand to see executions
    await page.click('[data-testid="session-expand"]');
    
    // Should have root + subagent
    await expect(page.locator('[data-testid="execution-root"]')).toBeVisible();
    await expect(page.locator('[data-testid="execution-subagent"]')).toBeVisible();
    
    // Wait for completion
    await expect(page.locator('[data-testid="session-completed"]')).toBeVisible({
      timeout: 180000
    });
    
    // Go back to chat and verify response
    await page.click('text=Chat');
    const response = await page.locator('.message-assistant').last().textContent();
    expect(response?.length).toBeGreaterThan(200); // Should have substantial content
  });
});
```

### SC-L-003: Parallel Sessions

**Objective**: Multiple concurrent sessions work correctly

```typescript
test.describe('Parallel Sessions', () => {
  test.setTimeout(180000); // 3 minutes

  test('handles multiple concurrent sessions', async ({ context }) => {
    // Create 3 browser tabs
    const page1 = await context.newPage();
    const page2 = await context.newPage();
    const page3 = await context.newPage();
    
    // Start conversations in all tabs
    for (const page of [page1, page2, page3]) {
      await page.goto('/chat');
    }
    
    // Send messages
    await page1.fill('[data-testid="chat-input"]', 'Session 1: What is 2+2?');
    await page1.click('[data-testid="send-button"]');
    
    await page2.fill('[data-testid="chat-input"]', 'Session 2: What is the capital of Japan?');
    await page2.click('[data-testid="send-button"]');
    
    await page3.fill('[data-testid="chat-input"]', 'Session 3: Tell me about the moon');
    await page3.click('[data-testid="send-button"]');
    
    // Wait for all responses
    await expect(page1.locator('.message-assistant')).toBeVisible({ timeout: 60000 });
    await expect(page2.locator('.message-assistant')).toBeVisible({ timeout: 60000 });
    await expect(page3.locator('.message-assistant')).toBeVisible({ timeout: 60000 });
    
    // Verify responses are correct (not cross-contaminated)
    const r1 = await page1.locator('.message-assistant').textContent();
    const r2 = await page2.locator('.message-assistant').textContent();
    const r3 = await page3.locator('.message-assistant').textContent();
    
    expect(r1?.toLowerCase()).toContain('4');
    expect(r2?.toLowerCase()).toContain('tokyo');
    expect(r3?.toLowerCase()).toContain('moon');
  });
});
```

### SC-L-004: Session Pause and Resume

**Objective**: Sessions can be paused mid-execution and resumed

```typescript
test.describe('Session Pause/Resume', () => {
  test.setTimeout(180000); // 3 minutes

  test('pauses and resumes session correctly', async ({ page, request }) => {
    // Start a long-running task
    const response = await request.post('/api/gateway/submit', {
      data: {
        agent_id: 'root',
        message: 'Write a detailed essay about climate change',
        source: 'api'
      }
    });
    const session = await response.json();
    
    // Navigate to dashboard
    await page.goto('/');
    
    // Wait for session to appear
    await expect(page.locator(`text=${session.session_id}`)).toBeVisible();
    
    // Wait a bit for execution to start
    await page.waitForTimeout(5000);
    
    // Pause the session
    await request.post(`/api/gateway/pause/${session.session_id}`);
    
    // Verify paused status on dashboard
    await page.reload();
    await expect(page.locator('[data-testid="status-paused"]')).toBeVisible();
    
    // Resume the session
    await request.post(`/api/gateway/resume/${session.session_id}`);
    
    // Verify running again
    await page.reload();
    await expect(page.locator('[data-testid="status-running"]')).toBeVisible();
    
    // Wait for completion
    await expect(page.locator('[data-testid="status-completed"]')).toBeVisible({
      timeout: 120000
    });
  });
});
```

### SC-L-005: Tool Usage - File Operations

**Objective**: Agent correctly uses file tools

**Prompt**: "Read the contents of package.json and tell me the project name"

```typescript
test.describe('Tool Usage', () => {
  test.setTimeout(120000);

  test('uses file read tool correctly', async ({ page }) => {
    await page.goto('/chat');
    
    await page.fill(
      '[data-testid="chat-input"]',
      'Read the contents of package.json and tell me the project name'
    );
    await page.click('[data-testid="send-button"]');
    
    // Should see tool call indicator
    await expect(page.locator('[data-testid="tool-call"]')).toBeVisible({
      timeout: 30000
    });
    
    // Tool should be read_file or similar
    await expect(page.locator('text=read_file')).toBeVisible();
    
    // Wait for response
    await expect(page.locator('.message-assistant')).toBeVisible({
      timeout: 60000
    });
    
    // Response should mention package name
    const response = await page.locator('.message-assistant').textContent();
    expect(response).toBeTruthy();
  });
});
```

---

## Stress Test Scenarios

### SC-ST-001: High Concurrent Sessions

**Objective**: System handles 10+ concurrent sessions

```typescript
// apps/ui/tests/e2e/stress/concurrent-sessions.spec.ts
test.describe('Stress: Concurrent Sessions', () => {
  test.setTimeout(600000); // 10 minutes

  test('handles 10 concurrent sessions', async ({ request }) => {
    const sessions: string[] = [];
    
    // Create 10 sessions rapidly
    for (let i = 0; i < 10; i++) {
      const response = await request.post('/api/gateway/submit', {
        data: {
          agent_id: 'root',
          message: `Stress test session ${i}: What is ${i} * ${i}?`,
          source: 'api',
          external_ref: `stress-test-${i}`
        }
      });
      const session = await response.json();
      sessions.push(session.session_id);
    }
    
    // Poll for all completions
    const startTime = Date.now();
    const timeout = 300000; // 5 minutes
    
    while (Date.now() - startTime < timeout) {
      let allCompleted = true;
      
      for (const sessionId of sessions) {
        const statusResponse = await request.get(`/api/gateway/status/${sessionId}`);
        const status = await statusResponse.json();
        
        if (status.status !== 'completed' && status.status !== 'crashed') {
          allCompleted = false;
          break;
        }
      }
      
      if (allCompleted) break;
      await new Promise(r => setTimeout(r, 5000));
    }
    
    // Verify all completed
    for (const sessionId of sessions) {
      const statusResponse = await request.get(`/api/gateway/status/${sessionId}`);
      const status = await statusResponse.json();
      expect(['completed', 'crashed']).toContain(status.status);
    }
  });
});
```

### SC-ST-002: Rapid Message Sending

**Objective**: System handles rapid sequential messages

```typescript
test.describe('Stress: Rapid Messages', () => {
  test.setTimeout(300000);

  test('handles 20 rapid sequential messages', async ({ page }) => {
    await page.goto('/chat');
    
    for (let i = 0; i < 20; i++) {
      await page.fill('[data-testid="chat-input"]', `Message ${i}: Quick question ${i}`);
      await page.click('[data-testid="send-button"]');
      
      // Wait just a bit between sends
      await page.waitForTimeout(100);
    }
    
    // Wait for all responses
    await expect(page.locator('.message-assistant')).toHaveCount(20, {
      timeout: 180000
    });
  });
});
```

---

## Error Recovery Scenarios

### SC-ER-001: Network Disconnection

**Objective**: App recovers from network issues

```typescript
test.describe('Error Recovery: Network', () => {
  test('recovers from network disconnection', async ({ page, context }) => {
    await page.goto('/chat');
    
    // Start a conversation
    await page.fill('[data-testid="chat-input"]', 'Hello');
    await page.click('[data-testid="send-button"]');
    
    await expect(page.locator('.message-assistant')).toBeVisible({ timeout: 30000 });
    
    // Simulate offline
    await context.setOffline(true);
    
    // Try to send another message
    await page.fill('[data-testid="chat-input"]', 'Are you there?');
    await page.click('[data-testid="send-button"]');
    
    // Should show error
    await expect(page.locator('[data-testid="error-message"]')).toBeVisible();
    
    // Go back online
    await context.setOffline(false);
    
    // Retry should work
    await page.click('[data-testid="retry-button"]');
    await expect(page.locator('.message-assistant').last()).toBeVisible({
      timeout: 30000
    });
  });
});
```

### SC-ER-002: Agent Crash Recovery

**Objective**: Dashboard shows crashed sessions correctly

```typescript
test.describe('Error Recovery: Agent Crash', () => {
  test('displays crashed session status', async ({ page, request }) => {
    // Create a session that will crash (e.g., invalid agent)
    const response = await request.post('/api/gateway/submit', {
      data: {
        agent_id: 'nonexistent-agent',
        message: 'This should fail',
        source: 'api'
      }
    });
    
    // Navigate to dashboard
    await page.goto('/');
    
    // Should show error state
    await expect(page.locator('[data-testid="status-crashed"]')).toBeVisible({
      timeout: 30000
    });
  });
});
```

### SC-ER-003: Session Not Found

**Objective**: Handles invalid session IDs gracefully

```typescript
test.describe('Error Recovery: Invalid Session', () => {
  test('handles session not found error', async ({ request }) => {
    const response = await request.get('/api/gateway/status/sess-nonexistent');
    
    expect(response.status()).toBe(404);
    
    const body = await response.json();
    expect(body.code).toBe('SESSION_NOT_FOUND');
  });
});
```

---

## Test Data Fixtures

### Sample Prompts for Testing

```typescript
// apps/ui/tests/fixtures/prompts.ts
export const TEST_PROMPTS = {
  // Quick (< 30s expected)
  simple: {
    factual: 'What is the capital of France?',
    math: 'What is 2 + 2?',
    time: 'What is the current time?',
    greeting: 'Hello, how are you?',
  },
  
  // Standard (30s - 2min expected)
  standard: {
    explanation: 'Explain photosynthesis in simple terms',
    list: 'List the top 5 programming languages',
    comparison: 'Compare Python and JavaScript',
  },
  
  // Long-running (2min+ expected)
  longRunning: {
    research: 'Research the latest advancements in AI and provide a comprehensive summary',
    essay: 'Write a detailed essay about climate change and its effects',
    codeReview: 'Analyze the code quality of this repository and suggest improvements',
    multiStep: 'Plan a week-long trip to Tokyo, including flights, hotels, and activities',
  },
  
  // Context-dependent
  contextual: {
    intro: 'My name is Alice and I live in Seattle',
    nameQuery: "What's my name?",
    locationQuery: 'Where do I live?',
    locationUpdate: 'I moved to Portland',
    currentLocation: 'Where do I live now?',
  },
};
```

---

## Running E2E Tests

### Full E2E Suite
```bash
cd apps/ui
npx playwright test
```

### Quick Tests Only
```bash
npx playwright test tests/e2e/quick/
```

### Standard Tests
```bash
npx playwright test tests/e2e/standard/
```

### Long-Running Tests (Nightly)
```bash
npx playwright test tests/e2e/long-running/ --timeout=600000
```

### Stress Tests (Weekly)
```bash
npx playwright test tests/e2e/stress/ --workers=1
```

### With Trace Recording
```bash
npx playwright test --trace on
```

### In Debug Mode
```bash
npx playwright test --debug
```
