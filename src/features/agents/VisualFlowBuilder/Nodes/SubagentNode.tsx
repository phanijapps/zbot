// ============================================================================
// ZERO IDE - SUBAGENT NODE
// Subagent nodes that the Orchestrator can delegate to
// ============================================================================

import { memo } from "react";
import { BaseNode } from "./BaseNode";
import { NODE_COLORS } from "../constants";
import type { NodeProps, SubagentNodeData } from "../types";

// -----------------------------------------------------------------------------
// Icons
// -----------------------------------------------------------------------------

const ListChecksIcon = () => (
  <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="M9 11 3 17l-2-2" /><path d="m21 9-5-5-5 5" /><path d="M11 14h10" /><path d="M11 18h7" />
  </svg>
);

const CheckCircleIcon = () => (
  <svg className="w-3 h-3" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14" /><path d="m9 11 3 3L22 4" />
  </svg>
);

// -----------------------------------------------------------------------------
// Helper: Get subagent data
// -----------------------------------------------------------------------------

function getSubagentData(data: unknown): SubagentNodeData {
  if (!data || typeof data !== "object") {
    return {
      subagentId: "",
      displayName: "Subagent",
    };
  }

  const d = data as Record<string, unknown>;

  return {
    subagentId: String(d.subagentId ?? ""),
    displayName: String(d.displayName ?? "Subagent"),
  };
}

// -----------------------------------------------------------------------------
// Subagent Node Component
// -----------------------------------------------------------------------------

export const SubagentNode = memo(({ node, isSelected, onSelect, onUpdate, onDelete, onPortMouseDown }: NodeProps) => {
  const subagentData = getSubagentData(node.data);
  const hasConfig = !!(node.data as unknown as Record<string, unknown>).config;

  return (
    <BaseNode node={node} isSelected={isSelected} onSelect={onSelect} onUpdate={onUpdate} onDelete={onDelete} onPortMouseDown={onPortMouseDown}>
      {/* Header */}
      <div className="flex items-center gap-2 mb-2">
        <div className={`p-1.5 rounded ${NODE_COLORS.subagent.icon} bg-white/10`}>
          <ListChecksIcon />
        </div>
        <div className="flex-1 min-w-0">
          <p className="text-xs font-semibold text-white truncate" title={subagentData.displayName}>
            {subagentData.displayName}
          </p>
        </div>
      </div>

      {/* Status */}
      <div className="space-y-1">
        {hasConfig ? (
          <div className="flex items-center gap-1.5 text-green-400">
            <CheckCircleIcon />
            <span className="text-[10px]">Configured</span>
          </div>
        ) : (
          <div className="text-[10px] text-gray-500">
            Configure in properties panel
          </div>
        )}
      </div>
    </BaseNode>
  );
});

SubagentNode.displayName = "SubagentNode";
