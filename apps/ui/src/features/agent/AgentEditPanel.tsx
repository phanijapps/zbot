// ============================================================================
// AGENT EDIT PANEL
// Slide-over panel for editing agent configuration
// ============================================================================

import { useState, useEffect } from "react";
import {
  Bot,
  X,
  Save,
  ArrowLeft,
  Cpu,
  Thermometer,
  Hash,
  FileText,
  Server,
  Zap,
  ChevronDown,
  ChevronRight,
  Loader2,
  Brain,
  Mic,
} from "lucide-react";
import {
  getTransport,
  type AgentResponse,
  type UpdateAgentRequest,
  type ProviderResponse,
  type SkillResponse,
  type McpListResponse,
} from "@/services/transport";

// ============================================================================
// Types
// ============================================================================

interface AgentEditPanelProps {
  agent: AgentResponse;
  onClose: () => void;
  onSave: () => void;
}

// ============================================================================
// Component
// ============================================================================

export function AgentEditPanel({ agent, onClose, onSave }: AgentEditPanelProps) {
  // Form state
  const [formData, setFormData] = useState<UpdateAgentRequest>({
    displayName: agent.displayName,
    description: agent.description,
    providerId: agent.providerId,
    model: agent.model,
    temperature: agent.temperature,
    maxTokens: agent.maxTokens,
    thinkingEnabled: agent.thinkingEnabled,
    voiceRecordingEnabled: agent.voiceRecordingEnabled,
    instructions: agent.instructions,
    mcps: agent.mcps || [],
    skills: agent.skills || [],
  });

  // Data for dropdowns
  const [providers, setProviders] = useState<ProviderResponse[]>([]);
  const [skills, setSkills] = useState<SkillResponse[]>([]);
  const [mcps, setMcps] = useState<McpListResponse["servers"]>([]);

  // UI state
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [advancedOpen, setAdvancedOpen] = useState(false);

  // Load providers, skills, and MCPs
  useEffect(() => {
    loadData();
  }, []);

  const loadData = async () => {
    setIsLoading(true);
    try {
      const transport = await getTransport();
      const [providersResult, skillsResult, mcpsResult] = await Promise.all([
        transport.listProviders(),
        transport.listSkills(),
        transport.listMcps(),
      ]);

      if (providersResult.success && providersResult.data) {
        setProviders(providersResult.data);
      }
      if (skillsResult.success && skillsResult.data) {
        setSkills(skillsResult.data);
      }
      if (mcpsResult.success && mcpsResult.data) {
        setMcps(mcpsResult.data.servers || []);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load data");
    } finally {
      setIsLoading(false);
    }
  };

  const handleSave = async () => {
    setIsSaving(true);
    setError(null);
    try {
      const transport = await getTransport();
      const result = await transport.updateAgent(agent.id, formData);
      if (result.success) {
        onSave();
        onClose();
      } else {
        setError(result.error || "Failed to update agent");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unknown error");
    } finally {
      setIsSaving(false);
    }
  };

  const selectedProvider = providers.find((p) => p.id === formData.providerId);

  const toggleMcp = (mcpId: string) => {
    const currentMcps = formData.mcps || [];
    if (currentMcps.includes(mcpId)) {
      setFormData({ ...formData, mcps: currentMcps.filter((id) => id !== mcpId) });
    } else {
      setFormData({ ...formData, mcps: [...currentMcps, mcpId] });
    }
  };

  const toggleSkill = (skillId: string) => {
    const currentSkills = formData.skills || [];
    if (currentSkills.includes(skillId)) {
      setFormData({ ...formData, skills: currentSkills.filter((id) => id !== skillId) });
    } else {
      setFormData({ ...formData, skills: [...currentSkills, skillId] });
    }
  };

  if (isLoading) {
    return (
      <div className="fixed inset-0 bg-black/50 z-50 flex justify-end">
        <div className="w-full max-w-2xl bg-[var(--background)] flex items-center justify-center">
          <Loader2 className="w-6 h-6 text-[var(--primary)] animate-spin" />
        </div>
      </div>
    );
  }

  return (
    <div className="fixed inset-0 bg-black/50 z-50 flex justify-end">
      <div className="w-full max-w-2xl bg-[var(--background)] flex flex-col shadow-2xl">
        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-[var(--border)]">
          <div className="flex items-center gap-3">
            <button
              onClick={onClose}
              className="p-1.5 hover:bg-[var(--muted)] rounded-lg transition-colors text-[var(--muted-foreground)] hover:text-[var(--foreground)]"
            >
              <ArrowLeft className="w-4 h-4" />
            </button>
            <div className="flex items-center gap-2">
              <div className="w-8 h-8 rounded-lg bg-[var(--primary)] flex items-center justify-center">
                <Bot className="w-4 h-4 text-white" />
              </div>
              <div>
                <h2 className="text-base font-semibold text-[var(--foreground)]">Edit Agent</h2>
                <p className="text-xs text-[var(--muted-foreground)]">{agent.id}</p>
              </div>
            </div>
          </div>
          <div className="flex items-center gap-2">
            <button
              onClick={onClose}
              className="px-3 py-1.5 text-sm text-[var(--muted-foreground)] hover:text-[var(--foreground)] transition-colors"
            >
              Cancel
            </button>
            <button
              onClick={handleSave}
              disabled={isSaving}
              className="inline-flex items-center gap-1.5 bg-[var(--primary)] hover:bg-[var(--primary)]/90 disabled:opacity-50 text-white px-3 py-1.5 rounded-lg text-sm font-medium transition-colors"
            >
              {isSaving ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <Save className="w-3.5 h-3.5" />}
              Save
            </button>
          </div>
        </div>

        {/* Error message */}
        {error && (
          <div className="mx-6 mt-4 p-3 bg-[var(--destructive)]/10 text-[var(--destructive)] rounded-lg flex items-center justify-between text-sm">
            <span>{error}</span>
            <button onClick={() => setError(null)} className="hover:opacity-70">
              <X className="w-4 h-4" />
            </button>
          </div>
        )}

        {/* Form */}
        <div className="flex-1 overflow-auto p-6 space-y-6">
          {/* Basic Info */}
          <section>
            <h3 className="text-sm font-semibold text-[var(--foreground)] mb-3">Basic Information</h3>
            <div className="space-y-3">
              <div>
                <label className="block text-sm text-[var(--muted-foreground)] mb-1.5">Display Name</label>
                <input
                  type="text"
                  value={formData.displayName || ""}
                  onChange={(e) => setFormData({ ...formData, displayName: e.target.value })}
                  className="w-full bg-[var(--card)] border border-[var(--border)] rounded-lg px-3 py-2 text-sm text-[var(--foreground)] focus:outline-none focus:ring-2 focus:ring-[var(--primary)] focus:border-transparent"
                  placeholder="My Agent"
                />
              </div>
              <div>
                <label className="block text-sm text-[var(--muted-foreground)] mb-1.5">Description</label>
                <textarea
                  value={formData.description || ""}
                  onChange={(e) => setFormData({ ...formData, description: e.target.value })}
                  rows={2}
                  className="w-full bg-[var(--card)] border border-[var(--border)] rounded-lg px-3 py-2 text-sm text-[var(--foreground)] focus:outline-none focus:ring-2 focus:ring-[var(--primary)] focus:border-transparent resize-none"
                  placeholder="What does this agent do?"
                />
              </div>
            </div>
          </section>

          {/* Model Configuration */}
          <section>
            <h3 className="text-sm font-semibold text-[var(--foreground)] mb-3 flex items-center gap-2">
              <Cpu className="w-4 h-4 text-[var(--muted-foreground)]" />
              Model Configuration
            </h3>
            <div className="space-y-3">
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-sm text-[var(--muted-foreground)] mb-1.5">Provider</label>
                  <select
                    value={formData.providerId || ""}
                    onChange={(e) => {
                      const provider = providers.find((p) => p.id === e.target.value);
                      setFormData({
                        ...formData,
                        providerId: e.target.value,
                        model: provider?.models[0] || "",
                      });
                    }}
                    className="w-full bg-[var(--card)] border border-[var(--border)] rounded-lg px-3 py-2 text-sm text-[var(--foreground)] focus:outline-none focus:ring-2 focus:ring-[var(--primary)] focus:border-transparent"
                  >
                    {providers.map((provider) => (
                      <option key={provider.id} value={provider.id}>
                        {provider.name}
                      </option>
                    ))}
                  </select>
                </div>
                <div>
                  <label className="block text-sm text-[var(--muted-foreground)] mb-1.5">Model</label>
                  <select
                    value={formData.model || ""}
                    onChange={(e) => setFormData({ ...formData, model: e.target.value })}
                    className="w-full bg-[var(--card)] border border-[var(--border)] rounded-lg px-3 py-2 text-sm text-[var(--foreground)] focus:outline-none focus:ring-2 focus:ring-[var(--primary)] focus:border-transparent"
                  >
                    {selectedProvider?.models.map((model) => (
                      <option key={model} value={model}>
                        {model}
                      </option>
                    )) || <option value="">Select a provider first</option>}
                  </select>
                </div>
              </div>
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-sm text-[var(--muted-foreground)] mb-1.5 flex items-center gap-1.5">
                    <Thermometer className="w-3.5 h-3.5" />
                    Temperature
                  </label>
                  <div className="flex items-center gap-3">
                    <input
                      type="range"
                      min="0"
                      max="2"
                      step="0.1"
                      value={formData.temperature || 0.7}
                      onChange={(e) => setFormData({ ...formData, temperature: parseFloat(e.target.value) })}
                      className="flex-1 accent-[var(--primary)]"
                    />
                    <span className="text-sm text-[var(--foreground)] w-10 text-right">
                      {formData.temperature?.toFixed(1) || "0.7"}
                    </span>
                  </div>
                </div>
                <div>
                  <label className="block text-sm text-[var(--muted-foreground)] mb-1.5 flex items-center gap-1.5">
                    <Hash className="w-3.5 h-3.5" />
                    Max Tokens
                  </label>
                  <input
                    type="number"
                    value={formData.maxTokens || 4096}
                    onChange={(e) => setFormData({ ...formData, maxTokens: parseInt(e.target.value) || 4096 })}
                    className="w-full bg-[var(--card)] border border-[var(--border)] rounded-lg px-3 py-2 text-sm text-[var(--foreground)] focus:outline-none focus:ring-2 focus:ring-[var(--primary)] focus:border-transparent"
                    min="1"
                    max="128000"
                  />
                </div>
              </div>
            </div>
          </section>

          {/* System Prompt */}
          <section>
            <h3 className="text-sm font-semibold text-[var(--foreground)] mb-3 flex items-center gap-2">
              <FileText className="w-4 h-4 text-[var(--muted-foreground)]" />
              System Prompt
            </h3>
            <textarea
              value={formData.instructions || ""}
              onChange={(e) => setFormData({ ...formData, instructions: e.target.value })}
              rows={10}
              className="w-full bg-[var(--card)] border border-[var(--border)] rounded-lg px-3 py-2 text-sm text-[var(--foreground)] focus:outline-none focus:ring-2 focus:ring-[var(--primary)] focus:border-transparent resize-none font-mono"
              placeholder="System instructions for the agent..."
            />
          </section>

          {/* MCPs */}
          <section>
            <h3 className="text-sm font-semibold text-[var(--foreground)] mb-3 flex items-center gap-2">
              <Server className="w-4 h-4 text-[var(--muted-foreground)]" />
              MCP Servers
            </h3>
            {mcps.length === 0 ? (
              <p className="text-sm text-[var(--muted-foreground)] bg-[var(--card)] rounded-lg p-3 border border-[var(--border)]">
                No MCP servers configured
              </p>
            ) : (
              <div className="space-y-1.5">
                {mcps.map((mcp) => (
                  <label
                    key={mcp.id}
                    className="flex items-center gap-3 p-2.5 bg-[var(--card)] rounded-lg border border-[var(--border)] cursor-pointer hover:border-[var(--primary)]/50 transition-colors"
                  >
                    <input
                      type="checkbox"
                      checked={(formData.mcps || []).includes(mcp.id)}
                      onChange={() => toggleMcp(mcp.id)}
                      className="w-4 h-4 rounded border-[var(--border)] text-[var(--primary)] focus:ring-[var(--primary)]"
                    />
                    <div className="flex-1 min-w-0">
                      <div className="text-sm font-medium text-[var(--foreground)]">{mcp.name}</div>
                      <div className="text-xs text-[var(--muted-foreground)] truncate">{mcp.description}</div>
                    </div>
                    <span className="text-xs text-[var(--muted-foreground)] bg-[var(--muted)] px-1.5 py-0.5 rounded">
                      {mcp.type}
                    </span>
                  </label>
                ))}
              </div>
            )}
          </section>

          {/* Skills */}
          <section>
            <h3 className="text-sm font-semibold text-[var(--foreground)] mb-3 flex items-center gap-2">
              <Zap className="w-4 h-4 text-[var(--warning)]" />
              Skills
            </h3>
            {skills.length === 0 ? (
              <p className="text-sm text-[var(--muted-foreground)] bg-[var(--card)] rounded-lg p-3 border border-[var(--border)]">
                No skills configured
              </p>
            ) : (
              <div className="space-y-1.5">
                {skills.map((skill) => (
                  <label
                    key={skill.id}
                    className="flex items-center gap-3 p-2.5 bg-[var(--card)] rounded-lg border border-[var(--border)] cursor-pointer hover:border-[var(--primary)]/50 transition-colors"
                  >
                    <input
                      type="checkbox"
                      checked={(formData.skills || []).includes(skill.id)}
                      onChange={() => toggleSkill(skill.id)}
                      className="w-4 h-4 rounded border-[var(--border)] text-[var(--primary)] focus:ring-[var(--primary)]"
                    />
                    <div className="flex-1 min-w-0">
                      <div className="text-sm font-medium text-[var(--foreground)]">{skill.displayName}</div>
                      <div className="text-xs text-[var(--muted-foreground)] truncate">{skill.description}</div>
                    </div>
                    <span className="text-xs text-[var(--muted-foreground)] bg-[var(--muted)] px-1.5 py-0.5 rounded">
                      {skill.category}
                    </span>
                  </label>
                ))}
              </div>
            )}
          </section>

          {/* Advanced Options */}
          <section>
            <button
              onClick={() => setAdvancedOpen(!advancedOpen)}
              className="flex items-center gap-2 text-sm font-semibold text-[var(--foreground)] hover:text-[var(--primary)] transition-colors w-full"
            >
              {advancedOpen ? <ChevronDown className="w-4 h-4" /> : <ChevronRight className="w-4 h-4" />}
              Advanced Options
            </button>
            {advancedOpen && (
              <div className="mt-3 space-y-3 pl-6">
                <label className="flex items-center gap-3 p-2.5 bg-[var(--card)] rounded-lg border border-[var(--border)] cursor-pointer hover:border-[var(--primary)]/50 transition-colors">
                  <input
                    type="checkbox"
                    checked={formData.thinkingEnabled || false}
                    onChange={(e) => setFormData({ ...formData, thinkingEnabled: e.target.checked })}
                    className="w-4 h-4 rounded border-[var(--border)] text-[var(--primary)] focus:ring-[var(--primary)]"
                  />
                  <Brain className="w-4 h-4 text-[var(--muted-foreground)]" />
                  <div className="flex-1">
                    <div className="text-sm font-medium text-[var(--foreground)]">Thinking Enabled</div>
                    <div className="text-xs text-[var(--muted-foreground)]">
                      Allow the model to show reasoning steps
                    </div>
                  </div>
                </label>
                <label className="flex items-center gap-3 p-2.5 bg-[var(--card)] rounded-lg border border-[var(--border)] cursor-pointer hover:border-[var(--primary)]/50 transition-colors">
                  <input
                    type="checkbox"
                    checked={formData.voiceRecordingEnabled || false}
                    onChange={(e) => setFormData({ ...formData, voiceRecordingEnabled: e.target.checked })}
                    className="w-4 h-4 rounded border-[var(--border)] text-[var(--primary)] focus:ring-[var(--primary)]"
                  />
                  <Mic className="w-4 h-4 text-[var(--muted-foreground)]" />
                  <div className="flex-1">
                    <div className="text-sm font-medium text-[var(--foreground)]">Voice Recording</div>
                    <div className="text-xs text-[var(--muted-foreground)]">
                      Enable voice input for this agent
                    </div>
                  </div>
                </label>
              </div>
            )}
          </section>
        </div>
      </div>
    </div>
  );
}
