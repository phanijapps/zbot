// ============================================================================
// WEB SETTINGS PANEL
// Application settings: tools, logging, gateway info
// ============================================================================

import { useState, useEffect } from "react";
import {
  Server, Loader2, Wrench, ChevronDown, ChevronRight,
  Terminal, Users, Eye, FileText, AlertTriangle,
  HardDrive, Shield,
} from "lucide-react";
import {
  getTransport,
  type ToolSettings,
  type LogSettings,
  type UpdateLogSettingsRequest,
} from "@/services/transport";

// ============================================================================
// Tool group definitions — must match backend ToolSettings fields
// ============================================================================

interface ToolDef {
  key: keyof ToolSettings;
  label: string;
  description: string;
}

interface ToolGroup {
  name: string;
  icon: React.ElementType;
  tools: ToolDef[];
}

const TOOL_GROUPS: ToolGroup[] = [
  {
    name: "Execution",
    icon: Terminal,
    tools: [
      { key: "python", label: "Python", description: "Execute Python scripts directly" },
      { key: "webFetch", label: "Web Fetch", description: "HTTP requests (can produce large responses)" },
    ],
  },
  {
    name: "File Tools",
    icon: HardDrive,
    tools: [
      { key: "fileTools", label: "File Tools", description: "Separate read, write, edit, glob tools (otherwise uses shell + apply_patch)" },
      { key: "todos", label: "Todos", description: "Heavyweight SQLite task persistence (otherwise uses lightweight update_plan)" },
    ],
  },
  {
    name: "UI & Agents",
    icon: Users,
    tools: [
      { key: "uiTools", label: "UI Tools", description: "Interactive UI components (request_input, show_content)" },
      { key: "createAgent", label: "Create Agent", description: "Allow agents to create new agents dynamically" },
    ],
  },
  {
    name: "Introspection",
    icon: Eye,
    tools: [
      { key: "introspection", label: "Introspection", description: "Self-analysis tools (list_tools, list_mcps)" },
    ],
  },
];

// ============================================================================
// Component
// ============================================================================

export function WebSettingsPanel() {
  // Tool settings
  const [toolSettings, setToolSettings] = useState<ToolSettings | null>(null);
  const [isLoadingTools, setIsLoadingTools] = useState(true);
  const [isSavingTools, setIsSavingTools] = useState(false);
  const [toolsError, setToolsError] = useState<string | null>(null);
  const [toolsSaved, setToolsSaved] = useState(false);
  const [advancedOpen, setAdvancedOpen] = useState(false);

  // Log settings
  const [logSettings, setLogSettings] = useState<LogSettings | null>(null);
  const [isLoadingLogs, setIsLoadingLogs] = useState(true);
  const [isSavingLogs, setIsSavingLogs] = useState(false);
  const [logsError, setLogsError] = useState<string | null>(null);
  const [logsSaved, setLogsSaved] = useState(false);
  const [logsOpen, setLogsOpen] = useState(false);

  // Memory management
  const [memoryOpen, setMemoryOpen] = useState(false);

  useEffect(() => {
    loadToolSettings();
    loadLogSettings();
  }, []);

  const loadToolSettings = async () => {
    setIsLoadingTools(true);
    setToolsError(null);
    try {
      const transport = await getTransport();
      const result = await transport.getToolSettings();
      if (result.success && result.data) {
        setToolSettings(result.data);
      } else {
        setToolsError(result.error || "Failed to load tool settings");
      }
    } catch (err) {
      setToolsError(err instanceof Error ? err.message : "Unknown error");
    } finally {
      setIsLoadingTools(false);
    }
  };

  const loadLogSettings = async () => {
    setIsLoadingLogs(true);
    setLogsError(null);
    try {
      const transport = await getTransport();
      const result = await transport.getLogSettings();
      if (result.success && result.data) {
        setLogSettings(result.data);
      } else {
        setLogsError(result.error || "Failed to load log settings");
      }
    } catch (err) {
      setLogsError(err instanceof Error ? err.message : "Unknown error");
    } finally {
      setIsLoadingLogs(false);
    }
  };

  const handleToolToggle = async (key: keyof ToolSettings) => {
    if (!toolSettings) return;
    const val = toolSettings[key];
    if (typeof val !== "boolean") return;

    const newSettings = { ...toolSettings, [key]: !val };
    setToolSettings(newSettings);
    setIsSavingTools(true);
    setToolsSaved(false);
    try {
      const transport = await getTransport();
      const result = await transport.updateToolSettings(newSettings);
      if (result.success) {
        setToolsSaved(true);
        setTimeout(() => setToolsSaved(false), 2000);
      } else {
        setToolsError(result.error || "Failed to save");
        setToolSettings(toolSettings);
      }
    } catch (err) {
      setToolsError(err instanceof Error ? err.message : "Unknown error");
      setToolSettings(toolSettings);
    } finally {
      setIsSavingTools(false);
    }
  };

  const handleOffloadChange = async (updates: Partial<ToolSettings>) => {
    if (!toolSettings) return;
    const newSettings = { ...toolSettings, ...updates };
    setToolSettings(newSettings);
    setIsSavingTools(true);
    try {
      const transport = await getTransport();
      const result = await transport.updateToolSettings(newSettings);
      if (result.success) {
        setToolsSaved(true);
        setTimeout(() => setToolsSaved(false), 2000);
      } else {
        setToolsError(result.error || "Failed to save");
        setToolSettings(toolSettings);
      }
    } catch (err) {
      setToolsError(err instanceof Error ? err.message : "Unknown error");
      setToolSettings(toolSettings);
    } finally {
      setIsSavingTools(false);
    }
  };

  const handleLogSettingChange = async (updates: Partial<UpdateLogSettingsRequest>) => {
    if (!logSettings) return;
    const newSettings = { ...logSettings, ...updates };
    setLogSettings(newSettings);
    setIsSavingLogs(true);
    setLogsSaved(false);
    try {
      const transport = await getTransport();
      const result = await transport.updateLogSettings(updates);
      if (result.success) {
        setLogsSaved(true);
        setTimeout(() => setLogsSaved(false), 2000);
      } else {
        setLogsError(result.error || "Failed to save log settings");
        setLogSettings(logSettings);
      }
    } catch (err) {
      setLogsError(err instanceof Error ? err.message : "Unknown error");
      setLogSettings(logSettings);
    } finally {
      setIsSavingLogs(false);
    }
  };

  return (
    <div className="page">
      <div className="page-container page-container--narrow">
        <div className="page-header">
          <div className="page-header__content">
            <h1 className="page-title">Settings</h1>
            <p className="page-subtitle">Configure your z-Bot environment</p>
          </div>
        </div>

        <div className="flex flex-col gap-4">

          {/* ── Gateway Connection ── */}
          <div className="card card__padding--lg">
            <div className="card__header">
              <div className="flex items-center gap-3">
                <div className="card__icon card__icon--primary">
                  <Server style={{ width: 18, height: 18 }} />
                </div>
                <div>
                  <h2 className="settings-section-header">Gateway Connection</h2>
                  <p className="page-subtitle">HTTP and WebSocket endpoints</p>
                </div>
              </div>
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div className="settings-info-card">
                <span className="settings-info-card__label">HTTP API</span>
                <code className="settings-info-card__value">http://localhost:18791</code>
              </div>
              <div className="settings-info-card">
                <span className="settings-info-card__label">WebSocket</span>
                <code className="settings-info-card__value">ws://localhost:18790</code>
              </div>
            </div>
          </div>

          {/* ── Data Location ── */}
          <div className="card card__padding--lg">
            <div className="card__header">
              <div className="flex items-center gap-3">
                <div className="card__icon card__icon--success">
                  <HardDrive style={{ width: 18, height: 18 }} />
                </div>
                <div>
                  <h2 className="settings-section-header">Data Location</h2>
                  <p className="page-subtitle">Where your configurations are stored</p>
                </div>
              </div>
            </div>
            <div className="settings-info-card">
              <code className="settings-info-card__value">~/Documents/zbot/</code>
            </div>
          </div>

          {/* ── Context Memory Protection ── */}
          <div className="card card__padding--lg">
            <button onClick={() => setMemoryOpen(!memoryOpen)} className="settings-toggle-btn">
              <div className="flex items-center gap-3">
                <div className="card__icon card__icon--warning">
                  <Shield style={{ width: 18, height: 18 }} />
                </div>
                <div>
                  <h2 className="settings-section-header">Context Memory</h2>
                  <p className="page-subtitle">Protect agents from context overflow</p>
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
                      When tool results are very large (file reads, shell output), they can fill up the agent's context window.
                      Offloading saves large results to disk and gives the agent a file path to read instead.
                    </p>

                    {/* Offload toggle */}
                    <label className={`settings-toggle-option ${toolSettings.offloadLargeResults ? "settings-toggle-option--active" : ""}`}
                      style={{ opacity: isSavingTools ? 0.7 : 1 }}>
                      <input
                        type="checkbox"
                        checked={toolSettings.offloadLargeResults}
                        onChange={() => handleOffloadChange({ offloadLargeResults: !toolSettings.offloadLargeResults })}
                        disabled={isSavingTools}
                        className="settings-toggle-option__checkbox"
                      />
                      <div className="flex-1">
                        <div className="settings-toggle-option__title">Offload Large Results</div>
                        <div className="settings-toggle-option__description">
                          Save results over the threshold to disk instead of filling the context window
                        </div>
                      </div>
                    </label>

                    {/* Threshold */}
                    {toolSettings.offloadLargeResults && (
                      <div>
                        <label className="settings-field-label">Threshold (tokens)</label>
                        <input
                          type="number"
                          value={toolSettings.offloadThresholdTokens}
                          onChange={(e) => {
                            const val = parseInt(e.target.value, 10) || 0;
                            handleOffloadChange({ offloadThresholdTokens: val });
                          }}
                          disabled={isSavingTools}
                          className="form-input"
                          min={1000}
                          step={1000}
                        />
                        <p className="settings-hint">
                          Results larger than this are saved to disk. Default: 5000 tokens (~20,000 characters).
                          Lower = more aggressive offloading.
                        </p>
                      </div>
                    )}

                    {toolsSaved && (
                      <div className="settings-alert settings-alert--success">Saved</div>
                    )}
                  </div>
                ) : null}
              </div>
            )}
          </div>

          {/* ── Log Settings ── */}
          <div className="card card__padding--lg">
            <button onClick={() => setLogsOpen(!logsOpen)} className="settings-toggle-btn">
              <div className="flex items-center gap-3">
                <div className="card__icon card__icon--primary">
                  <FileText style={{ width: 18, height: 18 }} />
                </div>
                <div>
                  <h2 className="settings-section-header">Log Settings</h2>
                  <p className="page-subtitle">File logging and rotation</p>
                </div>
              </div>
              {logsOpen ? <ChevronDown className="settings-chevron" /> : <ChevronRight className="settings-chevron" />}
            </button>

            {logsOpen && (
              <div className="settings-expandable">
                {logsError && <div className="settings-alert settings-alert--error">{logsError}</div>}

                <div className="settings-alert settings-alert--warning">
                  <AlertTriangle className="settings-alert__icon" />
                  Changes require daemon restart to take effect
                </div>

                {isLoadingLogs ? (
                  <div className="settings-loading"><Loader2 className="loading-spinner__icon" /></div>
                ) : logSettings ? (
                  <div className="flex flex-col gap-4">
                    {/* Enable file logging */}
                    <label className={`settings-toggle-option ${logSettings.enabled ? "settings-toggle-option--active" : ""}`}
                      style={{ opacity: isSavingLogs ? 0.7 : 1 }}>
                      <input type="checkbox" checked={logSettings.enabled}
                        onChange={() => handleLogSettingChange({ enabled: !logSettings.enabled })}
                        disabled={isSavingLogs} className="settings-toggle-option__checkbox" />
                      <div className="flex-1">
                        <div className="settings-toggle-option__title">Enable File Logging</div>
                        <div className="settings-toggle-option__description">Write logs to files in addition to stdout</div>
                      </div>
                    </label>

                    {/* Log level */}
                    <div>
                      <label className="settings-field-label">Log Level</label>
                      <select value={logSettings.level}
                        onChange={(e) => handleLogSettingChange({ level: e.target.value as LogSettings["level"] })}
                        disabled={isSavingLogs} className="form-select form-input">
                        <option value="trace">Trace (most verbose)</option>
                        <option value="debug">Debug</option>
                        <option value="info">Info (default)</option>
                        <option value="warn">Warn</option>
                        <option value="error">Error (least verbose)</option>
                      </select>
                    </div>

                    {/* Rotation */}
                    <div>
                      <label className="settings-field-label">Rotation</label>
                      <select value={logSettings.rotation}
                        onChange={(e) => handleLogSettingChange({ rotation: e.target.value as LogSettings["rotation"] })}
                        disabled={isSavingLogs} className="form-select form-input">
                        <option value="daily">Daily (default)</option>
                        <option value="hourly">Hourly</option>
                        <option value="minutely">Minutely (testing)</option>
                        <option value="never">Never</option>
                      </select>
                    </div>

                    {/* Max files */}
                    <div>
                      <label className="settings-field-label">Max Files to Keep</label>
                      <input type="number" value={logSettings.maxFiles}
                        onChange={(e) => handleLogSettingChange({ maxFiles: parseInt(e.target.value, 10) || 0 })}
                        disabled={isSavingLogs} className="form-input" min={0} />
                      <p className="settings-hint">Set to 0 for unlimited retention</p>
                    </div>

                    {/* Suppress stdout */}
                    <label className="settings-toggle-option" style={{ opacity: isSavingLogs ? 0.7 : 1 }}>
                      <input type="checkbox" checked={logSettings.suppressStdout}
                        onChange={() => handleLogSettingChange({ suppressStdout: !logSettings.suppressStdout })}
                        disabled={isSavingLogs} className="settings-toggle-option__checkbox" />
                      <div className="flex-1">
                        <div className="settings-toggle-option__title">Suppress Stdout</div>
                        <div className="settings-toggle-option__description">Only log to file (useful for daemon mode)</div>
                      </div>
                    </label>

                    {/* Log directory */}
                    {logSettings.enabled && (
                      <div className="settings-info-card">
                        <span className="settings-info-card__label">Log directory</span>
                        <code className="settings-info-card__value">{logSettings.directory || "~/Documents/zbot/logs/"}</code>
                      </div>
                    )}

                    {logsSaved && <div className="settings-alert settings-alert--success">Saved</div>}
                  </div>
                ) : null}
              </div>
            )}
          </div>

          {/* ── Advanced: Tool Toggles ── */}
          <div className="card card__padding--lg">
            <button onClick={() => setAdvancedOpen(!advancedOpen)} className="settings-toggle-btn">
              <div className="flex items-center gap-3">
                <div className="card__icon card__icon--primary">
                  <Wrench style={{ width: 18, height: 18 }} />
                </div>
                <div>
                  <h2 className="settings-section-header">Optional Tools</h2>
                  <p className="page-subtitle">Enable additional agent capabilities</p>
                </div>
              </div>
              {advancedOpen ? <ChevronDown className="settings-chevron" /> : <ChevronRight className="settings-chevron" />}
            </button>

            {advancedOpen && (
              <div className="settings-expandable">
                {toolsError && <div className="settings-alert settings-alert--error">{toolsError}</div>}

                {isLoadingTools ? (
                  <div className="settings-loading"><Loader2 className="loading-spinner__icon" /></div>
                ) : toolSettings ? (
                  <div className="flex flex-col gap-4">
                    <p className="settings-toggle-option__description">
                      Core tools are always available: shell, apply_patch, memory, ward, update_plan, grep, load_skill.
                      These optional tools can be enabled when needed.
                    </p>

                    {TOOL_GROUPS.map((group) => (
                      <div key={group.name}>
                        <div className="settings-group-header">
                          <group.icon className="settings-group-header__icon" />
                          <span className="settings-group-header__label">{group.name}</span>
                        </div>
                        <div className="flex flex-col gap-2">
                          {group.tools.map((tool) => (
                            <label key={tool.key} className="settings-toggle-option" style={{ opacity: isSavingTools ? 0.7 : 1 }}>
                              <input
                                type="checkbox"
                                checked={!!toolSettings[tool.key]}
                                onChange={() => handleToolToggle(tool.key)}
                                disabled={isSavingTools}
                                className="settings-toggle-option__checkbox"
                              />
                              <div className="flex-1">
                                <div className="settings-toggle-option__title">{tool.label}</div>
                                <div className="settings-toggle-option__description">{tool.description}</div>
                              </div>
                            </label>
                          ))}
                        </div>
                      </div>
                    ))}

                    {toolsSaved && <div className="settings-alert settings-alert--success">Saved</div>}
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
