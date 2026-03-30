// ============================================================================
// EXECUTION NARRATIVE
// Scrollable container rendering NarrativeBlock[] through block components
// ============================================================================

import { useEffect, useRef, useCallback, useState } from "react";
import type { NarrativeBlock } from "./mission-hooks";
import { UserMessage } from "./UserMessage";
import { AgentResponse } from "./AgentResponse";
import { RecallBlock } from "./RecallBlock";
import { ToolExecutionBlock } from "./ToolExecutionBlock";
import { DelegationBlock } from "./DelegationBlock";
import { PlanBlock } from "./PlanBlock";

// ============================================================================
// Types
// ============================================================================

export interface ExecutionNarrativeProps {
  blocks: NarrativeBlock[];
}

// ============================================================================
// Component
// ============================================================================

/**
 * ExecutionNarrative — renders the narrative block list, auto-scrolls on
 * new blocks, but preserves scroll position when user has scrolled up.
 */
export function ExecutionNarrative({ blocks }: ExecutionNarrativeProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const bottomRef = useRef<HTMLDivElement>(null);
  const [isUserScrolled, setIsUserScrolled] = useState(false);

  // Track tool expand/collapse state locally (keyed by block id)
  const [expandedTools, setExpandedTools] = useState<Set<string>>(new Set());

  const toggleToolExpand = useCallback((blockId: string) => {
    setExpandedTools((prev) => {
      const next = new Set(prev);
      if (next.has(blockId)) {
        next.delete(blockId);
      } else {
        next.add(blockId);
      }
      return next;
    });
  }, []);

  // Detect when user scrolls up
  const handleScroll = useCallback(() => {
    const el = containerRef.current;
    if (!el) return;
    // Consider "at bottom" if within 80px of the bottom
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 80;
    setIsUserScrolled(!atBottom);
  }, []);

  // Auto-scroll to bottom when new blocks arrive (unless user scrolled up)
  useEffect(() => {
    if (!isUserScrolled) {
      bottomRef.current?.scrollIntoView({ behavior: "smooth" });
    }
  }, [blocks, isUserScrolled]);

  return (
    <div
      ref={containerRef}
      className="mission-control__narrative"
      onScroll={handleScroll}
    >
      {blocks.length === 0 && (
        <div className="empty-state">
          <p>No messages yet. Start a conversation below.</p>
        </div>
      )}

      {blocks.map((block) => {
        switch (block.type) {
          case "user":
            return (
              <UserMessage
                key={block.id}
                content={(block.data.content ?? "") as string}
                timestamp={(block.data.timestamp ?? block.timestamp) as string}
                attachments={block.data.attachments as string[] | undefined}
              />
            );

          case "recall":
            return (
              <RecallBlock
                key={block.id}
                raw={(block.data.raw ?? "") as string}
              />
            );

          case "tool":
            return (
              <ToolExecutionBlock
                key={block.id}
                toolName={(block.data.toolName ?? "") as string}
                input={(block.data.input ?? "") as string}
                output={block.data.output as string | undefined}
                durationMs={block.data.durationMs as number | undefined}
                isError={block.data.isError as boolean | undefined}
                isExpanded={expandedTools.has(block.id)}
                onToggle={() => toggleToolExpand(block.id)}
              />
            );

          case "delegation":
            return (
              <DelegationBlock
                key={block.id}
                agentId={(block.data.agentId ?? "") as string}
                task={(block.data.task ?? "") as string}
                status={(block.data.status ?? "active") as "active" | "completed" | "error"}
                toolCallCount={block.data.toolCallCount as number | undefined}
                tokenCount={block.data.tokenCount as number | undefined}
                durationMs={block.data.durationMs as number | undefined}
                result={block.data.result as string | undefined}
              />
            );

          case "plan":
            return (
              <PlanBlock
                key={block.id}
                steps={(block.data.steps ?? []) as Array<{ text: string; status: "done" | "active" | "pending" }>}
              />
            );

          case "response":
            return (
              <AgentResponse
                key={block.id}
                content={(block.data.content ?? "") as string}
                timestamp={(block.data.timestamp ?? block.timestamp) as string}
              />
            );

          default:
            return null;
        }
      })}

      {/* Scroll anchor */}
      <div ref={bottomRef} />
    </div>
  );
}
