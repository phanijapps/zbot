// ============================================================================
// AGENTS FEATURE
// Agent management interface
// ============================================================================

import { useState, useEffect } from "react";
import { Bot, Plus, Trash2, Loader2, RefreshCw, Edit } from "lucide-react";
import { Button } from "@/shared/ui/button";
import { AgentIDEPage } from "./AgentIDEPage";
import * as agentService from "@/services/agent";
import * as providerService from "@/services/provider";
import type { Agent } from "@/shared/types";
import type { Provider } from "@/shared/types";
import { useVaults } from "@/features/vaults/useVaults";

export function AgentsPanel() {
  const { currentVault } = useVaults();
  const [agents, setAgents] = useState<Agent[]>([]);
  const [providers, setProviders] = useState<Provider[]>([]);
  const [loading, setLoading] = useState(true);
  const [showFullPageEditor, setShowFullPageEditor] = useState(false);
  const [editingAgent, setEditingAgent] = useState<Agent | null>(null);
  const [refreshing, setRefreshing] = useState(false);

  // Load agents and providers on mount and when vault changes
  useEffect(() => {
    loadAgents();
    loadProviders();
  }, [currentVault?.id]); // Reload when vault changes

  const loadAgents = async () => {
    setLoading(true);
    try {
      const loaded = await agentService.listAgents();
      // Filter out agent-creator - it's only accessible via + button in agent channels
      setAgents(loaded.filter(agent => agent.id !== "agent-creator"));
    } catch (error) {
      console.error("Failed to load agents:", error);
    } finally {
      setLoading(false);
    }
  };

  const loadProviders = async () => {
    try {
      const loaded = await providerService.listProviders();
      setProviders(loaded);
    } catch (error) {
      console.error("Failed to load providers:", error);
    }
  };

  const handleRefresh = async () => {
    setRefreshing(true);
    await loadAgents();
    setRefreshing(false);
  };

  const handleOpenCreateEditor = () => {
    setEditingAgent(null);
    setShowFullPageEditor(true);
  };

  const handleOpenEditEditor = (agent: Agent) => {
    setEditingAgent(agent);
    setShowFullPageEditor(true);
  };

  const handleSaveAgent = async (agent: Omit<Agent, "id" | "createdAt">) => {
    if (editingAgent) {
      await agentService.updateAgent(editingAgent.id, agent);
    } else {
      await agentService.createAgent(agent);
    }
    await loadAgents();
  };

  const handleDeleteAgent = async (id: string) => {
    if (confirm("Are you sure you want to delete this agent?")) {
      try {
        await agentService.deleteAgent(id);
        await loadAgents();
      } catch (error) {
        console.error("Failed to delete agent:", error);
      }
    }
  };

  const getProviderName = (providerId: string) => {
    const provider = providers.find((p) => p.id === providerId);
    return provider?.name || "Unknown";
  };

  const getGradientForAgent = (agentName: string) => {
    const gradients = [
      "from-blue-500 to-purple-600",
      "from-orange-500 to-pink-600",
      "from-green-500 to-teal-600",
      "from-cyan-500 to-blue-600",
      "from-pink-500 to-rose-600",
      "from-yellow-500 to-orange-600",
    ];
    const index = agentName.charCodeAt(0) % gradients.length;
    return gradients[index];
  };

  return (
    <>
      <div className="p-6">
        <div className="flex items-center justify-between mb-6">
          <div>
            <h2 className="text-2xl font-bold text-white">Agents</h2>
            <p className="text-gray-400 text-sm mt-1">
              Create and manage your AI agents
            </p>
          </div>
          <div className="flex items-center gap-2">
            <Button
              variant="outline"
              className="border-white/20 text-white hover:bg-white/5"
              onClick={handleRefresh}
              disabled={refreshing}
            >
              <RefreshCw className={`size-4 ${refreshing ? "animate-spin" : ""}`} />
            </Button>
            <Button
              className="bg-gradient-to-r from-blue-600 to-purple-600 hover:from-blue-700 hover:to-purple-700 text-white"
              onClick={handleOpenCreateEditor}
            >
              <Plus className="size-4 mr-2" />
              Add Agent
            </Button>
          </div>
        </div>

        {loading ? (
          <div className="flex items-center justify-center py-20">
            <Loader2 className="size-8 text-white animate-spin" />
          </div>
        ) : agents.length === 0 ? (
          <div className="text-center py-20">
            <Bot className="size-16 text-gray-600 mx-auto mb-4" />
            <h3 className="text-xl font-medium text-white mb-2">No Agents</h3>
            <p className="text-gray-400">Add your first agent to get started</p>
          </div>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            {agents.map((agent) => (
              <div
                key={agent.id}
                className="bg-gradient-to-br from-white/5 to-white/[0.02] rounded-xl p-5 border border-white/10 hover:border-white/20 transition-all"
              >
                <div className="flex items-start justify-between mb-4">
                  <div className="flex items-start gap-3">
                    <div className={`p-2.5 rounded-xl bg-gradient-to-br ${getGradientForAgent(agent.name)}`}>
                      <Bot className="size-4 text-white" />
                    </div>
                    <div>
                      <h3 className="text-white font-semibold">{agent.displayName}</h3>
                      <p className="text-xs text-gray-500">{agent.name}</p>
                    </div>
                  </div>
                  <div className="flex items-center gap-1">
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => handleOpenEditEditor(agent)}
                      className="text-gray-400 hover:text-white h-7 w-7 p-0"
                    >
                      <Edit className="size-3.5" />
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => handleDeleteAgent(agent.id)}
                      className="text-gray-400 hover:text-red-400 h-7 w-7 p-0"
                    >
                      <Trash2 className="size-3.5" />
                    </Button>
                  </div>
                </div>

                <p className="text-gray-400 text-sm mb-3">{agent.description}</p>

                {/* Provider and Model */}
                <div className="bg-black/30 rounded-lg p-2.5 mb-3 border border-white/5">
                  <p className="text-xs text-gray-500 mb-1">Provider</p>
                  <p className="text-xs text-gray-300 font-mono">
                    {getProviderName(agent.providerId)} · {agent.model.length > 25 ? agent.model.substring(0, 25) + "..." : agent.model}
                  </p>
                </div>

                {/* Temperature, MCPs, Skills */}
                <div className="flex flex-wrap gap-1.5">
                  <span className="px-2 py-0.5 bg-purple-500/10 rounded-full text-xs text-purple-300 border border-purple-500/20">
                    Temp: {agent.temperature.toFixed(1)}
                  </span>
                  {agent.mcps.length > 0 && (
                    <span className="px-2 py-0.5 bg-green-500/10 rounded-full text-xs text-green-300 border border-green-500/20">
                      {agent.mcps.length} MCP{agent.mcps.length > 1 ? "s" : ""}
                    </span>
                  )}
                  {agent.skills.length > 0 && (
                    <span className="px-2 py-0.5 bg-blue-500/10 rounded-full text-xs text-blue-300 border border-blue-500/20">
                      {agent.skills.length} skill{agent.skills.length > 1 ? "s" : ""}
                    </span>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}

        {/* Info Box */}
        <div className="mt-6 bg-purple-500/10 border border-purple-500/20 rounded-xl p-4">
          <div className="flex items-start gap-3">
            <Bot className="size-5 text-purple-400 shrink-0 mt-0.5" />
            <div className="flex-1">
              <p className="text-sm font-medium text-purple-200 mb-2">
                About Agents
              </p>
              <p className="text-xs text-purple-300">
                Agents are AI assistants with specific instructions, providers, models, and optional MCP servers and skills.
                Each agent is stored in its own folder with AGENTS.md (instructions + metadata) and mcp.json (MCP references).
              </p>
              <p className="text-xs text-purple-300 mt-2">
                💾 Configuration saved to: <code className="bg-white/10 px-1.5 py-0.5 rounded">~/.config/zeroagent/agents/</code>
              </p>
            </div>
          </div>
        </div>
      </div>

      {showFullPageEditor && (
        <AgentIDEPage
          onClose={() => setShowFullPageEditor(false)}
          onSave={handleSaveAgent}
          onAgentUpdated={(updatedAgent) => {
            setEditingAgent(updatedAgent);
            loadAgents();
          }}
          initialAgent={editingAgent}
        />
      )}
    </>
  );
}
