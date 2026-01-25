import React from 'react';
import { Settings, X } from 'lucide-react';
import { useWorkflowStore, selectSelectedNode } from '../../stores/workflowStore';
import type { SubagentNodeData } from '../../types/workflow';

export const PropertiesPanel: React.FC = () => {
  const selectedNode = useWorkflowStore(selectSelectedNode);
  const updateNode = useWorkflowStore((s) => s.updateNode);
  const deleteNode = useWorkflowStore((s) => s.deleteNode);

  if (!selectedNode) {
    return (
      <div className="w-80 border-l bg-gray-50 p-4 flex flex-col items-center justify-center text-gray-400">
        <Settings size={48} className="mb-4 opacity-50" />
        <p className="text-sm">Select a node to edit properties</p>
      </div>
    );
  }

  const handleUpdate = (field: string, value: any) => {
    updateNode(selectedNode.id, { [field]: value });
  };

  return (
    <div className="w-80 border-l bg-white overflow-y-auto">
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b bg-gray-50">
        <h3 className="font-semibold text-gray-700">Properties</h3>
        <button
          onClick={() => deleteNode(selectedNode.id)}
          className="p-1 text-red-500 hover:bg-red-50 rounded"
          title="Delete node"
        >
          <X size={18} />
        </button>
      </div>

      {/* Content */}
      <div className="p-4 space-y-4">
        {/* Common fields */}
        <div>
          <label className="block text-xs font-medium text-gray-500 mb-1">
            Display Name
          </label>
          <input
            type="text"
            className="w-full px-3 py-2 border rounded-md text-sm"
            value={(selectedNode.data as any).displayName || ''}
            onChange={(e) => handleUpdate('displayName', e.target.value)}
          />
        </div>

        <div>
          <label className="block text-xs font-medium text-gray-500 mb-1">
            Description
          </label>
          <textarea
            className="w-full px-3 py-2 border rounded-md text-sm"
            rows={3}
            value={(selectedNode.data as any).description || ''}
            onChange={(e) => handleUpdate('description', e.target.value)}
          />
        </div>

        {/* Subagent-specific fields */}
        {selectedNode.type === 'subagent' && (
          <>
            <div>
              <label className="block text-xs font-medium text-gray-500 mb-1">
                Subagent ID (folder name)
              </label>
              <input
                type="text"
                className="w-full px-3 py-2 border rounded-md text-sm font-mono"
                value={(selectedNode.data as SubagentNodeData).subagentId || ''}
                onChange={(e) => handleUpdate('subagentId', e.target.value.toLowerCase().replace(/\s+/g, '-'))}
              />
            </div>

            <div>
              <label className="block text-xs font-medium text-gray-500 mb-1">
                Provider
              </label>
              <select
                className="w-full px-3 py-2 border rounded-md text-sm"
                value={(selectedNode.data as SubagentNodeData).providerId || ''}
                onChange={(e) => handleUpdate('providerId', e.target.value)}
              >
                <option value="">Select provider...</option>
                <option value="openai">OpenAI</option>
                <option value="anthropic">Anthropic</option>
                <option value="deepseek">DeepSeek</option>
                {/* TODO: Load from configured providers */}
              </select>
            </div>

            <div>
              <label className="block text-xs font-medium text-gray-500 mb-1">
                Model
              </label>
              <input
                type="text"
                className="w-full px-3 py-2 border rounded-md text-sm"
                value={(selectedNode.data as SubagentNodeData).model || ''}
                onChange={(e) => handleUpdate('model', e.target.value)}
                placeholder="e.g., gpt-4o-mini"
              />
            </div>

            <div>
              <label className="block text-xs font-medium text-gray-500 mb-1">
                Temperature ({(selectedNode.data as SubagentNodeData).temperature || 0.7})
              </label>
              <input
                type="range"
                min="0"
                max="2"
                step="0.1"
                className="w-full"
                value={(selectedNode.data as SubagentNodeData).temperature || 0.7}
                onChange={(e) => handleUpdate('temperature', parseFloat(e.target.value))}
              />
            </div>

            <div>
              <label className="block text-xs font-medium text-gray-500 mb-1">
                System Prompt (AGENTS.md content)
              </label>
              <textarea
                className="w-full px-3 py-2 border rounded-md text-sm font-mono"
                rows={8}
                value={(selectedNode.data as SubagentNodeData).systemPrompt || ''}
                onChange={(e) => handleUpdate('systemPrompt', e.target.value)}
                placeholder="# Instructions for this subagent..."
              />
            </div>
          </>
        )}

        {/* Node type badge */}
        <div className="pt-4 border-t">
          <span className="text-xs text-gray-400">
            Node Type: {selectedNode.type}
          </span>
        </div>
      </div>
    </div>
  );
};
