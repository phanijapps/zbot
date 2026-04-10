// ============================================================================
// EXECUTION NARRATIVE
// Center panel: user messages, phase indicators, and agent responses.
// Tool calls, delegations, recalls moved to sidebar.
// ============================================================================

import { useEffect, useRef } from "react";
import { UserMessage } from "./UserMessage";
import { AgentResponse } from "./AgentResponse";
import { PhaseIndicators, type Phase } from "./PhaseIndicators";
import type { NarrativeBlock } from "./mission-hooks";
import type { SubagentStateData } from "@/services/transport/types";

export interface ExecutionNarrativeProps {
  blocks: NarrativeBlock[];
  status: string;
  phase: Phase;
  subagents?: SubagentStateData[];
}

export function ExecutionNarrative({ blocks, status, phase, subagents }: ExecutionNarrativeProps) {
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [blocks.length, phase]);

  // Only user messages and responses in the center
  const userBlocks = blocks.filter((b) => b.type === "user");
  const responseBlocks = blocks.filter((b) => b.type === "response");

  return (
    <div className="mission-control__narrative" ref={scrollRef}>
      {blocks.length === 0 && (
        <div className="empty-state">
          <p>Start a conversation to see execution progress here.</p>
        </div>
      )}

      {userBlocks.map((block, i) => {
        const isLastTurn = i === userBlocks.length - 1;
        return (
          <div key={block.id}>
            <UserMessage
              content={block.data.content as string}
              timestamp={(block.data.timestamp ?? block.timestamp) as string}
              attachments={block.data.attachments as string[] | undefined}
            />

            {/* Phase indicators only on the latest turn */}
            {isLastTurn && status !== "idle" && (
              <PhaseIndicators phase={phase} subagents={subagents} />
            )}

            {/* Matching response */}
            {responseBlocks[i] && (
              <AgentResponse
                content={responseBlocks[i].data.content as string}
                timestamp={(responseBlocks[i].data.timestamp ?? responseBlocks[i].timestamp) as string}
              />
            )}
          </div>
        );
      })}

      {/* Thinking indicator when working and no response streaming yet */}
      {status === "running" && phase !== "responding" && phase !== "completed" && (
        <div className="thinking-indicator">
          <div className="thinking-indicator__dots">
            <div className="thinking-indicator__dot" />
            <div className="thinking-indicator__dot" />
            <div className="thinking-indicator__dot" />
          </div>
        </div>
      )}
    </div>
  );
}
