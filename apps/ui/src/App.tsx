// ============================================================================
// WEB APP ENTRY POINT
// Main application for standalone web dashboard (no Tauri, no vaults)
// ============================================================================

import { useEffect, useState } from "react";
import { BrowserRouter, Routes, Route, Link, useLocation, useNavigate } from "react-router-dom";
import { Toaster } from "sonner";
import {
  Bot,
  Zap,
  Cable,
  Server,
  Settings,
  Loader2,
  AlertCircle,
  RefreshCw,
  Wrench,
  ChevronDown,
  ChevronRight,
  Search,
  Terminal,
  Layers,
  Users,
  Eye,
  LayoutDashboard,
  Plug,
  Calendar,
  FileText,
  AlertTriangle,
  Brain,
} from "lucide-react";
import { initializeTransport, getTransport, type ToolSettings, type LogSettings, type UpdateLogSettingsRequest } from "@/services/transport";
import { WebChatPanel } from "./features/agent/WebChatPanel";
import { WebAgentsPanel } from "./features/agent/WebAgentsPanel";
import { WebSkillsPanel } from "./features/skills/WebSkillsPanel";
import { WebCronPanel } from "./features/cron/WebCronPanel";
import { WebConnectorsPanel } from "./features/connectors/WebConnectorsPanel";
import { WebIntegrationsPanel } from "./features/integrations/WebIntegrationsPanel";
import { WebMcpsPanel } from "./features/mcps/WebMcpsPanel";
import { WebLogsPanel } from "./features/logs/WebLogsPanel";
import { WebOpsDashboard } from "./features/ops/WebOpsDashboard";
import { WebMemoryPanel } from "./features/memory";
import { ChatSlider } from "./components/ChatSlider";
import { ThemeToggle } from "./components/ThemeToggle";

// ============================================================================
// Types
// ============================================================================

interface ConnectionStatus {
  connected: boolean;
  error?: string;
}

// ============================================================================
// App Component
// ============================================================================

function App() {
  const [isInitializing, setIsInitializing] = useState(true);
  const [connectionStatus, setConnectionStatus] = useState<ConnectionStatus>({
    connected: false,
  });
  const [error, setError] = useState<string | null>(null);
  const [retryCount, setRetryCount] = useState(0);

  useEffect(() => {
    let cancelled = false;

    const initializeApp = async () => {
      try {
        await initializeTransport();

        // Check if cancelled before proceeding
        if (cancelled) return;

        const transport = await getTransport();
        const healthResult = await transport.health();

        if (cancelled) return;

        if (!healthResult.success) {
          setError(`Cannot connect to gateway: ${healthResult.error}`);
          setConnectionStatus({ connected: false, error: healthResult.error });
          return;
        }

        // Check again before connecting WebSocket
        if (cancelled) return;

        const connectResult = await transport.connect();

        if (cancelled) {
          // If cancelled during connect, disconnect immediately
          transport.disconnect();
          return;
        }

        if (connectResult.success) {
          setConnectionStatus({ connected: true });
        } else {
          setConnectionStatus({ connected: true });
        }
      } catch (err) {
        if (cancelled) return;
        const errorMessage = err instanceof Error ? err.message : String(err);
        setError(errorMessage);
        setConnectionStatus({ connected: false, error: errorMessage });
      } finally {
        if (!cancelled) {
          setIsInitializing(false);
        }
      }
    };

    initializeApp();

    return () => {
      cancelled = true;
      getTransport().then(t => t.disconnect());
    };
  }, [retryCount]);

  const handleRetry = () => {
    setError(null);
    setIsInitializing(true);
    setRetryCount(c => c + 1);
  };

  if (isInitializing) {
    return (
      <div className="loading-spinner">
        <div className="text-center">
          <Loader2 className="loading-spinner__icon" />
          <p className="page-subtitle">Connecting to gateway...</p>
        </div>
      </div>
    );
  }

  if (error && !connectionStatus.connected) {
    return (
      <div className="loading-spinner">
        <div className="card card__padding--lg page-container--narrow text-center">
          <div className="card__icon card__icon--destructive empty-state__icon">
            <AlertCircle style={{ width: 24, height: 24 }} />
          </div>
          <h2 className="empty-state__title">Connection Failed</h2>
          <p className="empty-state__description">{error}</p>
          <p className="page-subtitle mb-section">
            Make sure the z-Bot daemon is running:
            <br />
            <code className="badge mt-inline">
              zerod
            </code>
          </p>
          <button
            onClick={handleRetry}
            className="btn btn--primary btn--md"
          >
            <RefreshCw style={{ width: 16, height: 16 }} />
            Retry Connection
          </button>
        </div>
      </div>
    );
  }

  return (
    <BrowserRouter>
      <Toaster
        position="bottom-right"
        theme="system"
        toastOptions={{
          style: {
            fontWeight: 500,
            fontSize: '14px',
            borderRadius: 'var(--radius-lg)',
            boxShadow: 'var(--shadow-dropdown)',
          },
        }}
      />
      <WebAppShell connectionStatus={connectionStatus}>
        <Routes>
          {/* Dashboard is home, Chat is handled by slider */}
          <Route path="/" element={<WebOpsDashboard />} />
          <Route path="/chat" element={<WebOpsDashboard />} />
          <Route path="/logs" element={<WebLogsPanel />} />
          <Route path="/memory" element={<WebMemoryPanel />} />
          <Route path="/agents" element={<WebAgentsPanel />} />
          <Route path="/skills" element={<WebSkillsPanel />} />
          <Route path="/hooks" element={<WebCronPanel />} />
          <Route path="/connectors" element={<WebConnectorsPanel />} />
          <Route path="/providers" element={<WebIntegrationsPanel />} />
          <Route path="/mcps" element={<WebMcpsPanel />} />
          <Route path="/settings" element={<WebSettingsPanel />} />
        </Routes>
      </WebAppShell>
    </BrowserRouter>
  );
}

// ============================================================================
// Web App Shell
// ============================================================================

interface WebAppShellProps {
  children: React.ReactNode;
  connectionStatus: ConnectionStatus;
}

// Navigation structure with groups
interface NavItem {
  to: string;
  label: string;
  icon: React.ComponentType<{ className?: string; style?: React.CSSProperties }>;
}

interface NavGroup {
  label?: string;
  items: NavItem[];
}

const navGroups: NavGroup[] = [
  {
    // Main group - no label
    items: [
      { to: "/", label: "Dashboard", icon: LayoutDashboard },
      { to: "/logs", label: "Logs", icon: Eye },
      { to: "/memory", label: "Memory", icon: Brain },
    ],
  },
  {
    label: "Configure",
    items: [
      { to: "/agents", label: "Agents", icon: Bot },
      { to: "/skills", label: "Skills", icon: Zap },
    ],
  },
  {
    label: "Connect",
    items: [
      { to: "/connectors", label: "Workers", icon: Plug },
      { to: "/hooks", label: "Schedules", icon: Calendar },
      { to: "/providers", label: "Providers", icon: Cable },
      { to: "/mcps", label: "MCPs", icon: Server },
    ],
  },
  {
    label: "System",
    items: [
      { to: "/settings", label: "Settings", icon: Settings },
    ],
  },
];

function WebAppShell({ children, connectionStatus }: WebAppShellProps) {
  const location = useLocation();
  const navigate = useNavigate();
  const isChatOpen = location.pathname === "/chat";

  const handleCloseChat = () => {
    // Navigate back to dashboard when closing chat
    navigate("/");
  };

  return (
    <div className="app-shell">
      <nav className="sidebar">
        <div className="sidebar__header">
          <div className="sidebar__logo">
            <img src="/logo-dark.svg" alt="z-Bot" className="sidebar__logo-img" />
          </div>
        </div>

        <div className="sidebar__nav">
          {navGroups.map((group, groupIndex) => (
            <div key={groupIndex} className="sidebar__group">
              {group.label && (
                <div className="sidebar__group-label">{group.label}</div>
              )}
              {group.items.map((item) => (
                <NavLink key={item.to} to={item.to} label={item.label} icon={item.icon} />
              ))}
            </div>
          ))}
        </div>

        <div className="sidebar__footer">
          <ThemeToggle />
          <div className="connection-status">
            <div className={`connection-status__dot ${
              connectionStatus.connected
                ? 'connection-status__dot--connected'
                : 'connection-status__dot--disconnected'
            }`} />
            <span className="connection-status__text">
              {connectionStatus.connected ? "Connected" : "Disconnected"}
            </span>
          </div>
        </div>
      </nav>

      <main className="flex-1 overflow-hidden">{children}</main>

      {/* Chat Slider - overlays content when open */}
      <ChatSlider isOpen={isChatOpen} onClose={handleCloseChat}>
        <WebChatPanel />
      </ChatSlider>
    </div>
  );
}

interface NavLinkProps {
  to: string;
  label: string;
  icon: React.ComponentType<{ className?: string; style?: React.CSSProperties }>;
}

function NavLink({ to, label, icon: Icon }: NavLinkProps) {
  const location = useLocation();
  const isActive = location.pathname === to;

  return (
    <Link
      to={to}
      className={`nav-link ${isActive ? 'nav-link--active' : ''}`}
    >
      <Icon className="nav-link__icon" />
      <span className="nav-link__label">{label}</span>
    </Link>
  );
}

// ============================================================================
// Web Settings Panel
// ============================================================================

function WebSettingsPanel() {
  const [toolSettings, setToolSettings] = useState<ToolSettings | null>(null);
  const [isLoadingTools, setIsLoadingTools] = useState(true);
  const [isSavingTools, setIsSavingTools] = useState(false);
  const [toolsError, setToolsError] = useState<string | null>(null);
  const [advancedOpen, setAdvancedOpen] = useState(false);

  // Log settings state
  const [logSettings, setLogSettings] = useState<LogSettings | null>(null);
  const [isLoadingLogs, setIsLoadingLogs] = useState(true);
  const [isSavingLogs, setIsSavingLogs] = useState(false);
  const [logsError, setLogsError] = useState<string | null>(null);
  const [logsOpen, setLogsOpen] = useState(false);

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

    const newSettings = { ...toolSettings, [key]: !toolSettings[key] };
    setToolSettings(newSettings);
    setIsSavingTools(true);

    try {
      const transport = await getTransport();
      const result = await transport.updateToolSettings(newSettings);
      if (!result.success) {
        setToolsError(result.error || "Failed to save settings");
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

    try {
      const transport = await getTransport();
      const result = await transport.updateLogSettings(updates);
      if (!result.success) {
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

  const toolGroups = [
    {
      name: "Search Tools",
      icon: Search,
      tools: [
        { key: "grep" as const, label: "Grep", description: "Search file contents with patterns" },
        { key: "glob" as const, label: "Glob", description: "Find files by name patterns" },
      ],
    },
    {
      name: "Execution",
      icon: Terminal,
      tools: [
        { key: "python" as const, label: "Python", description: "Execute Python code" },
        { key: "loadSkill" as const, label: "Load Skill", description: "Dynamically load skills" },
      ],
    },
    {
      name: "UI Tools",
      icon: Layers,
      tools: [
        { key: "uiTools" as const, label: "UI Tools", description: "Interactive UI components" },
      ],
    },
    {
      name: "Agent Tools",
      icon: Users,
      tools: [
        { key: "createAgent" as const, label: "Create Agent", description: "Create new agents dynamically" },
      ],
    },
    {
      name: "Introspection",
      icon: Eye,
      tools: [
        { key: "introspection" as const, label: "Introspection", description: "Self-analysis tools" },
      ],
    },
  ];

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

          <div className="card card__padding--lg">
            <div className="card__header">
              <div className="flex items-center gap-3">
                <div className="card__icon card__icon--success">
                  <Settings style={{ width: 18, height: 18 }} />
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

          {/* Log Settings */}
          <div className="card card__padding--lg">
            <button
              onClick={() => setLogsOpen(!logsOpen)}
              className="settings-toggle-btn"
            >
              <div className="flex items-center gap-3">
                <div className="card__icon card__icon--primary">
                  <FileText style={{ width: 18, height: 18 }} />
                </div>
                <div>
                  <h2 className="settings-section-header">Log Settings</h2>
                  <p className="page-subtitle">File logging and rotation</p>
                </div>
              </div>
              {logsOpen ? (
                <ChevronDown className="settings-chevron" />
              ) : (
                <ChevronRight className="settings-chevron" />
              )}
            </button>

            {logsOpen && (
              <div className="settings-expandable">
                {logsError && (
                  <div className="settings-alert settings-alert--error">
                    {logsError}
                  </div>
                )}

                {/* Restart warning */}
                <div className="settings-alert settings-alert--warning">
                  <AlertTriangle className="settings-alert__icon" />
                  Changes require daemon restart to take effect
                </div>

                {isLoadingLogs ? (
                  <div className="settings-loading">
                    <Loader2 className="loading-spinner__icon" />
                  </div>
                ) : logSettings ? (
                  <div className="flex flex-col gap-4">
                    {/* Enable file logging */}
                    <label
                      className={`settings-toggle-option ${logSettings.enabled ? 'settings-toggle-option--active' : ''}`}
                      style={{ opacity: isSavingLogs ? 0.7 : 1 }}
                    >
                      <input
                        type="checkbox"
                        checked={logSettings.enabled}
                        onChange={() => handleLogSettingChange({ enabled: !logSettings.enabled })}
                        disabled={isSavingLogs}
                        className="settings-toggle-option__checkbox"
                      />
                      <div className="flex-1">
                        <div className="settings-toggle-option__title">
                          Enable File Logging
                        </div>
                        <div className="settings-toggle-option__description">
                          Write logs to files in addition to stdout
                        </div>
                      </div>
                    </label>

                    {/* Log level */}
                    <div>
                      <label className="settings-field-label">
                        Log Level
                      </label>
                      <select
                        value={logSettings.level}
                        onChange={(e) => handleLogSettingChange({ level: e.target.value as LogSettings['level'] })}
                        disabled={isSavingLogs}
                        className="form-select form-input"
                      >
                        <option value="trace">Trace (most verbose)</option>
                        <option value="debug">Debug</option>
                        <option value="info">Info (default)</option>
                        <option value="warn">Warn</option>
                        <option value="error">Error (least verbose)</option>
                      </select>
                    </div>

                    {/* Rotation strategy */}
                    <div>
                      <label className="settings-field-label">
                        Rotation
                      </label>
                      <select
                        value={logSettings.rotation}
                        onChange={(e) => handleLogSettingChange({ rotation: e.target.value as LogSettings['rotation'] })}
                        disabled={isSavingLogs}
                        className="form-select form-input"
                      >
                        <option value="daily">Daily (default)</option>
                        <option value="hourly">Hourly</option>
                        <option value="minutely">Minutely (testing)</option>
                        <option value="never">Never</option>
                      </select>
                    </div>

                    {/* Max files */}
                    <div>
                      <label className="settings-field-label">
                        Max Files to Keep
                      </label>
                      <input
                        type="number"
                        value={logSettings.maxFiles}
                        onChange={(e) => handleLogSettingChange({ maxFiles: parseInt(e.target.value, 10) || 0 })}
                        disabled={isSavingLogs}
                        className="form-input"
                        min={0}
                      />
                      <p className="settings-hint">
                        Set to 0 for unlimited retention
                      </p>
                    </div>

                    {/* Suppress stdout */}
                    <label
                      className="settings-toggle-option"
                      style={{ opacity: isSavingLogs ? 0.7 : 1 }}
                    >
                      <input
                        type="checkbox"
                        checked={logSettings.suppressStdout}
                        onChange={() => handleLogSettingChange({ suppressStdout: !logSettings.suppressStdout })}
                        disabled={isSavingLogs}
                        className="settings-toggle-option__checkbox"
                      />
                      <div className="flex-1">
                        <div className="settings-toggle-option__title">
                          Suppress Stdout
                        </div>
                        <div className="settings-toggle-option__description">
                          Only log to file (useful for daemon mode)
                        </div>
                      </div>
                    </label>

                    {/* Log directory info */}
                    {logSettings.enabled && (
                      <div className="settings-info-card">
                        <span className="settings-info-card__label">Log directory:</span>
                        <code className="settings-info-card__value">
                          {logSettings.directory || '~/Documents/zbot/logs/'}
                        </code>
                      </div>
                    )}
                  </div>
                ) : null}
              </div>
            )}
          </div>

          {/* Advanced Options - Tool Settings */}
          <div className="card card__padding--lg">
            <button
              onClick={() => setAdvancedOpen(!advancedOpen)}
              className="settings-toggle-btn"
            >
              <div className="flex items-center gap-3">
                <div className="card__icon card__icon--warning">
                  <Wrench style={{ width: 18, height: 18 }} />
                </div>
                <div>
                  <h2 className="settings-section-header">Advanced Options</h2>
                  <p className="page-subtitle">Configure optional agent tools</p>
                </div>
              </div>
              {advancedOpen ? (
                <ChevronDown className="settings-chevron" />
              ) : (
                <ChevronRight className="settings-chevron" />
              )}
            </button>

            {advancedOpen && (
              <div className="settings-expandable">
                {toolsError && (
                  <div className="settings-alert settings-alert--error">
                    {toolsError}
                  </div>
                )}

                {isLoadingTools ? (
                  <div className="settings-loading">
                    <Loader2 className="loading-spinner__icon" />
                  </div>
                ) : toolSettings ? (
                  <div className="flex flex-col gap-4">
                    <p className="settings-toggle-option__description">
                      Enable or disable optional tools. Core tools (shell, read, write, edit, memory, web_fetch, todo) are always available.
                    </p>

                    {toolGroups.map((group) => (
                      <div key={group.name}>
                        <div className="settings-group-header">
                          <group.icon className="settings-group-header__icon" />
                          <span className="settings-group-header__label">
                            {group.name}
                          </span>
                        </div>
                        <div className="flex flex-col gap-2">
                          {group.tools.map((tool) => (
                            <label
                              key={tool.key}
                              className="settings-toggle-option"
                              style={{ opacity: isSavingTools ? 0.7 : 1 }}
                            >
                              <input
                                type="checkbox"
                                checked={toolSettings[tool.key]}
                                onChange={() => handleToolToggle(tool.key)}
                                disabled={isSavingTools}
                                className="settings-toggle-option__checkbox"
                              />
                              <div className="flex-1">
                                <div className="settings-toggle-option__title">
                                  {tool.label}
                                </div>
                                <div className="settings-toggle-option__description">
                                  {tool.description}
                                </div>
                              </div>
                            </label>
                          ))}
                        </div>
                      </div>
                    ))}
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

export default App;
