// ============================================================================
// WEB AGENTS PANEL
// Agent management for web dashboard (uses transport layer)
// ============================================================================

import { useState, useEffect } from "react";
import { getTransport, type AgentResponse, type CreateAgentRequest, type ProviderResponse } from "@/services/transport";

// ============================================================================
// Component
// ============================================================================

export function WebAgentsPanel() {
  const [agents, setAgents] = useState<AgentResponse[]>([]);
  const [providers, setProviders] = useState<ProviderResponse[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [isCreating, setIsCreating] = useState(false);
  const [newAgent, setNewAgent] = useState<Partial<CreateAgentRequest>>({
    name: "",
    displayName: "",
    description: "",
    providerId: "",
    model: "",
    temperature: 0.7,
    maxTokens: 4096,
  });

  useEffect(() => {
    loadData();
  }, []);

  const loadData = async () => {
    setIsLoading(true);
    try {
      const transport = await getTransport();
      const [agentsResult, providersResult] = await Promise.all([
        transport.listAgents(),
        transport.listProviders(),
      ]);

      if (agentsResult.success && agentsResult.data) {
        setAgents(agentsResult.data);
      }
      if (providersResult.success && providersResult.data) {
        setProviders(providersResult.data);
        // Set default provider if not set
        if (!newAgent.providerId && providersResult.data.length > 0) {
          const defaultProvider = providersResult.data[0];
          setNewAgent(prev => ({
            ...prev,
            providerId: defaultProvider.id || "",
            model: defaultProvider.models[0] || "",
          }));
        }
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unknown error");
    } finally {
      setIsLoading(false);
    }
  };

  const loadAgents = async () => {
    try {
      const transport = await getTransport();
      const result = await transport.listAgents();
      if (result.success && result.data) {
        setAgents(result.data);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unknown error");
    }
  };

  const handleCreateAgent = async () => {
    if (!newAgent.name) return;

    try {
      const transport = await getTransport();
      const result = await transport.createAgent({
        name: newAgent.name,
        displayName: newAgent.displayName || newAgent.name,
        description: newAgent.description,
        providerId: newAgent.providerId || "openai",
        model: newAgent.model || "gpt-4o",
        temperature: newAgent.temperature,
        maxTokens: newAgent.maxTokens,
      });

      if (result.success) {
        setIsCreating(false);
        const defaultProvider = providers[0];
        setNewAgent({
          name: "",
          displayName: "",
          description: "",
          providerId: defaultProvider?.id || "",
          model: defaultProvider?.models[0] || "",
          temperature: 0.7,
          maxTokens: 4096,
        });
        loadAgents();
      } else {
        setError(result.error || "Failed to create agent");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unknown error");
    }
  };

  const handleDeleteAgent = async (id: string) => {
    if (!confirm("Are you sure you want to delete this agent?")) return;

    try {
      const transport = await getTransport();
      const result = await transport.deleteAgent(id);
      if (result.success) {
        loadAgents();
      } else {
        setError(result.error || "Failed to delete agent");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unknown error");
    }
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-violet-500" />
      </div>
    );
  }

  return (
    <div className="p-6 h-full overflow-auto">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-bold">Agents</h1>
        <button
          onClick={() => setIsCreating(true)}
          className="bg-violet-600 hover:bg-violet-700 text-white px-4 py-2 rounded-lg transition-colors"
        >
          Create Agent
        </button>
      </div>

      {error && (
        <div className="bg-red-900/30 border border-red-800 text-red-200 px-4 py-3 rounded-lg mb-4">
          {error}
          <button onClick={() => setError(null)} className="ml-2 text-red-400 hover:text-red-300">
            Dismiss
          </button>
        </div>
      )}

      {/* Create Agent Modal */}
      {isCreating && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="bg-[#1a1a1a] border border-gray-800 rounded-lg p-6 w-full max-w-md">
            <h2 className="text-xl font-bold mb-4">Create Agent</h2>

            <div className="space-y-4">
              <div>
                <label className="block text-sm text-gray-400 mb-1">Name (ID)</label>
                <input
                  type="text"
                  value={newAgent.name}
                  onChange={(e) => setNewAgent({ ...newAgent, name: e.target.value.toLowerCase().replace(/[^a-z0-9-]/g, "-") })}
                  placeholder="my-agent"
                  className="w-full bg-gray-800 border border-gray-700 rounded-lg px-3 py-2 focus:outline-none focus:border-violet-500"
                />
              </div>

              <div>
                <label className="block text-sm text-gray-400 mb-1">Display Name</label>
                <input
                  type="text"
                  value={newAgent.displayName}
                  onChange={(e) => setNewAgent({ ...newAgent, displayName: e.target.value })}
                  placeholder="My Agent"
                  className="w-full bg-gray-800 border border-gray-700 rounded-lg px-3 py-2 focus:outline-none focus:border-violet-500"
                />
              </div>

              <div>
                <label className="block text-sm text-gray-400 mb-1">Description</label>
                <textarea
                  value={newAgent.description}
                  onChange={(e) => setNewAgent({ ...newAgent, description: e.target.value })}
                  placeholder="What does this agent do?"
                  rows={2}
                  className="w-full bg-gray-800 border border-gray-700 rounded-lg px-3 py-2 focus:outline-none focus:border-violet-500 resize-none"
                />
              </div>

              <div className="grid grid-cols-2 gap-4">
                <div>
                  <label className="block text-sm text-gray-400 mb-1">Provider</label>
                  <select
                    value={newAgent.providerId}
                    onChange={(e) => {
                      const provider = providers.find(p => p.id === e.target.value);
                      setNewAgent({
                        ...newAgent,
                        providerId: e.target.value,
                        model: provider?.models[0] || "",
                      });
                    }}
                    className="w-full bg-gray-800 border border-gray-700 rounded-lg px-3 py-2 focus:outline-none focus:border-violet-500"
                  >
                    {providers.length === 0 ? (
                      <option value="">No providers configured</option>
                    ) : (
                      providers.map((provider) => (
                        <option key={provider.id} value={provider.id}>
                          {provider.name}
                        </option>
                      ))
                    )}
                  </select>
                </div>

                <div>
                  <label className="block text-sm text-gray-400 mb-1">Model</label>
                  <select
                    value={newAgent.model}
                    onChange={(e) => setNewAgent({ ...newAgent, model: e.target.value })}
                    className="w-full bg-gray-800 border border-gray-700 rounded-lg px-3 py-2 focus:outline-none focus:border-violet-500"
                  >
                    {providers
                      .find(p => p.id === newAgent.providerId)
                      ?.models.map((model) => (
                        <option key={model} value={model}>
                          {model}
                        </option>
                      )) || <option value="">Select a provider first</option>}
                  </select>
                </div>
              </div>
            </div>

            <div className="flex justify-end gap-3 mt-6">
              <button
                onClick={() => setIsCreating(false)}
                className="px-4 py-2 text-gray-400 hover:text-white transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={handleCreateAgent}
                disabled={!newAgent.name || !newAgent.providerId || !newAgent.model}
                className="bg-violet-600 hover:bg-violet-700 disabled:opacity-50 text-white px-4 py-2 rounded-lg transition-colors"
              >
                Create
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Agents List */}
      {agents.length === 0 ? (
        <div className="text-center py-12 text-gray-500">
          <p className="text-lg mb-2">No agents yet</p>
          <p className="text-sm">Create your first agent to get started</p>
        </div>
      ) : (
        <div className="grid gap-4">
          {agents.map((agent) => (
            <div
              key={agent.id}
              className="bg-[#141414] border border-gray-800 rounded-lg p-4 hover:border-gray-700 transition-colors"
            >
              <div className="flex items-start justify-between">
                <div>
                  <h3 className="text-lg font-semibold text-white">
                    {agent.displayName || agent.name}
                  </h3>
                  <p className="text-sm text-gray-500 mt-1">{agent.id}</p>
                  {agent.description && (
                    <p className="text-gray-400 mt-2">{agent.description}</p>
                  )}
                  <div className="flex gap-4 mt-3 text-sm text-gray-500">
                    <span>{agent.providerId}</span>
                    <span>{agent.model}</span>
                    <span>Temp: {agent.temperature}</span>
                  </div>
                </div>
                <button
                  onClick={() => handleDeleteAgent(agent.id)}
                  className="text-gray-500 hover:text-red-400 transition-colors"
                >
                  Delete
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
