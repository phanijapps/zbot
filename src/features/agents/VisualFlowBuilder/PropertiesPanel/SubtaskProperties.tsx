// ============================================================================
// VISUAL FLOW BUILDER - SUBTASK PROPERTIES
// Properties panel for subtask nodes
// ============================================================================

import { memo, useState, useEffect } from "react";
import type { BaseNode } from "../types";
import { Label } from "@/shared/ui/label";
import { Input } from "@/shared/ui/input";
import { Button } from "@/shared/ui/button";

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface SubtaskPropertiesProps {
  node: BaseNode;
  onUpdate: (updates: Partial<BaseNode>) => void;
}

interface SubtaskData {
  displayName?: string;
  goal?: string;
  context?: string;
  tasks?: string[];
  agentNodeId?: string;
  timeout?: number;
}

// -----------------------------------------------------------------------------
// Icons
// -----------------------------------------------------------------------------

const PlusIcon = () => (
  <svg className="w-3 h-3" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="M12 5v14M5 12h14" />
  </svg>
);

const TrashIcon = () => (
  <svg className="w-3 h-3" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="M3 6h18M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6m3 0V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2" />
  </svg>
);

// -----------------------------------------------------------------------------
// Helper: Get subtask data
// -----------------------------------------------------------------------------

function getSubtaskData(data: unknown): SubtaskData {
  if (!data || typeof data !== "object") return {};
  const d = data as Record<string, unknown>;
  return {
    displayName: d.displayName ? String(d.displayName) : undefined,
    goal: d.goal ? String(d.goal) : undefined,
    context: d.context ? String(d.context) : undefined,
    tasks: d.tasks ? d.tasks as string[] : [],
    agentNodeId: d.agentNodeId ? String(d.agentNodeId) : undefined,
    timeout: d.timeout ? Number(d.timeout) : undefined,
  };
}

// -----------------------------------------------------------------------------
// Component
// -----------------------------------------------------------------------------

export const SubtaskProperties = memo(({ node, onUpdate }: SubtaskPropertiesProps) => {
  const data = getSubtaskData(node.data);
  const [localData, setLocalData] = useState<SubtaskData>(data);

  useEffect(() => {
    setLocalData(data);
  }, [node.data]);

  const handleChange = (field: keyof SubtaskData, value: unknown) => {
    const newData = { ...localData, [field]: value };
    setLocalData(newData);
    onUpdate({ ...node, data: { ...node.data, ...newData } });
  };

  const handleTaskChange = (index: number, value: string) => {
    const tasks = [...(localData.tasks || [])];
    tasks[index] = value;
    handleChange("tasks", tasks);
  };

  const addTask = () => {
    const tasks = [...(localData.tasks || []), ""];
    handleChange("tasks", tasks);
  };

  const removeTask = (index: number) => {
    const tasks = (localData.tasks || []).filter((_, i) => i !== index);
    handleChange("tasks", tasks);
  };

  return (
    <div className="space-y-4">
      {/* Basic Settings */}
      <div>
        <h3 className="text-xs font-semibold text-gray-400 uppercase tracking-wide mb-3">
          Basic Settings
        </h3>
        <div className="space-y-3">
          <div>
            <Label className="text-white text-xs mb-1.5 block">Display Name</Label>
            <Input
              value={localData.displayName || ""}
              onChange={(e) => handleChange("displayName", e.target.value)}
              placeholder="Subtask"
              className="bg-white/5 border-white/10 text-white text-sm h-8"
            />
          </div>
        </div>
      </div>

      {/* Task Definition */}
      <div>
        <h3 className="text-xs font-semibold text-gray-400 uppercase tracking-wide mb-3">
          Task Definition
        </h3>
        <div className="space-y-3">
          <div>
            <Label className="text-white text-xs mb-1.5 block">Goal</Label>
            <textarea
              value={localData.goal || ""}
              onChange={(e) => handleChange("goal", e.target.value)}
              placeholder="What should this subtask accomplish?"
              rows={2}
              className="w-full bg-white/5 border border-white/10 rounded px-3 py-2 text-white text-sm placeholder:text-gray-600 resize-none focus:outline-none focus:ring-1 focus:ring-violet-500"
            />
          </div>

          <div>
            <Label className="text-white text-xs mb-1.5 block">Context (Optional)</Label>
            <textarea
              value={localData.context || ""}
              onChange={(e) => handleChange("context", e.target.value)}
              placeholder="Additional context for the agent..."
              rows={2}
              className="w-full bg-white/5 border border-white/10 rounded px-3 py-2 text-white text-sm placeholder:text-gray-600 resize-none focus:outline-none focus:ring-1 focus:ring-violet-500"
            />
          </div>
        </div>
      </div>

      {/* Tasks List */}
      <div>
        <div className="flex items-center justify-between mb-3">
          <h3 className="text-xs font-semibold text-gray-400 uppercase tracking-wide">
            Tasks
          </h3>
          <Button
            onClick={addTask}
            variant="outline"
            size="sm"
            className="h-6 px-2 text-xs border-white/20 text-white hover:bg-white/5"
          >
            <PlusIcon />
            <span className="ml-1">Add</span>
          </Button>
        </div>

        <div className="space-y-2">
          {(localData.tasks || []).map((task, index) => (
            <div key={index} className="flex items-center gap-2">
              <span className="text-[10px] text-gray-500 w-4">{index + 1}.</span>
              <Input
                value={task}
                onChange={(e) => handleTaskChange(index, e.target.value)}
                placeholder={`Task ${index + 1}`}
                className="flex-1 bg-white/5 border-white/10 text-white text-xs h-7"
              />
              <button
                onClick={() => removeTask(index)}
                className="p-1 rounded hover:bg-red-500/10 text-gray-500 hover:text-red-400 transition-colors"
              >
                <TrashIcon />
              </button>
            </div>
          ))}

          {(localData.tasks || []).length === 0 && (
            <div className="p-3 rounded border border-dashed border-white/20 text-center">
              <p className="text-[10px] text-gray-500">No tasks defined</p>
              <p className="text-[10px] text-gray-600">Add tasks to define the subtask steps</p>
            </div>
          )}
        </div>
      </div>

      {/* Agent Configuration */}
      <div>
        <h3 className="text-xs font-semibold text-gray-400 uppercase tracking-wide mb-3">
          Agent Configuration
        </h3>
        <div className="space-y-3">
          <div>
            <Label className="text-white text-xs mb-1.5 block">Agent Node</Label>
            <Input
              value={localData.agentNodeId || ""}
              onChange={(e) => handleChange("agentNodeId", e.target.value)}
              placeholder="Select an agent node..."
              className="bg-white/5 border-white/10 text-white text-sm h-8"
            />
            <p className="text-[10px] text-gray-500 mt-1">The agent that will execute this subtask</p>
          </div>

          <div>
            <Label className="text-white text-xs mb-1.5 block">Timeout (seconds)</Label>
            <Input
              type="number"
              min={1}
              value={localData.timeout ?? 300}
              onChange={(e) => handleChange("timeout", parseInt(e.target.value) || 300)}
              className="bg-white/5 border-white/10 text-white text-sm h-8"
            />
            <p className="text-[10px] text-gray-500 mt-1">Maximum time for this subtask to complete</p>
          </div>
        </div>
      </div>

      {/* Validation */}
      {(!localData.goal || localData.goal.trim() === "") && (
        <div className="p-3 rounded-lg bg-yellow-500/10 border border-yellow-500/20">
          <p className="text-[10px] text-yellow-300">
            ⚠️ Warning: No goal set. Define what this subtask should accomplish.
          </p>
        </div>
      )}

      {!localData.agentNodeId && (
        <div className="p-3 rounded-lg bg-yellow-500/10 border border-yellow-500/20">
          <p className="text-[10px] text-yellow-300">
            ⚠️ Warning: No agent selected. An agent is required to execute this subtask.
          </p>
        </div>
      )}

      {/* Info */}
      <div className="p-3 rounded-lg bg-green-500/10 border border-green-500/20">
        <p className="text-[10px] text-green-300">
          Subtask nodes define individual tasks within parallel or sequential workflows.
        </p>
      </div>
    </div>
  );
});

SubtaskProperties.displayName = "SubtaskProperties";
