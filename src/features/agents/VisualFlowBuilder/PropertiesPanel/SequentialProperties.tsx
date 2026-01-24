// ============================================================================
// VISUAL FLOW BUILDER - SEQUENTIAL PROPERTIES
// Properties panel for sequential nodes
// ============================================================================

import { memo, useState, useEffect } from "react";
import type { BaseNode } from "../types";
import { Label } from "@/shared/ui/label";
import { Input } from "@/shared/ui/input";

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface SequentialPropertiesProps {
  node: BaseNode;
  onUpdate: (updates: Partial<BaseNode>) => void;
}

interface SequentialData {
  displayName?: string;
  subtasks?: string[];
  continueOnError?: boolean;
  stopOnError?: boolean;
}

// -----------------------------------------------------------------------------
// Helper: Get sequential data
// -----------------------------------------------------------------------------

function getSequentialData(data: unknown): SequentialData {
  if (!data || typeof data !== "object") return {};
  const d = data as Record<string, unknown>;
  return {
    displayName: d.displayName ? String(d.displayName) : undefined,
    subtasks: d.subtasks ? d.subtasks as string[] : [],
    continueOnError: d.continueOnError ? Boolean(d.continueOnError) : false,
    stopOnError: d.stopOnError !== undefined ? Boolean(d.stopOnError) : true,
  };
}

// -----------------------------------------------------------------------------
// Component
// -----------------------------------------------------------------------------

export const SequentialProperties = memo(({ node, onUpdate }: SequentialPropertiesProps) => {
  const data = getSequentialData(node.data);
  const [localData, setLocalData] = useState<SequentialData>(data);

  useEffect(() => {
    setLocalData(data);
  }, [node.data]);

  const handleChange = (field: keyof SequentialData, value: unknown) => {
    const newData = { ...localData, [field]: value };
    setLocalData(newData);
    onUpdate({ ...node, data: { ...node.data, ...newData } });
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
              placeholder="Sequential Flow"
              className="bg-white/5 border-white/10 text-white text-sm h-8"
            />
          </div>
        </div>
      </div>

      {/* Error Handling */}
      <div>
        <h3 className="text-xs font-semibold text-gray-400 uppercase tracking-wide mb-3">
          Error Handling
        </h3>
        <div className="space-y-3">
          <label className="flex items-center gap-2 cursor-pointer">
            <input
              type="checkbox"
              checked={localData.stopOnError ?? true}
              onChange={(e) => handleChange("stopOnError", e.target.checked)}
              className="rounded"
            />
            <div>
              <p className="text-xs text-white">Stop on Error</p>
              <p className="text-[10px] text-gray-500">Pause sequence if a step fails</p>
            </div>
          </label>

          <label className="flex items-center gap-2 cursor-pointer">
            <input
              type="checkbox"
              checked={localData.continueOnError ?? false}
              onChange={(e) => handleChange("continueOnError", e.target.checked)}
              className="rounded"
            />
            <div>
              <p className="text-xs text-white">Continue on Error</p>
              <p className="text-[10px] text-gray-500">Skip failed steps and continue</p>
            </div>
          </label>
        </div>
      </div>

      {/* Subtasks Count */}
      {localData.subtasks && localData.subtasks.length > 0 && (
        <div>
          <Label className="text-white text-xs mb-1.5 block">Steps</Label>
          <div className="p-2 rounded bg-white/5 border border-white/10">
            <p className="text-[10px] text-gray-400">
              {localData.subtasks.length} step{localData.subtasks.length !== 1 ? "s" : ""} configured
            </p>
          </div>
        </div>
      )}

      {/* Info */}
      <div className="p-3 rounded-lg bg-blue-500/10 border border-blue-500/20">
        <p className="text-[10px] text-blue-300">
          Sequential nodes execute steps in order, with each step starting only after the previous one completes.
        </p>
      </div>
    </div>
  );
});

SequentialProperties.displayName = "SequentialProperties";
