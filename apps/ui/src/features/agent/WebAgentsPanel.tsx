// ============================================================================
// WEB AGENTS PANEL
// Agent management for web dashboard (uses transport layer)
// ============================================================================

import { useState, useEffect } from "react";
import { Bot, Plus, Trash2, Cpu, Thermometer, Hash, X, Loader2, Pencil } from "lucide-react";
import { getTransport, type AgentResponse, type CreateAgentRequest, type ProviderResponse } from "@/services/transport";
import { AgentEditPanel } from "./AgentEditPanel";

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
  const [editingAgent, setEditingAgent] = useState<AgentResponse | null>(null);

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
      <div className="flex items-center justify-center h-full bg-[var(--background)]">
        <Loader2 className="w-6 h-6 text-[var(--primary)] animate-spin" />
      </div>
    );
  }

  return (
    <div className="h-full overflow-auto bg-[var(--background)]">
      <div className="p-8 max-w-5xl mx-auto">
        {/* Header */}
        <div className="flex items-center justify-between mb-6">
          <div>
            <h1 className="text-[var(--foreground)]">Agents</h1>
            <p className="text-[var(--muted-foreground)] text-sm mt-1">
              Manage your AI agents and their configurations
            </p>
          </div>
          <button
            onClick={() => setIsCreating(true)}
            className="inline-flex items-center gap-2 bg-[var(--primary)] hover:bg-[var(--primary)]/90 text-white px-4 py-2 rounded-lg transition-colors text-sm font-medium"
          >
            <Plus className="w-4 h-4" />
            Create Agent
          </button>
        </div>

        {error && (
          <div className="mb-4 p-3 bg-[var(--destructive)]/10 text-[var(--destructive)] rounded-lg flex items-center justify-between text-sm">
            <span>{error}</span>
            <button onClick={() => setError(null)} className="hover:opacity-70">
              <X className="w-4 h-4" />
            </button>
          </div>
        )}

        {/* Create Agent Modal */}
        {isCreating && (
          <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
            <div className="bg-[var(--card)] rounded-xl p-6 w-full max-w-md card-shadow-lg">
              <div className="flex items-center gap-3 mb-5">
                <div className="w-9 h-9 rounded-lg bg-[var(--primary)]/10 flex items-center justify-center">
                  <Bot className="w-4.5 h-4.5 text-[var(--primary)]" />
                </div>
                <h2 className="text-lg font-semibold text-[var(--foreground)]">Create Agent</h2>
              </div>

              <div className="space-y-4">
                <div>
                  <label className="block text-sm font-medium text-[var(--foreground)] mb-1.5">Name (ID)</label>
                  <input
                    type="text"
                    value={newAgent.name}
                    onChange={(e) => setNewAgent({ ...newAgent, name: e.target.value.toLowerCase().replace(/[^a-z0-9-]/g, "-") })}
                    placeholder="my-agent"
                    className="w-full bg-[var(--background)] border border-[var(--border)] rounded-lg px-3 py-2 focus:outline-none focus:ring-2 focus:ring-[var(--primary)] focus:border-transparent text-[var(--foreground)] text-sm"
                  />
                </div>

                <div>
                  <label className="block text-sm font-medium text-[var(--foreground)] mb-1.5">Display Name</label>
                  <input
                    type="text"
                    value={newAgent.displayName}
                    onChange={(e) => setNewAgent({ ...newAgent, displayName: e.target.value })}
                    placeholder="My Agent"
                    className="w-full bg-[var(--background)] border border-[var(--border)] rounded-lg px-3 py-2 focus:outline-none focus:ring-2 focus:ring-[var(--primary)] focus:border-transparent text-[var(--foreground)] text-sm"
                  />
                </div>

                <div>
                  <label className="block text-sm font-medium text-[var(--foreground)] mb-1.5">Description</label>
                  <textarea
                    value={newAgent.description}
                    onChange={(e) => setNewAgent({ ...newAgent, description: e.target.value })}
                    placeholder="What does this agent do?"
                    rows={2}
                    className="w-full bg-[var(--background)] border border-[var(--border)] rounded-lg px-3 py-2 focus:outline-none focus:ring-2 focus:ring-[var(--primary)] focus:border-transparent resize-none text-[var(--foreground)] text-sm"
                  />
                </div>

                <div className="grid grid-cols-2 gap-3">
                  <div>
                    <label className="block text-sm font-medium text-[var(--foreground)] mb-1.5">Provider</label>
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
                      className="w-full bg-[var(--background)] border border-[var(--border)] rounded-lg px-3 py-2 focus:outline-none focus:ring-2 focus:ring-[var(--primary)] focus:border-transparent text-[var(--foreground)] text-sm"
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
                    <label className="block text-sm font-medium text-[var(--foreground)] mb-1.5">Model</label>
                    <select
                      value={newAgent.model}
                      onChange={(e) => setNewAgent({ ...newAgent, model: e.target.value })}
                      className="w-full bg-[var(--background)] border border-[var(--border)] rounded-lg px-3 py-2 focus:outline-none focus:ring-2 focus:ring-[var(--primary)] focus:border-transparent text-[var(--foreground)] text-sm"
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

              <div className="flex justify-end gap-2 mt-6 pt-4 border-t border-[var(--border)]">
                <button
                  onClick={() => setIsCreating(false)}
                  className="px-4 py-2 text-[var(--muted-foreground)] hover:text-[var(--foreground)] transition-colors text-sm font-medium"
                >
                  Cancel
                </button>
                <button
                  onClick={handleCreateAgent}
                  disabled={!newAgent.name || !newAgent.providerId || !newAgent.model}
                  className="bg-[var(--primary)] hover:bg-[var(--primary)]/90 disabled:opacity-50 text-white px-4 py-2 rounded-lg transition-colors text-sm font-medium"
                >
                  Create
                </button>
              </div>
            </div>
          </div>
        )}

        {/* Agents Grid */}
        {agents.length === 0 ? (
          <div className="bg-[var(--card)] rounded-xl p-10 text-center card-shadow">
            <div className="w-12 h-12 rounded-xl bg-[var(--primary)]/10 flex items-center justify-center mx-auto mb-4">
              <Bot className="w-6 h-6 text-[var(--primary)]" />
            </div>
            <h2 className="text-base font-semibold text-[var(--foreground)] mb-1">No agents yet</h2>
            <p className="text-[var(--muted-foreground)] text-sm mb-5">Create your first agent to get started</p>
            <button
              onClick={() => setIsCreating(true)}
              className="inline-flex items-center gap-2 bg-[var(--primary)] hover:bg-[var(--primary)]/90 text-white px-4 py-2 rounded-lg transition-colors text-sm font-medium"
            >
              <Plus className="w-4 h-4" />
              Create Agent
            </button>
          </div>
        ) : (
          <div className="grid gap-3 md:grid-cols-2 lg:grid-cols-3">
            {agents.map((agent) => (
              <div
                key={agent.id}
                className="bg-[var(--card)] rounded-xl p-4 card-shadow hover:shadow-md transition-shadow group"
              >
                <div className="flex items-start justify-between mb-3">
                  <div className="flex items-center gap-3">
                    <div className="w-9 h-9 rounded-lg bg-[var(--primary)] flex items-center justify-center">
                      <Bot className="w-4.5 h-4.5 text-white" />
                    </div>
                    <div>
                      <h3 className="font-medium text-[var(--foreground)] text-sm">
                        {agent.displayName || agent.name}
                      </h3>
                      <p className="text-xs text-[var(--muted-foreground)]">{agent.id}</p>
                    </div>
                  </div>
                  <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-all">
                    <button
                      onClick={() => setEditingAgent(agent)}
                      className="text-[var(--muted-foreground)] hover:text-[var(--primary)] p-1.5 hover:bg-[var(--primary)]/10 rounded-lg transition-colors"
                    >
                      <Pencil className="w-4 h-4" />
                    </button>
                    <button
                      onClick={() => handleDeleteAgent(agent.id)}
                      className="text-[var(--muted-foreground)] hover:text-[var(--destructive)] p-1.5 hover:bg-[var(--destructive)]/10 rounded-lg transition-colors"
                    >
                      <Trash2 className="w-4 h-4" />
                    </button>
                  </div>
                </div>

                {agent.description && (
                  <p className="text-sm text-[var(--muted-foreground)] mb-3 line-clamp-2">
                    {agent.description}
                  </p>
                )}

                <div className="flex flex-wrap gap-1.5">
                  <span className="inline-flex items-center gap-1 px-2 py-0.5 bg-[var(--muted)] rounded text-xs text-[var(--muted-foreground)]">
                    <Cpu className="w-3 h-3" />
                    {agent.model}
                  </span>
                  <span className="inline-flex items-center gap-1 px-2 py-0.5 bg-[var(--muted)] rounded text-xs text-[var(--muted-foreground)]">
                    <Thermometer className="w-3 h-3" />
                    {agent.temperature}
                  </span>
                  <span className="inline-flex items-center gap-1 px-2 py-0.5 bg-[var(--muted)] rounded text-xs text-[var(--muted-foreground)]">
                    <Hash className="w-3 h-3" />
                    {agent.maxTokens}
                  </span>
                </div>
              </div>
            ))}
          </div>
        )}

        {/* Edit Agent Panel */}
        {editingAgent && (
          <AgentEditPanel
            agent={editingAgent}
            onClose={() => setEditingAgent(null)}
            onSave={loadAgents}
          />
        )}
      </div>
    </div>
  );
}
