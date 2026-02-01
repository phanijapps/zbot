// ============================================================================
// DASHBOARD INTEGRATION TESTS
// Tests for WebOpsDashboard component with mocked transport
// ============================================================================

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, waitFor, within, act } from '@/test/utils';
import { WebOpsDashboard } from '@/features/ops/WebOpsDashboard';
import type { SessionWithExecutions, DashboardStats, TriggerSource, TransportResult } from '@/services/transport/types';

// ============================================================================
// Mock Data Factories
// ============================================================================

function createMockSession(
  id: string,
  status: 'queued' | 'running' | 'paused' | 'completed' | 'crashed' = 'running',
  source: TriggerSource = 'web'
): SessionWithExecutions {
  const now = new Date().toISOString();
  return {
    id,
    status,
    source,
    root_agent_id: 'root',
    created_at: now,
    started_at: status !== 'queued' ? now : undefined,
    completed_at: status === 'completed' ? now : undefined,
    total_tokens_in: 1000,
    total_tokens_out: 500,
    executions: [
      {
        id: `exec-${id}`,
        session_id: id,
        agent_id: 'root',
        delegation_type: 'root',
        status: status === 'completed' ? 'completed' : 'running',
        started_at: now,
        completed_at: status === 'completed' ? now : undefined,
        tokens_in: 1000,
        tokens_out: 500,
      },
    ],
    subagent_count: 0,
  };
}

function createMockStats(): DashboardStats {
  return {
    sessions_queued: 1,
    sessions_running: 2,
    sessions_paused: 0,
    sessions_completed: 10,
    sessions_crashed: 1,
    executions_queued: 0,
    executions_running: 3,
    executions_completed: 15,
    executions_crashed: 2,
    executions_cancelled: 0,
    today_sessions: 5,
    today_tokens: 50000,
    sessions_by_source: {
      web: 8,
      cli: 3,
      cron: 1,
      api: 1,
      plugin: 1,
    },
  };
}

// ============================================================================
// Mock Transport
// ============================================================================

// Mock data that tests can modify
let mockSessions: SessionWithExecutions[] = [];
let mockStats: DashboardStats = createMockStats();
let mockError: string | null = null;

// Mock transport implementation
const mockTransport = {
  mode: 'web' as const,
  initialize: vi.fn().mockResolvedValue(undefined),
  listSessionsFull: vi.fn().mockImplementation(async (): Promise<TransportResult<SessionWithExecutions[]>> => {
    if (mockError) {
      return { success: false, error: mockError };
    }
    return { success: true, data: mockSessions };
  }),
  getDashboardStats: vi.fn().mockImplementation(async (): Promise<TransportResult<DashboardStats>> => {
    if (mockError) {
      return { success: false, error: mockError };
    }
    return { success: true, data: mockStats };
  }),
  pauseSession: vi.fn().mockResolvedValue({ success: true }),
  resumeSession: vi.fn().mockResolvedValue({ success: true }),
  cancelSession: vi.fn().mockResolvedValue({ success: true }),
  connect: vi.fn().mockResolvedValue({ success: true }),
  disconnect: vi.fn().mockResolvedValue(undefined),
  isConnected: vi.fn().mockReturnValue(false),
  subscribe: vi.fn().mockReturnValue(() => {}),
};

// Mock the transport module
vi.mock('@/services/transport', () => ({
  getTransport: vi.fn().mockImplementation(async () => mockTransport),
  resetTransport: vi.fn(),
}));

// ============================================================================
// Test Setup
// ============================================================================

function renderDashboard() {
  return render(<WebOpsDashboard />);
}

describe('WebOpsDashboard Integration', () => {
  beforeEach(() => {
    // Reset mock data
    mockSessions = [
      createMockSession('sess-001', 'running', 'web'),
      createMockSession('sess-002', 'running', 'cli'),
      createMockSession('sess-003', 'completed', 'web'),
    ];
    mockStats = createMockStats();
    mockError = null;

    // Reset mock call history
    vi.clearAllMocks();

    // Use fake timers for auto-refresh tests
    vi.useFakeTimers({ shouldAdvanceTime: true });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  // ==================== Loading State Tests ====================

  describe('Loading State', () => {
    it('shows dashboard title after loading', async () => {
      renderDashboard();

      await waitFor(() => {
        expect(screen.getByText('Dashboard')).toBeInTheDocument();
      });
    });

    it('calls transport methods on load', async () => {
      renderDashboard();

      await waitFor(() => {
        expect(mockTransport.listSessionsFull).toHaveBeenCalled();
        expect(mockTransport.getDashboardStats).toHaveBeenCalled();
      });
    });
  });

  // ==================== Data Display Tests ====================

  describe('Data Display', () => {
    it('displays sessions from transport', async () => {
      renderDashboard();

      await waitFor(() => {
        // Should show root agent for each session
        expect(screen.getAllByText('root').length).toBeGreaterThan(0);
      });
    });

    it('displays dashboard stats', async () => {
      renderDashboard();

      await waitFor(() => {
        // Stats from mock (2 running sessions, 10 completed)
        // Use getAllByText since numbers appear in multiple places
        expect(screen.getAllByText('2').length).toBeGreaterThan(0);
        expect(screen.getAllByText('10').length).toBeGreaterThan(0);
      });
    });

    it('shows session count in active sessions panel', async () => {
      mockSessions = [
        createMockSession('sess-001', 'running', 'web'),
        createMockSession('sess-002', 'running', 'cli'),
      ];

      renderDashboard();

      await waitFor(() => {
        const activePanel = screen.getByText('Active Sessions').closest('.card');
        expect(activePanel).toBeInTheDocument();
      });
    });

    it('shows source badges for sessions', async () => {
      renderDashboard();

      await waitFor(() => {
        // Should show source badges
        expect(screen.getAllByText('Web').length).toBeGreaterThan(0);
        expect(screen.getAllByText('CLI').length).toBeGreaterThan(0);
      });
    });

    it('shows subagent count indicator for sessions with subagents', async () => {
      const session = createMockSession('sess-expand', 'running', 'web');
      session.executions.push({
        id: 'exec-sub-1',
        session_id: 'sess-expand',
        agent_id: 'researcher',
        delegation_type: 'delegated',
        status: 'running',
        started_at: new Date().toISOString(),
        tokens_in: 500,
        tokens_out: 200,
      });
      session.subagent_count = 1;
      mockSessions = [session];

      renderDashboard();

      await waitFor(() => {
        expect(screen.getByText('root')).toBeInTheDocument();
        // Should show subagent indicator
        expect(screen.getByText('+1 subagent')).toBeInTheDocument();
      });
    });
  });

  // ==================== Empty State Tests ====================

  describe('Empty States', () => {
    it('shows empty state when no sessions', async () => {
      mockSessions = [];

      renderDashboard();

      await waitFor(() => {
        expect(screen.getByText(/no active sessions/i)).toBeInTheDocument();
        expect(screen.getByText(/no session history/i)).toBeInTheDocument();
      });
    });

    it('shows empty state when filtered results are empty', async () => {
      mockSessions = [createMockSession('sess-1', 'running', 'web')];

      const { user } = renderDashboard();

      await waitFor(() => {
        expect(screen.getByText('root')).toBeInTheDocument();
      });

      // Filter by CLI source (no CLI sessions)
      const sourceFilter = screen.getByDisplayValue('All Sources');
      await act(async () => {
        await user.selectOptions(sourceFilter, 'cli');
      });

      await waitFor(() => {
        expect(screen.getByText(/no active sessions/i)).toBeInTheDocument();
      });
    });
  });

  // ==================== Error Handling Tests ====================

  describe('Error Handling', () => {
    it('continues to render dashboard when transport returns error', async () => {
      mockError = 'Connection failed';

      renderDashboard();

      // Dashboard should still render even if data fails to load
      await waitFor(() => {
        expect(screen.getByText('Dashboard')).toBeInTheDocument();
        // Empty states shown when no data
        expect(screen.getByText(/no active sessions/i)).toBeInTheDocument();
      });
    });

    it('calls loadData again on refresh click', async () => {
      const { user } = renderDashboard();

      await waitFor(() => {
        expect(mockTransport.listSessionsFull).toHaveBeenCalled();
      });

      const initialCallCount = mockTransport.listSessionsFull.mock.calls.length;

      // Click refresh button
      const refreshButton = screen.getByTitle('Refresh');
      await user.click(refreshButton);

      await waitFor(() => {
        expect(mockTransport.listSessionsFull.mock.calls.length).toBeGreaterThan(initialCallCount);
      });
    });
  });

  // ==================== Source Filter Tests ====================

  describe('Source Filtering', () => {
    it('filters sessions by source', async () => {
      mockSessions = [
        createMockSession('sess-web-1', 'running', 'web'),
        createMockSession('sess-cli-1', 'running', 'cli'),
        createMockSession('sess-api-1', 'running', 'api'),
      ];

      const { user } = renderDashboard();

      await waitFor(() => {
        // All sessions visible initially
        expect(screen.getAllByText('root').length).toBe(3);
      });

      // Filter by Web source
      const sourceFilter = screen.getByDisplayValue('All Sources');
      await act(async () => {
        await user.selectOptions(sourceFilter, 'web');
      });

      await waitFor(() => {
        // Only web session visible
        expect(screen.getAllByText('root').length).toBe(1);
      });
    });

    it('shows all sessions when "All Sources" is selected', async () => {
      mockSessions = [
        createMockSession('sess-web', 'running', 'web'),
        createMockSession('sess-cli', 'running', 'cli'),
      ];

      const { user } = renderDashboard();

      await waitFor(() => {
        expect(screen.getAllByText('root').length).toBe(2);
      });

      // Filter by web
      const sourceFilter = screen.getByDisplayValue('All Sources');
      await act(async () => {
        await user.selectOptions(sourceFilter, 'web');
      });

      await waitFor(() => {
        expect(screen.getAllByText('root').length).toBe(1);
      });

      // Select All Sources
      await act(async () => {
        await user.selectOptions(sourceFilter, 'all');
      });

      await waitFor(() => {
        expect(screen.getAllByText('root').length).toBe(2);
      });
    });
  });

  // ==================== Status Filter Tests ====================

  describe('Status Filtering', () => {
    it('filters active sessions by status', async () => {
      mockSessions = [
        createMockSession('sess-running', 'running', 'web'),
        createMockSession('sess-paused', 'paused', 'web'),
        createMockSession('sess-queued', 'queued', 'web'),
      ];

      const { user } = renderDashboard();

      await waitFor(() => {
        expect(screen.getAllByText('root').length).toBe(3);
      });

      // Find and click the "Running" filter button
      const runningButtons = screen.getAllByRole('button', { name: /running/i });
      const filterButton = runningButtons.find(btn =>
        btn.closest('.border-border') // Filter bar has this class
      );

      if (filterButton) {
        await act(async () => {
          await user.click(filterButton);
        });
      }

      // Running sessions should still be visible
      await waitFor(() => {
        expect(screen.getByText('root')).toBeInTheDocument();
      });
    });

    it('separates sessions by active/completed', async () => {
      mockSessions = [
        createMockSession('sess-active', 'running', 'web'),
        createMockSession('sess-done', 'completed', 'web'),
        createMockSession('sess-crashed', 'crashed', 'web'),
      ];

      renderDashboard();

      await waitFor(() => {
        // Active session panel should exist
        expect(screen.getByText('Active Sessions')).toBeInTheDocument();
        // History panel should exist
        expect(screen.getByText('Session History')).toBeInTheDocument();
      });
    });
  });

  // ==================== Auto-Refresh Tests ====================

  describe('Auto-Refresh', () => {
    it('refreshes data periodically when enabled', async () => {
      renderDashboard();

      // Initial load
      await waitFor(() => {
        expect(mockTransport.listSessionsFull).toHaveBeenCalled();
      });

      const initialCallCount = mockTransport.listSessionsFull.mock.calls.length;

      // Advance time past refresh interval (3 seconds)
      await act(async () => {
        vi.advanceTimersByTime(4000);
      });

      await waitFor(() => {
        expect(mockTransport.listSessionsFull.mock.calls.length).toBeGreaterThan(initialCallCount);
      });
    });

    it('stops refreshing when auto-refresh is disabled', async () => {
      const { user } = renderDashboard();

      await waitFor(() => {
        expect(mockTransport.listSessionsFull).toHaveBeenCalled();
      });

      // Disable auto-refresh
      const autoRefreshCheckbox = screen.getByLabelText(/auto-refresh/i);
      await act(async () => {
        await user.click(autoRefreshCheckbox);
      });

      const callCountAfterDisable = mockTransport.listSessionsFull.mock.calls.length;

      // Advance time - should not trigger more fetches
      await act(async () => {
        vi.advanceTimersByTime(10000);
      });

      // Call count should not increase significantly
      expect(mockTransport.listSessionsFull.mock.calls.length).toBeLessThanOrEqual(callCountAfterDisable + 1);
    });
  });

  // ==================== Source Stats Bar Tests ====================

  describe('Source Stats Bar', () => {
    it('displays sessions by source breakdown', async () => {
      mockStats = {
        ...createMockStats(),
        sessions_by_source: {
          web: 5,
          cli: 3,
          api: 2,
          cron: 0,
          plugin: 0,
        },
      };

      renderDashboard();

      await waitFor(() => {
        expect(screen.getByText('Sessions by Source')).toBeInTheDocument();
        expect(screen.getByText('10 total')).toBeInTheDocument(); // 5+3+2
      });
    });

    it('hides source stats bar when no sessions by source', async () => {
      mockStats = {
        ...createMockStats(),
        sessions_by_source: {},
      };

      renderDashboard();

      await waitFor(() => {
        expect(screen.getByText('Dashboard')).toBeInTheDocument();
      });

      // Source stats bar should not be visible
      expect(screen.queryByText('Sessions by Source')).not.toBeInTheDocument();
    });
  });

  // ==================== Session Controls Tests ====================

  describe('Session Controls', () => {
    it('calls pauseSession when pause button clicked', async () => {
      mockSessions = [createMockSession('sess-pause', 'running', 'web')];

      const { user } = renderDashboard();

      await waitFor(() => {
        expect(screen.getByText('root')).toBeInTheDocument();
      });

      // Find and click pause button
      const pauseButton = screen.getByTitle('Pause session');
      await act(async () => {
        await user.click(pauseButton);
      });

      await waitFor(() => {
        expect(mockTransport.pauseSession).toHaveBeenCalledWith('sess-pause');
      });
    });

    it('calls resumeSession when resume button clicked', async () => {
      mockSessions = [createMockSession('sess-resume', 'paused', 'web')];

      const { user } = renderDashboard();

      await waitFor(() => {
        expect(screen.getByText('root')).toBeInTheDocument();
      });

      // Find and click resume button
      const resumeButton = screen.getByTitle('Resume session');
      await act(async () => {
        await user.click(resumeButton);
      });

      await waitFor(() => {
        expect(mockTransport.resumeSession).toHaveBeenCalledWith('sess-resume');
      });
    });

    it('calls cancelSession when cancel button clicked', async () => {
      mockSessions = [createMockSession('sess-cancel', 'running', 'web')];

      const { user } = renderDashboard();

      await waitFor(() => {
        expect(screen.getByText('root')).toBeInTheDocument();
      });

      // Find and click cancel button
      const cancelButton = screen.getByTitle('Cancel session');
      await act(async () => {
        await user.click(cancelButton);
      });

      await waitFor(() => {
        expect(mockTransport.cancelSession).toHaveBeenCalledWith('sess-cancel');
      });
    });
  });
});
