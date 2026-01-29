import React, { useEffect, useState, useCallback } from 'react';
import { Settings, X, Loader2, Crown, ArrowRight, Play, Square, HelpCircle, ChevronDown, ChevronRight, Plug, Sparkles, Code } from 'lucide-react';
import { useWorkflowStore, selectSelectedNode, selectSelectedEdge } from '../../stores/workflowStore';
import type { SubagentNodeData } from '../../types/workflow';
import * as providerService from '@/services/provider';
import * as skillsService from '@/services/skills';
import * as mcpService from '@/services/mcp';
import type { Provider, Skill } from '@/shared/types';
import type { MCPServer } from '@/features/mcp/types';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/shared/ui/select';

export const PropertiesPanel: React.FC = () => {
  const selectedNode = useWorkflowStore(selectSelectedNode);
  const selectedEdge = useWorkflowStore(selectSelectedEdge);
  const orchestratorConfig = useWorkflowStore((s) => s.orchestratorConfig);
  const updateNode = useWorkflowStore((s) => s.updateNode);
  const deleteNode = useWorkflowStore((s) => s.deleteNode);
  const updateOrchestratorConfig = useWorkflowStore((s) => s.updateOrchestratorConfig);
  const updateEdge = useWorkflowStore((s) => s.updateEdge);
  const setSelectedEdgeId = useWorkflowStore((s) => s.setSelectedEdgeId);

  const [providers, setProviders] = useState<Provider[]>([]);
  const [loadingProviders, setLoadingProviders] = useState(true);
  const [availableModels, setAvailableModels] = useState<string[]>([]);

  // Skills and MCPs state
  const [skills, setSkills] = useState<Skill[]>([]);
  const [mcpServers, setMcpServers] = useState<MCPServer[]>([]);
  const [loadingSkills, setLoadingSkills] = useState(true);
  const [loadingMcps, setLoadingMcps] = useState(true);

  // Collapsible sections state
  const [expandedSections, setExpandedSections] = useState<Record<string, boolean>>({
    mcps: false,
    skills: false,
    middleware: false,
  });

  const toggleSection = (section: string) => {
    setExpandedSections(prev => ({ ...prev, [section]: !prev[section] }));
  };

  // Load providers, skills, and MCPs on mount
  useEffect(() => {
    loadProviders();
    loadSkills();
    loadMcps();
  }, []);

  const loadSkills = async () => {
    setLoadingSkills(true);
    try {
      const loaded = await skillsService.listSkills();
      setSkills(loaded);
    } catch (error) {
      console.error('Failed to load skills:', error);
    } finally {
      setLoadingSkills(false);
    }
  };

  const loadMcps = async () => {
    setLoadingMcps(true);
    try {
      const loaded = await mcpService.listMCPServers();
      setMcpServers(loaded);
    } catch (error) {
      console.error('Failed to load MCP servers:', error);
    } finally {
      setLoadingMcps(false);
    }
  };

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

  // Show Edge properties panel when edge is selected
  if (selectedEdge) {
    return (
      <div className="w-80 border-l border-gray-800 bg-gray-900 overflow-y-auto">
        {/* Header */}
        <div className="flex items-center justify-between p-4 border-b border-gray-800 bg-gray-950">
          <div className="flex items-center gap-2">
            <ArrowRight size={18} className="text-blue-400" />
            <h3 className="font-semibold text-white">Connection</h3>
          </div>
          <button
            onClick={() => setSelectedEdgeId(null)}
            className="p-1 text-gray-400 hover:text-white hover:bg-gray-800 rounded"
            title="Deselect"
          >
            <X size={18} />
          </button>
        </div>

        {/* Edge Properties */}
        <div className="p-4 space-y-4">
          <div>
            <label className="block text-xs font-medium text-gray-400 mb-1">
              Label
            </label>
            <input
              type="text"
              className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-md text-sm text-white focus:border-blue-500 focus:outline-none"
              value={(selectedEdge.label as string) || ''}
              onChange={(e: React.ChangeEvent<HTMLInputElement>) => updateEdge(selectedEdge.id, { label: e.target.value })}
              placeholder="Connection label (optional)"
            />
            <p className="text-xs text-gray-500 mt-1">
              Optional label to describe this connection
            </p>
          </div>

          {/* Connection info */}
          <div className="pt-4 border-t border-gray-800">
            <p className="text-xs text-gray-500">
              From: <span className="text-gray-300 font-mono">{selectedEdge.source}</span>
            </p>
            <p className="text-xs text-gray-500 mt-1">
              To: <span className="text-gray-300 font-mono">{selectedEdge.target}</span>
            </p>
          </div>
        </div>
      </div>
    );
  }

  // Show Orchestrator config panel when no node or edge is selected
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

              {/* MCPs Section */}
              <div className="border border-gray-700 rounded-lg overflow-hidden">
                <button
                  type="button"
                  onClick={() => toggleSection('mcps')}
                  className="w-full flex items-center justify-between px-3 py-2 bg-gray-800 hover:bg-gray-750 text-left"
                >
                  <div className="flex items-center gap-2">
                    <Plug size={14} className="text-blue-400" />
                    <span className="text-xs font-medium text-gray-300">MCPs</span>
                    {(orchestratorConfig.mcps || []).length > 0 && (
                      <span className="text-xs text-blue-400">({(orchestratorConfig.mcps || []).length})</span>
                    )}
                  </div>
                  {expandedSections.mcps ? <ChevronDown size={14} className="text-gray-400" /> : <ChevronRight size={14} className="text-gray-400" />}
                </button>
                {expandedSections.mcps && (
                  <div className="p-3 bg-gray-850 border-t border-gray-700 space-y-2 max-h-48 overflow-y-auto">
                    {loadingMcps ? (
                      <div className="flex items-center gap-2 text-gray-500 text-xs py-2">
                        <Loader2 size={12} className="animate-spin" />
                        Loading MCPs...
                      </div>
                    ) : mcpServers.length === 0 ? (
                      <p className="text-xs text-gray-500">No MCP servers configured</p>
                    ) : (
                      mcpServers.map((mcp) => (
                        <label key={mcp.id} className="flex items-center gap-2 cursor-pointer">
                          <input
                            type="checkbox"
                            className="rounded border-gray-600 bg-gray-700 text-purple-500 focus:ring-purple-500 focus:ring-offset-0"
                            checked={(orchestratorConfig.mcps || []).includes(mcp.id)}
                            onChange={(e) => {
                              const current = orchestratorConfig.mcps || [];
                              if (e.target.checked) {
                                handleOrchestratorUpdate('mcps', [...current, mcp.id]);
                              } else {
                                handleOrchestratorUpdate('mcps', current.filter((id: string) => id !== mcp.id));
                              }
                            }}
                          />
                          <span className="text-xs text-gray-300">{mcp.name}</span>
                        </label>
                      ))
                    )}
                  </div>
                )}
              </div>

              {/* Skills Section */}
              <div className="border border-gray-700 rounded-lg overflow-hidden">
                <button
                  type="button"
                  onClick={() => toggleSection('skills')}
                  className="w-full flex items-center justify-between px-3 py-2 bg-gray-800 hover:bg-gray-750 text-left"
                >
                  <div className="flex items-center gap-2">
                    <Sparkles size={14} className="text-yellow-400" />
                    <span className="text-xs font-medium text-gray-300">Skills</span>
                    {(orchestratorConfig.skills || []).length > 0 && (
                      <span className="text-xs text-yellow-400">({(orchestratorConfig.skills || []).length})</span>
                    )}
                  </div>
                  {expandedSections.skills ? <ChevronDown size={14} className="text-gray-400" /> : <ChevronRight size={14} className="text-gray-400" />}
                </button>
                {expandedSections.skills && (
                  <div className="p-3 bg-gray-850 border-t border-gray-700 space-y-2 max-h-48 overflow-y-auto">
                    {loadingSkills ? (
                      <div className="flex items-center gap-2 text-gray-500 text-xs py-2">
                        <Loader2 size={12} className="animate-spin" />
                        Loading skills...
                      </div>
                    ) : skills.length === 0 ? (
                      <p className="text-xs text-gray-500">No skills available</p>
                    ) : (
                      skills.map((skill) => (
                        <label key={skill.id} className="flex items-center gap-2 cursor-pointer">
                          <input
                            type="checkbox"
                            className="rounded border-gray-600 bg-gray-700 text-purple-500 focus:ring-purple-500 focus:ring-offset-0"
                            checked={(orchestratorConfig.skills || []).includes(skill.id)}
                            onChange={(e) => {
                              const current = orchestratorConfig.skills || [];
                              if (e.target.checked) {
                                handleOrchestratorUpdate('skills', [...current, skill.id]);
                              } else {
                                handleOrchestratorUpdate('skills', current.filter((id: string) => id !== skill.id));
                              }
                            }}
                          />
                          <div>
                            <span className="text-xs text-gray-300">{skill.displayName || skill.name}</span>
                            {skill.description && (
                              <p className="text-xs text-gray-500 truncate max-w-[200px]">{skill.description}</p>
                            )}
                          </div>
                        </label>
                      ))
                    )}
                  </div>
                )}
              </div>

              {/* Middleware Section */}
              <div className="border border-gray-700 rounded-lg overflow-hidden">
                <button
                  type="button"
                  onClick={() => toggleSection('middleware')}
                  className="w-full flex items-center justify-between px-3 py-2 bg-gray-800 hover:bg-gray-750 text-left"
                >
                  <div className="flex items-center gap-2">
                    <Code size={14} className="text-green-400" />
                    <span className="text-xs font-medium text-gray-300">Middleware</span>
                    {orchestratorConfig.middleware && (
                      <span className="text-xs text-green-400">(configured)</span>
                    )}
                  </div>
                  {expandedSections.middleware ? <ChevronDown size={14} className="text-gray-400" /> : <ChevronRight size={14} className="text-gray-400" />}
                </button>
                {expandedSections.middleware && (
                  <div className="p-3 bg-gray-850 border-t border-gray-700">
                    <textarea
                      className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-md text-xs font-mono text-white focus:border-blue-500 focus:outline-none resize-none"
                      rows={6}
                      value={orchestratorConfig.middleware || ''}
                      onChange={(e) => handleOrchestratorUpdate('middleware', e.target.value)}
                      placeholder="# Middleware YAML configuration&#10;middleware:&#10;  summarization:&#10;    enabled: true"
                    />
                    <p className="text-xs text-gray-500 mt-1">
                      YAML configuration for middleware (summarization, context editing)
                    </p>
                  </div>
                )}
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
      case 'start':
        return { icon: <Play size={18} className="text-green-400" fill="currentColor" />, label: 'Start Event' };
      case 'end':
        return { icon: <Square size={18} className="text-red-400" fill="currentColor" />, label: 'End Event' };
      case 'conditional':
        return { icon: <HelpCircle size={18} className="text-amber-400" />, label: 'Conditional (Draft)' };
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

        {/* Start node specific fields */}
        {selectedNode.type === 'start' && (
          <>
            <div>
              <label className="block text-xs font-medium text-gray-400 mb-1">
                Trigger Type
              </label>
              <Select
                value={nodeData.triggerType || 'manual'}
                onValueChange={(value) => handleUpdate('triggerType', value)}
              >
                <SelectTrigger className="bg-white/5 border-white/10 text-white h-9 text-sm">
                  <SelectValue placeholder="Select trigger type" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="manual">Manual (via chat)</SelectItem>
                  <SelectItem value="scheduled">Scheduled (cron)</SelectItem>
                  <SelectItem value="webhook">Webhook</SelectItem>
                </SelectContent>
              </Select>
              <p className="text-xs text-gray-500 mt-1">
                How this workflow is triggered
              </p>
            </div>

            {nodeData.triggerType === 'scheduled' && (
              <div>
                <label className="block text-xs font-medium text-gray-400 mb-1">
                  Schedule (Cron Expression)
                </label>
                <input
                  type="text"
                  className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-md text-sm font-mono text-white focus:border-blue-500 focus:outline-none"
                  value={nodeData.schedule || ''}
                  onChange={(e) => handleUpdate('schedule', e.target.value)}
                  placeholder="0 9 * * * (daily at 9am)"
                />
                <p className="text-xs text-gray-500 mt-1">
                  Standard cron expression for scheduling
                </p>
              </div>
            )}
          </>
        )}

        {/* End node - minimal config */}
        {selectedNode.type === 'end' && (
          <div className="text-center py-4">
            <p className="text-sm text-gray-400">
              This is the workflow exit point. No configuration needed.
            </p>
          </div>
        )}

        {/* Conditional node - DRAFT */}
        {selectedNode.type === 'conditional' && (
          <>
            <div className="bg-amber-500/10 border border-amber-500/30 rounded-lg p-3 mb-4">
              <div className="flex items-center gap-2 text-amber-300 text-sm font-medium mb-1">
                <HelpCircle size={14} />
                <span>DRAFT - Not Yet Implemented</span>
              </div>
              <p className="text-xs text-amber-200/70">
                Conditional branching is a work in progress. This node will evaluate conditions and route to different branches.
              </p>
            </div>

            <div>
              <label className="block text-xs font-medium text-gray-400 mb-1">
                Condition Expression
              </label>
              <textarea
                className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-md text-sm text-white font-mono focus:border-blue-500 focus:outline-none resize-none"
                rows={3}
                value={nodeData.condition || ''}
                onChange={(e) => handleUpdate('condition', e.target.value)}
                placeholder="data.inputType === 'premium'"
              />
              <p className="text-xs text-gray-500 mt-1">
                JavaScript expression that evaluates to true/false
              </p>
            </div>

            <div className="pt-4 border-t border-gray-800">
              <div className="flex items-center justify-between mb-2">
                <label className="text-xs font-medium text-gray-400">
                  Branches
                </label>
                <button
                  type="button"
                  className="text-xs text-blue-400 hover:text-blue-300"
                  onClick={() => {
                    const branches = nodeData.branches || [];
                    handleUpdate('branches', [
                      ...branches,
                      { id: crypto.randomUUID(), name: 'New Branch', condition: 'true' }
                    ]);
                  }}
                >
                  + Add Branch
                </button>
              </div>

              {(!nodeData.branches || nodeData.branches.length === 0) ? (
                <p className="text-xs text-gray-500 text-center py-4">
                  No branches defined. Add branches to configure conditional paths.
                </p>
              ) : (
                <div className="space-y-2">
                  {(nodeData.branches as Array<{ id: string; name: string; condition: string }>).map((branch, index) => (
                    <div key={branch.id} className="bg-gray-800 rounded-lg p-3 border border-gray-700">
                      <div className="flex items-center justify-between mb-2">
                        <span className="text-xs font-medium text-gray-300">Branch {index + 1}</span>
                        <button
                          type="button"
                          className="text-xs text-red-400 hover:text-red-300"
                          onClick={() => {
                            const branches = nodeData.branches || [];
                            handleUpdate('branches', branches.filter((_: any, i: number) => i !== index));
                          }}
                        >
                          Remove
                        </button>
                      </div>
                      <input
                        type="text"
                        className="w-full px-2 py-1 bg-gray-700 border border-gray-600 rounded text-sm text-white mb-2 focus:border-blue-500 focus:outline-none"
                        value={branch.name}
                        onChange={(e) => {
                          const branches = [...(nodeData.branches || [])];
                          branches[index] = { ...branch, name: e.target.value };
                          handleUpdate('branches', branches);
                        }}
                        placeholder="Branch name"
                      />
                      <input
                        type="text"
                        className="w-full px-2 py-1 bg-gray-700 border border-gray-600 rounded text-sm text-white font-mono focus:border-blue-500 focus:outline-none"
                        value={branch.condition}
                        onChange={(e) => {
                          const branches = [...(nodeData.branches || [])];
                          branches[index] = { ...branch, condition: e.target.value };
                          handleUpdate('branches', branches);
                        }}
                        placeholder="Condition (e.g., data.value > 10)"
                      />
                    </div>
                  ))}
                </div>
              )}
            </div>
          </>
        )}

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

            {/* Note: MCPs, Skills, Tools, Middleware are configured via chips on the node */}
            <p className="text-xs text-gray-500 italic">
              Configure MCPs, Skills, and Middleware using the chips on the node.
            </p>
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
