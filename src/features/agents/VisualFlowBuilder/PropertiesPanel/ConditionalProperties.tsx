// ============================================================================
// VISUAL FLOW BUILDER - CONDITIONAL PROPERTIES
// Properties panel for conditional nodes
// ============================================================================

import { memo, useState, useEffect } from "react";
import type { BaseNode } from "../types";
import { Label } from "@/shared/ui/label";
import { Input } from "@/shared/ui/input";
import { Button } from "@/shared/ui/button";

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface ConditionalPropertiesProps {
  node: BaseNode;
  onUpdate: (updates: Partial<BaseNode>) => void;
}

interface Condition {
  expression: string;
  label: string;
  targetNodeId?: string;
}

interface ConditionalData {
  displayName?: string;
  conditions?: Condition[];
  defaultTarget?: string;
  variable?: string;
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
// Helper: Get conditional data
// -----------------------------------------------------------------------------

function getConditionalData(data: unknown): ConditionalData {
  if (!data || typeof data !== "object") return { conditions: [] };
  const d = data as Record<string, unknown>;
  return {
    displayName: d.displayName ? String(d.displayName) : undefined,
    conditions: d.conditions ? d.conditions as Condition[] : [],
    defaultTarget: d.defaultTarget ? String(d.defaultTarget) : undefined,
    variable: d.variable ? String(d.variable) : undefined,
  };
}

// -----------------------------------------------------------------------------
// Component
// -----------------------------------------------------------------------------

export const ConditionalProperties = memo(({ node, onUpdate }: ConditionalPropertiesProps) => {
  const data = getConditionalData(node.data);
  const [localData, setLocalData] = useState<ConditionalData>(data);

  useEffect(() => {
    setLocalData(data);
  }, [node.data]);

  const handleChange = (field: keyof ConditionalData, value: unknown) => {
    const newData = { ...localData, [field]: value };
    setLocalData(newData);
    onUpdate({ ...node, data: { ...node.data, ...newData } as typeof node.data });
  };

  const handleConditionChange = (index: number, field: keyof Condition, value: string) => {
    const conditions = [...(localData.conditions || [])];
    conditions[index] = { ...conditions[index], [field]: value };
    handleChange("conditions", conditions);
  };

  const addCondition = () => {
    const conditions = [...(localData.conditions || []), { expression: "", label: `Branch ${(localData.conditions?.length || 0) + 1}` }];
    handleChange("conditions", conditions);
  };

  const removeCondition = (index: number) => {
    const conditions = (localData.conditions || []).filter((_, i) => i !== index);
    handleChange("conditions", conditions);
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
              placeholder="Conditional Router"
              className="bg-white/5 border-white/10 text-white text-sm h-8"
            />
          </div>

          <div>
            <Label className="text-white text-xs mb-1.5 block">Variable to Check</Label>
            <Input
              value={localData.variable || ""}
              onChange={(e) => handleChange("variable", e.target.value)}
              placeholder="task_type"
              className="bg-white/5 border-white/10 text-white text-sm h-8"
            />
            <p className="text-[10px] text-gray-500 mt-1">Variable name to evaluate for routing</p>
          </div>
        </div>
      </div>

      {/* Conditions */}
      <div>
        <div className="flex items-center justify-between mb-3">
          <h3 className="text-xs font-semibold text-gray-400 uppercase tracking-wide">
            Conditions
          </h3>
          <Button
            onClick={addCondition}
            variant="outline"
            size="sm"
            className="h-6 px-2 text-xs border-white/20 text-white hover:bg-white/5"
          >
            <PlusIcon />
            <span className="ml-1">Add</span>
          </Button>
        </div>

        <div className="space-y-2">
          {(localData.conditions || []).map((condition, index) => (
            <div key={index} className="p-2 rounded bg-white/5 border border-white/10 space-y-2">
              <div className="flex items-center justify-between">
                <Label className="text-white text-xs">Branch {index + 1}</Label>
                <button
                  onClick={() => removeCondition(index)}
                  className="p-1 rounded hover:bg-red-500/10 text-gray-500 hover:text-red-400 transition-colors"
                >
                  <TrashIcon />
                </button>
              </div>

              <div>
                <Label className="text-white text-[10px] mb-1 block">Label</Label>
                <Input
                  value={condition.label}
                  onChange={(e) => handleConditionChange(index, "label", e.target.value)}
                  placeholder={`Branch ${index + 1}`}
                  className="bg-white/5 border-white/10 text-white text-xs h-7"
                />
              </div>

              <div>
                <Label className="text-white text-[10px] mb-1 block">Expression</Label>
                <Input
                  value={condition.expression}
                  onChange={(e) => handleConditionChange(index, "expression", e.target.value)}
                  placeholder={`{{${localData.variable || "value"}}} === "research"`}
                  className="bg-white/5 border-white/10 text-white text-xs h-7 font-mono"
                />
              </div>
            </div>
          ))}

          {(localData.conditions || []).length === 0 && (
            <div className="p-3 rounded border border-dashed border-white/20 text-center">
              <p className="text-[10px] text-gray-500">No conditions defined</p>
              <p className="text-[10px] text-gray-600">Add at least 2 conditions to create branches</p>
            </div>
          )}
        </div>
      </div>

      {/* Info */}
      <div className="p-3 rounded-lg bg-orange-500/10 border border-orange-500/20">
        <p className="text-[10px] text-orange-300">
          Conditional nodes route to different branches based on expression evaluation.
        </p>
      </div>
    </div>
  );
});

ConditionalProperties.displayName = "ConditionalProperties";
