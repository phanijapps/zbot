// ============================================================================
// VISUAL FLOW BUILDER - PARALLEL PROPERTIES
// Properties panel for parallel nodes
// ============================================================================

import { memo, useState, useEffect } from "react";
import type { BaseNode } from "../types";
import { Label } from "@/shared/ui/label";
import { Input } from "@/shared/ui/input";

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface ParallelPropertiesProps {
  node: BaseNode;
  onUpdate: (updates: Partial<BaseNode>) => void;
}

interface ParallelData {
  displayName?: string;
  subagents?: string[];
  mergeStrategy?: "all" | "first" | "last" | "concat" | "summarize";
  maxParallel?: number;
}

// -----------------------------------------------------------------------------
// Helper: Get parallel data
// -----------------------------------------------------------------------------

function getParallelData(data: unknown): ParallelData {
  if (!data || typeof data !== "object") return {};
  const d = data as Record<string, unknown>;
  return {
    displayName: d.displayName ? String(d.displayName) : undefined,
    subagents: d.subagents ? d.subagents as string[] : [],
    mergeStrategy: (d.mergeStrategy === "all" || d.mergeStrategy === "first" || d.mergeStrategy === "last" || d.mergeStrategy === "concat" || d.mergeStrategy === "summarize")
      ? d.mergeStrategy as ParallelData["mergeStrategy"]
      : "all",
    maxParallel: d.maxParallel ? Number(d.maxParallel) : undefined,
  };
}

// -----------------------------------------------------------------------------
// Component
// -----------------------------------------------------------------------------

export const ParallelProperties = memo(({ node, onUpdate }: ParallelPropertiesProps) => {
  const data = getParallelData(node.data);
  const [localData, setLocalData] = useState<ParallelData>(data);

  useEffect(() => {
    setLocalData(data);
  }, [node.data]);

  const handleChange = (field: keyof ParallelData, value: unknown) => {
    const newData = { ...localData, [field]: value };
    setLocalData(newData);
    onUpdate({ ...node, data: { ...node.data, ...newData } as typeof node.data });
  };

  const mergeStrategies = [
    { value: "all", label: "All Responses", description: "Include all agent responses" },
    { value: "first", label: "First Only", description: "Only use the first response" },
    { value: "last", label: "Last Only", description: "Only use the last response" },
    { value: "concat", label: "Concatenate", description: "Join all responses together" },
    { value: "summarize", label: "Summarize", description: "Summarize all responses" },
  ];

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
              placeholder="Parallel Execution"
              className="bg-white/5 border-white/10 text-white text-sm h-8"
            />
          </div>

          <div>
            <Label className="text-white text-xs mb-1.5 block">Max Parallel Tasks</Label>
            <Input
              type="number"
              min={1}
              value={localData.maxParallel ?? 5}
              onChange={(e) => handleChange("maxParallel", parseInt(e.target.value) || 5)}
              className="bg-white/5 border-white/10 text-white text-sm h-8"
            />
            <p className="text-[10px] text-gray-500 mt-1">Maximum number of parallel tasks to run</p>
          </div>
        </div>
      </div>

      {/* Merge Strategy */}
      <div>
        <h3 className="text-xs font-semibold text-gray-400 uppercase tracking-wide mb-3">
          Merge Strategy
        </h3>
        <div className="space-y-2">
          {mergeStrategies.map((strategy) => (
            <label
              key={strategy.value}
              className={`flex items-start gap-2 p-2 rounded border cursor-pointer transition-colors ${
                localData.mergeStrategy === strategy.value
                  ? "bg-violet-500/10 border-violet-500/30"
                  : "bg-white/5 border-white/10 hover:bg-white/10"
              }`}
            >
              <input
                type="radio"
                name="mergeStrategy"
                value={strategy.value}
                checked={localData.mergeStrategy === strategy.value}
                onChange={(e) => handleChange("mergeStrategy", e.target.value)}
                className="mt-0.5"
              />
              <div>
                <p className="text-xs text-white">{strategy.label}</p>
                <p className="text-[10px] text-gray-500">{strategy.description}</p>
              </div>
            </label>
          ))}
        </div>
      </div>

      {/* Subagents Count */}
      {localData.subagents && localData.subagents.length > 0 && (
        <div>
          <Label className="text-white text-xs mb-1.5 block">Connected Subtasks</Label>
          <div className="p-2 rounded bg-white/5 border border-white/10">
            <p className="text-[10px] text-gray-400">
              {localData.subagents.length} subtask{localData.subagents.length !== 1 ? "s" : ""} configured
            </p>
          </div>
        </div>
      )}

      {/* Info */}
      <div className="p-3 rounded-lg bg-violet-500/10 border border-violet-500/20">
        <p className="text-[10px] text-violet-300">
          Parallel nodes execute multiple subtasks simultaneously and merge their results.
        </p>
      </div>
    </div>
  );
});

ParallelProperties.displayName = "ParallelProperties";
