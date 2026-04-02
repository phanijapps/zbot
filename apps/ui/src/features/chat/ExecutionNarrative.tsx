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
import { IntentAnalysisBlock } from "./IntentAnalysisBlock";

// ============================================================================
// Types
// ============================================================================

export interface ExecutionNarrativeProps {
  blocks: NarrativeBlock[];
  status: string;
}

// ============================================================================
// Component
// ============================================================================

/**
 * ExecutionNarrative — renders the narrative block list, auto-scrolls on
 * new blocks, but preserves scroll position when user has scrolled up.
 */
export function ExecutionNarrative({ blocks, status }: ExecutionNarrativeProps) {
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

      {(() => {
        // Collect tool block indices to show only the last 2
        const toolIndices: number[] = [];
        blocks.forEach((b, i) => { if (b.type === "tool") toolIndices.push(i); });
        const hiddenToolCount = Math.max(0, toolIndices.length - 2);
        const hiddenToolSet = new Set(toolIndices.slice(0, hiddenToolCount));
        // Track whether the collapsed summary has been rendered
        let collapsedSummaryRendered = false;

        return blocks.map((block, idx) => {
          // Collapse older tool calls into a single summary line
          if (block.type === "tool" && hiddenToolSet.has(idx)) {
            if (!collapsedSummaryRendered) {
              collapsedSummaryRendered = true;
              return (
                <div key="tool-collapsed" className="tool-collapsed">
                  <span className="tool-collapsed__icon">&#9881;</span>
                  <span>{hiddenToolCount} tool call{hiddenToolCount > 1 ? "s" : ""} completed</span>
                </div>
              );
            }
            return null;
          }

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

            // Delegation and plan blocks render only in the sidebar
            case "delegation":
            case "plan":
              return null;

            case "response":
              return (
                <AgentResponse
                  key={block.id}
                  content={(block.data.content ?? "") as string}
                  timestamp={(block.data.timestamp ?? block.timestamp) as string}
                />
              );

            case "intent_analysis":
              return (
                <IntentAnalysisBlock
                  key={block.id}
                  analysis={block.data.analysis as import("./mission-hooks").IntentAnalysis | null ?? null}
                  isStreaming={block.isStreaming}
                />
              );

            default:
              return null;
          }
        });
      })()}

      {/* Thinking indicator — shows when running and last block is user or no response yet */}
      {status === "running" && blocks.length > 0 && !blocks.some(b => (b.type === 'response' || b.type === 'intent_analysis') && b.isStreaming) && (
        (() => {
          const lastBlock = blocks[blocks.length - 1];
          const isWaiting = lastBlock?.type === 'user' ||
            (lastBlock?.type === 'tool' && !lastBlock.data.output) ||
            (lastBlock?.type === 'delegation' && lastBlock.data.status === 'active');
          if (!isWaiting) return null;
          return (
            <div className="thinking-indicator">
              <div className="thinking-indicator__dots">
                <span className="thinking-indicator__dot" />
                <span className="thinking-indicator__dot" />
                <span className="thinking-indicator__dot" />
              </div>
              <span className="thinking-indicator__text">
                {lastBlock?.type === 'delegation' ? 'Subagent working...' :
                 lastBlock?.type === 'tool' ? 'Running...' : 'Thinking...'}
              </span>
            </div>
          );
        })()
      )}

      {/* Scroll anchor */}
      <div ref={bottomRef} />
    </div>
  );
}
