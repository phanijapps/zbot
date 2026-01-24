// ============================================================================
// VISUAL FLOW BUILDER - YAML PREVIEW
// YAML preview and edit component for agent configuration
// ============================================================================

import { memo, useState, useEffect, useMemo } from "react";
import type { BaseNode, AgentNodeData, TriggerNodeData, ParallelNodeData, SequentialNodeData, ConditionalNodeData, LoopNodeData, AggregatorNodeData, SubtaskNodeData } from "../types";

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface YamlPreviewProps {
  node: BaseNode | null;
  onUpdate?: (yaml: string) => void;
  readOnly?: boolean;
}

// -----------------------------------------------------------------------------
// Icons
// -----------------------------------------------------------------------------

const CopyIcon = () => (
  <svg className="w-3 h-3" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <rect width="14" height="14" x="8" y="8" rx="2" ry="2" />
    <path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2" />
  </svg>
);

const CheckIcon = () => (
  <svg className="w-3 h-3" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="M20 6 9 17l-5-5" />
  </svg>
);

const EditIcon = () => (
  <svg className="w-3 h-3" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="M17 3a2.85 2.83 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z" />
    <path d="m15 5 4 4" />
  </svg>
);

// -----------------------------------------------------------------------------
// Helper: Convert node data to YAML
// -----------------------------------------------------------------------------

function nodeToYaml(node: BaseNode): string {
  const lines: string[] = [];

  // Header
  lines.push(`# ${node.type.toUpperCase()} Node: ${node.data.displayName || node.id}`);
  lines.push(`id: ${node.id}`);
  lines.push(`type: ${node.type}`);

  // Position (commented out by default)
  lines.push(`# position:`);
  lines.push(`#   x: ${node.position.x}`);
  lines.push(`#   y: ${node.position.y}`);

  lines.push("");

  // Node-specific data
  switch (node.type) {
    case "agent":
      lines.push(...agentNodeToYaml(node.data as AgentNodeData));
      break;
    case "trigger":
      lines.push(...triggerNodeToYaml(node.data as TriggerNodeData));
      break;
    case "parallel":
      lines.push(...parallelNodeToYaml(node.data as ParallelNodeData));
      break;
    case "sequential":
      lines.push(...sequentialNodeToYaml(node.data as SequentialNodeData));
      break;
    case "conditional":
      lines.push(...conditionalNodeToYaml(node.data as ConditionalNodeData));
      break;
    case "loop":
      lines.push(...loopNodeToYaml(node.data as LoopNodeData));
      break;
    case "aggregator":
      lines.push(...aggregatorNodeToYaml(node.data as AggregatorNodeData));
      break;
    case "subtask":
      lines.push(...subtaskNodeToYaml(node.data as SubtaskNodeData));
      break;
  }

  return lines.join("\n");
}

function agentNodeToYaml(data: AgentNodeData): string[] {
  const lines: string[] = [];

  lines.push(`name: ${data.displayName || "Unnamed Agent"}`);
  if (data.description) {
    lines.push(`description: ${data.description}`);
  }

  lines.push("");
  lines.push(`# Model Configuration`);
  lines.push(`model:`);
  lines.push(`  provider: ${data.providerId || "openai"}`);
  lines.push(`  name: ${data.model || "gpt-4o"}`);
  lines.push(`  temperature: ${data.temperature ?? 0.7}`);
  lines.push(`  max_tokens: ${data.maxTokens ?? 4096}`);

  if (data.tools && data.tools.length > 0) {
    lines.push("");
    lines.push(`# Tools`);
    lines.push(`tools:`);
    for (const tool of data.tools) {
      lines.push(`  - ${tool}`);
    }
  }

  if (data.mcps && data.mcps.length > 0) {
    lines.push("");
    lines.push(`# MCP Servers`);
    lines.push(`mcps:`);
    for (const mcp of data.mcps) {
      lines.push(`  - ${mcp}`);
    }
  }

  if (data.skills && data.skills.length > 0) {
    lines.push("");
    lines.push(`# Skills`);
    lines.push(`skills:`);
    for (const skill of data.skills) {
      lines.push(`  - ${skill}`);
    }
  }

  if (data.middleware && data.middleware.length > 0) {
    lines.push("");
    lines.push(`# Middleware`);
    lines.push(`middleware:`);
    for (const mw of data.middleware) {
      lines.push(`  - ${mw}`);
    }
  }

  if (data.systemInstructions) {
    lines.push("");
    lines.push(`# System Instructions`);
    lines.push(`system_instructions: |`);
    for (const line of data.systemInstructions.split("\n")) {
      lines.push(`  ${line}`);
    }
  }

  return lines;
}

function triggerNodeToYaml(data: TriggerNodeData): string[] {
  const lines: string[] = [];

  lines.push(`name: ${data.displayName || "Trigger"}`);
  lines.push(`trigger_type: ${data.triggerType}`);
  if (data.schedule) {
    lines.push(`schedule: ${data.schedule}`);
  }

  return lines;
}

function parallelNodeToYaml(data: ParallelNodeData): string[] {
  const lines: string[] = [];

  lines.push(`name: ${data.displayName || "Parallel"}`);
  lines.push(`merge_strategy: ${data.mergeStrategy}`);
  if (data.subagents && data.subagents.length > 0) {
    lines.push(`subagents:`);
    for (const agent of data.subagents) {
      lines.push(`  - ${agent}`);
    }
  }

  return lines;
}

function sequentialNodeToYaml(data: SequentialNodeData): string[] {
  const lines: string[] = [];

  lines.push(`name: ${data.displayName || "Sequential"}`);
  if (data.subtasks && data.subtasks.length > 0) {
    lines.push(`subtasks:`);
    for (const task of data.subtasks) {
      lines.push(`  - ${task}`);
    }
  }

  return lines;
}

function conditionalNodeToYaml(data: ConditionalNodeData): string[] {
  const lines: string[] = [];

  lines.push(`name: ${data.displayName || "Conditional"}`);
  if (data.conditions && data.conditions.length > 0) {
    lines.push(`conditions:`);
    for (const condition of data.conditions) {
      lines.push(`  - label: ${condition.label}`);
      lines.push(`    expression: ${condition.expression}`);
      if (condition.targetNodeId) {
        lines.push(`    target: ${condition.targetNodeId}`);
      }
    }
  }

  return lines;
}

function loopNodeToYaml(data: LoopNodeData): string[] {
  const lines: string[] = [];

  lines.push(`name: ${data.displayName || "Loop"}`);
  lines.push(`exit_condition: ${data.exitCondition}`);
  lines.push(`max_iterations: ${data.maxIterations}`);
  if (data.bodyNodeId) {
    lines.push(`body_node: ${data.bodyNodeId}`);
  }

  return lines;
}

function aggregatorNodeToYaml(data: AggregatorNodeData): string[] {
  const lines: string[] = [];

  lines.push(`name: ${data.displayName || "Aggregator"}`);
  lines.push(`strategy: ${data.strategy}`);
  if (data.template) {
    lines.push(`template: |`);
    for (const line of data.template.split("\n")) {
      lines.push(`  ${line}`);
    }
  }
  if (data.customInstructions) {
    lines.push(`custom_instructions: |`);
    for (const line of data.customInstructions.split("\n")) {
      lines.push(`  ${line}`);
    }
  }

  return lines;
}

function subtaskNodeToYaml(data: SubtaskNodeData): string[] {
  const lines: string[] = [];

  lines.push(`name: ${data.displayName || "Subtask"}`);
  if (data.context) {
    lines.push(`context: ${data.context}`);
  }
  lines.push(`goal: ${data.goal}`);
  if (data.tasks && data.tasks.length > 0) {
    lines.push(`tasks:`);
    for (const task of data.tasks) {
      lines.push(`  - ${task}`);
    }
  }
  if (data.agentNodeId) {
    lines.push(`agent_node: ${data.agentNodeId}`);
  }

  return lines;
}

// -----------------------------------------------------------------------------
// Main Component
// -----------------------------------------------------------------------------

export const YamlPreview = memo(({ node, onUpdate, readOnly = false }: YamlPreviewProps) => {
  const [isEditing, setIsEditing] = useState(false);
  const [editedYaml, setEditedYaml] = useState("");
  const [copyStatus, setCopyStatus] = useState<"idle" | "copied">("idle");

  // Generate YAML from node
  const yaml = useMemo(() => {
    return node ? nodeToYaml(node) : "# No node selected";
  }, [node]);

  useEffect(() => {
    if (!isEditing) {
      setEditedYaml(yaml);
    }
  }, [yaml, isEditing]);

  const handleCopy = () => {
    navigator.clipboard.writeText(yaml);
    setCopyStatus("copied");
    setTimeout(() => setCopyStatus("idle"), 2000);
  };

  const handleEdit = () => {
    setIsEditing(true);
    setEditedYaml(yaml);
  };

  const handleSave = () => {
    if (onUpdate && editedYaml !== yaml) {
      onUpdate(editedYaml);
    }
    setIsEditing(false);
  };

  const handleCancel = () => {
    setIsEditing(false);
    setEditedYaml(yaml);
  };

  if (!node) {
    return (
      <div className="p-4 text-center text-gray-500 text-sm">
        Select a node to view its YAML configuration
      </div>
    );
  }

  return (
    <div className="space-y-2">
      {/* Header */}
      <div className="flex items-center justify-between">
        <h3 className="text-xs font-semibold text-gray-400 uppercase tracking-wide">
          YAML Preview
        </h3>
        <div className="flex items-center gap-1">
          {!readOnly && !isEditing && (
            <button
              onClick={handleEdit}
              className="p-1 rounded hover:bg-white/10 text-gray-400 hover:text-white transition-colors"
              title="Edit YAML"
            >
              <EditIcon />
            </button>
          )}
          <button
            onClick={handleCopy}
            className="p-1 rounded hover:bg-white/10 text-gray-400 hover:text-white transition-colors"
            title="Copy to clipboard"
          >
            {copyStatus === "copied" ? (
              <span className="text-green-400">
                <CheckIcon />
              </span>
            ) : (
              <CopyIcon />
            )}
          </button>
        </div>
      </div>

      {/* YAML Content */}
      <div className="relative">
        <pre
          className={`bg-[#0a0a0a] border border-white/10 rounded-lg p-3 text-xs font-mono overflow-x-auto ${
            isEditing ? "hidden" : ""
          }`}
        >
          <code className="text-gray-300">{yaml}</code>
        </pre>

        {isEditing && (
          <textarea
            value={editedYaml}
            onChange={(e) => setEditedYaml(e.target.value)}
            className="w-full h-[200px] bg-[#0a0a0a] border border-violet-500/30 rounded-lg p-3 text-xs font-mono text-gray-300 resize-none focus:outline-none focus:ring-1 focus:ring-violet-500"
            spellCheck={false}
          />
        )}
      </div>

      {/* Edit Actions */}
      {isEditing && (
        <div className="flex items-center justify-end gap-2">
          <button
            onClick={handleCancel}
            className="px-3 py-1 text-xs bg-white/5 hover:bg-white/10 border border-white/10 rounded text-white transition-colors"
          >
            Cancel
          </button>
          <button
            onClick={handleSave}
            className="px-3 py-1 text-xs bg-violet-600 hover:bg-violet-700 rounded text-white transition-colors"
          >
            Save Changes
          </button>
        </div>
      )}

      {/* Info */}
      <div className="p-2 rounded bg-blue-500/10 border border-blue-500/20">
        <p className="text-[10px] text-blue-300">
          This is the YAML representation of the selected node. Editing this YAML will update the node configuration.
        </p>
      </div>
    </div>
  );
});

YamlPreview.displayName = "YamlPreview";
