// ============================================================================
// MISSION CONTROL
// Full-page execution theater: SessionBar + Narrative + Sidebar + Input
// ============================================================================

import { useMissionControl } from "./mission-hooks";
import { SessionBar } from "./SessionBar";
import { ExecutionNarrative } from "./ExecutionNarrative";
import { IntelligenceFeed } from "./IntelligenceFeed";
import { ChatInput } from "./ChatInput";

// ============================================================================
// Component
// ============================================================================

/**
 * MissionControl — composes all Mission Control sub-components:
 *   - SessionBar (top bar with status, title, metrics)
 *   - ExecutionNarrative (scrollable block list)
 *   - IntelligenceFeed (right sidebar)
 *   - ChatInput (bottom input area)
 */
export function MissionControl() {
  const { state, sendMessage, stopAgent } = useMissionControl();

  const isDisabled = state.status === "running";

  return (
    <div className="mission-control">
      <SessionBar
        title={state.sessionTitle}
        agentId="root"
        status={state.status}
        tokenCount={state.tokenCount}
        durationMs={state.durationMs}
        modelName={state.modelName || undefined}
        onStop={stopAgent}
      />

      <div className="mission-control__body">
        <ExecutionNarrative blocks={state.blocks} />

        <div className="mission-control__sidebar">
          <IntelligenceFeed
            ward={state.activeWard}
            recalledFacts={state.recalledFacts}
            subagents={state.subagents}
            plan={state.plan}
          />
        </div>
      </div>

      <div className="mission-control__input">
        <ChatInput
          onSend={sendMessage}
          disabled={isDisabled}
        />
      </div>
    </div>
  );
}
