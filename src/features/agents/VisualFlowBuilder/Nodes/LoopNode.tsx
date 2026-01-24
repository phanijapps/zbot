// ============================================================================
// VISUAL FLOW BUILDER - LOOP NODE
// Repeat execution until exit condition is met
// ============================================================================

import { memo } from "react";
import { BaseNode } from "./BaseNode";
import { NODE_COLORS } from "../constants";
import type { BaseNode as BaseNodeType } from "../types";

// -----------------------------------------------------------------------------
// Icons
// -----------------------------------------------------------------------------

const RepeatIcon = () => (
  <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="m17 2 4 4-4 4" /><path d="M3 11V9a4 4 0 0 1 4-4h14" /><path d="m7 22-4-4 4-4" /><path d="M21 13v2a4 4 0 0 1-4 4H3" />
  </svg>
);

const InfinityIcon = () => (
  <svg className="w-3 h-3" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="M4.93 19.07a10 10 0 0 1 14.14 0M19.07 4.93a10 10 0 0 1 0 14.14M4.93 4.93a10 10 0 0 0 0 14.14M19.07 19.07a10 10 0 0 0-14.14 0" />
  </svg>
);

// -----------------------------------------------------------------------------
// Helper: Get loop data
// -----------------------------------------------------------------------------

function getLoopData(data: unknown): {
  displayName?: string;
  exitCondition?: string;
  maxIterations?: number;
} | null {
  if (!data || typeof data !== "object") return null;
  const d = data as Record<string, unknown>;
  return {
    displayName: d.displayName ? String(d.displayName) : undefined,
    exitCondition: d.exitCondition ? String(d.exitCondition) : undefined,
    maxIterations: d.maxIterations ? Number(d.maxIterations) : undefined,
  };
}

// -----------------------------------------------------------------------------
// Loop Node Component
// -----------------------------------------------------------------------------

interface LoopNodeProps {
  node: BaseNodeType;
  isSelected: boolean;
  onSelect: () => void;
  onUpdate: (updates: Partial<BaseNodeType>) => void;
  onDelete: () => void;
}

export const LoopNode = memo(({ node, isSelected, onSelect, onUpdate, onDelete }: LoopNodeProps) => {
  const loopData = getLoopData(node.data);
  const maxIterations = loopData?.maxIterations;

  return (
    <BaseNode node={node} isSelected={isSelected} onSelect={onSelect} onUpdate={onUpdate} onDelete={onDelete}>
      {/* Header */}
      <div className="flex items-center gap-2 mb-2">
        <div className={`p-1.5 rounded ${NODE_COLORS.loop.icon} bg-white/10`}>
          <RepeatIcon />
        </div>
        <div className="flex-1 min-w-0">
          <p className="text-xs font-semibold text-white truncate">
            {loopData?.displayName || "Loop"}
          </p>
        </div>
      </div>

      {/* Loop Info */}
      <div className="space-y-1">
        {/* Exit condition */}
        {loopData?.exitCondition && (
          <div className="flex items-center gap-1">
            <InfinityIcon />
            <span className="text-[10px] text-gray-400 truncate">
              {loopData.exitCondition}
            </span>
          </div>
        )}

        {/* Max iterations */}
        {maxIterations !== undefined && maxIterations > 0 && (
          <div className="flex items-center gap-1.5">
            <span className="text-[10px] text-gray-500 uppercase tracking-wide">Max</span>
            <span className="text-[10px] px-1.5 py-0.5 rounded bg-orange-500/20 text-orange-400">
              {maxIterations}x
            </span>
          </div>
        )}

        {/* Loop visual indicator */}
        <div className="flex items-center gap-0.5 mt-1">
          <div className="w-1.5 h-1.5 rounded-full bg-violet-500" />
          <div className="w-1 h-1 rounded-full bg-white/30" />
          <div className="w-0.5 h-0.5 rounded-full bg-white/20" />
          <div className="text-[10px] text-gray-500 ml-1">repeat</div>
        </div>
      </div>
    </BaseNode>
  );
});

LoopNode.displayName = "LoopNode";
