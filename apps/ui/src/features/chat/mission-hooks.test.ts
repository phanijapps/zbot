import { afterEach, describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';
import type { LogSession } from '@/services/transport/types';
import {
  handleToolCallRespond,
  handleTurnComplete,
  handleWardChanged,
  handleAgentCompleted,
  switchToSession,
  timeAgo,
  useRecentSessions,
  type EventHandlerCtx,
  type NarrativeBlock,
} from './mission-hooks';

// ---------------------------------------------------------------------------
// Transport mock — captures listLogSessions for useRecentSessions tests.
// Other handlers don't touch transport.
// ---------------------------------------------------------------------------

const listLogSessionsMock = vi.fn<
  (filter?: { limit?: number; root_only?: boolean }) => Promise<{
    success: boolean;
    data?: LogSession[];
    error?: string;
  }>
>();

vi.mock('@/services/transport', async () => {
  const actual = await vi.importActual<Record<string, unknown>>('@/services/transport');
  return {
    ...actual,
    getTransport: async () => ({
      listLogSessions: listLogSessionsMock,
    }),
  };
});

// ============================================================================
// Test harness — synthesizes a partial EventHandlerCtx with spy-able setters.
// ============================================================================

interface HarnessState {
  blocks: NarrativeBlock[];
  activeWard: { name: string; content: string } | null;
  phase: string;
  status: string;
  artifactsBumps: number;
  durationStopped: boolean;
}

function makeCtx(initial: Partial<HarnessState> = {}): {
  ctx: EventHandlerCtx;
  state: HarnessState;
} {
  const state: HarnessState = {
    blocks: initial.blocks ?? [],
    activeWard: initial.activeWard ?? null,
    phase: initial.phase ?? 'idle',
    status: initial.status ?? 'idle',
    artifactsBumps: 0,
    durationStopped: false,
  };

  const apply = <T,>(current: T, next: T | ((prev: T) => T)): T =>
    typeof next === 'function' ? (next as (p: T) => T)(current) : next;

  const ctx = {
    setBlocks: (u: NarrativeBlock[] | ((p: NarrativeBlock[]) => NarrativeBlock[])) => {
      state.blocks = apply(state.blocks, u);
    },
    setStatus: (u: string | ((p: string) => string)) => {
      state.status = apply(state.status, u);
    },
    setPhase: (u: string | ((p: string) => string)) => {
      state.phase = apply(state.phase, u);
    },
    setActiveWard: (u: HarnessState['activeWard'] | ((p: HarnessState['activeWard']) => HarnessState['activeWard'])) => {
      state.activeWard = apply(state.activeWard, u);
    },
    // Unused in the handlers we test — stub to satisfy the type.
    setSessionTitle: vi.fn(),
    setTokenCount: vi.fn(),
    setModelName: vi.fn(),
    setSubagents: vi.fn(),
    setPlan: vi.fn(),
    setRecalledFacts: vi.fn(),
    setIntentAnalysis: vi.fn(),
    setActiveSessionId: vi.fn(),
    toolCallBlockMapRef: { current: new Map() },
    executionAgentMapRef: { current: new Map() },
    titleFallbackTimerRef: { current: null },
    lastUserMessageRef: { current: '' },
    streamingBufferRef: { current: '' },
    rafIdRef: { current: null },
    flushTokenBuffer: vi.fn(),
    startDurationTimer: vi.fn(),
    stopDurationTimer: () => {
      state.durationStopped = true;
    },
    generateFallbackTitle: vi.fn(() => 'fallback'),
    bumpArtifactsRefresh: () => {
      state.artifactsBumps += 1;
    },
    activeSessionIdRef: { current: 'sess-test' },
  } as unknown as EventHandlerCtx;

  return { ctx, state };
}

// ============================================================================
// Bug 5 — final response not shown live (tool_call respond + turn_complete race)
// ============================================================================

describe('handleToolCallRespond + handleTurnComplete — final response rendering', () => {
  beforeEach(() => {
    // stable UUIDs for deterministic assertions (not strictly required)
  });

  it('marks the respond block as streaming so turn_complete updates it instead of duplicating', () => {
    const { ctx, state } = makeCtx();

    handleToolCallRespond({ message: 'Here is the answer.' }, ctx);

    expect(state.blocks).toHaveLength(1);
    expect(state.blocks[0].type).toBe('response');
    expect(state.blocks[0].isStreaming).toBe(true);
    expect(state.phase).toBe('responding');

    // Now turn_complete arrives with the same final message.
    handleTurnComplete({ type: 'turn_complete', final_message: 'Here is the answer.' } as any, ctx);

    // One block — the existing one finalized, not a duplicate.
    expect(state.blocks).toHaveLength(1);
    expect(state.blocks[0].type).toBe('response');
    expect(state.blocks[0].isStreaming).toBe(false);
    expect(state.blocks[0].data.content).toBe('Here is the answer.');
  });

  it('turn_complete creates a response when no prior respond block exists', () => {
    const { ctx, state } = makeCtx();

    handleTurnComplete({ type: 'turn_complete', final_message: 'Direct final.' } as any, ctx);

    expect(state.blocks).toHaveLength(1);
    expect(state.blocks[0].type).toBe('response');
    expect(state.blocks[0].data.content).toBe('Direct final.');
    expect(state.blocks[0].isStreaming).toBe(false);
  });

  it('handleToolCallRespond with empty message does not create a block but still sets phase', () => {
    const { ctx, state } = makeCtx();

    handleToolCallRespond({ message: '' }, ctx);

    expect(state.blocks).toHaveLength(0);
    expect(state.phase).toBe('responding');
  });
});

// ============================================================================
// Bug 1 — ward flips to "Unknown" after intent analysis populated it
// ============================================================================

describe('handleWardChanged — preserves richer ward name from intent analysis', () => {
  it('does not overwrite an already-populated ward name with the raw ward_id', () => {
    const { ctx, state } = makeCtx({
      activeWard: { name: 'news-research', content: 'Picked because topic is India elections.' },
    });

    handleWardChanged({ type: 'ward_changed', ward_id: 'news-research' } as any, ctx);

    // Still the richer value — not the bare id.
    expect(state.activeWard).toEqual({
      name: 'news-research',
      content: 'Picked because topic is India elections.',
    });
  });

  it('sets ward from ward_id when no prior ward exists', () => {
    const { ctx, state } = makeCtx({ activeWard: null });

    handleWardChanged({ type: 'ward_changed', ward_id: 'research' } as any, ctx);

    expect(state.activeWard).toEqual({ name: 'research', content: '' });
  });

  it('ignores events with empty ward_id', () => {
    const { ctx, state } = makeCtx({
      activeWard: { name: 'keep-me', content: 'prior' },
    });

    handleWardChanged({ type: 'ward_changed', ward_id: '' } as any, ctx);

    expect(state.activeWard).toEqual({ name: 'keep-me', content: 'prior' });
  });
});

// ============================================================================
// Bug 4 — artifacts not fetched on agent_completed
// ============================================================================

describe('handleAgentCompleted — triggers artifacts refresh', () => {
  it('bumps the artifacts refresh signal exactly once per completion', () => {
    const { ctx, state } = makeCtx();

    handleAgentCompleted({ type: 'agent_completed' } as any, ctx);

    expect(state.artifactsBumps).toBe(1);
    expect(state.status).toBe('completed');
    expect(state.phase).toBe('completed');
    expect(state.durationStopped).toBe(true);
  });

  it('does not add a duplicate response block when one already exists', () => {
    const { ctx, state } = makeCtx({
      blocks: [
        {
          id: 'existing',
          type: 'response',
          timestamp: 't',
          data: { content: 'already here' },
          isStreaming: false,
        },
      ],
    });

    handleAgentCompleted({ type: 'agent_completed', result: 'some result' } as any, ctx);

    expect(state.blocks).toHaveLength(1);
    expect(state.blocks[0].data.content).toBe('already here');
    // Still refreshes artifacts.
    expect(state.artifactsBumps).toBe(1);
  });

  it('creates a response block from result when none exists', () => {
    const { ctx, state } = makeCtx();

    handleAgentCompleted({ type: 'agent_completed', result: 'final text' } as any, ctx);

    expect(state.blocks).toHaveLength(1);
    expect(state.blocks[0].type).toBe('response');
    expect(state.blocks[0].data.content).toBe('final text');
    expect(state.artifactsBumps).toBe(1);
  });
});

// ============================================================================
// timeAgo — relative-time formatter for recent-session cards
// ============================================================================

describe('timeAgo', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2026-04-24T12:00:00Z'));
  });
  afterEach(() => vi.useRealTimers());

  it("returns 'just now' for sub-minute deltas (and zero / future)", () => {
    expect(timeAgo('2026-04-24T11:59:30Z')).toBe('just now');
    expect(timeAgo('2026-04-24T12:00:00Z')).toBe('just now');
    expect(timeAgo('2026-04-25T00:00:00Z')).toBe('just now'); // future → diff negative → < 1
  });

  it('formats minutes between 1 and 59', () => {
    expect(timeAgo('2026-04-24T11:59:00Z')).toBe('1m ago');
    expect(timeAgo('2026-04-24T11:30:00Z')).toBe('30m ago');
    expect(timeAgo('2026-04-24T11:01:00Z')).toBe('59m ago');
  });

  it('formats hours between 1 and 23', () => {
    expect(timeAgo('2026-04-24T11:00:00Z')).toBe('1h ago');
    expect(timeAgo('2026-04-23T13:00:00Z')).toBe('23h ago');
  });

  it('formats days for any age ≥ 24h', () => {
    expect(timeAgo('2026-04-23T12:00:00Z')).toBe('1d ago');
    expect(timeAgo('2026-04-17T12:00:00Z')).toBe('7d ago');
  });
});

// ============================================================================
// switchToSession — session-id rotation + reload trigger
// ============================================================================

describe('switchToSession', () => {
  let reloadSpy: ReturnType<typeof vi.fn>;
  let originalLocation: Location;

  beforeEach(() => {
    localStorage.clear();
    reloadSpy = vi.fn();
    // jsdom seals window.location.reload — redefine the whole `location`
    // object instead. Save and restore so other tests aren't affected.
    originalLocation = window.location;
    Object.defineProperty(window, 'location', {
      configurable: true,
      writable: true,
      value: { ...originalLocation, reload: reloadSpy },
    });
  });

  afterEach(() => {
    Object.defineProperty(window, 'location', {
      configurable: true,
      writable: true,
      value: originalLocation,
    });
  });

  it('writes conversation_id to both session and conv keys, exec id to log key, then reloads', () => {
    switchToSession('exec-XYZ', 'sess-CONV-1');
    // The frontend tracks the conversation_id under two legacy keys for
    // back-compat with sendMessage's resume path.
    expect(localStorage.getItem('agentzero_web_session_id')).toBe('sess-CONV-1');
    expect(localStorage.getItem('agentzero_web_conv_id')).toBe('sess-CONV-1');
    // The exec id stays separate so log loaders can still find the row.
    expect(localStorage.getItem('agentzero_log_session_id')).toBe('exec-XYZ');
    expect(reloadSpy).toHaveBeenCalledTimes(1);
  });
});

// ============================================================================
// useRecentSessions — the exclude predicate + over-fetch logic
// ============================================================================

function makeRow(overrides: Partial<LogSession> & { conversation_id: string }): LogSession {
  return {
    session_id: `exec-${overrides.conversation_id}`,
    agent_id: 'root',
    agent_name: 'root',
    title: 'test',
    started_at: '2026-04-24T10:00:00Z',
    status: 'completed',
    token_count: 0,
    tool_call_count: 0,
    error_count: 0,
    child_session_ids: [],
    ...overrides,
  };
}

describe('useRecentSessions', () => {
  beforeEach(() => listLogSessionsMock.mockReset());

  it('requests limit=5 when no exclude predicate is provided', async () => {
    listLogSessionsMock.mockResolvedValue({
      success: true,
      data: Array.from({ length: 3 }, (_, i) => makeRow({ conversation_id: `s-${i}` })),
    });

    const { result } = renderHook(() => useRecentSessions());
    await waitFor(() => expect(result.current.sessions.length).toBe(3));
    expect(listLogSessionsMock).toHaveBeenCalledWith({ limit: 5, root_only: true });
  });

  it('over-fetches limit=20 when an exclude predicate is provided', async () => {
    listLogSessionsMock.mockResolvedValue({ success: true, data: [] });

    renderHook(() => useRecentSessions({ exclude: () => false }));
    await waitFor(() => expect(listLogSessionsMock).toHaveBeenCalled());
    expect(listLogSessionsMock).toHaveBeenCalledWith({ limit: 20, root_only: true });
  });

  it('filters via the exclude predicate and caps the final slice at 5', async () => {
    // 12 rows; exclude every odd-indexed one. Survivors = 6 → must cap to 5.
    listLogSessionsMock.mockResolvedValue({
      success: true,
      data: Array.from({ length: 12 }, (_, i) => makeRow({ conversation_id: `s-${i}` })),
    });
    const exclude = (row: LogSession) =>
      Number.parseInt(row.conversation_id.split('-')[1], 10) % 2 === 1;

    const { result } = renderHook(() => useRecentSessions({ exclude }));
    await waitFor(() => expect(result.current.sessions.length).toBe(5));
    for (const s of result.current.sessions) {
      const idx = Number.parseInt(s.conversation_id.split('-')[1], 10);
      expect(idx % 2).toBe(0); // only even-indexed survived
    }
  });

  it('keeps sessions empty when transport reports failure', async () => {
    listLogSessionsMock.mockResolvedValue({ success: false, error: 'boom' });

    const { result } = renderHook(() => useRecentSessions());
    await waitFor(() => expect(listLogSessionsMock).toHaveBeenCalled());
    expect(result.current.sessions).toEqual([]);
  });

  it('keeps sessions empty when transport returns success but no data', async () => {
    listLogSessionsMock.mockResolvedValue({ success: true });

    const { result } = renderHook(() => useRecentSessions());
    await waitFor(() => expect(listLogSessionsMock).toHaveBeenCalled());
    expect(result.current.sessions).toEqual([]);
  });
});
