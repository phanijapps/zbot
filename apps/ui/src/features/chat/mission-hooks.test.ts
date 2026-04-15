import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  handleToolCallRespond,
  handleTurnComplete,
  handleWardChanged,
  handleAgentCompleted,
  type EventHandlerCtx,
  type NarrativeBlock,
} from './mission-hooks';

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
