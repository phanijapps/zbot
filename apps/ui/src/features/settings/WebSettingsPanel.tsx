// ============================================================================
// WEB SETTINGS PANEL
// Tabbed settings page: Providers, General, Logging
// ============================================================================

import { useState, useEffect, useCallback } from "react";
import { useSearchParams } from "react-router-dom";
import {
  Loader2, ChevronDown, ChevronRight, Plus,
  Shield, Activity, Check,
} from "lucide-react";
import {
  getTransport,
  type ToolSettings,
  type LogSettings,
  type UpdateLogSettingsRequest,
  type ExecutionSettings,
  type ProviderResponse,
  type ModelRegistryResponse,
} from "@/services/transport";
import { TabBar, TabPanel } from "@/components/TabBar";
import { HelpBox } from "@/components/HelpBox";
import { ProvidersEmptyState } from "./ProvidersEmptyState";
import { ProvidersGrid } from "./ProvidersGrid";
import { ProviderSlideover } from "./ProviderSlideover";
import type { ProviderPreset } from "./providerPresets";
import { getAvailablePresets } from "./providerPresets";

// ============================================================================
// Component
// ============================================================================

export function WebSettingsPanel() {
  const [searchParams, setSearchParams] = useSearchParams();
  const activeTab = searchParams.get("tab") || "providers";

  const setActiveTab = useCallback(
    (tab: string) => {
      setSearchParams(tab === "providers" ? {} : { tab });
    },
    [setSearchParams],
  );

  // ── Provider state ──
  const [providers, setProviders] = useState<ProviderResponse[]>([]);
  const [modelRegistry, setModelRegistry] = useState<ModelRegistryResponse>({});
  const [isLoadingProviders, setIsLoadingProviders] = useState(true);
  const [providerError, setProviderError] = useState<string | null>(null);

  // Slide-over state
  const [slideoverOpen, setSlideoverOpen] = useState(false);
  const [slideoverMode, setSlideoverMode] = useState<"view" | "create">("view");
  const [selectedProvider, setSelectedProvider] = useState<ProviderResponse | null>(null);
  const [createPreset, setCreatePreset] = useState<ProviderPreset | null>(null);

  // Add-more state (shows preset grid when providers exist)
  const [showAddPresets, setShowAddPresets] = useState(false);

  const defaultProvider = providers.find((p) => p.isDefault);
  const defaultProviderId = defaultProvider?.id;

  // ── Tool settings (offload) state ──
  const [toolSettings, setToolSettings] = useState<ToolSettings | null>(null);
  const [isLoadingTools, setIsLoadingTools] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [saveMessage, setSaveMessage] = useState<string | null>(null);

  // ── Log settings state ──
  const [logSettings, setLogSettings] = useState<LogSettings | null>(null);
  const [isLoadingLogs, setIsLoadingLogs] = useState(true);
  const [isSavingLogs, setIsSavingLogs] = useState(false);
  const [logsSaveMessage, setLogsSaveMessage] = useState<string | null>(null);

  // ── Execution settings state ──
  const [execSettings, setExecSettings] = useState<ExecutionSettings | null>(null);
  const [isLoadingExec, setIsLoadingExec] = useState(true);
  const [isSavingExec, setIsSavingExec] = useState(false);
  const [execSaveMessage, setExecSaveMessage] = useState<string | null>(null);
  const [execRestartRequired, setExecRestartRequired] = useState(false);

  // ── Section open/close (General tab) ──
  const [memoryOpen, setMemoryOpen] = useState(false);

  // ── Data loading ──
  useEffect(() => {
    loadAll();
  }, []);

  const loadAll = async () => {
    setIsLoadingProviders(true);
    setIsLoadingTools(true);
    setIsLoadingLogs(true);
    setIsLoadingExec(true);
    setProviderError(null);
    try {
      const transport = await getTransport();
      const [providersResult, modelsResult, toolsResult, logsResult, execResult] = await Promise.all([
        transport.listProviders(),
        transport.listModels(),
        transport.getToolSettings(),
        transport.getLogSettings(),
        transport.getExecutionSettings(),
      ]);
      if (providersResult.success && providersResult.data) {
        setProviders(providersResult.data);
      } else {
        setProviderError(providersResult.error || "Failed to load providers");
      }
      if (modelsResult.success && modelsResult.data) {
        setModelRegistry(modelsResult.data);
      }
      if (toolsResult.success && toolsResult.data) setToolSettings(toolsResult.data);
      if (logsResult.success && logsResult.data) setLogSettings(logsResult.data);
      if (execResult.success && execResult.data) setExecSettings(execResult.data);
    } catch (err) {
      setProviderError(err instanceof Error ? err.message : "Unknown error");
    } finally {
      setIsLoadingProviders(false);
      setIsLoadingTools(false);
      setIsLoadingLogs(false);
      setIsLoadingExec(false);
    }
  };

  const loadProviders = async () => {
    setIsLoadingProviders(true);
    setProviderError(null);
    try {
      const transport = await getTransport();
      const [providersResult, modelsResult] = await Promise.all([
        transport.listProviders(),
        transport.listModels(),
      ]);
      if (providersResult.success && providersResult.data) {
        setProviders(providersResult.data);
      } else {
        setProviderError(providersResult.error || "Failed to load providers");
      }
      if (modelsResult.success && modelsResult.data) {
        setModelRegistry(modelsResult.data);
      }
    } catch (err) {
      setProviderError(err instanceof Error ? err.message : "Unknown error");
    } finally {
      setIsLoadingProviders(false);
    }
  };

  // ── Provider actions ──
  const handleSelectProvider = useCallback((provider: ProviderResponse) => {
    setSelectedProvider(provider);
    setSlideoverMode("view");
    setCreatePreset(null);
    setSlideoverOpen(true);
  }, []);

  const handleOpenCreate = useCallback((preset?: ProviderPreset) => {
    setSelectedProvider(null);
    setSlideoverMode("create");
    setCreatePreset(preset || null);
    setSlideoverOpen(true);
    setShowAddPresets(false);
  }, []);

  const handleCloseSlider = useCallback(() => {
    setSlideoverOpen(false);
    setSelectedProvider(null);
    setCreatePreset(null);
  }, []);

  const handleProviderSaved = useCallback(() => {
    handleCloseSlider();
    loadProviders();
  }, [handleCloseSlider]);

  const handleProviderDeleted = useCallback(() => {
    handleCloseSlider();
    loadProviders();
  }, [handleCloseSlider]);

  const handleSetActive = useCallback(async (id: string) => {
    try {
      const transport = await getTransport();
      const result = await transport.setDefaultProvider(id);
      if (result.success) {
        await loadProviders();
      } else {
        setProviderError(result.error || "Failed to set active provider");
      }
    } catch {
      setProviderError("Failed to set active provider");
    }
  }, []);

  // ── Offload save handler ──
  const handleOffloadChange = async (updates: Partial<ToolSettings>) => {
    if (!toolSettings) return;
    const newSettings = { ...toolSettings, ...updates };
    setToolSettings(newSettings);
    setIsSaving(true);
    setSaveMessage(null);
    try {
      const transport = await getTransport();
      const result = await transport.updateToolSettings(newSettings);
      setSaveMessage(result.success ? "Saved" : result.error || "Failed to save");
      if (!result.success) setToolSettings(toolSettings);
    } catch {
      setSaveMessage("Failed to save");
      setToolSettings(toolSettings);
    } finally {
      setIsSaving(false);
      setTimeout(() => setSaveMessage(null), 2500);
    }
  };

  // ── Log setting save handler ──
  const handleLogChange = async (updates: Partial<UpdateLogSettingsRequest>) => {
    if (!logSettings) return;
    const newSettings = { ...logSettings, ...updates };
    setLogSettings(newSettings);
    setIsSavingLogs(true);
    setLogsSaveMessage(null);
    try {
      const transport = await getTransport();
      const result = await transport.updateLogSettings(updates);
      setLogsSaveMessage(result.success ? "Saved" : result.error || "Failed to save");
      if (!result.success) setLogSettings(logSettings);
    } catch {
      setLogsSaveMessage("Failed to save");
      setLogSettings(logSettings);
    } finally {
      setIsSavingLogs(false);
      setTimeout(() => setLogsSaveMessage(null), 2500);
    }
  };

  // ── Execution setting save handler ──
  const handleExecChange = async (updates: Partial<ExecutionSettings>) => {
    if (!execSettings) return;
    const newSettings = { ...execSettings, ...updates };
    // Strip restartRequired before sending
    const { restartRequired: _, ...clean } = newSettings as ExecutionSettings & { restartRequired?: boolean };
    setExecSettings(clean);
    setIsSavingExec(true);
    setExecSaveMessage(null);
    try {
      const transport = await getTransport();
      const result = await transport.updateExecutionSettings(clean);
      if (result.success) {
        setExecSaveMessage("Saved");
        setExecRestartRequired(result.data?.restartRequired ?? false);
      } else {
        setExecSaveMessage(result.error || "Failed to save");
        setExecSettings(execSettings);
      }
    } catch {
      setExecSaveMessage("Failed to save");
      setExecSettings(execSettings);
    } finally {
      setIsSavingExec(false);
      setTimeout(() => setExecSaveMessage(null), 2500);
    }
  };

  // ── Derived ──
  const hasProviders = providers.length > 0;
  const availablePresets = getAvailablePresets(providers);

  // ── Render ──
  return (
    <div className="page" style={{ display: "flex", flexDirection: "column" }}>
      <div className="page-header-v2">
        <h1 className="page-title-v2">Settings</h1>
        <p className="page-subtitle-v2">
          Configure your AI providers, system preferences, and logging.
          Start here if you're new — add a provider to get going.
        </p>
      </div>

      <TabBar
        tabs={[
          { id: "providers", label: "Providers", count: providers.length },
          { id: "general", label: "General" },
          { id: "logging", label: "Logging" },
          { id: "advanced", label: "Advanced" },
        ]}
        activeTab={activeTab}
        onTabChange={setActiveTab}
      />

      <div className="page-content-v2">
        {/* ═══════════════════════════════════════════════════════════════════
            PROVIDERS TAB
           ═══════════════════════════════════════════════════════════════════ */}
        <TabPanel id="providers" activeTab={activeTab}>
          {isLoadingProviders ? (
            <div style={{ display: "flex", alignItems: "center", justifyContent: "center", padding: "var(--spacing-12)" }}>
              <Loader2 className="loading-spinner__icon" style={{ color: "var(--primary)" }} />
            </div>
          ) : hasProviders ? (
            <>
              {/* Subheader with count + add button */}
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "var(--spacing-5)" }}>
                <p style={{ fontSize: "var(--text-sm)", color: "var(--muted-foreground)" }}>
                  {providers.filter((p) => p.verified).length} connected · {defaultProvider ? "1 active" : "none active"}
                </p>
                <button
                  className="btn btn--primary btn--sm"
                  onClick={() => availablePresets.length > 0 ? setShowAddPresets(!showAddPresets) : handleOpenCreate()}
                >
                  <Plus size={14} /> Add Provider
                </button>
              </div>

              {/* Error */}
              {providerError && (
                <div className="alert alert--error" style={{ marginBottom: "var(--spacing-4)" }}>
                  <span>{providerError}</span>
                  <button className="btn btn--ghost btn--sm" onClick={() => setProviderError(null)} style={{ marginLeft: "auto" }}>Dismiss</button>
                </div>
              )}

              {/* Add-more preset section */}
              {showAddPresets && (
                <div style={{ marginBottom: "var(--spacing-5)" }}>
                  <div style={{ display: "flex", flexWrap: "wrap", gap: "var(--spacing-2)", marginBottom: "var(--spacing-3)" }}>
                    {availablePresets.map((preset) => (
                      <button
                        key={preset.name}
                        className="btn btn--outline btn--sm"
                        onClick={() => handleOpenCreate(preset)}
                      >
                        {preset.name}
                      </button>
                    ))}
                    <button className="btn btn--ghost btn--sm" onClick={() => handleOpenCreate()}>
                      Custom...
                    </button>
                  </div>
                </div>
              )}

              {/* Provider card grid */}
              <ProvidersGrid
                providers={providers}
                modelRegistry={modelRegistry}
                defaultProviderId={defaultProviderId}
                onSelect={handleSelectProvider}
              />

              {/* Add another link */}
              {availablePresets.length > 0 && !showAddPresets && (
                <div style={{ textAlign: "center", marginTop: "var(--spacing-4)" }}>
                  <button className="btn btn--ghost btn--sm" onClick={() => setShowAddPresets(true)}>
                    <Plus size={14} /> Add another provider
                  </button>
                </div>
              )}
            </>
          ) : (
            /* Empty state for new users */
            <ProvidersEmptyState
              existingProviders={providers}
              onProviderCreated={loadProviders}
              onOpenCustom={() => handleOpenCreate()}
            />
          )}

          <HelpBox>
            <strong>What are providers?</strong> Providers are the AI services that power your agents.
            Each provider gives access to one or more models with different capabilities — like reasoning,
            tool use, or vision. You need at least one provider to start.
          </HelpBox>
        </TabPanel>

        {/* ═══════════════════════════════════════════════════════════════════
            GENERAL TAB
           ═══════════════════════════════════════════════════════════════════ */}
        <TabPanel id="general" activeTab={activeTab}>
          <div className="flex flex-col gap-4">
            {/* System info bar */}
            <div className="card card__padding">
              <div className="flex items-center gap-2" style={{ fontSize: "var(--text-xs)", color: "var(--muted-foreground)" }}>
                <Activity style={{ width: 14, height: 14, color: "var(--primary)", flexShrink: 0 }} />
                <span>API <code style={{ color: "var(--foreground)" }}>localhost:18791</code></span>
                <span style={{ opacity: 0.3 }}>|</span>
                <span>WS <code style={{ color: "var(--foreground)" }}>localhost:18790</code></span>
                <span style={{ opacity: 0.3 }}>|</span>
                <span>Data <code style={{ color: "var(--foreground)" }}>~/Documents/zbot/</code></span>
              </div>
            </div>

            {/* Context protection */}
            <div className="card card__padding--lg">
              <button onClick={() => setMemoryOpen(!memoryOpen)} className="settings-toggle-btn">
                <div className="flex items-center gap-3">
                  <div className="card__icon card__icon--warning">
                    <Shield style={{ width: 18, height: 18 }} />
                  </div>
                  <div>
                    <h2 className="settings-section-header">Context Protection</h2>
                    <p className="page-subtitle">Prevent agents from running out of context window</p>
                  </div>
                </div>
                {memoryOpen ? <ChevronDown className="settings-chevron" /> : <ChevronRight className="settings-chevron" />}
              </button>

              {memoryOpen && (
                <div className="settings-expandable">
                  {isLoadingTools ? (
                    <div className="settings-loading"><Loader2 className="loading-spinner__icon" /></div>
                  ) : toolSettings ? (
                    <div className="flex flex-col gap-4">
                      <p className="settings-toggle-option__description">
                        Large tool results (file reads, shell output, web pages) can overwhelm the agent's context window.
                        When enabled, results exceeding the threshold are saved to a temp file and the agent reads them on demand.
                      </p>

                      <label className={`settings-toggle-option ${toolSettings.offloadLargeResults ? "settings-toggle-option--active" : ""}`}
                        style={{ opacity: isSaving ? 0.7 : 1 }}>
                        <input
                          type="checkbox"
                          checked={toolSettings.offloadLargeResults}
                          onChange={() => handleOffloadChange({ offloadLargeResults: !toolSettings.offloadLargeResults })}
                          disabled={isSaving}
                          className="settings-toggle-option__checkbox"
                        />
                        <div className="flex-1">
                          <div className="settings-toggle-option__title">Offload Large Results</div>
                          <div className="settings-toggle-option__description">
                            Recommended. Saves large outputs to disk instead of filling the context.
                          </div>
                        </div>
                      </label>

                      {toolSettings.offloadLargeResults && (
                        <div>
                          <label className="settings-field-label">Threshold (tokens)</label>
                          <input
                            type="number"
                            value={toolSettings.offloadThresholdTokens}
                            onChange={(e) => handleOffloadChange({ offloadThresholdTokens: parseInt(e.target.value, 10) || 1000 })}
                            disabled={isSaving}
                            className="form-input"
                            min={1000}
                            step={1000}
                            style={{ maxWidth: 200 }}
                          />
                          <p className="settings-hint">
                            Default: 5,000 tokens (~20K characters). Lower values offload more aggressively.
                          </p>
                        </div>
                      )}

                      {saveMessage && (
                        <div className={`settings-alert ${saveMessage === "Saved" ? "settings-alert--success" : "settings-alert--error"}`}>
                          {saveMessage === "Saved" && <Check style={{ width: 14, height: 14 }} />}
                          {saveMessage}
                        </div>
                      )}
                    </div>
                  ) : null}
                </div>
              )}
            </div>
          </div>

          <HelpBox>
            <strong>Context protection</strong> helps prevent slowdowns. When an agent's response is very large,
            z-Bot saves it to disk instead of keeping it in memory.
          </HelpBox>
        </TabPanel>

        {/* ═══════════════════════════════════════════════════════════════════
            LOGGING TAB
           ═══════════════════════════════════════════════════════════════════ */}
        <TabPanel id="logging" activeTab={activeTab}>
          <div className="flex flex-col gap-4">
            <p style={{ fontSize: "var(--text-sm)", color: "var(--muted-foreground)" }}>
              Log changes require a daemon restart to take effect.
            </p>

            {isLoadingLogs ? (
              <div className="settings-loading"><Loader2 className="loading-spinner__icon" /></div>
            ) : logSettings ? (
              <div className="flex flex-col gap-4">

                {/* Enable toggle */}
                <label className={`settings-toggle-option ${logSettings.enabled ? "settings-toggle-option--active" : ""}`}
                  style={{ opacity: isSavingLogs ? 0.7 : 1 }}>
                  <input type="checkbox" checked={logSettings.enabled}
                    onChange={() => handleLogChange({ enabled: !logSettings.enabled })}
                    disabled={isSavingLogs} className="settings-toggle-option__checkbox" />
                  <div className="flex-1">
                    <div className="settings-toggle-option__title">Enable File Logging</div>
                    <div className="settings-toggle-option__description">Write structured logs to disk for debugging and auditing</div>
                  </div>
                </label>

                {logSettings.enabled && (
                  <>
                    {/* Level + Rotation side by side */}
                    <div className="grid grid-cols-2 gap-3">
                      <div>
                        <label className="settings-field-label">Level</label>
                        <select value={logSettings.level}
                          onChange={(e) => handleLogChange({ level: e.target.value as LogSettings["level"] })}
                          disabled={isSavingLogs} className="form-select form-input">
                          <option value="info">Info (default)</option>
                          <option value="debug">Debug</option>
                          <option value="trace">Trace (verbose)</option>
                          <option value="warn">Warn</option>
                          <option value="error">Error only</option>
                        </select>
                      </div>
                      <div>
                        <label className="settings-field-label">Rotation</label>
                        <select value={logSettings.rotation}
                          onChange={(e) => handleLogChange({ rotation: e.target.value as LogSettings["rotation"] })}
                          disabled={isSavingLogs} className="form-select form-input">
                          <option value="daily">Daily (default)</option>
                          <option value="hourly">Hourly</option>
                          <option value="never">Never</option>
                        </select>
                      </div>
                    </div>

                    {/* Max files + Suppress stdout side by side */}
                    <div className="grid grid-cols-2 gap-3">
                      <div>
                        <label className="settings-field-label">Keep Last N Files</label>
                        <input type="number" value={logSettings.maxFiles}
                          onChange={(e) => handleLogChange({ maxFiles: parseInt(e.target.value, 10) || 0 })}
                          disabled={isSavingLogs} className="form-input" min={0} />
                        <p className="settings-hint">0 = unlimited</p>
                      </div>
                      <div>
                        <label className="settings-field-label">&nbsp;</label>
                        <label className="settings-toggle-option" style={{ opacity: isSavingLogs ? 0.7 : 1, marginTop: 0 }}>
                          <input type="checkbox" checked={logSettings.suppressStdout}
                            onChange={() => handleLogChange({ suppressStdout: !logSettings.suppressStdout })}
                            disabled={isSavingLogs} className="settings-toggle-option__checkbox" />
                          <div className="flex-1">
                            <div className="settings-toggle-option__title">Quiet Mode</div>
                            <div className="settings-toggle-option__description">Log to file only, no stdout</div>
                          </div>
                        </label>
                      </div>
                    </div>
                  </>
                )}

                {logsSaveMessage && (
                  <div className={`settings-alert ${logsSaveMessage === "Saved" ? "settings-alert--success" : "settings-alert--error"}`}>
                    {logsSaveMessage === "Saved" && <Check style={{ width: 14, height: 14 }} />}
                    {logsSaveMessage}
                  </div>
                )}
              </div>
            ) : null}
          </div>

          <HelpBox>
            <strong>Logs</strong> help you troubleshoot when something goes wrong. You usually don't need to
            change these unless you're debugging. Changes take effect after restarting the daemon.
          </HelpBox>
        </TabPanel>

        {/* ═══════════════════════════════════════════════════════════════════
            ADVANCED TAB
           ═══════════════════════════════════════════════════════════════════ */}
        <TabPanel id="advanced" activeTab={activeTab}>
          <div className="flex flex-col gap-4">
            <p style={{ fontSize: "var(--text-sm)", color: "var(--muted-foreground)" }}>
              Changes to execution settings require a daemon restart to take effect.
            </p>

            {isLoadingExec ? (
              <div className="settings-loading"><Loader2 className="loading-spinner__icon" /></div>
            ) : execSettings ? (
              <div className="flex flex-col gap-4">

                {/* Max Parallel Agents */}
                <div className="card card__padding--lg">
                  <div className="flex items-center gap-3" style={{ marginBottom: "var(--spacing-3)" }}>
                    <div className="card__icon card__icon--primary">
                      <Activity style={{ width: 18, height: 18 }} />
                    </div>
                    <div>
                      <h2 className="settings-section-header">Execution</h2>
                      <p className="page-subtitle">Control how agents run in parallel</p>
                    </div>
                  </div>

                  <div style={{ marginTop: "var(--spacing-3)" }}>
                    <label className="settings-field-label">Max Parallel Agents</label>
                    <input
                      type="number"
                      value={execSettings.maxParallelAgents}
                      onChange={(e) => handleExecChange({ maxParallelAgents: Math.max(1, parseInt(e.target.value, 10) || 1) })}
                      disabled={isSavingExec}
                      className="form-input"
                      min={1}
                      max={10}
                      style={{ maxWidth: 200 }}
                    />
                    <p className="settings-hint">
                      Maximum subagents that can execute simultaneously across all sessions.
                      Lower values reduce API load; higher values speed up parallel tasks.
                      Default: 2.
                    </p>
                  </div>

                  {execRestartRequired && (
                    <div className="settings-alert settings-alert--warning" style={{ marginTop: "var(--spacing-3)" }}>
                      Restart the daemon for changes to take effect.
                    </div>
                  )}

                  {execSaveMessage && (
                    <div className={`settings-alert ${execSaveMessage === "Saved" ? "settings-alert--success" : "settings-alert--error"}`} style={{ marginTop: "var(--spacing-3)" }}>
                      {execSaveMessage === "Saved" && <Check style={{ width: 14, height: 14 }} />}
                      {execSaveMessage}
                    </div>
                  )}

                  <div style={{ marginTop: "var(--spacing-5)", paddingTop: "var(--spacing-4)", borderTop: "1px solid var(--border)" }}>
                    <h3 className="settings-field-label" style={{ marginBottom: "var(--spacing-2)" }}>Setup Wizard</h3>
                    <p className="settings-hint" style={{ marginBottom: "var(--spacing-3)" }}>
                      Re-run the first-time setup wizard to reconfigure providers, agents, and defaults.
                    </p>
                    <button
                      className="btn btn--outline btn--sm"
                      onClick={() => { window.location.href = "/setup"; }}
                    >
                      Re-run Setup Wizard
                    </button>
                  </div>
                </div>
              </div>
            ) : null}

            {/* Orchestrator Config */}
            {execSettings && (
              <div className="card card__padding--lg">
                <div className="flex items-center gap-3" style={{ marginBottom: "var(--spacing-3)" }}>
                  <div className="card__icon card__icon--primary">
                    <Activity style={{ width: 18, height: 18 }} />
                  </div>
                  <div>
                    <h2 className="settings-section-header">Orchestrator</h2>
                    <p className="page-subtitle">Configure the root agent that handles your conversations</p>
                  </div>
                </div>

                <div className="grid grid-cols-2 gap-3">
                  <div>
                    <label className="settings-field-label">Provider</label>
                    <select
                      className="form-input form-select"
                      value={execSettings.orchestrator?.providerId || ""}
                      onChange={(e) => handleExecChange({
                        orchestrator: {
                          ...execSettings.orchestrator || { temperature: 0.7, maxTokens: 16384, thinkingEnabled: true },
                          providerId: e.target.value || null,
                          model: null,
                        },
                      })}
                    >
                      <option value="">Default Provider</option>
                      {providers.filter((p) => p.verified).map((p) => (
                        <option key={p.id} value={p.id}>{p.name}</option>
                      ))}
                    </select>
                  </div>
                  <div>
                    <label className="settings-field-label">Model</label>
                    <select
                      className="form-input form-select"
                      value={execSettings.orchestrator?.model || ""}
                      onChange={(e) => handleExecChange({
                        orchestrator: {
                          ...execSettings.orchestrator || { temperature: 0.7, maxTokens: 16384, thinkingEnabled: true },
                          model: e.target.value || null,
                        },
                      })}
                    >
                      <option value="">Default Model</option>
                      {(providers.find((p) => p.id === (execSettings.orchestrator?.providerId || defaultProviderId))?.models || []).map((m) => (
                        <option key={m} value={m}>{m}</option>
                      ))}
                    </select>
                  </div>
                  <div>
                    <label className="settings-field-label">Temperature</label>
                    <input
                      className="form-input"
                      type="number"
                      min={0} max={2} step={0.1}
                      value={execSettings.orchestrator?.temperature ?? 0.7}
                      onChange={(e) => handleExecChange({
                        orchestrator: {
                          ...execSettings.orchestrator || { temperature: 0.7, maxTokens: 16384, thinkingEnabled: true },
                          temperature: parseFloat(e.target.value) || 0.7,
                        },
                      })}
                    />
                  </div>
                  <div>
                    <label className="settings-field-label">Max Output Tokens</label>
                    <input
                      className="form-input"
                      type="number"
                      min={1024} step={1024}
                      value={execSettings.orchestrator?.maxTokens ?? 16384}
                      onChange={(e) => handleExecChange({
                        orchestrator: {
                          ...execSettings.orchestrator || { temperature: 0.7, maxTokens: 16384, thinkingEnabled: true },
                          maxTokens: parseInt(e.target.value) || 16384,
                        },
                      })}
                    />
                  </div>
                </div>

                <label className={`settings-toggle-option ${execSettings.orchestrator?.thinkingEnabled !== false ? "settings-toggle-option--active" : ""}`}
                  style={{ marginTop: "var(--spacing-3)" }}>
                  <input
                    type="checkbox"
                    checked={execSettings.orchestrator?.thinkingEnabled !== false}
                    onChange={() => handleExecChange({
                      orchestrator: {
                        ...execSettings.orchestrator || { temperature: 0.7, maxTokens: 16384, thinkingEnabled: true },
                        thinkingEnabled: execSettings.orchestrator?.thinkingEnabled === false,
                      },
                    })}
                    className="settings-toggle-option__checkbox"
                  />
                  <div className="flex-1">
                    <div className="settings-toggle-option__title">Thinking Mode</div>
                    <div className="settings-toggle-option__description">
                      Enable extended reasoning — the orchestrator thinks before delegating, improving plan quality.
                    </div>
                  </div>
                </label>

                {/* Distillation Config */}
                <div style={{ marginTop: "var(--spacing-4)", paddingTop: "var(--spacing-3)", borderTop: "1px solid var(--border-secondary)" }}>
                  <div style={{ marginBottom: "var(--spacing-2)" }}>
                    <h3 className="settings-field-label" style={{ fontSize: "var(--font-size-sm)", fontWeight: 600 }}>Distillation</h3>
                    <p className="page-subtitle" style={{ fontSize: "var(--font-size-xs)" }}>Override the model used for memory extraction. Inherits from orchestrator by default.</p>
                  </div>
                  <div className="grid grid-cols-2 gap-3">
                    <div>
                      <label className="settings-field-label">Provider</label>
                      <select
                        className="form-input form-select"
                        value={execSettings.distillation?.providerId || ""}
                        onChange={(e) => handleExecChange({
                          distillation: {
                            ...execSettings.distillation,
                            providerId: e.target.value || null,
                            model: e.target.value ? (execSettings.distillation?.model || null) : null,
                          },
                        })}
                      >
                        <option value="">Inherit from Orchestrator</option>
                        {providers.filter((p) => p.verified).map((p) => (
                          <option key={p.id} value={p.id}>{p.name}</option>
                        ))}
                      </select>
                    </div>
                    <div>
                      <label className="settings-field-label">Model</label>
                      <select
                        className="form-input form-select"
                        value={execSettings.distillation?.model || ""}
                        onChange={(e) => handleExecChange({
                          distillation: {
                            ...execSettings.distillation,
                            model: e.target.value || null,
                          },
                        })}
                      >
                        <option value="">Inherit from Orchestrator</option>
                        {(() => {
                          const distProviderId = execSettings.distillation?.providerId
                            || execSettings.orchestrator?.providerId
                            || defaultProviderId;
                          return (providers.find((p) => p.id === distProviderId)?.models || []).map((m) => (
                            <option key={m} value={m}>{m}</option>
                          ));
                        })()}
                      </select>
                    </div>
                  </div>
                </div>
              </div>
            )}
          </div>

          <HelpBox>
            <strong>Advanced settings</strong> control low-level execution behavior.
            Most users won't need to change these. Changes require a daemon restart.
          </HelpBox>
        </TabPanel>
      </div>

      {/* Slide-over detail/edit/create panel (always mounted for Providers tab) */}
      <ProviderSlideover
        provider={selectedProvider}
        modelRegistry={modelRegistry}
        isActive={selectedProvider?.id === defaultProviderId}
        isOpen={slideoverOpen}
        mode={slideoverMode}
        preset={createPreset}
        onClose={handleCloseSlider}
        onSaved={handleProviderSaved}
        onDeleted={handleProviderDeleted}
        onSetActive={handleSetActive}
      />
    </div>
  );
}
