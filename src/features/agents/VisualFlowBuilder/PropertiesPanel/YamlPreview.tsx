// ============================================================================
// ZERO IDE - YAML PREVIEW
// YAML preview and edit component for node configuration
// ============================================================================

import { memo, useState, useEffect, useMemo } from "react";
import type { BaseNode, StartNodeData, EndNodeData, ConditionalNodeData, SubagentNodeData } from "../types";

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
    case "start":
      lines.push(...startNodeToYaml(node.data as StartNodeData));
      break;
    case "end":
      lines.push(...endNodeToYaml(node.data as EndNodeData));
      break;
    case "conditional":
      lines.push(...conditionalNodeToYaml(node.data as ConditionalNodeData));
      break;
    case "subagent":
      lines.push(...subagentNodeToYaml(node.data as SubagentNodeData));
      break;
  }

  return lines.join("\n");
}

function startNodeToYaml(data: StartNodeData): string[] {
  const lines: string[] = [];

  lines.push(`name: ${data.displayName || "Start"}`);
  lines.push(`trigger_type: ${data.triggerType}`);
  if (data.schedule) {
    lines.push(`schedule: ${data.schedule}`);
  }

  return lines;
}

function endNodeToYaml(data: EndNodeData): string[] {
  const lines: string[] = [];

  lines.push(`name: ${data.displayName || "End"}`);

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

function subagentNodeToYaml(data: SubagentNodeData): string[] {
  const lines: string[] = [];

  lines.push(`displayName: ${data.displayName || "Subagent"}`);
  if (data.subagentId) {
    lines.push(`subagentId: ${data.subagentId}`);
  }

  const dataRecord = data as unknown as Record<string, unknown>;
  const hasConfig = !!dataRecord.config;

  if (hasConfig) {
    lines.push(``);
    lines.push(`# Subagent Configuration`);
    lines.push(`# The full config will be created in .subagents/${data.subagentId}/ when saved`);
  } else {
    lines.push(``);
    lines.push(`# Configure this subagent in the properties panel`);
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
