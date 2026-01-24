// ============================================================================
// VISUAL FLOW BUILDER - PARALLEL NODE
// Execute multiple subagents concurrently and merge results
// ============================================================================

import { memo } from "react";
import { BaseNode } from "./BaseNode";
import { NODE_COLORS } from "../constants";
import type { BaseNode as BaseNodeType } from "../types";

// -----------------------------------------------------------------------------
// Icons
// -----------------------------------------------------------------------------

const ZapIcon = () => (
  <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2" />
  </svg>
);

const UsersIcon = () => (
  <svg className="w-3 h-3" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2" /><circle cx="9" cy="7" r="4" /><path d="M22 21v-2a4 4 0 0 0-3-3.87" /><path d="M16 3.13a4 4 0 0 1 0 7.75" />
  </svg>
);

// -----------------------------------------------------------------------------
// Helper: Get parallel data
// -----------------------------------------------------------------------------

function getParallelData(data: unknown): {
  displayName?: string;
  subagents?: string[];
  mergeStrategy?: string;
} | null {
  if (!data || typeof data !== "object") return null;
  const d = data as Record<string, unknown>;
  return {
    displayName: d.displayName ? String(d.displayName) : undefined,
    subagents: d.subagents ? d.subagents as string[] : undefined,
    mergeStrategy: d.mergeStrategy ? String(d.mergeStrategy) : undefined,
  };
}

// -----------------------------------------------------------------------------
// Parallel Node Component
// -----------------------------------------------------------------------------

interface ParallelNodeProps {
  node: BaseNodeType;
  isSelected: boolean;
  onSelect: () => void;
  onUpdate: (updates: Partial<BaseNodeType>) => void;
  onDelete: () => void;
}

export const ParallelNode = memo(({ node, isSelected, onSelect, onUpdate, onDelete }: ParallelNodeProps) => {
  const parallelData = getParallelData(node.data);
  const subagentCount = parallelData?.subagents?.length ?? 0;
  const mergeStrategy = parallelData?.mergeStrategy ?? "all";

  return (
    <BaseNode node={node} isSelected={isSelected} onSelect={onSelect} onUpdate={onUpdate} onDelete={onDelete}>
      {/* Header */}
      <div className="flex items-center gap-2 mb-2">
        <div className={`p-1.5 rounded ${NODE_COLORS.parallel.icon} bg-white/10`}>
          <ZapIcon />
        </div>
        <div className="flex-1 min-w-0">
          <p className="text-xs font-semibold text-white truncate">
            {parallelData?.displayName || "Parallel"}
          </p>
        </div>
      </div>

      {/* Subagents Count */}
      <div className="space-y-1">
        <div className="flex items-center gap-1.5">
          <UsersIcon />
          <span className="text-[10px] text-gray-400">
            {subagentCount} subagent{subagentCount !== 1 ? "s" : ""}
          </span>
        </div>

        {/* Output ports indicator */}
        {subagentCount > 0 && (
          <div className="flex items-center gap-1">
            <span className="text-[10px] text-gray-500">→</span>
            <span className="text-[10px] text-gray-500">{subagentCount} outputs</span>
          </div>
        )}

        {/* Merge Strategy */}
        <div className="flex items-center gap-1.5">
          <span className="text-[10px] text-gray-500 uppercase tracking-wide">Merge</span>
          <span className="text-[10px] px-1.5 py-0.5 rounded bg-violet-500/20 text-violet-400">
            {mergeStrategy}
          </span>
        </div>
      </div>
    </BaseNode>
  );
});

ParallelNode.displayName = "ParallelNode";
