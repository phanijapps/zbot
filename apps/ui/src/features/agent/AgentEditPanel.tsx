// ============================================================================
// AGENT EDIT PANEL
// Slide-over panel for editing agent configuration
// Uses the shared Slideover component with sections:
//   Basic | Model | Skills | Schedules | Advanced
// ============================================================================

import { useState, useEffect, useCallback } from "react";
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
  Zap,
  Calendar,
  Play,
  Pause,
  Plus,
} from "lucide-react";
import {
  getTransport,
  getProviderDefaultModel,
  type AgentResponse,
  type UpdateAgentRequest,
  type ProviderResponse,
  type SkillResponse,
  type McpListResponse,
  type ModelRegistryResponse,
  type CronJobResponse,
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
  const [skills, setSkills] = useState<SkillResponse[]>([]);
  const [mcps, setMcps] = useState<McpListResponse["servers"]>([]);
  const [agentSchedules, setAgentSchedules] = useState<CronJobResponse[]>([]);

  // UI state
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [advancedOpen, setAdvancedOpen] = useState(false);

  // Load skills, MCPs, and schedules for this agent
  useEffect(() => {
    loadData();
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const loadData = async () => {
    setIsLoading(true);
    try {
      const transport = await getTransport();
      const [skillsResult, mcpsResult, schedulesResult] = await Promise.all([
        transport.listSkills(),
        transport.listMcps(),
        transport.listCronJobs(),
      ]);

      if (skillsResult.success && skillsResult.data) {
        setSkills(skillsResult.data);
      }
      if (mcpsResult.success && mcpsResult.data) {
        setMcps(mcpsResult.data.servers || []);
      }
      if (schedulesResult.success && schedulesResult.data) {
        setAgentSchedules(schedulesResult.data.filter((j) => j.agent_id === agent.id));
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

  const handleToggleSchedule = useCallback(async (job: CronJobResponse) => {
    try {
      const transport = await getTransport();
      const result = job.enabled
        ? await transport.disableCronJob(job.id)
        : await transport.enableCronJob(job.id);
      if (result.success && result.data) {
        setAgentSchedules((prev) => prev.map((j) => (j.id === job.id ? result.data! : j)));
      }
    } catch {
      // silently ignore toggle errors in inline view
    }
  }, []);

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
      <section style={{ marginBottom: "var(--spacing-6)" }}>
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
      <section style={{ marginBottom: "var(--spacing-6)" }}>
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

      {/* ── Skills ── */}
      <section style={{ marginBottom: "var(--spacing-6)" }}>
        <h3 style={{
          fontSize: "var(--text-sm)", fontWeight: 600, color: "var(--foreground)",
          marginBottom: "var(--spacing-3)", display: "flex", alignItems: "center", gap: "var(--spacing-2)",
        }}>
          <Zap style={{ width: 16, height: 16, color: "var(--warning)" }} />
          Skills
        </h3>
        {skills.length === 0 ? (
          <p style={{ fontSize: "var(--text-sm)", color: "var(--muted-foreground)", padding: "var(--spacing-3)", background: "var(--background-elevated)", borderRadius: "var(--radius-md)", border: "1px solid var(--border)" }}>
            No skills configured. Create skills in the Skills Library tab.
          </p>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: "var(--spacing-2)" }}>
            {skills.map((skill) => {
              const isOn = (formData.skills || []).includes(skill.id);
              return (
                <div
                  key={skill.id}
                  className={`skill-toggle ${isOn ? "skill-toggle--on" : ""}`}
                  onClick={() => toggleSkill(skill.id)}
                >
                  <button
                    className={`toggle-switch ${isOn ? "toggle-switch--on" : "toggle-switch--off"}`}
                    onClick={(e) => {
                      e.stopPropagation();
                      toggleSkill(skill.id);
                    }}
                  />
                  <div className="skill-toggle__info">
                    <div className="skill-toggle__name">{skill.displayName}</div>
                    <div className="skill-toggle__desc">{skill.description}</div>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </section>

      {/* ── Schedules targeting this agent ── */}
      <section style={{ marginBottom: "var(--spacing-6)" }}>
        <h3 style={{
          fontSize: "var(--text-sm)", fontWeight: 600, color: "var(--foreground)",
          marginBottom: "var(--spacing-3)", display: "flex", alignItems: "center", gap: "var(--spacing-2)",
        }}>
          <Calendar style={{ width: 16, height: 16, color: "var(--muted-foreground)" }} />
          Schedules
        </h3>
        {agentSchedules.length === 0 ? (
          <p style={{ fontSize: "var(--text-sm)", color: "var(--muted-foreground)", padding: "var(--spacing-3)", background: "var(--background-elevated)", borderRadius: "var(--radius-md)", border: "1px solid var(--border)" }}>
            No schedules target this agent.
          </p>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: "var(--spacing-2)" }}>
            {agentSchedules.map((job) => (
              <div key={job.id} className="schedule-card" style={{ padding: "var(--spacing-3)" }}>
                <div className={`schedule-card__icon ${job.enabled ? "schedule-card__icon--active" : "schedule-card__icon--paused"}`} style={{ width: 32, height: 32 }}>
                  {job.enabled ? <Play style={{ width: 14, height: 14 }} /> : <Pause style={{ width: 14, height: 14 }} />}
                </div>
                <div className="schedule-card__info">
                  <div className="schedule-card__name">{job.name}</div>
                  <div className="schedule-card__cron">{job.schedule}</div>
                </div>
                <button
                  className={`toggle-switch ${job.enabled ? "toggle-switch--on" : "toggle-switch--off"}`}
                  onClick={() => handleToggleSchedule(job)}
                />
              </div>
            ))}
          </div>
        )}
        <button
          className="add-link"
          onClick={() => {
            onClose();
            // Navigate to schedules tab
            window.location.hash = "";
            const params = new URLSearchParams(window.location.search);
            params.set("tab", "schedules");
            window.history.pushState(null, "", `?${params.toString()}`);
            window.dispatchEvent(new PopStateEvent("popstate"));
          }}
        >
          <Plus style={{ width: 14, height: 14 }} />
          Add schedule
        </button>
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
