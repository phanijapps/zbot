// ============================================================================
// WEB SETTINGS PANEL
// Clean settings page: system info, context protection, logging
// ============================================================================

import { useState, useEffect } from "react";
import {
  Loader2, ChevronDown, ChevronRight,
  FileText, AlertTriangle, Shield, Activity, Check,
} from "lucide-react";
import {
  getTransport,
  type ToolSettings,
  type LogSettings,
  type UpdateLogSettingsRequest,
} from "@/services/transport";

// ============================================================================
// Component
// ============================================================================

export function WebSettingsPanel() {
  // Tool settings (for offload controls only)
  const [toolSettings, setToolSettings] = useState<ToolSettings | null>(null);
  const [isLoadingTools, setIsLoadingTools] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [saveMessage, setSaveMessage] = useState<string | null>(null);

  // Log settings
  const [logSettings, setLogSettings] = useState<LogSettings | null>(null);
  const [isLoadingLogs, setIsLoadingLogs] = useState(true);
  const [isSavingLogs, setIsSavingLogs] = useState(false);
  const [logsSaveMessage, setLogsSaveMessage] = useState<string | null>(null);

  // Section open/close
  const [memoryOpen, setMemoryOpen] = useState(false);
  const [logsOpen, setLogsOpen] = useState(false);

  useEffect(() => {
    loadAll();
  }, []);

  const loadAll = async () => {
    setIsLoadingTools(true);
    setIsLoadingLogs(true);
    try {
      const transport = await getTransport();
      const [toolsResult, logsResult] = await Promise.all([
        transport.getToolSettings(),
        transport.getLogSettings(),
      ]);
      if (toolsResult.success && toolsResult.data) setToolSettings(toolsResult.data);
      if (logsResult.success && logsResult.data) setLogSettings(logsResult.data);
    } catch {
      // Silently degrade — sections show loading state
    } finally {
      setIsLoadingTools(false);
      setIsLoadingLogs(false);
    }
  };

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

  // ── Render ──
  return (
    <div className="page">
      <div className="page-container page-container--narrow">
        <div className="page-header">
          <div className="page-header__content">
            <h1 className="page-title">Settings</h1>
            <p className="page-subtitle">System configuration and performance tuning</p>
          </div>
        </div>

        <div className="flex flex-col gap-4">

          {/* ═══════════════════════════════════════════════════════
              SYSTEM INFO — compact info bar
             ═══════════════════════════════════════════════════════ */}
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

          {/* ═══════════════════════════════════════════════════════
              CONTEXT MEMORY — offload protection
             ═══════════════════════════════════════════════════════ */}
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

          {/* ═══════════════════════════════════════════════════════
              LOG SETTINGS — file logging and rotation
             ═══════════════════════════════════════════════════════ */}
          <div className="card card__padding--lg">
            <button onClick={() => setLogsOpen(!logsOpen)} className="settings-toggle-btn">
              <div className="flex items-center gap-3">
                <div className="card__icon card__icon--primary">
                  <FileText style={{ width: 18, height: 18 }} />
                </div>
                <div>
                  <h2 className="settings-section-header">Logging</h2>
                  <p className="page-subtitle">File logging, rotation, and retention</p>
                </div>
              </div>
              {logsOpen ? <ChevronDown className="settings-chevron" /> : <ChevronRight className="settings-chevron" />}
            </button>

            {logsOpen && (
              <div className="settings-expandable">
                <div className="settings-alert settings-alert--warning">
                  <AlertTriangle className="settings-alert__icon" />
                  Log changes require a daemon restart to take effect
                </div>

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
            )}
          </div>

        </div>
      </div>
    </div>
  );
}
