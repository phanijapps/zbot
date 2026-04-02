// ============================================================================
// MISSION CONTROL
// Full-page execution theater: SessionBar + Narrative + Sidebar + Input
// Shows HeroInput when idle with no blocks, full layout otherwise.
// ============================================================================

import { useMissionControl, useRecentSessions } from "./mission-hooks";
import { SessionBar } from "./SessionBar";
import { ExecutionNarrative } from "./ExecutionNarrative";
import { IntelligenceFeed } from "./IntelligenceFeed";
import { ChatInput } from "./ChatInput";
import { HeroInput } from "./HeroInput";

// ============================================================================
// Component
// ============================================================================

/**
 * MissionControl — composes all Mission Control sub-components:
 *   - HeroInput (landing state — no blocks, idle)
 *   - SessionBar (top bar with status, title, metrics)
 *   - ExecutionNarrative (scrollable block list)
 *   - IntelligenceFeed (right sidebar)
 *   - ChatInput (bottom input area)
 */
export function MissionControl() {
  const { state, sendMessage, stopAgent, startNewSession } = useMissionControl();
  const recentSessions = useRecentSessions();

  // No blocks and idle — show the beautiful landing input
  if (state.blocks.length === 0 && state.status === "idle") {
    return <HeroInput onSend={sendMessage} recentSessions={recentSessions} />;
  }

  // Active session — full execution theater
  const isDisabled = state.status === "running";

  // Derive current session ID from localStorage for active highlight
  const currentSessionId = typeof window !== "undefined"
    ? localStorage.getItem("agentzero_web_session_id")
    : null;

  return (
    <div className="mission-control">
      <SessionBar
        title={state.sessionTitle}
        agentId="root"
        status={state.status}
        tokenCount={state.tokenCount}
        durationMs={state.durationMs}
        modelName={state.modelName || undefined}
        recentSessions={recentSessions}
        currentSessionId={currentSessionId}
        onStop={stopAgent}
        onNewSession={startNewSession}
      />

      <div className="mission-control__body">
        <div className="mission-control__main">
          <ExecutionNarrative blocks={state.blocks} status={state.status} />
          <div className="mission-control__input">
            <ChatInput
              onSend={sendMessage}
              disabled={isDisabled}
            />
          </div>
        </div>

        <div className="mission-control__sidebar">
          <IntelligenceFeed
            ward={state.activeWard}
            recalledFacts={state.recalledFacts}
            subagents={state.subagents}
            plan={state.plan}
            intentAnalysis={state.intentAnalysis}
          />
        </div>
      </div>
    </div>
  );
}
