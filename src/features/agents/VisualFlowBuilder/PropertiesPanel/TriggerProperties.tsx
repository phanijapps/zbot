// ============================================================================
// VISUAL FLOW BUILDER - TRIGGER PROPERTIES
// Properties panel for trigger nodes
// ============================================================================

import { memo, useState, useEffect } from "react";
import type { BaseNode } from "../types";
import { Label } from "@/shared/ui/label";
import { Input } from "@/shared/ui/input";

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface TriggerPropertiesProps {
  node: BaseNode;
  onUpdate: (updates: Partial<BaseNode>) => void;
}

interface TriggerData {
  displayName?: string;
  triggerType?: "manual" | "scheduled";
  schedule?: string;
  timezone?: string;
}

// -----------------------------------------------------------------------------
// Helper: Get trigger data
// -----------------------------------------------------------------------------

function getTriggerData(data: unknown): TriggerData {
  if (!data || typeof data !== "object") return {};
  const d = data as Record<string, unknown>;
  return {
    displayName: d.displayName ? String(d.displayName) : undefined,
    triggerType: (d.triggerType === "scheduled" ? "scheduled" : "manual") as TriggerData["triggerType"],
    schedule: d.schedule ? String(d.schedule) : undefined,
    timezone: d.timezone ? String(d.timezone) : undefined,
  };
}

// -----------------------------------------------------------------------------
// Component
// -----------------------------------------------------------------------------

export const TriggerProperties = memo(({ node, onUpdate }: TriggerPropertiesProps) => {
  const data = getTriggerData(node.data);
  const [localData, setLocalData] = useState<TriggerData>(data);

  useEffect(() => {
    setLocalData(data);
  }, [node.data]);

  const handleChange = (field: keyof TriggerData, value: unknown) => {
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
              placeholder="Start Trigger"
              className="bg-white/5 border-white/10 text-white text-sm h-8"
            />
          </div>

          <div>
            <Label className="text-white text-xs mb-1.5 block">Trigger Type</Label>
            <select
              value={localData.triggerType || "manual"}
              onChange={(e) => handleChange("triggerType", e.target.value)}
              className="w-full bg-white/5 border border-white/10 rounded px-3 py-1.5 text-white text-sm focus:outline-none focus:ring-1 focus:ring-violet-500"
            >
              <option value="manual">Manual Trigger</option>
              <option value="scheduled">Scheduled Trigger</option>
            </select>
          </div>

          {localData.triggerType === "scheduled" && (
            <>
              <div>
                <Label className="text-white text-xs mb-1.5 block">Schedule</Label>
                <Input
                  value={localData.schedule || ""}
                  onChange={(e) => handleChange("schedule", e.target.value)}
                  placeholder="0 9 * * * (cron expression)"
                  className="bg-white/5 border-white/10 text-white text-sm h-8"
                />
                <p className="text-[10px] text-gray-500 mt-1">Use cron expression (e.g., "0 9 * * *" for daily at 9am)</p>
              </div>

              <div>
                <Label className="text-white text-xs mb-1.5 block">Timezone</Label>
                <Input
                  value={localData.timezone || "UTC"}
                  onChange={(e) => handleChange("timezone", e.target.value)}
                  placeholder="UTC"
                  className="bg-white/5 border-white/10 text-white text-sm h-8"
                />
              </div>
            </>
          )}
        </div>
      </div>

      {/* Info */}
      <div className="p-3 rounded-lg bg-blue-500/10 border border-blue-500/20">
        <p className="text-[10px] text-blue-300">
          {localData.triggerType === "manual"
            ? "Manual triggers start the workflow when clicked by a user."
            : "Scheduled triggers automatically start the workflow according to the cron schedule."}
        </p>
      </div>
    </div>
  );
});

TriggerProperties.displayName = "TriggerProperties";
