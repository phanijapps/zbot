// ============================================================================
// VISUAL FLOW BUILDER - PROPERTIES PANEL
// Right panel for configuring selected node properties
// ============================================================================

import { memo, useState } from "react";
import type { PropertiesPanelProps } from "../types";
import { NODE_COLORS } from "../constants";
import { Button } from "@/shared/ui/button";
import { AgentProperties } from "./AgentProperties";
import { TriggerProperties } from "./TriggerProperties";
import { ParallelProperties } from "./ParallelProperties";
import { SequentialProperties } from "./SequentialProperties";
import { ConditionalProperties } from "./ConditionalProperties";
import { LoopProperties } from "./LoopProperties";
import { AggregatorProperties } from "./AggregatorProperties";
import { SubtaskProperties } from "./SubtaskProperties";
import { ValidationPanel } from "./ValidationPanel";
import { YamlPreview } from "./YamlPreview";

// -----------------------------------------------------------------------------
// Icons
// -----------------------------------------------------------------------------

const XIcon = () => (
  <svg className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="M18 6 6 18M6 6l12 12" />
  </svg>
);

// Node type icons
const BotIcon = () => (
  <svg className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="M12 8V4H8" /><rect width="16" height="12" x="4" y="8" rx="2" /><path d="M2 14h2" /><path d="M20 14h2" /><path d="M15 13v2" /><path d="M9 13v2" />
  </svg>
);

const PlayIcon = () => (
  <svg className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <polygon points="5 3 19 12 5 21 5 3" />
  </svg>
);

const ZapIcon = () => (
  <svg className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2" />
  </svg>
);

const ArrowRightIcon = () => (
  <svg className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="M5 12h14" /><path d="m12 5 7 7-7 7" />
  </svg>
);

const GitBranchIcon = () => (
  <svg className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="M6 3v12" /><circle cx="18" cy="6" r="3" /><circle cx="6" cy="18" r="3" /><path d="M18 9a9 9 0 0 1-9 9" />
  </svg>
);

const RepeatIcon = () => (
  <svg className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="m17 2 4 4-4 4" /><path d="M3 11V9a4 4 0 0 1 4-4h14" /><path d="m7 22-4-4 4-4" /><path d="M21 13v2a4 4 0 0 1-4 4H3" />
  </svg>
);

const MergeIcon = () => (
  <svg className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="m6 8 6 6-6 6" /><path d="m18 8-6 6 6 6" />
  </svg>
);

const ListChecksIcon = () => (
  <svg className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="M9 11 3 17l-2-2" /><path d="m21 9-5-5-5 5" /><path d="M11 14h10" /><path d="M11 18h7" />
  </svg>
);

// Get icon for node type
function getNodeIcon(nodeType: string): React.ReactElement {
  const icons: Record<string, React.ReactElement> = {
    agent: <BotIcon />,
    trigger: <PlayIcon />,
    parallel: <ZapIcon />,
    sequential: <ArrowRightIcon />,
    conditional: <GitBranchIcon />,
    loop: <RepeatIcon />,
    aggregator: <MergeIcon />,
    subtask: <ListChecksIcon />,
  };
  return icons[nodeType] || <BotIcon />;
}

// -----------------------------------------------------------------------------
// Main Properties Panel Component
// -----------------------------------------------------------------------------

export const PropertiesPanel = memo(({
  node,
  onClose,
  onUpdate,
  validationResults = [],
}: PropertiesPanelProps) => {
  const [activeTab, setActiveTab] = useState<"properties" | "yaml">("properties");
  if (!node) {
    return (
      <div className="w-[280px] bg-[#141414] border-l border-white/10 flex flex-col">
        <div className="p-4 border-b border-white/10">
          <h2 className="text-sm font-semibold text-white">Properties</h2>
        </div>
        <div className="flex-1 flex items-center justify-center p-6">
          <p className="text-sm text-gray-500 text-center">
            Select a node to view its properties
          </p>
        </div>
      </div>
    );
  }

  const nodeStyle = NODE_COLORS[node.type] || NODE_COLORS.agent;
  const nodeIcon = getNodeIcon(node.type);

  // Render properties based on node type
  const renderProperties = () => {
    switch (node.type) {
      case "agent":
        return <AgentProperties node={node} onUpdate={onUpdate} />;
      case "trigger":
        return <TriggerProperties node={node} onUpdate={onUpdate} />;
      case "parallel":
        return <ParallelProperties node={node} onUpdate={onUpdate} />;
      case "sequential":
        return <SequentialProperties node={node} onUpdate={onUpdate} />;
      case "conditional":
        return <ConditionalProperties node={node} onUpdate={onUpdate} />;
      case "loop":
        return <LoopProperties node={node} onUpdate={onUpdate} />;
      case "aggregator":
        return <AggregatorProperties node={node} onUpdate={onUpdate} />;
      case "subtask":
        return <SubtaskProperties node={node} onUpdate={onUpdate} />;
      default:
        return (
          <div className="space-y-4">
            <div>
              <label className="text-white text-xs mb-1.5 block">Display Name</label>
              <input
                type="text"
                value={node.data.displayName || ""}
                onChange={(e) => onUpdate({ ...node, data: { ...node.data, displayName: e.target.value } })}
                placeholder="Node Name"
                className="w-full bg-white/5 border border-white/10 rounded px-3 py-1.5 text-white text-sm placeholder:text-gray-600 focus:outline-none focus:ring-1 focus:ring-violet-500"
              />
            </div>
            <div className="text-xs text-gray-500 italic">
              Properties for {node.type} nodes
            </div>
          </div>
        );
    }
  };

  return (
    <div className="w-[280px] bg-[#141414] border-l border-white/10 flex flex-col">
      {/* Header */}
      <div className="p-4 border-b border-white/10 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <div className={`p-1.5 rounded ${nodeStyle.icon} bg-white/10`}>
            {nodeIcon}
          </div>
          <div>
            <h2 className="text-sm font-semibold text-white">Properties</h2>
            <p className="text-[10px] text-gray-400 capitalize">{node.type}</p>
          </div>
        </div>
        <button
          onClick={onClose}
          className="p-1 rounded hover:bg-white/10 text-gray-400 hover:text-white transition-colors"
        >
          <XIcon />
        </button>
      </div>

      {/* Content */}
      <div className="flex-1 flex flex-col overflow-hidden">
        {/* Tabs */}
        <div className="flex border-b border-white/10">
          <button
            onClick={() => setActiveTab("properties")}
            className={`flex-1 px-4 py-2 text-xs font-medium transition-colors ${
              activeTab === "properties"
                ? "text-white border-b-2 border-violet-500"
                : "text-gray-500 hover:text-gray-400"
            }`}
          >
            Properties
          </button>
          <button
            onClick={() => setActiveTab("yaml")}
            className={`flex-1 px-4 py-2 text-xs font-medium transition-colors ${
              activeTab === "yaml"
                ? "text-white border-b-2 border-violet-500"
                : "text-gray-500 hover:text-gray-400"
            }`}
          >
            YAML
          </button>
        </div>

        {/* Tab Content */}
        <div className="flex-1 overflow-y-auto p-4">
          {activeTab === "properties" ? (
            <div className="space-y-4">
              {renderProperties()}

              {/* Validation Panel */}
              {node && (
                <div>
                  <h3 className="text-xs font-semibold text-gray-400 uppercase tracking-wide mb-3">
                    Validation
                  </h3>
                  <ValidationPanel
                    validationResults={validationResults}
                    nodeId={node.id}
                  />
                </div>
              )}
            </div>
          ) : (
            <YamlPreview node={node} />
          )}
        </div>
      </div>

      {/* Footer - Quick Actions */}
      <div className="p-3 border-t border-white/10">
        <div className="flex gap-2">
          <Button
            variant="outline"
            size="sm"
            className="flex-1 h-7 text-xs border-white/20 text-white hover:bg-white/5"
          >
            Duplicate
          </Button>
          <Button
            variant="outline"
            size="sm"
            className="flex-1 h-7 text-xs border-red-500/30 text-red-400 hover:bg-red-500/10"
          >
            Delete
          </Button>
        </div>
      </div>
    </div>
  );
});

PropertiesPanel.displayName = "PropertiesPanel";
