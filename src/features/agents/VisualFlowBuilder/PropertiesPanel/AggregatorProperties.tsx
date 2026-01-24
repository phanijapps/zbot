// ============================================================================
// VISUAL FLOW BUILDER - AGGREGATOR PROPERTIES
// Properties panel for aggregator nodes
// ============================================================================

import { memo, useState, useEffect } from "react";
import type { BaseNode } from "../types";
import { Label } from "@/shared/ui/label";
import { Input } from "@/shared/ui/input";

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface AggregatorPropertiesProps {
  node: BaseNode;
  onUpdate: (updates: Partial<BaseNode>) => void;
}

interface AggregatorData {
  displayName?: string;
  strategy?: "concat" | "all" | "first" | "last" | "summarize" | "vote";
  template?: string;
  sortBy?: string;
  sortOrder?: "asc" | "desc";
}

// -----------------------------------------------------------------------------
// Helper: Get aggregator data
// -----------------------------------------------------------------------------

function getAggregatorData(data: unknown): AggregatorData {
  if (!data || typeof data !== "object") return {};
  const d = data as Record<string, unknown>;
  return {
    displayName: d.displayName ? String(d.displayName) : undefined,
    strategy: (d.strategy === "concat" || d.strategy === "all" || d.strategy === "first" || d.strategy === "last" || d.strategy === "summarize" || d.strategy === "vote")
      ? d.strategy as AggregatorData["strategy"]
      : "concat",
    template: d.template ? String(d.template) : undefined,
    sortBy: d.sortBy ? String(d.sortBy) : undefined,
    sortOrder: (d.sortOrder === "asc" || d.sortOrder === "desc") ? d.sortOrder : undefined,
  };
}

// -----------------------------------------------------------------------------
// Component
// -----------------------------------------------------------------------------

export const AggregatorProperties = memo(({ node, onUpdate }: AggregatorPropertiesProps) => {
  const data = getAggregatorData(node.data);
  const [localData, setLocalData] = useState<AggregatorData>(data);

  useEffect(() => {
    setLocalData(data);
  }, [node.data]);

  const handleChange = (field: keyof AggregatorData, value: unknown) => {
    const newData = { ...localData, [field]: value };
    setLocalData(newData);
    onUpdate({ ...node, data: { ...node.data, ...newData } as typeof node.data });
  };

  const strategies = [
    { value: "concat", label: "Concatenate", description: "Join all responses together" },
    { value: "all", label: "All Responses", description: "Return all responses as a list" },
    { value: "first", label: "First", description: "Return only the first response" },
    { value: "last", label: "Last", description: "Return only the last response" },
    { value: "summarize", label: "Summarize", description: "Generate a summary of all responses" },
    { value: "vote", label: "Vote", description: "Select most common response" },
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
              placeholder="Aggregator"
              className="bg-white/5 border-white/10 text-white text-sm h-8"
            />
          </div>
        </div>
      </div>

      {/* Merge Strategy */}
      <div>
        <h3 className="text-xs font-semibold text-gray-400 uppercase tracking-wide mb-3">
          Merge Strategy
        </h3>
        <div className="space-y-2">
          {strategies.map((strategy) => (
            <label
              key={strategy.value}
              className={`flex items-start gap-2 p-2 rounded border cursor-pointer transition-colors ${
                localData.strategy === strategy.value
                  ? "bg-violet-500/10 border-violet-500/30"
                  : "bg-white/5 border-white/10 hover:bg-white/10"
              }`}
            >
              <input
                type="radio"
                name="strategy"
                value={strategy.value}
                checked={localData.strategy === strategy.value}
                onChange={(e) => handleChange("strategy", e.target.value)}
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

      {/* Output Template */}
      {localData.strategy === "concat" && (
        <div>
          <Label className="text-white text-xs mb-1.5 block">Output Template</Label>
          <textarea
            value={localData.template || ""}
            onChange={(e) => handleChange("template", e.target.value)}
            placeholder={`Here are the responses:\n\n{{#each responses}}\n{{this}}\n\n{{/each}}`}
            rows={4}
            className="w-full bg-white/5 border border-white/10 rounded px-3 py-2 text-white text-sm placeholder:text-gray-600 resize-none focus:outline-none focus:ring-1 focus:ring-violet-500"
          />
          <p className="text-[10px] text-gray-500 mt-1">Template for formatting concatenated output (optional)</p>
        </div>
      )}

      {/* Sorting */}
      {(localData.strategy === "all" || localData.strategy === "concat") && (
        <div>
          <h3 className="text-xs font-semibold text-gray-400 uppercase tracking-wide mb-3">
            Sorting
          </h3>
          <div className="space-y-3">
            <div>
              <Label className="text-white text-xs mb-1.5 block">Sort By</Label>
              <Input
                value={localData.sortBy || ""}
                onChange={(e) => handleChange("sortBy", e.target.value)}
                placeholder="timestamp"
                className="bg-white/5 border-white/10 text-white text-sm h-8"
              />
              <p className="text-[10px] text-gray-500 mt-1">Field name to sort by (optional)</p>
            </div>

            {localData.sortBy && (
              <div>
                <Label className="text-white text-xs mb-1.5 block">Sort Order</Label>
                <select
                  value={localData.sortOrder || "asc"}
                  onChange={(e) => handleChange("sortOrder", e.target.value)}
                  className="w-full bg-white/5 border border-white/10 rounded px-3 py-1.5 text-white text-sm focus:outline-none focus:ring-1 focus:ring-violet-500"
                >
                  <option value="asc">Ascending</option>
                  <option value="desc">Descending</option>
                </select>
              </div>
            )}
          </div>
        </div>
      )}

      {/* Info */}
      <div className="p-3 rounded-lg bg-blue-500/10 border border-blue-500/20">
        <p className="text-[10px] text-blue-300">
          Aggregator nodes combine multiple inputs into a single output using the selected strategy.
        </p>
      </div>
    </div>
  );
});

AggregatorProperties.displayName = "AggregatorProperties";
