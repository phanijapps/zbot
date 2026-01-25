import React, { useEffect, useState, useCallback } from 'react';
import { Settings, X, Loader2 } from 'lucide-react';
import { useWorkflowStore, selectSelectedNode } from '../../stores/workflowStore';
import type { SubagentNodeData } from '../../types/workflow';
import * as providerService from '@/services/provider';
import type { Provider } from '@/shared/types';

export const PropertiesPanel: React.FC = () => {
  const selectedNode = useWorkflowStore(selectSelectedNode);
  const updateNode = useWorkflowStore((s) => s.updateNode);
  const deleteNode = useWorkflowStore((s) => s.deleteNode);

  const [providers, setProviders] = useState<Provider[]>([]);
  const [loadingProviders, setLoadingProviders] = useState(true);
  const [availableModels, setAvailableModels] = useState<string[]>([]);

  // Load providers on mount
  useEffect(() => {
    loadProviders();
  }, []);

  // Update available models when provider changes
  useEffect(() => {
    if (selectedNode?.type === 'subagent' || selectedNode?.type === 'orchestrator') {
      const providerId = (selectedNode.data as any).providerId;
      if (providerId) {
        const provider = providers.find(p => p.id === providerId);
        setAvailableModels(provider?.models || []);
      } else {
        setAvailableModels([]);
      }
    }
  }, [selectedNode?.data, providers, selectedNode?.type]);

  const loadProviders = async () => {
    setLoadingProviders(true);
    try {
      const loaded = await providerService.listProviders();
      setProviders(loaded);
    } catch (error) {
      console.error('Failed to load providers:', error);
    } finally {
      setLoadingProviders(false);
    }
  };

  const handleUpdate = useCallback((field: string, value: any) => {
    if (!selectedNode) return;
    updateNode(selectedNode.id, { [field]: value });
  }, [selectedNode, updateNode]);

  if (!selectedNode) {
    return (
      <div className="w-80 border-l border-gray-800 bg-gray-900 p-4 flex flex-col items-center justify-center text-gray-500">
        <Settings size={48} className="mb-4 opacity-50" />
        <p className="text-sm">Select a node to edit properties</p>
      </div>
    );
  }

  const nodeData = selectedNode.data as any;

  return (
    <div className="w-80 border-l border-gray-800 bg-gray-900 overflow-y-auto">
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b border-gray-800 bg-gray-950">
        <h3 className="font-semibold text-white">Properties</h3>
        <button
          onClick={() => deleteNode(selectedNode.id)}
          className="p-1 text-red-400 hover:bg-red-500/10 rounded"
          title="Delete node"
        >
          <X size={18} />
        </button>
      </div>

      {/* Content */}
      <div className="p-4 space-y-4">
        {/* Common fields */}
        <div>
          <label className="block text-xs font-medium text-gray-400 mb-1">
            Display Name
          </label>
          <input
            type="text"
            className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-md text-sm text-white focus:border-blue-500 focus:outline-none"
            value={nodeData.displayName || ''}
            onChange={(e) => handleUpdate('displayName', e.target.value)}
          />
        </div>

        <div>
          <label className="block text-xs font-medium text-gray-400 mb-1">
            Description
          </label>
          <textarea
            className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-md text-sm text-white focus:border-blue-500 focus:outline-none resize-none"
            rows={3}
            value={nodeData.description || ''}
            onChange={(e) => handleUpdate('description', e.target.value)}
          />
        </div>

        {/* Subagent-specific fields */}
        {selectedNode.type === 'subagent' && (
          <>
            <div>
              <label className="block text-xs font-medium text-gray-400 mb-1">
                Subagent ID (folder name)
              </label>
              <input
                type="text"
                className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-md text-sm font-mono text-white focus:border-blue-500 focus:outline-none"
                value={(selectedNode.data as SubagentNodeData).subagentId || ''}
                onChange={(e) => handleUpdate('subagentId', e.target.value.toLowerCase().replace(/\s+/g, '-'))}
              />
            </div>

            <div>
              <label className="block text-xs font-medium text-gray-400 mb-1">
                Provider
              </label>
              {loadingProviders ? (
                <div className="flex items-center gap-2 text-gray-500 text-sm py-2">
                  <Loader2 size={14} className="animate-spin" />
                  Loading providers...
                </div>
              ) : (
                <select
                  className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-md text-sm text-white focus:border-blue-500 focus:outline-none [&>option]:bg-gray-800 [&>option]:text-white"
                  value={(selectedNode.data as SubagentNodeData).providerId || ''}
                  onChange={(e) => {
                    handleUpdate('providerId', e.target.value);
                    // Clear model if it's not in the new provider's models
                    const newProvider = providers.find(p => p.id === e.target.value);
                    if (newProvider && !newProvider.models.includes(nodeData.model || '')) {
                      handleUpdate('model', '');
                    }
                  }}
                >
                  <option value="">Select provider...</option>
                  {providers.map((provider) => (
                    <option key={provider.id} value={provider.id}>
                      {provider.name}
                    </option>
                  ))}
                </select>
              )}
            </div>

            <div>
              <label className="block text-xs font-medium text-gray-400 mb-1">
                Model
              </label>
              {availableModels.length > 0 ? (
                <select
                  className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-md text-sm text-white focus:border-blue-500 focus:outline-none [&>option]:bg-gray-800 [&>option]:text-white"
                  value={(selectedNode.data as SubagentNodeData).model || ''}
                  onChange={(e) => handleUpdate('model', e.target.value)}
                >
                  <option value="">Select model...</option>
                  {availableModels.map((model) => (
                    <option key={model} value={model}>
                      {model}
                    </option>
                  ))}
                </select>
              ) : (
                <input
                  type="text"
                  className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-md text-sm text-white focus:border-blue-500 focus:outline-none"
                  value={(selectedNode.data as SubagentNodeData).model || ''}
                  onChange={(e) => handleUpdate('model', e.target.value)}
                  placeholder={nodeData.providerId ? "No models found for this provider" : "Select a provider first"}
                  disabled={!nodeData.providerId}
                />
              )}
            </div>

            <div>
              <label className="block text-xs font-medium text-gray-400 mb-1">
                Temperature ({(selectedNode.data as SubagentNodeData).temperature || 0.7})
              </label>
              <div className="flex items-center gap-3">
                <input
                  type="range"
                  min="0"
                  max="2"
                  step="0.1"
                  className="flex-1"
                  value={(selectedNode.data as SubagentNodeData).temperature || 0.7}
                  onChange={(e) => handleUpdate('temperature', parseFloat(e.target.value))}
                />
                <span className="text-xs text-gray-400 w-12 text-right">
                  {(selectedNode.data as SubagentNodeData).temperature || 0.7}
                </span>
              </div>
            </div>

            <div>
              <label className="block text-xs font-medium text-gray-400 mb-1">
                System Prompt (AGENTS.md content)
              </label>
              <textarea
                className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-md text-sm font-mono text-white focus:border-blue-500 focus:outline-none resize-none"
                rows={8}
                value={(selectedNode.data as SubagentNodeData).systemPrompt || ''}
                onChange={(e) => handleUpdate('systemPrompt', e.target.value)}
                placeholder="# Instructions for this subagent..."
              />
            </div>
          </>
        )}

        {/* Orchestrator-specific fields */}
        {selectedNode.type === 'orchestrator' && (
          <>
            <div>
              <label className="block text-xs font-medium text-gray-400 mb-1">
                Provider
              </label>
              {loadingProviders ? (
                <div className="flex items-center gap-2 text-gray-500 text-sm py-2">
                  <Loader2 size={14} className="animate-spin" />
                  Loading providers...
                </div>
              ) : (
                <select
                  className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-md text-sm text-white focus:border-blue-500 focus:outline-none [&>option]:bg-gray-800 [&>option]:text-white"
                  value={nodeData.providerId || ''}
                  onChange={(e) => {
                    handleUpdate('providerId', e.target.value);
                    const newProvider = providers.find(p => p.id === e.target.value);
                    if (newProvider && !newProvider.models.includes(nodeData.model || '')) {
                      handleUpdate('model', '');
                    }
                  }}
                >
                  <option value="">Select provider...</option>
                  {providers.map((provider) => (
                    <option key={provider.id} value={provider.id}>
                      {provider.name}
                    </option>
                  ))}
                </select>
              )}
            </div>

            <div>
              <label className="block text-xs font-medium text-gray-400 mb-1">
                Model
              </label>
              {availableModels.length > 0 ? (
                <select
                  className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-md text-sm text-white focus:border-blue-500 focus:outline-none [&>option]:bg-gray-800 [&>option]:text-white"
                  value={nodeData.model || ''}
                  onChange={(e) => handleUpdate('model', e.target.value)}
                >
                  <option value="">Select model...</option>
                  {availableModels.map((model) => (
                    <option key={model} value={model}>
                      {model}
                    </option>
                  ))}
                </select>
              ) : (
                <input
                  type="text"
                  className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-md text-sm text-white focus:border-blue-500 focus:outline-none"
                  value={nodeData.model || ''}
                  onChange={(e) => handleUpdate('model', e.target.value)}
                  placeholder={nodeData.providerId ? "No models found" : "Select a provider first"}
                  disabled={!nodeData.providerId}
                />
              )}
            </div>
          </>
        )}

        {/* Node type badge */}
        <div className="pt-4 border-t border-gray-800">
          <span className="text-xs text-gray-500">
            Node Type: {selectedNode.type}
          </span>
        </div>
      </div>
    </div>
  );
};
