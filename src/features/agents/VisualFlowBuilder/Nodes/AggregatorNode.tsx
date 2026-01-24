// ============================================================================
// VISUAL FLOW BUILDER - AGGREGATOR NODE
// Merge multiple inputs into a single output
// ============================================================================

import { memo } from "react";
import { BaseNode } from "./BaseNode";
import { NODE_COLORS } from "../constants";
import type { BaseNode as BaseNodeType } from "../types";

// -----------------------------------------------------------------------------
// Icons
// -----------------------------------------------------------------------------

const MergeIcon = () => (
  <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="m6 8 6 6-6 6" /><path d="m18 8-6 6 6 6" />
  </svg>
);

const LayersIcon = () => (
  <svg className="w-3 h-3" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="m12 2-2.2 7.8L2 12l7.8 2.2L12 22l2.2-7.8L22 12l-7.8-2.2L12 2" />
  </svg>
);

// -----------------------------------------------------------------------------
// Helper: Get aggregator data
// -----------------------------------------------------------------------------

function getAggregatorData(data: unknown): {
  displayName?: string;
  strategy?: string;
  template?: string;
} | null {
  if (!data || typeof data !== "object") return null;
  const d = data as Record<string, unknown>;
  return {
    displayName: d.displayName ? String(d.displayName) : undefined,
    strategy: d.strategy ? String(d.strategy) : undefined,
    template: d.template ? String(d.template) : undefined,
  };
}

// -----------------------------------------------------------------------------
// Aggregator Node Component
// -----------------------------------------------------------------------------

interface AggregatorNodeProps {
  node: BaseNodeType;
  isSelected: boolean;
  onSelect: () => void;
  onUpdate: (updates: Partial<BaseNodeType>) => void;
  onDelete: () => void;
}

export const AggregatorNode = memo(({ node, isSelected, onSelect, onUpdate, onDelete }: AggregatorNodeProps) => {
  const aggregatorData = getAggregatorData(node.data);
  const strategy = aggregatorData?.strategy ?? "concat";

  const strategyLabels: Record<string, string> = {
    concat: "Concatenate",
    all: "All Responses",
    first: "First",
    last: "Last",
    summarize: "Summarize",
    vote: "Vote",
  };

  return (
    <BaseNode node={node} isSelected={isSelected} onSelect={onSelect} onUpdate={onUpdate} onDelete={onDelete}>
      {/* Header */}
      <div className="flex items-center gap-2 mb-2">
        <div className={`p-1.5 rounded ${NODE_COLORS.aggregator.icon} bg-white/10`}>
          <MergeIcon />
        </div>
        <div className="flex-1 min-w-0">
          <p className="text-xs font-semibold text-white truncate">
            {aggregatorData?.displayName || "Aggregator"}
          </p>
        </div>
      </div>

      {/* Merge Strategy */}
      <div className="space-y-1">
        <div className="flex items-center gap-1.5">
          <LayersIcon />
          <span className="text-[10px] text-gray-500 uppercase tracking-wide">Strategy</span>
        </div>

        <div className="flex items-center gap-1.5">
          <span className={`text-[10px] px-1.5 py-0.5 rounded ${
            strategy === "concat" || strategy === "all"
              ? "bg-blue-500/20 text-blue-400"
              : strategy === "summarize" || strategy === "vote"
              ? "bg-violet-500/20 text-violet-400"
              : "bg-gray-500/20 text-gray-400"
          }`}>
            {strategyLabels[strategy] || strategy}
          </span>
        </div>

        {/* Template indicator */}
        {aggregatorData?.template && (
          <div className="flex items-center gap-1">
            <span className="text-[10px] text-gray-500 truncate">
              "{aggregatorData.template.slice(0, 20)}{aggregatorData.template.length > 20 ? "..." : ""}"
            </span>
          </div>
        )}

        {/* Input ports indicator */}
        <div className="flex items-center gap-1">
          <span className="text-[10px] text-gray-500">←</span>
          <span className="text-[10px] text-gray-500">multiple inputs</span>
        </div>
      </div>
    </BaseNode>
  );
});

AggregatorNode.displayName = "AggregatorNode";
