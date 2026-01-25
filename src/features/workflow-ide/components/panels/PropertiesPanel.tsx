import React, { useEffect, useState, useCallback } from 'react';
import { Settings, X, Loader2, Crown } from 'lucide-react';
import { useWorkflowStore, selectSelectedNode } from '../../stores/workflowStore';
import type { SubagentNodeData } from '../../types/workflow';
import * as providerService from '@/services/provider';
import type { Provider } from '@/shared/types';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/shared/ui/select';

export const PropertiesPanel: React.FC = () => {
  const selectedNode = useWorkflowStore(selectSelectedNode);
  const orchestratorConfig = useWorkflowStore((s) => s.orchestratorConfig);
  const updateNode = useWorkflowStore((s) => s.updateNode);
  const deleteNode = useWorkflowStore((s) => s.deleteNode);
  const updateOrchestratorConfig = useWorkflowStore((s) => s.updateOrchestratorConfig);

  const [providers, setProviders] = useState<Provider[]>([]);
  const [loadingProviders, setLoadingProviders] = useState(true);
  const [availableModels, setAvailableModels] = useState<string[]>([]);

  // Load providers on mount
  useEffect(() => {
    loadProviders();
  }, []);

  // Update available models when provider changes (for node or orchestrator)
  useEffect(() => {
    let providerId: string | undefined;

    if (selectedNode?.type === 'subagent' || selectedNode?.type === 'orchestrator') {
      providerId = (selectedNode.data as any).providerId;
    } else if (!selectedNode && orchestratorConfig) {
      providerId = orchestratorConfig.providerId;
    }

    if (providerId) {
      const provider = providers.find(p => p.id === providerId);
      setAvailableModels(provider?.models || []);
    } else {
      setAvailableModels([]);
    }
  }, [selectedNode?.data, orchestratorConfig?.providerId, providers, selectedNode?.type]);

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

  const handleOrchestratorUpdate = useCallback((field: string, value: any) => {
    updateOrchestratorConfig({ [field]: value });
  }, [updateOrchestratorConfig]);

  // Show Orchestrator config panel when no node is selected
  if (!selectedNode) {
    return (
      <div className="w-80 border-l border-gray-800 bg-gray-900 overflow-y-auto">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-gray-800">
          <div className="flex items-center gap-2">
            <Crown size={18} className="text-amber-400" />
            <h3 className="font-semibold text-white">Orchestrator Settings</h3>
          </div>
        </div>

        {/* Orchestrator Configuration */}
        <div className="p-4 space-y-4">
          {orchestratorConfig ? (
            <>
              <div>
                <label className="block text-xs font-medium text-gray-400 mb-1">
                  Display Name
                </label>
                <input
                  type="text"
                  className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-md text-sm text-white focus:border-blue-500 focus:outline-none"
                  value={orchestratorConfig.displayName || ''}
                  onChange={(e) => handleOrchestratorUpdate('displayName', e.target.value)}
                  placeholder="Orchestrator name"
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-gray-400 mb-1">
                  Description
                </label>
                <textarea
                  className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-md text-sm text-white focus:border-blue-500 focus:outline-none min-h-[60px] resize-y"
                  value={orchestratorConfig.description || ''}
                  onChange={(e) => handleOrchestratorUpdate('description', e.target.value)}
                  placeholder="Describe what this orchestrator does..."
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
                  <Select
                    value={orchestratorConfig.providerId || ''}
                    onValueChange={(value) => {
                      handleOrchestratorUpdate('providerId', value);
                      const newProvider = providers.find(p => p.id === value);
                      if (newProvider && !newProvider.models.includes(orchestratorConfig.model || '')) {
                        handleOrchestratorUpdate('model', '');
                      }
                    }}
                  >
                    <SelectTrigger className="bg-white/5 border-white/10 text-white h-9 text-sm">
                      <SelectValue placeholder="Select provider" />
                    </SelectTrigger>
                    <SelectContent>
                      {providers.map((provider) => (
                        <SelectItem key={provider.id} value={provider.id}>
                          {provider.name}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                )}
              </div>

              <div>
                <label className="block text-xs font-medium text-gray-400 mb-1">
                  Model
                </label>
                {availableModels.length > 0 ? (
                  <Select
                    value={orchestratorConfig.model || ''}
                    onValueChange={(value) => handleOrchestratorUpdate('model', value)}
                  >
                    <SelectTrigger className="bg-white/5 border-white/10 text-white h-9 text-sm">
                      <SelectValue placeholder="Select model" />
                    </SelectTrigger>
                    <SelectContent>
                      {availableModels.map((model) => (
                        <SelectItem key={model} value={model}>
                          {model.length > 30 ? model.substring(0, 30) + '...' : model}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                ) : (
                  <input
                    type="text"
                    className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-md text-sm text-white focus:border-blue-500 focus:outline-none"
                    value={orchestratorConfig.model || ''}
                    onChange={(e) => handleOrchestratorUpdate('model', e.target.value)}
                    placeholder={orchestratorConfig.providerId ? "No models found" : "Select a provider first"}
                    disabled={!orchestratorConfig.providerId}
                  />
                )}
              </div>

              <div>
                <label className="block text-xs font-medium text-gray-400 mb-1">
                  Temperature ({orchestratorConfig.temperature || 0.7})
                </label>
                <div className="flex items-center gap-3">
                  <input
                    type="range"
                    min="0"
                    max="2"
                    step="0.1"
                    className="flex-1 h-1.5 bg-gray-700 rounded-lg appearance-none cursor-pointer accent-purple-500"
                    value={orchestratorConfig.temperature || 0.7}
                    onChange={(e) => handleOrchestratorUpdate('temperature', parseFloat(e.target.value))}
                  />
                  <span className="text-xs text-gray-400 w-12 text-right">
                    {orchestratorConfig.temperature || 0.7}
                  </span>
                </div>
              </div>

              <div>
                <label className="block text-xs font-medium text-gray-400 mb-1">
                  Max Tokens
                </label>
                <input
                  type="number"
                  min="1"
                  max="32000"
                  className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-md text-sm text-white focus:border-blue-500 focus:outline-none"
                  value={orchestratorConfig.maxTokens || 2000}
                  onChange={(e) => handleOrchestratorUpdate('maxTokens', parseInt(e.target.value) || 2000)}
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-gray-400 mb-1">
                  System Instructions
                </label>
                <textarea
                  className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-md text-sm text-white focus:border-blue-500 focus:outline-none min-h-[120px] resize-y"
                  value={orchestratorConfig.systemInstructions || ''}
                  onChange={(e) => handleOrchestratorUpdate('systemInstructions', e.target.value)}
                  placeholder="Define the orchestrator's behavior and instructions..."
                />
                <p className="text-xs text-gray-500 mt-1">
                  This serves as the system prompt for the orchestrator agent.
                </p>
              </div>
            </>
          ) : (
            <div className="flex flex-col items-center justify-center py-12 text-gray-500">
              <Crown size={48} className="mb-4 opacity-50" />
              <p className="text-sm text-center">
                No orchestrator configuration found.
              </p>
              <p className="text-xs text-gray-600 mt-2">
                Add an Orchestrator node to configure workflow settings.
              </p>
            </div>
          )}
        </div>
      </div>
    );
  }

  const nodeData = selectedNode.data as any;

  // Get node type info
  const getNodeTypeInfo = () => {
    switch (selectedNode.type) {
      case 'orchestrator':
        return { icon: <Crown size={18} className="text-amber-400" />, label: 'Orchestrator' };
      case 'subagent':
        return { icon: <Settings size={18} className="text-purple-400" />, label: 'Subagent' };
      default:
        return { icon: <Settings size={18} className="text-gray-400" />, label: 'Node' };
    }
  };

  const nodeTypeInfo = getNodeTypeInfo();

  return (
    <div className="w-80 border-l border-gray-800 bg-gray-900 overflow-y-auto">
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b border-gray-800 bg-gray-950">
        <div className="flex items-center gap-2">
          {nodeTypeInfo.icon}
          <h3 className="font-semibold text-white">{nodeTypeInfo.label}</h3>
        </div>
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
                <Select
                  value={(selectedNode.data as SubagentNodeData).providerId || ''}
                  onValueChange={(value) => {
                    handleUpdate('providerId', value);
                    const newProvider = providers.find(p => p.id === value);
                    if (newProvider && !newProvider.models.includes(nodeData.model || '')) {
                      handleUpdate('model', '');
                    }
                  }}
                >
                  <SelectTrigger className="bg-white/5 border-white/10 text-white h-9 text-sm">
                    <SelectValue placeholder="Select provider" />
                  </SelectTrigger>
                  <SelectContent>
                    {providers.map((provider) => (
                      <SelectItem key={provider.id} value={provider.id}>
                        {provider.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              )}
            </div>

            <div>
              <label className="block text-xs font-medium text-gray-400 mb-1">
                Model
              </label>
              {availableModels.length > 0 ? (
                <Select
                  value={(selectedNode.data as SubagentNodeData).model || ''}
                  onValueChange={(value) => handleUpdate('model', value)}
                >
                  <SelectTrigger className="bg-white/5 border-white/10 text-white h-9 text-sm">
                    <SelectValue placeholder="Select model" />
                  </SelectTrigger>
                  <SelectContent>
                    {availableModels.map((model) => (
                      <SelectItem key={model} value={model}>
                        {model.length > 30 ? model.substring(0, 30) + '...' : model}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
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
                  className="flex-1 h-1.5 bg-gray-700 rounded-lg appearance-none cursor-pointer accent-purple-500"
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
                <Select
                  value={nodeData.providerId || ''}
                  onValueChange={(value) => {
                    handleUpdate('providerId', value);
                    const newProvider = providers.find(p => p.id === value);
                    if (newProvider && !newProvider.models.includes(nodeData.model || '')) {
                      handleUpdate('model', '');
                    }
                  }}
                >
                  <SelectTrigger className="bg-white/5 border-white/10 text-white h-9 text-sm">
                    <SelectValue placeholder="Select provider" />
                  </SelectTrigger>
                  <SelectContent>
                    {providers.map((provider) => (
                      <SelectItem key={provider.id} value={provider.id}>
                        {provider.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              )}
            </div>

            <div>
              <label className="block text-xs font-medium text-gray-400 mb-1">
                Model
              </label>
              {availableModels.length > 0 ? (
                <Select
                  value={nodeData.model || ''}
                  onValueChange={(value) => handleUpdate('model', value)}
                >
                  <SelectTrigger className="bg-white/5 border-white/10 text-white h-9 text-sm">
                    <SelectValue placeholder="Select model" />
                  </SelectTrigger>
                  <SelectContent>
                    {availableModels.map((model) => (
                      <SelectItem key={model} value={model}>
                        {model.length > 30 ? model.substring(0, 30) + '...' : model}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
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
