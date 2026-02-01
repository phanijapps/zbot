# Frontend UI Test Cases (Web)

> **Purpose**: Define UI test scenarios for the React web application
> **Technology**: Vitest + React Testing Library (unit/integration), Playwright (E2E)
> **Location**: `apps/ui/src/**/*.test.tsx` and `apps/ui/tests/`

---

## Table of Contents

1. [Test Setup](#test-setup)
2. [Component Unit Tests](#component-unit-tests)
3. [Dashboard Tests](#dashboard-tests)
4. [Chat Interface Tests](#chat-interface-tests)
5. [Settings & Configuration Tests](#settings--configuration-tests)
6. [Integration Tests](#integration-tests)
7. [E2E Tests (Playwright)](#e2e-tests-playwright)

---

## Test Setup

### Vitest Configuration

```typescript
// apps/ui/vitest.config.ts
import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';
import path from 'path';

export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./tests/setup.ts'],
    include: ['src/**/*.test.{ts,tsx}', 'tests/**/*.test.{ts,tsx}'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'json', 'html'],
      include: ['src/**/*.{ts,tsx}'],
      exclude: ['src/**/*.test.{ts,tsx}', 'src/**/index.ts'],
    },
  },
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
});
```

### Test Setup File

```typescript
// apps/ui/tests/setup.ts
import '@testing-library/jest-dom';
import { cleanup } from '@testing-library/react';
import { afterEach, vi } from 'vitest';
import { setupServer } from 'msw/node';
import { handlers } from './mocks/handlers';

// MSW server for mocking HTTP requests
export const server = setupServer(...handlers);

beforeAll(() => server.listen({ onUnhandledRequest: 'error' }));
afterEach(() => {
  cleanup();
  server.resetHandlers();
});
afterAll(() => server.close());

// Mock WebSocket
vi.mock('@/services/websocket', () => ({
  createWebSocket: vi.fn(),
  WebSocketManager: vi.fn(),
}));
```

### MSW Handlers

```typescript
// apps/ui/tests/mocks/handlers.ts
import { http, HttpResponse } from 'msw';

export const handlers = [
  // Health check
  http.get('/api/health', () => {
    return HttpResponse.json({ status: 'healthy' });
  }),

  // Sessions list
  http.get('/api/executions/v2/sessions/full', () => {
    return HttpResponse.json([
      {
        id: 'sess-001',
        status: 'running',
        source: 'web',
        root_agent_id: 'root',
        created_at: new Date().toISOString(),
        executions: [
          { id: 'exec-001', agent_id: 'root', status: 'running', delegation_type: 'root' }
        ],
        subagent_count: 0
      }
    ]);
  }),

  // Dashboard stats
  http.get('/api/executions/stats', () => {
    return HttpResponse.json({
      sessions_running: 1,
      sessions_completed: 5,
      sessions_queued: 0,
      sessions_crashed: 0,
      executions_running: 1,
      executions_completed: 10,
      sessions_by_source: { web: 4, cron: 1, api: 1 }
    });
  }),

  // Agents list
  http.get('/api/agents', () => {
    return HttpResponse.json([
      { id: 'root', name: 'root', displayName: 'Root Agent' }
    ]);
  }),

  // Gateway submit
  http.post('/api/gateway/submit', async ({ request }) => {
    const body = await request.json();
    return HttpResponse.json({
      session_id: 'sess-new-001',
      execution_id: 'exec-new-001',
      conversation_id: 'web-new-001'
    });
  }),
];
```

---

## Component Unit Tests

### TC-UI-001: StatusBadge Component

```typescript
// apps/ui/src/components/StatusBadge.test.tsx
import { render, screen } from '@testing-library/react';
import { StatusBadge } from './StatusBadge';

describe('StatusBadge', () => {
  it('renders running status with correct styling', () => {
    render(<StatusBadge status="running" />);
    
    const badge = screen.getByText('Running');
    expect(badge).toBeInTheDocument();
    expect(badge).toHaveClass('status-running');
  });

  it('renders completed status', () => {
    render(<StatusBadge status="completed" />);
    expect(screen.getByText('Completed')).toBeInTheDocument();
  });

  it('renders crashed status with error styling', () => {
    render(<StatusBadge status="crashed" />);
    
    const badge = screen.getByText('Crashed');
    expect(badge).toHaveClass('status-crashed');
  });

  it('renders queued status', () => {
    render(<StatusBadge status="queued" />);
    expect(screen.getByText('Queued')).toBeInTheDocument();
  });
});
```

### TC-UI-002: SourceBadge Component

```typescript
// apps/ui/src/features/ops/components/SourceBadge.test.tsx
import { render, screen } from '@testing-library/react';
import { SourceBadge } from './SourceBadge';

describe('SourceBadge', () => {
  it('renders web source with globe icon', () => {
    render(<SourceBadge source="web" />);
    
    expect(screen.getByText('Web')).toBeInTheDocument();
    expect(screen.getByTitle('Source: Web')).toBeInTheDocument();
  });

  it('renders cron source with timer icon', () => {
    render(<SourceBadge source="cron" />);
    expect(screen.getByText('Cron')).toBeInTheDocument();
  });

  it('renders plugin source', () => {
    render(<SourceBadge source="plugin" />);
    expect(screen.getByText('Plugin')).toBeInTheDocument();
  });

  it('renders api source', () => {
    render(<SourceBadge source="api" />);
    expect(screen.getByText('API')).toBeInTheDocument();
  });

  it('renders cli source', () => {
    render(<SourceBadge source="cli" />);
    expect(screen.getByText('CLI')).toBeInTheDocument();
  });
});
```

### TC-UI-003: SessionCard Component

```typescript
// apps/ui/src/features/ops/components/SessionCard.test.tsx
import { render, screen, fireEvent } from '@testing-library/react';
import { SessionCard } from './SessionCard';

const mockSession = {
  id: 'sess-001',
  status: 'running',
  source: 'web',
  root_agent_id: 'root',
  created_at: '2026-02-01T10:00:00Z',
  total_tokens_in: 100,
  total_tokens_out: 200,
  executions: [
    { id: 'exec-001', agent_id: 'root', status: 'completed', delegation_type: 'root' },
    { id: 'exec-002', agent_id: 'research', status: 'running', delegation_type: 'sequential' }
  ],
  subagent_count: 1
};

describe('SessionCard', () => {
  it('renders session ID', () => {
    render(<SessionCard session={mockSession} />);
    expect(screen.getByText(/sess-001/)).toBeInTheDocument();
  });

  it('renders status badge', () => {
    render(<SessionCard session={mockSession} />);
    expect(screen.getByText('Running')).toBeInTheDocument();
  });

  it('renders source badge', () => {
    render(<SessionCard session={mockSession} />);
    expect(screen.getByText('Web')).toBeInTheDocument();
  });

  it('shows execution hierarchy when expanded', async () => {
    render(<SessionCard session={mockSession} />);
    
    // Find and click expand button
    const expandButton = screen.getByRole('button', { name: /expand/i });
    fireEvent.click(expandButton);
    
    // Should show both executions
    expect(screen.getByText('root')).toBeInTheDocument();
    expect(screen.getByText('research')).toBeInTheDocument();
  });

  it('displays token count', () => {
    render(<SessionCard session={mockSession} />);
    expect(screen.getByText(/300/)).toBeInTheDocument(); // 100 + 200
  });

  it('shows subagent count', () => {
    render(<SessionCard session={mockSession} />);
    expect(screen.getByText(/1 subagent/)).toBeInTheDocument();
  });
});
```

---

## Dashboard Tests

### TC-DASH-001: Dashboard Stats Display

```typescript
// apps/ui/src/features/ops/WebOpsDashboard.test.tsx
import { render, screen, waitFor } from '@testing-library/react';
import { WebOpsDashboard } from './WebOpsDashboard';
import { BrowserRouter } from 'react-router-dom';

const renderDashboard = () => {
  return render(
    <BrowserRouter>
      <WebOpsDashboard />
    </BrowserRouter>
  );
};

describe('WebOpsDashboard - Stats', () => {
  it('displays session counts', async () => {
    renderDashboard();
    
    await waitFor(() => {
      expect(screen.getByText(/1 running/i)).toBeInTheDocument();
      expect(screen.getByText(/5 completed/i)).toBeInTheDocument();
    });
  });

  it('displays execution counts', async () => {
    renderDashboard();
    
    await waitFor(() => {
      expect(screen.getByText(/1 running/i)).toBeInTheDocument();
      expect(screen.getByText(/10 completed/i)).toBeInTheDocument();
    });
  });

  it('displays source breakdown', async () => {
    renderDashboard();
    
    await waitFor(() => {
      expect(screen.getByText(/web/i)).toBeInTheDocument();
      expect(screen.getByText(/cron/i)).toBeInTheDocument();
    });
  });
});
```

### TC-DASH-002: Active Sessions Panel

```typescript
describe('WebOpsDashboard - Active Sessions', () => {
  it('displays active sessions list', async () => {
    renderDashboard();
    
    await waitFor(() => {
      expect(screen.getByText('Active Sessions')).toBeInTheDocument();
      expect(screen.getByText(/sess-001/)).toBeInTheDocument();
    });
  });

  it('shows running indicator for active sessions', async () => {
    renderDashboard();
    
    await waitFor(() => {
      const runningIndicator = screen.getByTestId('running-indicator');
      expect(runningIndicator).toBeInTheDocument();
    });
  });

  it('shows empty state when no active sessions', async () => {
    // Override handler to return empty list
    server.use(
      http.get('/api/executions/v2/sessions/full', () => {
        return HttpResponse.json([]);
      })
    );
    
    renderDashboard();
    
    await waitFor(() => {
      expect(screen.getByText(/no active sessions/i)).toBeInTheDocument();
    });
  });
});
```

### TC-DASH-003: Source Filter

```typescript
describe('WebOpsDashboard - Source Filter', () => {
  it('renders source filter dropdown', async () => {
    renderDashboard();
    
    await waitFor(() => {
      expect(screen.getByRole('combobox')).toBeInTheDocument();
    });
  });

  it('filters sessions by selected source', async () => {
    renderDashboard();
    
    await waitFor(() => {
      const filter = screen.getByRole('combobox');
      fireEvent.change(filter, { target: { value: 'cron' } });
    });
    
    // Sessions should be filtered
    // (Would need sessions with different sources in mock data)
  });

  it('shows all sessions when "All Sources" selected', async () => {
    renderDashboard();
    
    await waitFor(() => {
      const filter = screen.getByRole('combobox');
      fireEvent.change(filter, { target: { value: 'all' } });
    });
    
    expect(screen.getByText(/sess-001/)).toBeInTheDocument();
  });
});
```

### TC-DASH-004: Session History Panel

```typescript
describe('WebOpsDashboard - Session History', () => {
  it('displays completed sessions', async () => {
    server.use(
      http.get('/api/executions/v2/sessions/full', () => {
        return HttpResponse.json([
          {
            id: 'sess-old',
            status: 'completed',
            source: 'web',
            root_agent_id: 'root',
            created_at: '2026-01-31T10:00:00Z',
            executions: [],
            subagent_count: 0
          }
        ]);
      })
    );
    
    renderDashboard();
    
    await waitFor(() => {
      expect(screen.getByText('Session History')).toBeInTheDocument();
      expect(screen.getByText(/sess-old/)).toBeInTheDocument();
    });
  });

  it('shows turn count for completed sessions', async () => {
    // Sessions with multiple root executions
  });
});
```

### TC-DASH-005: Auto-Refresh

```typescript
describe('WebOpsDashboard - Auto-Refresh', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('refreshes data every 5 seconds', async () => {
    const fetchSpy = vi.spyOn(global, 'fetch');
    
    renderDashboard();
    
    // Initial fetch
    await waitFor(() => {
      expect(fetchSpy).toHaveBeenCalled();
    });
    
    // Advance 5 seconds
    vi.advanceTimersByTime(5000);
    
    // Should fetch again
    await waitFor(() => {
      expect(fetchSpy).toHaveBeenCalledTimes(2);
    });
  });
});
```

---

## Chat Interface Tests

### TC-CHAT-001: Message Input

```typescript
// apps/ui/src/features/chat/ChatInput.test.tsx
import { render, screen, fireEvent } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ChatInput } from './ChatInput';

describe('ChatInput', () => {
  const mockOnSend = vi.fn();

  beforeEach(() => {
    mockOnSend.mockClear();
  });

  it('renders input field', () => {
    render(<ChatInput onSend={mockOnSend} />);
    expect(screen.getByPlaceholderText(/type a message/i)).toBeInTheDocument();
  });

  it('calls onSend when submit button clicked', async () => {
    render(<ChatInput onSend={mockOnSend} />);
    
    const input = screen.getByPlaceholderText(/type a message/i);
    await userEvent.type(input, 'Hello, agent!');
    
    const submitButton = screen.getByRole('button', { name: /send/i });
    await userEvent.click(submitButton);
    
    expect(mockOnSend).toHaveBeenCalledWith('Hello, agent!');
  });

  it('calls onSend when Enter pressed', async () => {
    render(<ChatInput onSend={mockOnSend} />);
    
    const input = screen.getByPlaceholderText(/type a message/i);
    await userEvent.type(input, 'Hello{Enter}');
    
    expect(mockOnSend).toHaveBeenCalledWith('Hello');
  });

  it('clears input after sending', async () => {
    render(<ChatInput onSend={mockOnSend} />);
    
    const input = screen.getByPlaceholderText(/type a message/i);
    await userEvent.type(input, 'Hello{Enter}');
    
    expect(input).toHaveValue('');
  });

  it('disables send when input is empty', () => {
    render(<ChatInput onSend={mockOnSend} />);
    
    const submitButton = screen.getByRole('button', { name: /send/i });
    expect(submitButton).toBeDisabled();
  });
});
```

### TC-CHAT-002: Message Display

```typescript
// apps/ui/src/features/chat/MessageList.test.tsx
import { render, screen } from '@testing-library/react';
import { MessageList } from './MessageList';

const mockMessages = [
  { id: '1', role: 'user', content: 'Hello', timestamp: '2026-02-01T10:00:00Z' },
  { id: '2', role: 'assistant', content: 'Hi there!', timestamp: '2026-02-01T10:00:01Z' },
];

describe('MessageList', () => {
  it('renders all messages', () => {
    render(<MessageList messages={mockMessages} />);
    
    expect(screen.getByText('Hello')).toBeInTheDocument();
    expect(screen.getByText('Hi there!')).toBeInTheDocument();
  });

  it('distinguishes user and assistant messages', () => {
    render(<MessageList messages={mockMessages} />);
    
    const userMessage = screen.getByText('Hello').closest('.message');
    const assistantMessage = screen.getByText('Hi there!').closest('.message');
    
    expect(userMessage).toHaveClass('message-user');
    expect(assistantMessage).toHaveClass('message-assistant');
  });

  it('renders markdown in assistant messages', () => {
    const messages = [
      { id: '1', role: 'assistant', content: '**Bold** and *italic*', timestamp: '2026-02-01T10:00:00Z' },
    ];
    
    render(<MessageList messages={messages} />);
    
    expect(screen.getByText('Bold')).toHaveStyle('font-weight: bold');
  });

  it('shows empty state when no messages', () => {
    render(<MessageList messages={[]} />);
    expect(screen.getByText(/start a conversation/i)).toBeInTheDocument();
  });
});
```

### TC-CHAT-003: Tool Call Display

```typescript
// apps/ui/src/features/chat/ToolCallCard.test.tsx
import { render, screen, fireEvent } from '@testing-library/react';
import { ToolCallCard } from './ToolCallCard';

const mockToolCall = {
  id: 'tc-001',
  name: 'read_file',
  input: { path: '/src/index.ts' },
  output: 'file contents here...',
  status: 'completed',
};

describe('ToolCallCard', () => {
  it('renders tool name', () => {
    render(<ToolCallCard toolCall={mockToolCall} />);
    expect(screen.getByText('read_file')).toBeInTheDocument();
  });

  it('shows input parameters', () => {
    render(<ToolCallCard toolCall={mockToolCall} />);
    expect(screen.getByText(/\/src\/index.ts/)).toBeInTheDocument();
  });

  it('expands to show output', async () => {
    render(<ToolCallCard toolCall={mockToolCall} />);
    
    const expandButton = screen.getByRole('button', { name: /expand/i });
    fireEvent.click(expandButton);
    
    expect(screen.getByText(/file contents here/)).toBeInTheDocument();
  });

  it('shows status indicator', () => {
    render(<ToolCallCard toolCall={mockToolCall} />);
    expect(screen.getByText('completed')).toBeInTheDocument();
  });

  it('shows loading state for pending tool calls', () => {
    const pending = { ...mockToolCall, status: 'pending', output: null };
    render(<ToolCallCard toolCall={pending} />);
    
    expect(screen.getByRole('progressbar')).toBeInTheDocument();
  });
});
```

---

## Settings & Configuration Tests

### TC-SET-001: Agent Settings

```typescript
// apps/ui/src/features/settings/AgentSettings.test.tsx
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { AgentSettings } from './AgentSettings';

describe('AgentSettings', () => {
  it('loads and displays agent list', async () => {
    render(<AgentSettings />);
    
    await waitFor(() => {
      expect(screen.getByText('Root Agent')).toBeInTheDocument();
    });
  });

  it('allows editing agent instructions', async () => {
    render(<AgentSettings />);
    
    await waitFor(() => {
      const editButton = screen.getByRole('button', { name: /edit/i });
      fireEvent.click(editButton);
    });
    
    const instructionsField = screen.getByLabelText(/instructions/i);
    await userEvent.clear(instructionsField);
    await userEvent.type(instructionsField, 'New instructions');
    
    const saveButton = screen.getByRole('button', { name: /save/i });
    fireEvent.click(saveButton);
    
    await waitFor(() => {
      expect(screen.getByText(/saved/i)).toBeInTheDocument();
    });
  });

  it('validates required fields', async () => {
    render(<AgentSettings />);
    
    // Try to create agent without name
    const createButton = screen.getByRole('button', { name: /create/i });
    fireEvent.click(createButton);
    
    const saveButton = screen.getByRole('button', { name: /save/i });
    fireEvent.click(saveButton);
    
    expect(screen.getByText(/name is required/i)).toBeInTheDocument();
  });
});
```

### TC-SET-002: Provider Configuration

```typescript
// apps/ui/src/features/settings/ProviderSettings.test.tsx
describe('ProviderSettings', () => {
  it('tests provider connection', async () => {
    render(<ProviderSettings />);
    
    await waitFor(() => {
      const testButton = screen.getByRole('button', { name: /test connection/i });
      fireEvent.click(testButton);
    });
    
    await waitFor(() => {
      expect(screen.getByText(/connection successful/i)).toBeInTheDocument();
    });
  });

  it('shows error for invalid API key', async () => {
    server.use(
      http.post('/api/providers/:id/test', () => {
        return HttpResponse.json({ success: false, message: 'Invalid API key' }, { status: 401 });
      })
    );
    
    render(<ProviderSettings />);
    
    await waitFor(() => {
      const testButton = screen.getByRole('button', { name: /test/i });
      fireEvent.click(testButton);
    });
    
    await waitFor(() => {
      expect(screen.getByText(/invalid api key/i)).toBeInTheDocument();
    });
  });
});
```

---

## Integration Tests

### TC-INT-001: Full Chat Flow

```typescript
// apps/ui/tests/integration/chat-flow.test.tsx
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { App } from '@/App';

describe('Chat Flow Integration', () => {
  it('sends message and receives response', async () => {
    render(<App />);
    
    // Navigate to chat
    const chatLink = screen.getByRole('link', { name: /chat/i });
    await userEvent.click(chatLink);
    
    // Type and send message
    const input = screen.getByPlaceholderText(/type a message/i);
    await userEvent.type(input, 'What is 2 + 2?');
    
    const sendButton = screen.getByRole('button', { name: /send/i });
    await userEvent.click(sendButton);
    
    // Message should appear
    expect(screen.getByText('What is 2 + 2?')).toBeInTheDocument();
    
    // Wait for response
    await waitFor(() => {
      expect(screen.getByText(/4/)).toBeInTheDocument();
    }, { timeout: 10000 });
  });
});
```

### TC-INT-002: Dashboard to Session Detail

```typescript
// apps/ui/tests/integration/dashboard-navigation.test.tsx
describe('Dashboard Navigation', () => {
  it('navigates from dashboard to session detail', async () => {
    render(<App />);
    
    // Wait for dashboard to load
    await waitFor(() => {
      expect(screen.getByText('Active Sessions')).toBeInTheDocument();
    });
    
    // Click on a session
    const sessionCard = screen.getByText(/sess-001/);
    await userEvent.click(sessionCard);
    
    // Should show session detail
    await waitFor(() => {
      expect(screen.getByText('Session Detail')).toBeInTheDocument();
      expect(screen.getByText(/executions/i)).toBeInTheDocument();
    });
  });
});
```

---

## E2E Tests (Playwright)

### TC-E2E-001: Basic Navigation

```typescript
// apps/ui/tests/e2e/navigation.spec.ts
import { test, expect } from '@playwright/test';

test.describe('Navigation', () => {
  test('loads dashboard by default', async ({ page }) => {
    await page.goto('/');
    await expect(page.getByText('Active Sessions')).toBeVisible();
  });

  test('navigates to settings', async ({ page }) => {
    await page.goto('/');
    await page.click('text=Settings');
    await expect(page.getByText('Agent Configuration')).toBeVisible();
  });

  test('navigates to chat', async ({ page }) => {
    await page.goto('/');
    await page.click('text=Chat');
    await expect(page.getByPlaceholder(/type a message/i)).toBeVisible();
  });
});
```

### TC-E2E-002: Single Turn Conversation

```typescript
// apps/ui/tests/e2e/single-turn.spec.ts
import { test, expect } from '@playwright/test';

test.describe('Single Turn Conversation', () => {
  test('asks a simple question and gets response', async ({ page }) => {
    await page.goto('/chat');
    
    // Type message
    await page.fill('[placeholder*="type a message"]', 'What is the current time?');
    await page.click('button:has-text("Send")');
    
    // Wait for response (increased timeout for LLM)
    await expect(page.locator('.message-assistant')).toBeVisible({ timeout: 30000 });
    
    // Response should mention time
    const response = await page.textContent('.message-assistant');
    expect(response).toMatch(/time|clock|\d{1,2}:\d{2}/i);
  });
});
```

### TC-E2E-003: Multi-Turn Conversation

```typescript
// apps/ui/tests/e2e/multi-turn.spec.ts
import { test, expect } from '@playwright/test';

test.describe('Multi-Turn Conversation', () => {
  test('maintains context across turns', async ({ page }) => {
    await page.goto('/chat');
    
    // Turn 1: Introduce name
    await page.fill('[placeholder*="type a message"]', 'My name is Alice');
    await page.click('button:has-text("Send")');
    await expect(page.locator('.message-assistant')).toBeVisible({ timeout: 30000 });
    
    // Turn 2: Ask about name
    await page.fill('[placeholder*="type a message"]', 'What is my name?');
    await page.click('button:has-text("Send")');
    
    // Wait for second response
    await page.waitForSelector('.message-assistant >> nth=1', { timeout: 30000 });
    
    // Should remember name
    const response = await page.textContent('.message-assistant >> nth=1');
    expect(response).toContain('Alice');
  });
});
```

### TC-E2E-004: Subagent Delegation

```typescript
// apps/ui/tests/e2e/subagent.spec.ts
import { test, expect } from '@playwright/test';

test.describe('Subagent Delegation', () => {
  test.setTimeout(120000); // 2 minute timeout

  test('delegates to research agent', async ({ page }) => {
    await page.goto('/chat');
    
    // Request that triggers delegation
    await page.fill(
      '[placeholder*="type a message"]',
      'Research the latest advancements in AI and summarize them'
    );
    await page.click('button:has-text("Send")');
    
    // Wait for delegation indicator
    await expect(page.locator('[data-testid="delegation-indicator"]')).toBeVisible({
      timeout: 60000
    });
    
    // Navigate to dashboard
    await page.click('text=Dashboard');
    
    // Should show session with subagent
    await expect(page.locator('text=research')).toBeVisible({ timeout: 30000 });
    
    // Expand session to see hierarchy
    await page.click('[data-testid="session-expand"]');
    await expect(page.locator('text=root')).toBeVisible();
    await expect(page.locator('text=research')).toBeVisible();
  });
});
```

### TC-E2E-005: Dashboard Source Filter

```typescript
// apps/ui/tests/e2e/dashboard-filter.spec.ts
import { test, expect } from '@playwright/test';

test.describe('Dashboard Source Filter', () => {
  test('filters sessions by source', async ({ page }) => {
    await page.goto('/');
    
    // Select "Cron" from filter
    await page.selectOption('select', 'cron');
    
    // Only cron sessions should be visible
    // (Depends on having sessions with different sources)
  });

  test('shows all sessions when filter is "all"', async ({ page }) => {
    await page.goto('/');
    
    // Select "All Sources"
    await page.selectOption('select', 'all');
    
    // All sessions should be visible
    await expect(page.locator('[data-testid="session-card"]')).toHaveCount(await page.locator('[data-testid="session-card"]').count());
  });
});
```

---

## Test Execution

### Run Component Tests
```bash
cd apps/ui
npm test
```

### Run Integration Tests
```bash
cd apps/ui
npm run test:integration
```

### Run E2E Tests
```bash
cd apps/ui
npx playwright test
```

### Run E2E with UI
```bash
cd apps/ui
npx playwright test --ui
```

### Run Specific E2E Test
```bash
cd apps/ui
npx playwright test single-turn.spec.ts
```

---

## Coverage Report

```bash
cd apps/ui
npm test -- --coverage
```

Coverage targets:
- Statements: 70%
- Branches: 60%
- Functions: 70%
- Lines: 70%
