// ============================================================================
// VISUAL FLOW BUILDER - TRIGGER NODE
// Node that starts a workflow (manual or scheduled)
// ============================================================================

import { memo } from "react";
import { BaseNode } from "./BaseNode";
import { NODE_COLORS } from "../constants";
import type { NodeProps } from "../types";

// -----------------------------------------------------------------------------
// Icons
// -----------------------------------------------------------------------------

const PlayIcon = () => (
  <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <polygon points="5 3 19 12 5 21 5 3" />
  </svg>
);

const ClockIcon = () => (
  <svg className="w-3 h-3" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <circle cx="12" cy="12" r="10" /><path d="M12 6v6l4 2" />
  </svg>
);

// -----------------------------------------------------------------------------
// Helper: Get trigger data
// -----------------------------------------------------------------------------

function getTriggerData(data: unknown): { triggerType: string; schedule?: string } | null {
  if (!data || typeof data !== "object") return null;
  const d = data as Record<string, unknown>;
  return {
    triggerType: String(d.triggerType ?? "manual"),
    schedule: d.schedule ? String(d.schedule) : undefined,
  };
}

// -----------------------------------------------------------------------------
// Trigger Node Component
// -----------------------------------------------------------------------------

export const TriggerNode = memo(({ node, isSelected, onSelect, onUpdate, onDelete, onPortMouseDown }: NodeProps) => {
  const triggerData = getTriggerData(node.data);
  const triggerType = triggerData?.triggerType ?? "manual";

  const isScheduled = triggerType === "scheduled";

  return (
    <BaseNode node={node} isSelected={isSelected} onSelect={onSelect} onUpdate={onUpdate} onDelete={onDelete} onPortMouseDown={onPortMouseDown}>
      {/* Header */}
      <div className="flex items-center gap-2 mb-2">
        <div className={`p-1.5 rounded ${NODE_COLORS.start.icon} bg-white/10`}>
          {isScheduled ? <ClockIcon /> : <PlayIcon />}
        </div>
        <div className="flex-1 min-w-0">
          <p className="text-xs font-semibold text-white truncate">Start</p>
        </div>
      </div>

      {/* Trigger Type */}
      <div className="space-y-1">
        <div className="flex items-center gap-1.5">
          <span className="text-[10px] text-gray-500 uppercase tracking-wide">Type</span>
          <span className={`text-[10px] px-1.5 py-0.5 rounded ${
            isScheduled
              ? "bg-blue-500/20 text-blue-400"
              : "bg-green-500/20 text-green-400"
          }`}>
            {isScheduled ? "Scheduled" : "Manual"}
          </span>
        </div>

        {isScheduled && triggerData?.schedule && (
          <div className="flex items-center gap-1.5">
            <ClockIcon />
            <span className="text-[10px] text-gray-400">{triggerData.schedule}</span>
          </div>
        )}
      </div>
    </BaseNode>
  );
});

TriggerNode.displayName = "TriggerNode";
