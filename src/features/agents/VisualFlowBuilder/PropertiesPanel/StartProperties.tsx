// ============================================================================
// ZERO IDE - START PROPERTIES
// Properties panel for Start event nodes (BPMN-style)
// ============================================================================

import { memo, useState, useEffect } from "react";
import type { BaseNode, StartNodeData } from "../types";
import { Label } from "@/shared/ui/label";
import { Input } from "@/shared/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/shared/ui/select";

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface StartPropertiesProps {
  node: BaseNode;
  onUpdate: (updates: Partial<BaseNode>) => void;
}

// -----------------------------------------------------------------------------
// Helper: Get start node data
// -----------------------------------------------------------------------------

function getStartNodeData(data: unknown): StartNodeData {
  if (!data || typeof data !== "object") {
    return {
      displayName: "Start",
      triggerType: "manual",
    };
  }

  const d = data as Record<string, unknown>;

  return {
    displayName: String(d.displayName ?? "Start"),
    triggerType: (d.triggerType as StartNodeData["triggerType"]) ?? "manual",
    schedule: d.schedule ? String(d.schedule) : undefined,
  };
}

// -----------------------------------------------------------------------------
// Component
// -----------------------------------------------------------------------------

export const StartProperties = memo(({ node, onUpdate }: StartPropertiesProps) => {
  const data = getStartNodeData(node.data);
  const [localData, setLocalData] = useState<StartNodeData>(data);

  useEffect(() => {
    setLocalData(data);
  }, [node.data]);

  const handleChange = (field: keyof StartNodeData, value: unknown) => {
    const newData = { ...localData, [field]: value };
    setLocalData(newData);
    onUpdate({ ...node, data: newData });
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
              placeholder="Start"
              className="bg-white/5 border-white/10 text-white text-sm h-8"
            />
          </div>
        </div>
      </div>

      {/* Trigger Configuration */}
      <div>
        <h3 className="text-xs font-semibold text-gray-400 uppercase tracking-wide mb-3">
          Trigger Configuration
        </h3>
        <div className="space-y-3">
          <div>
            <Label className="text-white text-xs mb-1.5 block">Trigger Type</Label>
            <Select
              value={localData.triggerType}
              onValueChange={(value: StartNodeData["triggerType"]) => handleChange("triggerType", value)}
            >
              <SelectTrigger size="sm" className="bg-white/5 border-white/10 text-white">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="manual">Manual</SelectItem>
                <SelectItem value="scheduled">Scheduled</SelectItem>
                <SelectItem value="webhook">Webhook</SelectItem>
              </SelectContent>
            </Select>
            <p className="text-[10px] text-gray-500 mt-1">
              {localData.triggerType === "manual" && "Triggered manually by the user via chat interface"}
              {localData.triggerType === "scheduled" && "Triggered automatically on a schedule (cron expression)"}
              {localData.triggerType === "webhook" && "Triggered by an incoming webhook request"}
            </p>
          </div>

          {localData.triggerType === "scheduled" && (
            <div>
              <Label className="text-white text-xs mb-1.5 block">Schedule (Cron Expression)</Label>
              <Input
                value={localData.schedule || ""}
                onChange={(e) => handleChange("schedule", e.target.value)}
                placeholder="0 0 * * *"
                className="bg-white/5 border-white/10 text-white text-sm h-8 font-mono"
              />
              <p className="text-[10px] text-gray-500 mt-1">
                Unix cron format: minute hour day month weekday
              </p>
            </div>
          )}
        </div>
      </div>

      {/* Info */}
      <div className="p-3 rounded-lg bg-green-500/10 border border-green-500/20">
        <p className="text-[10px] text-green-300">
          The Start event defines how the workflow is triggered. For manual triggers, the workflow starts when the user initiates it via the chat interface.
        </p>
      </div>
    </div>
  );
});

StartProperties.displayName = "StartProperties";
