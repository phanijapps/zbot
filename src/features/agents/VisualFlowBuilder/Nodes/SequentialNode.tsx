// ============================================================================
// VISUAL FLOW BUILDER - SEQUENTIAL NODE
// Execute subtasks in order
// ============================================================================

import { memo } from "react";
import { BaseNode } from "./BaseNode";
import { NODE_COLORS } from "../constants";
import type { BaseNode as BaseNodeType } from "../types";

// -----------------------------------------------------------------------------
// Icons
// -----------------------------------------------------------------------------

const ArrowRightIcon = () => (
  <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="M5 12h14" /><path d="m12 5 7 7-7 7" />
  </svg>
);

const ListIcon = () => (
  <svg className="w-3 h-3" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="M8 6h13" /><path d="M8 12h13" /><path d="M8 18h13" /><path d="M3 6h.01" /><path d="M3 12h.01" /><path d="M3 18h.01" />
  </svg>
);

// -----------------------------------------------------------------------------
// Helper: Get sequential data
// -----------------------------------------------------------------------------

function getSequentialData(data: unknown): {
  displayName?: string;
  subtasks?: string[];
} | null {
  if (!data || typeof data !== "object") return null;
  const d = data as Record<string, unknown>;
  return {
    displayName: d.displayName ? String(d.displayName) : undefined,
    subtasks: d.subtasks ? d.subtasks as string[] : undefined,
  };
}

// -----------------------------------------------------------------------------
// Sequential Node Component
// -----------------------------------------------------------------------------

interface SequentialNodeProps {
  node: BaseNodeType;
  isSelected: boolean;
  onSelect: () => void;
  onUpdate: (updates: Partial<BaseNodeType>) => void;
  onDelete: () => void;
}

export const SequentialNode = memo(({ node, isSelected, onSelect, onUpdate, onDelete }: SequentialNodeProps) => {
  const sequentialData = getSequentialData(node.data);
  const subtaskCount = sequentialData?.subtasks?.length ?? 0;

  return (
    <BaseNode node={node} isSelected={isSelected} onSelect={onSelect} onUpdate={onUpdate} onDelete={onDelete}>
      {/* Header */}
      <div className="flex items-center gap-2 mb-2">
        <div className={`p-1.5 rounded ${NODE_COLORS.sequential.icon} bg-white/10`}>
          <ArrowRightIcon />
        </div>
        <div className="flex-1 min-w-0">
          <p className="text-xs font-semibold text-white truncate">
            {sequentialData?.displayName || "Sequential"}
          </p>
        </div>
      </div>

      {/* Subtasks */}
      <div className="space-y-1">
        <div className="flex items-center gap-1.5">
          <ListIcon />
          <span className="text-[10px] text-gray-400">
            {subtaskCount} step{subtaskCount !== 1 ? "s" : ""}
          </span>
        </div>

        {/* Visual indicator of sequential flow */}
        {subtaskCount > 0 && (
          <div className="flex items-center gap-0.5 mt-1">
            {Array.from({ length: Math.min(subtaskCount, 5) }).map((_, i) => (
              <div
                key={i}
                className={`w-1.5 h-1.5 rounded-full ${
                  i === 0 ? "bg-violet-500" : "bg-white/30"
                }`}
              />
            ))}
            {subtaskCount > 5 && (
              <span className="text-[10px] text-gray-500 ml-1">+{subtaskCount - 5}</span>
            )}
          </div>
        )}
      </div>
    </BaseNode>
  );
});

SequentialNode.displayName = "SequentialNode";
