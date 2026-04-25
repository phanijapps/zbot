import { afterEach, describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';
import type { LogSession } from '@/services/transport/types';
import {
  __testInternals,
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

const {
  handleAgentStarted,
  handleInvokeAccepted,
  handleTokenEvent,
  handleToolCallEvent,
  handleToolResultEvent,
  handleDelegationStarted,
  handleDelegationCompleted,
  handleDelegationError,
  handleSessionTitleChanged,
  handleIntentAnalysisStarted,
  handleIntentAnalysisComplete,
  handleErrorEvent,
  handleSystemOrMessage,
  parsePlanSteps,
  parseIntentAnalysisFromState,
} = __testInternals;

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
  sessionTitle: string;
  tokenCount: number;
  modelName: string;
  activeSessionId: string | null;
  subagents: Array<{ agentId: string; task: string; status: string }>;
  plan: Array<{ text: string; status: string }>;
  recalledFacts: Array<unknown>;
  intentAnalysis: unknown;
  artifactsBumps: number;
  durationStopped: boolean;
  durationStarted: boolean;
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
    sessionTitle: initial.sessionTitle ?? '',
    tokenCount: initial.tokenCount ?? 0,
    modelName: initial.modelName ?? '',
    activeSessionId: initial.activeSessionId ?? null,
    subagents: initial.subagents ?? [],
    plan: initial.plan ?? [],
    recalledFacts: initial.recalledFacts ?? [],
    intentAnalysis: initial.intentAnalysis ?? null,
    artifactsBumps: 0,
    durationStopped: false,
    durationStarted: false,
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
    setSessionTitle: (u: string | ((p: string) => string)) => {
      state.sessionTitle = apply(state.sessionTitle, u);
    },
    setTokenCount: (u: number | ((p: number) => number)) => {
      state.tokenCount = apply(state.tokenCount, u);
    },
    setModelName: (u: string | ((p: string) => string)) => {
      state.modelName = apply(state.modelName, u);
    },
    setSubagents: (u: HarnessState['subagents'] | ((p: HarnessState['subagents']) => HarnessState['subagents'])) => {
      state.subagents = apply(state.subagents, u);
    },
    setPlan: (u: HarnessState['plan'] | ((p: HarnessState['plan']) => HarnessState['plan'])) => {
      state.plan = apply(state.plan, u);
    },
    setRecalledFacts: (u: HarnessState['recalledFacts'] | ((p: HarnessState['recalledFacts']) => HarnessState['recalledFacts'])) => {
      state.recalledFacts = apply(state.recalledFacts, u);
    },
    setIntentAnalysis: (u: unknown | ((p: unknown) => unknown)) => {
      state.intentAnalysis = typeof u === 'function' ? (u as (p: unknown) => unknown)(state.intentAnalysis) : u;
    },
    setActiveSessionId: (u: string | null | ((p: string | null) => string | null)) => {
      state.activeSessionId = apply(state.activeSessionId, u);
    },
    toolCallBlockMapRef: { current: new Map<string, string>() },
    executionAgentMapRef: { current: new Map<string, string>() },
    titleFallbackTimerRef: { current: null as ReturnType<typeof setTimeout> | null },
    lastUserMessageRef: { current: '' },
    streamingBufferRef: { current: '' },
    rafIdRef: { current: null as number | null },
    flushTokenBuffer: vi.fn(),
    startDurationTimer: () => {
      state.durationStarted = true;
    },
    stopDurationTimer: () => {
      state.durationStopped = true;
    },
    generateFallbackTitle: vi.fn(() => 'fallback'),
    bumpArtifactsRefresh: () => {
      state.artifactsBumps += 1;
    },
    activeSessionIdRef: { current: 'sess-test' as string | null },
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

// ============================================================================
// Internal handlers — tested via `__testInternals` namespace export
// ============================================================================

describe('handleAgentStarted', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
    localStorage.clear();
  });

  it('flips status to running, starts the duration timer, persists session id, sets model', () => {
    const { ctx, state } = makeCtx();
    handleAgentStarted(
      { type: 'agent_started', session_id: 'sess-abc', model: 'gpt-test' } as any,
      ctx,
    );
    expect(state.status).toBe('running');
    expect(state.durationStarted).toBe(true);
    expect(state.modelName).toBe('gpt-test');
    expect(state.activeSessionId).toBe('sess-abc');
    expect(localStorage.getItem('agentzero_web_session_id')).toBe('sess-abc');
  });

  it('arms a fallback-title timer that fires after 10s if no title arrived', () => {
    const { ctx, state } = makeCtx();
    ctx.lastUserMessageRef.current = 'do the thing';
    handleAgentStarted({ type: 'agent_started' } as any, ctx);
    expect(state.sessionTitle).toBe('');
    vi.advanceTimersByTime(10_000);
    expect(state.sessionTitle).toBe('fallback');
  });

  it('cancels and replaces a prior fallback timer when called again', () => {
    const { ctx } = makeCtx();
    ctx.lastUserMessageRef.current = 'first message';
    handleAgentStarted({ type: 'agent_started' } as any, ctx);
    const first = ctx.titleFallbackTimerRef.current;
    expect(first).not.toBeNull();
    handleAgentStarted({ type: 'agent_started' } as any, ctx);
    const second = ctx.titleFallbackTimerRef.current;
    expect(second).not.toBeNull();
    expect(second).not.toBe(first);
  });
});

describe('handleInvokeAccepted', () => {
  afterEach(() => localStorage.clear());

  it('persists the session_id when present', () => {
    const { ctx, state } = makeCtx();
    handleInvokeAccepted({ type: 'invoke_accepted', session_id: 'sess-xyz' } as any, ctx);
    expect(state.activeSessionId).toBe('sess-xyz');
    expect(localStorage.getItem('agentzero_web_session_id')).toBe('sess-xyz');
  });

  it('is a no-op when session_id is missing', () => {
    const { ctx, state } = makeCtx();
    handleInvokeAccepted({ type: 'invoke_accepted' } as any, ctx);
    expect(state.activeSessionId).toBeNull();
  });
});

describe('handleTokenEvent', () => {
  beforeEach(() => {
    // Stub rAF so tests don't depend on real frame scheduling.
    vi.stubGlobal('requestAnimationFrame', vi.fn(() => 42));
    vi.stubGlobal('cancelAnimationFrame', vi.fn());
  });
  afterEach(() => vi.unstubAllGlobals());

  it('appends delta to the streaming buffer and schedules a flush', () => {
    const { ctx } = makeCtx();
    handleTokenEvent({ type: 'token', delta: 'Hello' } as any, ctx);
    expect(ctx.streamingBufferRef.current).toBe('Hello');
    expect(ctx.rafIdRef.current).toBe(42);
  });

  it('falls back to event.content when delta is absent', () => {
    const { ctx } = makeCtx();
    handleTokenEvent({ type: 'token', content: 'world' } as any, ctx);
    expect(ctx.streamingBufferRef.current).toBe('world');
  });

  it('uses total_tokens when present, otherwise increments by 1', () => {
    const { ctx, state } = makeCtx();
    handleTokenEvent({ type: 'token', delta: 'a', total_tokens: 42 } as any, ctx);
    expect(state.tokenCount).toBe(42);
    handleTokenEvent({ type: 'token', delta: 'b' } as any, ctx);
    expect(state.tokenCount).toBe(43); // increment-by-1 fallback
  });

  it('does not schedule rAF when delta is empty', () => {
    const { ctx } = makeCtx();
    handleTokenEvent({ type: 'token', delta: '' } as any, ctx);
    expect(ctx.rafIdRef.current).toBeNull();
  });
});

// ----------------------------------------------------------------------------
// handleToolCallEvent — dispatcher; covers the 6 sub-handlers in one shot
// ----------------------------------------------------------------------------

describe('handleToolCallEvent dispatcher', () => {
  it("routes 'set_session_title' → updates sessionTitle, clears fallback timer", () => {
    const { ctx, state } = makeCtx();
    ctx.titleFallbackTimerRef.current = setTimeout(() => {}, 1_000_000);
    handleToolCallEvent(
      { type: 'tool_call', tool: 'set_session_title', arguments: { title: 'My Session' } } as any,
      ctx,
    );
    expect(state.sessionTitle).toBe('My Session');
    expect(ctx.titleFallbackTimerRef.current).toBeNull();
  });

  it("routes memory(action='recall') → adds a recall block + maps tool_call_id → blockId", () => {
    const { ctx, state } = makeCtx();
    handleToolCallEvent(
      {
        type: 'tool_call',
        tool: 'memory',
        arguments: { action: 'recall' },
        tool_call_id: 'tc-1',
      } as any,
      ctx,
    );
    expect(state.blocks).toHaveLength(1);
    expect(state.blocks[0].type).toBe('recall');
    expect(ctx.toolCallBlockMapRef.current.get('tc-1')).toBe(state.blocks[0].id);
  });

  it("routes 'update_plan' → parses steps array and sets plan + plan block", () => {
    const { ctx, state } = makeCtx();
    handleToolCallEvent(
      {
        type: 'tool_call',
        tool: 'update_plan',
        arguments: {
          plan: [
            { step: 'one', status: 'completed' },
            { step: 'two', status: 'pending' },
          ],
        },
      } as any,
      ctx,
    );
    expect(state.plan).toEqual([
      { text: 'one', status: 'completed' },
      { text: 'two', status: 'pending' },
    ]);
    const planBlock = state.blocks.find((b) => b.type === 'plan');
    expect(planBlock).toBeTruthy();
  });

  it("routes 'delegate_to_agent' → adds delegation block + tracks subagent + transitions phase", () => {
    const { ctx, state } = makeCtx({ phase: 'planning' });
    handleToolCallEvent(
      {
        type: 'tool_call',
        tool: 'delegate_to_agent',
        arguments: { agent_id: 'writer-agent', task: 'compose summary' },
        tool_call_id: 'tc-2',
      } as any,
      ctx,
    );
    expect(state.blocks).toHaveLength(1);
    expect(state.blocks[0].type).toBe('delegation');
    expect(state.blocks[0].data.agentId).toBe('writer-agent');
    expect(state.subagents).toEqual([
      { agentId: 'writer-agent', task: 'compose summary', status: 'active' },
    ]);
    expect(state.phase).toBe('executing');
  });

  it('routes unknown tool names → generic tool block', () => {
    const { ctx, state } = makeCtx();
    handleToolCallEvent(
      {
        type: 'tool_call',
        tool: 'shell',
        arguments: { command: 'ls' },
        tool_call_id: 'tc-3',
      } as any,
      ctx,
    );
    expect(state.blocks).toHaveLength(1);
    expect(state.blocks[0].type).toBe('tool');
    expect(state.blocks[0].data.toolName).toBe('shell');
    expect(ctx.toolCallBlockMapRef.current.get('tc-3')).toBe(state.blocks[0].id);
  });
});

// ----------------------------------------------------------------------------
// handleToolResultEvent — dispatcher for tool-result branches
// ----------------------------------------------------------------------------

describe('handleToolResultEvent dispatcher', () => {
  it('updates a recall block with raw + parses recalled facts when JSON has results array', () => {
    const { ctx, state } = makeCtx();
    state.blocks = [
      { id: 'b-recall', type: 'recall', timestamp: 't', data: { raw: '' } },
    ];
    ctx.toolCallBlockMapRef.current.set('tc-1', 'b-recall');
    handleToolResultEvent(
      {
        type: 'tool_result',
        tool_call_id: 'tc-1',
        result: JSON.stringify({ results: [{ key: 'pref.lang', value: 'rust' }] }),
      } as any,
      ctx,
    );
    expect((state.blocks[0].data as { raw: string }).raw).toContain('pref.lang');
    expect(state.recalledFacts).toHaveLength(1);
    // Map entry consumed.
    expect(ctx.toolCallBlockMapRef.current.has('tc-1')).toBe(false);
  });

  it("updates a 'tool' block; ward switch payload promotes activeWard", () => {
    const { ctx, state } = makeCtx();
    state.blocks = [
      {
        id: 'b-ward',
        type: 'tool',
        timestamp: 't',
        data: { toolName: 'ward', input: 'use stocks' },
      },
    ];
    ctx.toolCallBlockMapRef.current.set('tc-w', 'b-ward');
    handleToolResultEvent(
      {
        type: 'tool_result',
        tool_call_id: 'tc-w',
        result: JSON.stringify({ __ward_changed__: true, ward_name: 'stocks', agents_md: 'ABC' }),
      } as any,
      ctx,
    );
    expect(state.activeWard).toEqual({ name: 'stocks', content: 'ABC' });
  });

  it('marks delegation block completed (or error) on result arrival', () => {
    const { ctx, state } = makeCtx();
    state.blocks = [
      {
        id: 'b-deleg',
        type: 'delegation',
        timestamp: 't',
        data: { agentId: 'writer-agent', task: 't', status: 'active' },
      },
    ];
    ctx.toolCallBlockMapRef.current.set('tc-d', 'b-deleg');

    // Success path
    handleToolResultEvent(
      { type: 'tool_result', tool_call_id: 'tc-d', result: 'done' } as any,
      ctx,
    );
    expect((state.blocks[0].data as { status: string }).status).toBe('completed');

    // Error path (separate block)
    state.blocks.push({
      id: 'b-deleg-2',
      type: 'delegation',
      timestamp: 't',
      data: { agentId: 'writer-agent', task: 't', status: 'active' },
    });
    ctx.toolCallBlockMapRef.current.set('tc-d2', 'b-deleg-2');
    handleToolResultEvent(
      {
        type: 'tool_result',
        tool_call_id: 'tc-d2',
        result: 'oops',
        is_error: true,
      } as any,
      ctx,
    );
    expect((state.blocks[1].data as { status: string }).status).toBe('error');
  });

  it('updates a plan block with parsed steps and refreshes plan state', () => {
    const { ctx, state } = makeCtx();
    state.blocks = [
      { id: 'b-plan', type: 'plan', timestamp: 't', data: { steps: [] } },
    ];
    ctx.toolCallBlockMapRef.current.set('tc-p', 'b-plan');
    handleToolResultEvent(
      {
        type: 'tool_result',
        tool_call_id: 'tc-p',
        result: JSON.stringify({
          steps: [
            { text: 'one', status: 'completed' },
            { text: 'two', status: 'pending' },
          ],
        }),
      } as any,
      ctx,
    );
    expect(state.plan).toEqual([
      { text: 'one', status: 'completed' },
      { text: 'two', status: 'pending' },
    ]);
  });

  it('is a no-op when tool_call_id has no mapped block', () => {
    const { ctx, state } = makeCtx();
    state.blocks = [];
    handleToolResultEvent(
      { type: 'tool_result', tool_call_id: 'unknown', result: 'x' } as any,
      ctx,
    );
    expect(state.blocks).toEqual([]);
  });
});

// ----------------------------------------------------------------------------
// Delegation lifecycle handlers
// ----------------------------------------------------------------------------

describe('handleDelegationStarted', () => {
  it('appends a fresh delegation block when no active block exists for that agent', () => {
    const { ctx, state } = makeCtx();
    handleDelegationStarted(
      {
        type: 'delegation_started',
        child_agent_id: 'planner-agent',
        task: 'plan everything',
        child_execution_id: 'exec-1',
      } as any,
      ctx,
    );
    expect(state.blocks).toHaveLength(1);
    expect(state.blocks[0].data).toMatchObject({
      agentId: 'planner-agent',
      task: 'plan everything',
      status: 'active',
    });
    expect(state.subagents).toEqual([
      { agentId: 'planner-agent', task: 'plan everything', status: 'active' },
    ]);
    expect(ctx.executionAgentMapRef.current.get('exec-1')).toBe('planner-agent');
  });

  it('updates an existing active delegation block instead of duplicating it', () => {
    const { ctx, state } = makeCtx({
      blocks: [
        {
          id: 'b-d',
          type: 'delegation',
          timestamp: 't',
          data: { agentId: 'planner-agent', task: 'old', status: 'active' },
        },
      ],
      subagents: [{ agentId: 'planner-agent', task: 'old', status: 'active' }],
    });
    handleDelegationStarted(
      { type: 'delegation_started', child_agent_id: 'planner-agent', task: 'newer task' } as any,
      ctx,
    );
    expect(state.blocks).toHaveLength(1);
    expect((state.blocks[0].data as { task: string }).task).toBe('newer task');
    expect(state.subagents).toHaveLength(1);
  });
});

describe('handleDelegationCompleted', () => {
  it("flips the active block's status to 'completed' and updates the subagent", () => {
    const { ctx, state } = makeCtx({
      blocks: [
        {
          id: 'b-d',
          type: 'delegation',
          timestamp: 't',
          data: { agentId: 'writer-agent', task: 'compose', status: 'active' },
        },
      ],
      subagents: [{ agentId: 'writer-agent', task: 'compose', status: 'active' }],
    });
    handleDelegationCompleted(
      { type: 'delegation_completed', child_agent_id: 'writer-agent', result: 'final text' } as any,
      ctx,
    );
    expect((state.blocks[0].data as { status: string }).status).toBe('completed');
    expect((state.blocks[0].data as { result: string }).result).toBe('final text');
    expect(state.subagents[0].status).toBe('completed');
  });

  it('is a no-op when no active block matches the agent id', () => {
    const { ctx, state } = makeCtx();
    handleDelegationCompleted(
      { type: 'delegation_completed', child_agent_id: 'unknown', result: 'x' } as any,
      ctx,
    );
    expect(state.blocks).toEqual([]);
  });
});

describe('handleDelegationError', () => {
  it("flips the active block's status to 'error' and surfaces the error message", () => {
    const { ctx, state } = makeCtx({
      blocks: [
        {
          id: 'b-d',
          type: 'delegation',
          timestamp: 't',
          data: { agentId: 'builder-agent', task: 'build', status: 'active' },
        },
      ],
      subagents: [{ agentId: 'builder-agent', task: 'build', status: 'active' }],
    });
    handleDelegationError(
      {
        type: 'delegation_error',
        child_agent_id: 'builder-agent',
        error: 'subagent crashed',
      } as any,
      ctx,
    );
    expect((state.blocks[0].data as { status: string }).status).toBe('error');
    expect((state.blocks[0].data as { result: string }).result).toBe('subagent crashed');
    expect(state.subagents[0].status).toBe('error');
  });
});

// ----------------------------------------------------------------------------
// Title + intent + terminal/system event handlers
// ----------------------------------------------------------------------------

describe('handleSessionTitleChanged', () => {
  it('updates sessionTitle and clears the fallback timer', () => {
    const { ctx, state } = makeCtx();
    ctx.titleFallbackTimerRef.current = setTimeout(() => {}, 1_000_000);
    handleSessionTitleChanged(
      { type: 'session_title_changed', title: 'My Cool Session' } as any,
      ctx,
    );
    expect(state.sessionTitle).toBe('My Cool Session');
    expect(ctx.titleFallbackTimerRef.current).toBeNull();
  });

  it('ignores empty titles', () => {
    const { ctx, state } = makeCtx({ sessionTitle: 'Existing' });
    handleSessionTitleChanged({ type: 'session_title_changed', title: '' } as any, ctx);
    expect(state.sessionTitle).toBe('Existing');
  });
});

describe('handleIntentAnalysisStarted', () => {
  it('seeds an intent_analysis block in streaming state', () => {
    const { ctx, state } = makeCtx();
    handleIntentAnalysisStarted(ctx);
    expect(state.blocks).toHaveLength(1);
    expect(state.blocks[0].type).toBe('intent_analysis');
    expect(state.blocks[0].isStreaming).toBe(true);
  });
});

describe('handleIntentAnalysisComplete', () => {
  it('replaces the streaming placeholder with the final analysis and transitions phase=planning', () => {
    const { ctx, state } = makeCtx();
    handleIntentAnalysisStarted(ctx);
    handleIntentAnalysisComplete(
      {
        type: 'intent_analysis_complete',
        primary_intent: 'analyze',
        ward_recommendation: { ward_name: 'stocks', reason: 'finance' },
        execution_strategy: { approach: 'graph' },
      } as any,
      ctx,
    );
    expect(state.phase).toBe('planning');
    expect(state.blocks).toHaveLength(1);
    expect(state.blocks[0].type).toBe('intent_analysis');
    expect(state.blocks[0].isStreaming).toBe(false);
    // Ward recommendation should be promoted when no ward is yet active.
    expect(state.activeWard).toEqual({ name: 'stocks', content: 'finance' });
  });

  it("appends a fresh non-streaming block when no streaming placeholder exists", () => {
    const { ctx, state } = makeCtx();
    handleIntentAnalysisComplete(
      {
        type: 'intent_analysis_complete',
        primary_intent: 'do thing',
        ward_recommendation: {},
        execution_strategy: {},
      } as any,
      ctx,
    );
    expect(state.blocks).toHaveLength(1);
    expect(state.blocks[0].isStreaming).toBe(false);
  });
});

describe('handleErrorEvent', () => {
  it('flips status to error, stops the timer, finalizes any streaming blocks', () => {
    const { ctx, state } = makeCtx({
      blocks: [{ id: 'x', type: 'response', timestamp: 't', data: {}, isStreaming: true }],
    });
    handleErrorEvent(ctx);
    expect(state.status).toBe('error');
    expect(state.phase).toBe('error');
    expect(state.durationStopped).toBe(true);
    expect(state.blocks[0].isStreaming).toBe(false);
  });
});

describe('handleSystemOrMessage', () => {
  it('appends a response block when the event has content', () => {
    const { ctx, state } = makeCtx();
    handleSystemOrMessage({ type: 'system', content: 'system note' } as any, ctx);
    expect(state.blocks).toHaveLength(1);
    expect(state.blocks[0].type).toBe('response');
    expect((state.blocks[0].data as { content: string }).content).toBe('system note');
  });

  it('falls back to event.message when content is missing', () => {
    const { ctx, state } = makeCtx();
    handleSystemOrMessage({ type: 'message', message: 'fallback' } as any, ctx);
    expect((state.blocks[0].data as { content: string }).content).toBe('fallback');
  });

  it('is a no-op when both content and message are empty', () => {
    const { ctx, state } = makeCtx();
    handleSystemOrMessage({ type: 'system' } as any, ctx);
    expect(state.blocks).toEqual([]);
  });
});

// ----------------------------------------------------------------------------
// Pure helpers
// ----------------------------------------------------------------------------

describe('parsePlanSteps', () => {
  it('parses an array of strings as pending steps', () => {
    expect(parsePlanSteps({ plan: ['one', 'two'] })).toEqual([
      { text: 'one', status: 'pending' },
      { text: 'two', status: 'pending' },
    ]);
  });

  it('parses an array of {step, status} objects', () => {
    expect(
      parsePlanSteps({
        plan: [
          { step: 'a', status: 'completed' },
          { step: 'b' }, // status defaults to pending
        ],
      }),
    ).toEqual([
      { text: 'a', status: 'completed' },
      { text: 'b', status: 'pending' },
    ]);
  });

  it("flags lines containing [x] or ✓ as done; otherwise pending (markers kept in text)", () => {
    // The regex strip only removes leading whitespace/dashes/asterisks/
    // digits/dots — `[x]` and `✓` survive in the text. Status is derived
    // from line.includes('[x]') || line.includes('✓').
    const md = `- [x] first\n- second\n✓ third`;
    const out = parsePlanSteps({ plan: md });
    expect(out).toHaveLength(3);
    expect(out[0]).toEqual({ text: '[x] first', status: 'done' });
    expect(out[1]).toEqual({ text: 'second', status: 'pending' });
    expect(out[2]).toEqual({ text: '✓ third', status: 'done' });
  });

  it('returns an empty array for missing/empty inputs', () => {
    expect(parsePlanSteps({})).toEqual([]);
    expect(parsePlanSteps({ plan: '' })).toEqual([]);
  });
});

describe('parseIntentAnalysisFromState', () => {
  it('converts the snake_case wire shape into the camelCase IntentAnalysis type', () => {
    const ia = parseIntentAnalysisFromState({
      primary_intent: 'analyze',
      hidden_intents: ['speed'],
      recommended_skills: ['html-report'],
      recommended_agents: ['research-agent'],
      ward_recommendation: {
        action: 'use',
        ward_name: 'stocks',
        subdirectory: 'india-gdp',
        reason: 'finance',
      },
      execution_strategy: {
        approach: 'graph',
        explanation: 'multi-step',
      },
    });
    expect(ia.primaryIntent).toBe('analyze');
    expect(ia.hiddenIntents).toEqual(['speed']);
    expect(ia.recommendedSkills).toEqual(['html-report']);
    expect(ia.wardRecommendation).toEqual({
      action: 'use',
      wardName: 'stocks',
      subdirectory: 'india-gdp',
      reason: 'finance',
    });
    expect(ia.executionStrategy.approach).toBe('graph');
  });

  it('fills sensible defaults when fields are missing', () => {
    const ia = parseIntentAnalysisFromState({});
    expect(ia.primaryIntent).toBe('');
    expect(ia.hiddenIntents).toEqual([]);
    expect(ia.recommendedSkills).toEqual([]);
    expect(ia.wardRecommendation.action).toBe('');
    expect(ia.executionStrategy.approach).toBe('simple');
  });
});
