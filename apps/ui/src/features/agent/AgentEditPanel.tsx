// ============================================================================
// AGENT EDIT PANEL
// Slide-over panel for editing agent configuration
// Uses the shared Slideover component with sections:
//   Basic | Model | Schedules | Advanced
// ============================================================================

import { useState, useEffect } from "react";
import {
  Bot,
  Cpu,
  Thermometer,
  Hash,
  FileText,
  Server,
  ChevronDown,
  ChevronRight,
  Loader2,
  Brain,
  Mic,
} from "lucide-react";
import {
  getTransport,
  getProviderDefaultModel,
  type AgentResponse,
  type UpdateAgentRequest,
  type ProviderResponse,
  type McpListResponse,
  type ModelRegistryResponse,
} from "@/services/transport";
import { Slideover } from "@/components/Slideover";
import { ModelChip } from "@/shared/ui/ModelChip";

// ============================================================================
// Types
// ============================================================================

interface AgentEditPanelProps {
  agent: AgentResponse;
  providers: ProviderResponse[];
  modelRegistry: ModelRegistryResponse;
  onClose: () => void;
  onSave: () => void;
}

// ============================================================================
// Component
// ============================================================================

export function AgentEditPanel({ agent, providers, modelRegistry, onClose, onSave }: AgentEditPanelProps) {
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
  const [mcps, setMcps] = useState<McpListResponse["servers"]>([]);

  // UI state
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [advancedOpen, setAdvancedOpen] = useState(false);

  // Load skills, MCPs, and schedules for this agent
  useEffect(() => {
    loadData();
  }, []);  

  const loadData = async () => {
    setIsLoading(true);
    try {
      const transport = await getTransport();
      const mcpsResult = await transport.listMcps();
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

  if (isLoading) {
    return (
      <Slideover
        open={true}
        onClose={onClose}
        title="Loading..."
        icon={<Bot style={{ width: 18, height: 18 }} />}
      >
        <div style={{ display: "flex", alignItems: "center", justifyContent: "center", padding: "var(--spacing-12)" }}>
          <Loader2 style={{ width: 24, height: 24, animation: "spin 1s linear infinite" }} />
        </div>
      </Slideover>
    );
  }

  return (
    <Slideover
      open={true}
      onClose={onClose}
      title={agent.displayName || agent.name}
      subtitle={agent.id}
      icon={<Bot style={{ width: 18, height: 18 }} />}
      footer={
        <>
          <button className="btn btn--secondary btn--md" onClick={onClose}>
            Cancel
          </button>
          <button
            className="btn btn--primary btn--md"
            onClick={handleSave}
            disabled={isSaving}
          >
            {isSaving ? "Saving..." : "Save Changes"}
          </button>
        </>
      }
    >
      {/* Error message */}
      {error && (
        <div className="alert alert--error" style={{ marginBottom: "var(--spacing-4)" }}>
          {error}
        </div>
      )}

      {/* ── Basic ── */}
      <section className="slideover__section">
        <h3 style={{ fontSize: "var(--text-sm)", fontWeight: 600, color: "var(--foreground)", marginBottom: "var(--spacing-3)" }}>
          Basic Information
        </h3>
        <div className="form-group">
          <label className="form-label">Display Name</label>
          <input
            className="form-input"
            type="text"
            value={formData.displayName || ""}
            onChange={(e) => setFormData({ ...formData, displayName: e.target.value })}
            placeholder="My Agent"
          />
        </div>
        <div className="form-group">
          <label className="form-label">Description</label>
          <textarea
            className="form-textarea"
            value={formData.description || ""}
            onChange={(e) => setFormData({ ...formData, description: e.target.value })}
            rows={2}
            placeholder="What does this agent do?"
          />
        </div>
      </section>

      {/* ── Model ── */}
      <section className="slideover__section">
        <h3 style={{
          fontSize: "var(--text-sm)", fontWeight: 600, color: "var(--foreground)",
          marginBottom: "var(--spacing-3)", display: "flex", alignItems: "center", gap: "var(--spacing-2)",
        }}>
          <Cpu style={{ width: 16, height: 16, color: "var(--muted-foreground)" }} />
          Model Configuration
        </h3>
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "var(--spacing-3)" }}>
          <div className="form-group">
            <label className="form-label">Provider</label>
            <select
              className="form-select"
              value={formData.providerId || ""}
              onChange={(e) => {
                const provider = providers.find((p) => p.id === e.target.value);
                setFormData({
                  ...formData,
                  providerId: e.target.value,
                  model: provider ? getProviderDefaultModel(provider) : "",
                });
              }}
            >
              {providers.map((p) => (
                <option key={p.id} value={p.id}>{p.name}</option>
              ))}
            </select>
          </div>
          <div className="form-group">
            <label className="form-label">Model</label>
            <select
              className="form-select"
              value={formData.model || ""}
              onChange={(e) => setFormData({ ...formData, model: e.target.value })}
            >
              {selectedProvider?.models.map((m) => (
                <option key={m} value={m}>{m}</option>
              )) || <option value="">Select a provider first</option>}
            </select>
            {formData.model && modelRegistry[formData.model] && (
              <div style={{ marginTop: "var(--spacing-2)" }}>
                <ModelChip
                  modelId={formData.model}
                  profile={modelRegistry[formData.model]}
                  showContext
                />
              </div>
            )}
          </div>
        </div>
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "var(--spacing-3)", marginTop: "var(--spacing-3)" }}>
          <div className="form-group">
            <label className="form-label" style={{ display: "flex", alignItems: "center", gap: "var(--spacing-1)" }}>
              <Thermometer style={{ width: 14, height: 14 }} />
              Temperature
            </label>
            <div style={{ display: "flex", alignItems: "center", gap: "var(--spacing-3)" }}>
              <input
                type="range"
                min="0"
                max="2"
                step="0.1"
                value={formData.temperature || 0.7}
                onChange={(e) => setFormData({ ...formData, temperature: parseFloat(e.target.value) })}
                style={{ flex: 1 }}
              />
              <span style={{ fontSize: "var(--text-sm)", color: "var(--foreground)", width: 40, textAlign: "right" }}>
                {formData.temperature?.toFixed(1) || "0.7"}
              </span>
            </div>
          </div>
          <div className="form-group">
            <label className="form-label" style={{ display: "flex", alignItems: "center", gap: "var(--spacing-1)" }}>
              <Hash style={{ width: 14, height: 14 }} />
              Max Tokens
            </label>
            <input
              className="form-input"
              type="number"
              value={formData.maxTokens || 4096}
              onChange={(e) => setFormData({ ...formData, maxTokens: parseInt(e.target.value) || 4096 })}
              min="1"
              max="128000"
            />
          </div>
        </div>
      </section>

      {/* ── Advanced ── */}
      <section>
        <button
          onClick={() => setAdvancedOpen(!advancedOpen)}
          style={{
            display: "flex", alignItems: "center", gap: "var(--spacing-2)",
            fontSize: "var(--text-sm)", fontWeight: 600, color: "var(--foreground)",
            background: "none", border: "none", cursor: "pointer", padding: 0,
            fontFamily: "var(--font-body)",
          }}
        >
          {advancedOpen ? <ChevronDown style={{ width: 16, height: 16 }} /> : <ChevronRight style={{ width: 16, height: 16 }} />}
          Advanced Options
        </button>

        {advancedOpen && (
          <div style={{ marginTop: "var(--spacing-3)", display: "flex", flexDirection: "column", gap: "var(--spacing-3)" }}>
            {/* Thinking toggle */}
            <div
              className="skill-toggle"
              onClick={() => setFormData({ ...formData, thinkingEnabled: !formData.thinkingEnabled })}
            >
              <button
                className={`toggle-switch ${formData.thinkingEnabled ? "toggle-switch--on" : "toggle-switch--off"}`}
                onClick={(e) => {
                  e.stopPropagation();
                  setFormData({ ...formData, thinkingEnabled: !formData.thinkingEnabled });
                }}
              />
              <Brain style={{ width: 16, height: 16, color: "var(--muted-foreground)" }} />
              <div className="skill-toggle__info">
                <div className="skill-toggle__name">Thinking Enabled</div>
                <div className="skill-toggle__desc">Allow the model to show reasoning steps</div>
              </div>
            </div>

            {/* Voice toggle */}
            <div
              className="skill-toggle"
              onClick={() => setFormData({ ...formData, voiceRecordingEnabled: !formData.voiceRecordingEnabled })}
            >
              <button
                className={`toggle-switch ${formData.voiceRecordingEnabled ? "toggle-switch--on" : "toggle-switch--off"}`}
                onClick={(e) => {
                  e.stopPropagation();
                  setFormData({ ...formData, voiceRecordingEnabled: !formData.voiceRecordingEnabled });
                }}
              />
              <Mic style={{ width: 16, height: 16, color: "var(--muted-foreground)" }} />
              <div className="skill-toggle__info">
                <div className="skill-toggle__name">Voice Recording</div>
                <div className="skill-toggle__desc">Enable voice input for this agent</div>
              </div>
            </div>

            {/* System Prompt */}
            <div className="form-group">
              <label className="form-label" style={{ display: "flex", alignItems: "center", gap: "var(--spacing-1)" }}>
                <FileText style={{ width: 14, height: 14 }} />
                System Prompt
              </label>
              <textarea
                className="form-textarea"
                value={formData.instructions || ""}
                onChange={(e) => setFormData({ ...formData, instructions: e.target.value })}
                rows={10}
                placeholder="System instructions for the agent..."
                style={{ fontFamily: "var(--font-mono)" }}
              />
            </div>

            {/* MCP Servers */}
            <div className="form-group">
              <label className="form-label" style={{ display: "flex", alignItems: "center", gap: "var(--spacing-1)" }}>
                <Server style={{ width: 14, height: 14 }} />
                MCP Servers
              </label>
              {mcps.length === 0 ? (
                <p style={{ fontSize: "var(--text-sm)", color: "var(--muted-foreground)", padding: "var(--spacing-3)", background: "var(--background-elevated)", borderRadius: "var(--radius-md)", border: "1px solid var(--border)" }}>
                  No MCP servers configured
                </p>
              ) : (
                <div style={{ display: "flex", flexDirection: "column", gap: "var(--spacing-2)" }}>
                  {mcps.map((mcp) => {
                    const isOn = (formData.mcps || []).includes(mcp.id);
                    return (
                      <div
                        key={mcp.id}
                        className={`skill-toggle ${isOn ? "skill-toggle--on" : ""}`}
                        onClick={() => toggleMcp(mcp.id)}
                      >
                        <button
                          className={`toggle-switch ${isOn ? "toggle-switch--on" : "toggle-switch--off"}`}
                          onClick={(e) => {
                            e.stopPropagation();
                            toggleMcp(mcp.id);
                          }}
                        />
                        <div className="skill-toggle__info">
                          <div className="skill-toggle__name">{mcp.name}</div>
                          <div className="skill-toggle__desc">{mcp.description}</div>
                        </div>
                        <span style={{ fontSize: "var(--text-xs)", color: "var(--dim-foreground)", background: "var(--background-elevated)", padding: "2px 6px", borderRadius: "var(--radius-sm)" }}>
                          {mcp.type}
                        </span>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          </div>
        )}
      </section>
    </Slideover>
  );
}
