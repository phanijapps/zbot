// ============================================================================
// VISUAL FLOW BUILDER - AGENT NODE
// Agent node component with model, tools, and MCPs display
// ============================================================================

import { memo } from "react";
import { BaseNode } from "./BaseNode";
import type { NodeProps } from "../types";
import type { AgentNodeData } from "../types";

// -----------------------------------------------------------------------------
// Icons
// -----------------------------------------------------------------------------

const CpuIcon = () => (
  <svg className="w-3 h-3" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <rect x="4" y="4" width="16" height="16" rx="2" /><path d="M9 9h6" /><path d="M9 12h6" /><path d="M9 15h6" /><path d="M9 2v2" /><path d="M15 2v2" /><path d="M9 20v2" /><path d="M15 20v2" /><path d="M20 9h2" /><path d="M20 14h2" /><path d="M2 9h2" /><path d="M2 14h2" />
  </svg>
);

const PlugIcon = () => (
  <svg className="w-3 h-3" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="M12 2v20M2 12h20" />
  </svg>
);

const ToolIcon = () => (
  <svg className="w-3 h-3" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z" />
  </svg>
);

// -----------------------------------------------------------------------------
// Helper to safely get agent data
// -----------------------------------------------------------------------------

function getAgentData(data: unknown): AgentNodeData | null {
  if (!data || typeof data !== "object") return null;

  const d = data as Record<string, unknown>;

  // Check if this has the expected structure
  if ("displayName" in d && "providerId" in d && "model" in d) {
    return {
      displayName: String(d.displayName ?? ""),
      description: String(d.description ?? ""),
      providerId: String(d.providerId ?? ""),
      model: String(d.model ?? ""),
      temperature: Number(d.temperature ?? 0.7),
      maxTokens: Number(d.maxTokens ?? 4096),
      tools: (d.tools as string[]) ?? [],
      mcps: (d.mcps as string[]) ?? [],
      skills: (d.skills as string[]) ?? [],
      systemInstructions: String(d.systemInstructions ?? ""),
      middleware: (d.middleware as string[]) ?? [],
    };
  }

  return null;
}

// -----------------------------------------------------------------------------
// Agent Node Component
// -----------------------------------------------------------------------------

export const AgentNode = memo((props: NodeProps) => {
  const agentData = getAgentData(props.node.data);

  if (!agentData) {
    // Fallback for invalid data
    return (
      <BaseNode {...props}>
        <div className="text-xs text-red-400">Invalid agent configuration</div>
      </BaseNode>
    );
  }

  // Count items
  const toolsCount = agentData.tools?.length ?? 0;
  const mcpsCount = agentData.mcps?.length ?? 0;
  const skillsCount = agentData.skills?.length ?? 0;

  return (
    <BaseNode {...props}>
      <div className="flex flex-col h-full justify-between">
        {/* Top: Model info */}
        <div className="space-y-1">
          {agentData.model ? (
            <div className="flex items-center gap-1.5">
              <CpuIcon />
              <span className="text-white text-xs font-medium truncate">
                {agentData.model}
              </span>
            </div>
          ) : (
            <div className="text-xs text-gray-500 italic">No model selected</div>
          )}

          {agentData.description && (
            <p className="text-[10px] text-gray-400 line-clamp-2">
              {agentData.description}
            </p>
          )}
        </div>

        {/* Bottom: Stats badges */}
        <div className="flex flex-wrap gap-1.5 mt-auto">
          {toolsCount > 0 && (
            <div className="flex items-center gap-1 px-1.5 py-0.5 rounded bg-violet-500/10 border border-violet-500/20">
              <ToolIcon />
              <span className="text-[10px] text-violet-300">{toolsCount}</span>
            </div>
          )}

          {mcpsCount > 0 && (
            <div className="flex items-center gap-1 px-1.5 py-0.5 rounded bg-blue-500/10 border border-blue-500/20">
              <PlugIcon />
              <span className="text-[10px] text-blue-300">{mcpsCount}</span>
            </div>
          )}

          {skillsCount > 0 && (
            <div className="px-1.5 py-0.5 rounded bg-green-500/10 border border-green-500/20">
              <span className="text-[10px] text-green-300">
                📚 {skillsCount}
              </span>
            </div>
          )}

          {/* Temperature indicator */}
          <div className="ml-auto flex items-center gap-1">
            <div className="w-8 h-1.5 rounded-full bg-white/10 overflow-hidden">
              <div
                className="h-full rounded-full bg-gradient-to-r from-blue-500 to-purple-500"
                style={{ width: `${(agentData.temperature / 2) * 100}%` }}
              />
            </div>
            <span className="text-[9px] text-gray-500">{agentData.temperature.toFixed(1)}</span>
          </div>
        </div>
      </div>
    </BaseNode>
  );
});

AgentNode.displayName = "AgentNode";
