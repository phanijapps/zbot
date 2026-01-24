// ============================================================================
// VISUAL FLOW BUILDER - LOOP PROPERTIES
// Properties panel for loop nodes
// ============================================================================

import { memo, useState, useEffect } from "react";
import type { BaseNode } from "../types";
import { Label } from "@/shared/ui/label";
import { Input } from "@/shared/ui/input";

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface LoopPropertiesProps {
  node: BaseNode;
  onUpdate: (updates: Partial<BaseNode>) => void;
}

interface LoopData {
  displayName?: string;
  exitCondition?: string;
  maxIterations?: number;
  delayMs?: number;
  breakOnError?: boolean;
}

// -----------------------------------------------------------------------------
// Helper: Get loop data
// -----------------------------------------------------------------------------

function getLoopData(data: unknown): LoopData {
  if (!data || typeof data !== "object") return {};
  const d = data as Record<string, unknown>;
  return {
    displayName: d.displayName ? String(d.displayName) : undefined,
    exitCondition: d.exitCondition ? String(d.exitCondition) : undefined,
    maxIterations: d.maxIterations ? Number(d.maxIterations) : undefined,
    delayMs: d.delayMs ? Number(d.delayMs) : undefined,
    breakOnError: d.breakOnError ? Boolean(d.breakOnError) : false,
  };
}

// -----------------------------------------------------------------------------
// Component
// -----------------------------------------------------------------------------

export const LoopProperties = memo(({ node, onUpdate }: LoopPropertiesProps) => {
  const data = getLoopData(node.data);
  const [localData, setLocalData] = useState<LoopData>(data);

  useEffect(() => {
    setLocalData(data);
  }, [node.data]);

  const handleChange = (field: keyof LoopData, value: unknown) => {
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
              placeholder="Loop"
              className="bg-white/5 border-white/10 text-white text-sm h-8"
            />
          </div>
        </div>
      </div>

      {/* Loop Configuration */}
      <div>
        <h3 className="text-xs font-semibold text-gray-400 uppercase tracking-wide mb-3">
          Loop Configuration
        </h3>
        <div className="space-y-3">
          <div>
            <Label className="text-white text-xs mb-1.5 block">Exit Condition</Label>
            <textarea
              value={localData.exitCondition || ""}
              onChange={(e) => handleChange("exitCondition", e.target.value)}
              placeholder={`{{quality_score}} >= 0.9`}
              rows={3}
              className="w-full bg-white/5 border border-white/10 rounded px-3 py-2 text-white text-sm font-mono placeholder:text-gray-600 resize-none focus:outline-none focus:ring-1 focus:ring-violet-500"
            />
            <p className="text-[10px] text-gray-500 mt-1">Expression that evaluates to true to exit the loop</p>
          </div>

          <div>
            <Label className="text-white text-xs mb-1.5 block">Max Iterations</Label>
            <Input
              type="number"
              min={1}
              max={100}
              value={localData.maxIterations ?? 10}
              onChange={(e) => handleChange("maxIterations", parseInt(e.target.value) || 10)}
              className="bg-white/5 border-white/10 text-white text-sm h-8"
            />
            <p className="text-[10px] text-gray-500 mt-1">Safety limit to prevent infinite loops</p>
          </div>

          <div>
            <Label className="text-white text-xs mb-1.5 block">Delay Between Iterations (ms)</Label>
            <Input
              type="number"
              min={0}
              value={localData.delayMs ?? 0}
              onChange={(e) => handleChange("delayMs", parseInt(e.target.value) || 0)}
              className="bg-white/5 border-white/10 text-white text-sm h-8"
            />
            <p className="text-[10px] text-gray-500 mt-1">Optional delay in milliseconds</p>
          </div>
        </div>
      </div>

      {/* Error Handling */}
      <div>
        <h3 className="text-xs font-semibold text-gray-400 uppercase tracking-wide mb-3">
          Error Handling
        </h3>
        <label className="flex items-center gap-2 cursor-pointer">
          <input
            type="checkbox"
            checked={localData.breakOnError ?? false}
            onChange={(e) => handleChange("breakOnError", e.target.checked)}
            className="rounded"
          />
          <div>
            <p className="text-xs text-white">Break on Error</p>
            <p className="text-[10px] text-gray-500">Exit loop immediately if an error occurs</p>
          </div>
        </label>
      </div>

      {/* Warning */}
      {(!localData.exitCondition || localData.exitCondition.trim() === "") && (
        <div className="p-3 rounded-lg bg-yellow-500/10 border border-yellow-500/20">
          <p className="text-[10px] text-yellow-300">
            ⚠️ Warning: No exit condition set. This loop may run indefinitely until max iterations.
          </p>
        </div>
      )}

      {/* Info */}
      <div className="p-3 rounded-lg bg-orange-500/10 border border-orange-500/20">
        <p className="text-[10px] text-orange-300">
          Loop nodes repeat their content until the exit condition is true or max iterations is reached.
        </p>
      </div>
    </div>
  );
});

LoopProperties.displayName = "LoopProperties";
