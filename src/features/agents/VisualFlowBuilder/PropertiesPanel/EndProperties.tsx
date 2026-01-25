// ============================================================================
// ZERO IDE - END PROPERTIES
// Properties panel for End event nodes (BPMN-style)
// ============================================================================

import { memo, useState, useEffect } from "react";
import type { BaseNode, EndNodeData, NodeData } from "../types";
import { Label } from "@/shared/ui/label";
import { Input } from "@/shared/ui/input";

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface EndPropertiesProps {
  node: BaseNode;
  onUpdate: (updates: Partial<BaseNode>) => void;
}

// -----------------------------------------------------------------------------
// Helper: Get end node data
// -----------------------------------------------------------------------------

function getEndNodeData(data: unknown): EndNodeData {
  if (!data || typeof data !== "object") {
    return {
      displayName: "End",
    };
  }

  const d = data as Record<string, unknown>;

  return {
    displayName: String(d.displayName ?? "End"),
  };
}

// -----------------------------------------------------------------------------
// Component
// -----------------------------------------------------------------------------

export const EndProperties = memo(({ node, onUpdate }: EndPropertiesProps) => {
  const data = getEndNodeData(node.data);
  const [localData, setLocalData] = useState<EndNodeData>(data);

  useEffect(() => {
    setLocalData(data);
  }, [node.data]);

  const handleChange = (field: keyof EndNodeData, value: string) => {
    const newData: EndNodeData = { ...localData, [field]: value };
    setLocalData(newData);
    onUpdate({ ...node, data: newData as NodeData });
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
              placeholder="End"
              className="bg-white/5 border-white/10 text-white text-sm h-8"
            />
          </div>
        </div>
      </div>

      {/* Info */}
      <div className="p-3 rounded-lg bg-red-500/10 border border-red-500/20">
        <p className="text-[10px] text-red-300">
          The End event marks the termination point of the workflow. When the Orchestrator reaches this point, the workflow completes and returns any results to the user.
        </p>
      </div>

      {/* Additional Info */}
      <div className="p-3 rounded-lg bg-gray-500/10 border border-gray-500/20">
        <p className="text-[10px] text-gray-300">
          End events have no additional configuration. The workflow completes when the Orchestrator agent reaches a natural conclusion or determines the task is complete.
        </p>
      </div>
    </div>
  );
});

EndProperties.displayName = "EndProperties";
