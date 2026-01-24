// ============================================================================
// VISUAL FLOW BUILDER - SUBTASK NODE
// Individual task within a parallel or sequential workflow
// ============================================================================

import { memo } from "react";
import { BaseNode } from "./BaseNode";
import { NODE_COLORS } from "../constants";
import type { BaseNode as BaseNodeType } from "../types";

// -----------------------------------------------------------------------------
// Icons
// -----------------------------------------------------------------------------

const ListChecksIcon = () => (
  <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="M9 11 3 17l-2-2" /><path d="m21 9-5-5-5 5" /><path d="M11 14h10" /><path d="M11 18h7" />
  </svg>
);

const TargetIcon = () => (
  <svg className="w-3 h-3" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <circle cx="12" cy="12" r="10" /><circle cx="12" cy="12" r="6" /><circle cx="12" cy="12" r="2" />
  </svg>
);

const BotIcon = () => (
  <svg className="w-3 h-3" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="M12 8V4H8" /><rect width="16" height="12" x="4" y="8" rx="2" /><path d="M2 14h2" /><path d="M20 14h2" /><path d="M15 13v2" /><path d="M9 13v2" />
  </svg>
);

// -----------------------------------------------------------------------------
// Helper: Get subtask data
// -----------------------------------------------------------------------------

function getSubtaskData(data: unknown): {
  displayName?: string;
  goal?: string;
  tasks?: string[];
  agentNodeId?: string;
} | null {
  if (!data || typeof data !== "object") return null;
  const d = data as Record<string, unknown>;
  return {
    displayName: d.displayName ? String(d.displayName) : undefined,
    goal: d.goal ? String(d.goal) : undefined,
    tasks: d.tasks ? d.tasks as string[] : undefined,
    agentNodeId: d.agentNodeId ? String(d.agentNodeId) : undefined,
  };
}

// -----------------------------------------------------------------------------
// Subtask Node Component
// -----------------------------------------------------------------------------

interface SubtaskNodeProps {
  node: BaseNodeType;
  isSelected: boolean;
  onSelect: () => void;
  onUpdate: (updates: Partial<BaseNodeType>) => void;
  onDelete: () => void;
}

export const SubtaskNode = memo(({ node, isSelected, onSelect, onUpdate, onDelete }: SubtaskNodeProps) => {
  const subtaskData = getSubtaskData(node.data);
  const taskCount = subtaskData?.tasks?.length ?? 0;
  const hasGoal = !!subtaskData?.goal;
  const hasAgent = !!subtaskData?.agentNodeId;

  return (
    <BaseNode node={node} isSelected={isSelected} onSelect={onSelect} onUpdate={onUpdate} onDelete={onDelete}>
      {/* Header */}
      <div className="flex items-center gap-2 mb-2">
        <div className={`p-1.5 rounded ${NODE_COLORS.subtask.icon} bg-white/10`}>
          <ListChecksIcon />
        </div>
        <div className="flex-1 min-w-0">
          <p className="text-xs font-semibold text-white truncate">
            {subtaskData?.displayName || "Subtask"}
          </p>
        </div>
      </div>

      {/* Subtask Details */}
      <div className="space-y-1">
        {/* Goal indicator */}
        {hasGoal && (
          <div className="flex items-center gap-1">
            <TargetIcon />
            <span className="text-[10px] text-gray-400 truncate">
              {subtaskData?.goal}
            </span>
          </div>
        )}

        {/* Tasks count */}
        {taskCount > 0 && (
          <div className="flex items-center gap-1.5">
            <span className="text-[10px] text-gray-500 uppercase tracking-wide">Tasks</span>
            <span className="text-[10px] px-1.5 py-0.5 rounded bg-green-500/20 text-green-400">
              {taskCount}
            </span>
          </div>
        )}

        {/* Agent reference */}
        {hasAgent && (
          <div className="flex items-center gap-1">
            <BotIcon />
            <span className="text-[10px] text-gray-400 truncate">
              Agent configured
            </span>
          </div>
        )}

        {/* Validation warnings */}
        {!hasGoal && (
          <div className="flex items-center gap-1">
            <span className="text-[10px] text-yellow-500">⚠ No goal set</span>
          </div>
        )}
        {!hasAgent && (
          <div className="flex items-center gap-1">
            <span className="text-[10px] text-yellow-500">⚠ No agent</span>
          </div>
        )}
      </div>
    </BaseNode>
  );
});

SubtaskNode.displayName = "SubtaskNode";
