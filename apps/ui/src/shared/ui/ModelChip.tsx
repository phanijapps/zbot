// ============================================================================
// MODEL CHIP
// Displays a model name with capability badges and optional context window
// ============================================================================

import { Wrench, Eye, Brain, Volume2, X } from "lucide-react";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/shared/ui/tooltip";
import type { ModelProfile } from "@/services/transport";
import { formatContextWindow } from "@/shared/utils/format";

interface ModelChipProps {
  modelId: string;
  profile?: ModelProfile;
  showContext?: boolean;
  removable?: boolean;
  onRemove?: () => void;
}

const CAPABILITY_ICONS = [
  { key: "tools" as const, icon: Wrench, label: "Tool Calling" },
  { key: "vision" as const, icon: Eye, label: "Vision" },
  { key: "thinking" as const, icon: Brain, label: "Thinking" },
  { key: "voice" as const, icon: Volume2, label: "Voice" },
];

export function ModelChip({ modelId, profile, showContext, removable, onRemove }: ModelChipProps) {
  const caps = profile?.capabilities;
  const ctx = profile?.context;

  return (
    <span className={removable ? "model-chip model-chip--removable" : "model-chip"}>
      <span className="model-chip__name">{modelId}</span>
      {caps && (
        <TooltipProvider delayDuration={300}>
          <span className="model-chip__capabilities">
            {CAPABILITY_ICONS.map(({ key, icon: Icon, label }) =>
              caps[key] ? (
                <Tooltip key={key}>
                  <TooltipTrigger asChild>
                    <Icon />
                  </TooltipTrigger>
                  <TooltipContent side="top" className="text-xs">
                    {label}
                  </TooltipContent>
                </Tooltip>
              ) : null
            )}
          </span>
        </TooltipProvider>
      )}
      {showContext && ctx && (
        <span className="model-chip__context">{formatContextWindow(ctx.input)}</span>
      )}
      {removable && onRemove && (
        <button className="model-chip__remove" onClick={onRemove} aria-label={`Remove ${modelId}`}>
          <X size={12} />
        </button>
      )}
    </span>
  );
}
