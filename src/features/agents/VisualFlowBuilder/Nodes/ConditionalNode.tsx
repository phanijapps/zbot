// ============================================================================
// VISUAL FLOW BUILDER - CONDITIONAL NODE
// Route to different branches based on conditions
// ============================================================================

import { memo } from "react";
import { BaseNode } from "./BaseNode";
import { NODE_COLORS } from "../constants";
import type { NodeProps } from "../types";

// -----------------------------------------------------------------------------
// Icons
// -----------------------------------------------------------------------------

const GitBranchIcon = () => (
  <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="M6 3v12" /><circle cx="18" cy="6" r="3" /><circle cx="6" cy="18" r="3" /><path d="M18 9a9 9 0 0 1-9 9" />
  </svg>
);

const CodeIcon = () => (
  <svg className="w-3 h-3" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="M16 18l6-6-6-6M8 6l-6 6 6 6" />
  </svg>
);

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface Condition {
  expression: string;
  label: string;
  targetNodeId?: string;
}

// -----------------------------------------------------------------------------
// Helper: Get conditional data
// -----------------------------------------------------------------------------

function getConditionalData(data: unknown): {
  displayName?: string;
  conditions?: Condition[];
  defaultTarget?: string;
} | null {
  if (!data || typeof data !== "object") return null;
  const d = data as Record<string, unknown>;
  return {
    displayName: d.displayName ? String(d.displayName) : undefined,
    conditions: d.conditions ? d.conditions as Condition[] : undefined,
    defaultTarget: d.defaultTarget ? String(d.defaultTarget) : undefined,
  };
}

// -----------------------------------------------------------------------------
// Conditional Node Component
// -----------------------------------------------------------------------------

export const ConditionalNode = memo(({ node, isSelected, onSelect, onUpdate, onDelete, onPortMouseDown }: NodeProps) => {
  const conditionalData = getConditionalData(node.data);
  const conditions = conditionalData?.conditions ?? [];
  const conditionCount = conditions.length;

  return (
    <BaseNode node={node} isSelected={isSelected} onSelect={onSelect} onUpdate={onUpdate} onDelete={onDelete} onPortMouseDown={onPortMouseDown}>
      {/* Header */}
      <div className="flex items-center gap-2 mb-2">
        <div className={`p-1.5 rounded ${NODE_COLORS.conditional.icon} bg-white/10`}>
          <GitBranchIcon />
        </div>
        <div className="flex-1 min-w-0">
          <p className="text-xs font-semibold text-white truncate">
            {conditionalData?.displayName || "Conditional"}
          </p>
        </div>
      </div>

      {/* Conditions */}
      <div className="space-y-1">
        <div className="flex items-center gap-1.5">
          <CodeIcon />
          <span className="text-[10px] text-gray-400">
            {conditionCount} branch{conditionCount !== 1 ? "es" : ""}
          </span>
        </div>

        {/* Show first few conditions */}
        {conditions.length > 0 && (
          <div className="flex flex-col gap-0.5 mt-1">
            {conditions.slice(0, 3).map((condition, i) => (
              <div key={i} className="flex items-center gap-1">
                <span className="text-[10px] text-gray-500">
                  {condition.label || `Branch ${i + 1}`}
                </span>
              </div>
            ))}
            {conditions.length > 3 && (
              <span className="text-[10px] text-gray-500">
                +{conditions.length - 3} more
              </span>
            )}
          </div>
        )}

        {/* Default branch indicator */}
        {conditionalData?.defaultTarget && (
          <div className="flex items-center gap-1">
            <span className="text-[10px] text-gray-500">else</span>
            <span className="text-[10px] text-gray-400">→ default</span>
          </div>
        )}
      </div>
    </BaseNode>
  );
});

ConditionalNode.displayName = "ConditionalNode";
