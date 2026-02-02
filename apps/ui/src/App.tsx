// ============================================================================
// WEB APP ENTRY POINT
// Main application for standalone web dashboard (no Tauri, no vaults)
// ============================================================================

import { useEffect, useState } from "react";
import { BrowserRouter, Routes, Route, Link, useLocation, useNavigate } from "react-router-dom";
import { Toaster } from "sonner";
import {
  MessageSquare,
  Bot,
  Zap,
  Webhook,
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
  Brain,
  Users,
  Eye,
  LayoutDashboard,
} from "lucide-react";
import { initializeTransport, getTransport, type ToolSettings } from "@/services/transport";
import { WebChatPanel } from "./features/agent/WebChatPanel";
import { WebAgentsPanel } from "./features/agent/WebAgentsPanel";
import { WebSkillsPanel } from "./features/skills/WebSkillsPanel";
import { WebCronPanel } from "./features/cron/WebCronPanel";
import { WebIntegrationsPanel } from "./features/integrations/WebIntegrationsPanel";
import { WebMcpsPanel } from "./features/mcps/WebMcpsPanel";
import { WebLogsPanel } from "./features/logs/WebLogsPanel";
import { WebOpsDashboard } from "./features/ops/WebOpsDashboard";
import { ChatSlider } from "./components/ChatSlider";

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
      <div className="loading-spinner" style={{ minHeight: '100vh' }}>
        <div className="text-center">
          <Loader2 className="loading-spinner__icon" />
          <p className="page-subtitle">Connecting to gateway...</p>
        </div>
      </div>
    );
  }

  if (error && !connectionStatus.connected) {
    return (
      <div className="loading-spinner" style={{ minHeight: '100vh' }}>
        <div className="card card__padding--lg" style={{ maxWidth: '28rem', textAlign: 'center' }}>
          <div className="empty-state__icon" style={{ backgroundColor: 'var(--destructive-muted)' }}>
            <AlertCircle style={{ width: 24, height: 24, color: 'var(--destructive)' }} />
          </div>
          <h2 className="empty-state__title">Connection Failed</h2>
          <p className="empty-state__description">{error}</p>
          <p className="page-subtitle" style={{ marginBottom: 'var(--spacing-6)' }}>
            Make sure the AgentZero daemon is running:
            <br />
            <code className="badge" style={{ marginTop: 'var(--spacing-2)', display: 'inline-block' }}>
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
        theme="light"
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
          <Route path="/agents" element={<WebAgentsPanel />} />
          <Route path="/skills" element={<WebSkillsPanel />} />
          <Route path="/hooks" element={<WebCronPanel />} />
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
      { to: "/hooks", label: "Hooks", icon: Webhook },
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
            <img src="/logo-dark.svg" alt="AgentZero" className="sidebar__logo-img" />
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

  useEffect(() => {
    loadToolSettings();
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
      name: "Knowledge",
      icon: Brain,
      tools: [
        { key: "knowledgeGraph" as const, label: "Knowledge Graph", description: "Graph-based memory" },
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
            <p className="page-subtitle">Configure your AgentZero environment</p>
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
                  <h2 style={{ fontSize: 'var(--text-base)', fontWeight: 600 }}>Gateway Connection</h2>
                  <p className="page-subtitle">HTTP and WebSocket endpoints</p>
                </div>
              </div>
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div className="badge" style={{ padding: 'var(--spacing-3)', flexDirection: 'column', alignItems: 'flex-start' }}>
                <span style={{ fontSize: 'var(--text-xs)', textTransform: 'uppercase', letterSpacing: '0.05em' }}>HTTP API</span>
                <code className="font-mono" style={{ marginTop: 'var(--spacing-1)' }}>http://localhost:18791</code>
              </div>
              <div className="badge" style={{ padding: 'var(--spacing-3)', flexDirection: 'column', alignItems: 'flex-start' }}>
                <span style={{ fontSize: 'var(--text-xs)', textTransform: 'uppercase', letterSpacing: '0.05em' }}>WebSocket</span>
                <code className="font-mono" style={{ marginTop: 'var(--spacing-1)' }}>ws://localhost:18790</code>
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
                  <h2 style={{ fontSize: 'var(--text-base)', fontWeight: 600 }}>Data Location</h2>
                  <p className="page-subtitle">Where your configurations are stored</p>
                </div>
              </div>
            </div>
            <div className="badge font-mono" style={{ padding: 'var(--spacing-3)' }}>
              ~/Documents/agentzero/
            </div>
          </div>

          {/* Advanced Options - Tool Settings */}
          <div className="card card__padding--lg">
            <button
              onClick={() => setAdvancedOpen(!advancedOpen)}
              className="w-full flex items-center justify-between"
              style={{ background: 'none', border: 'none', cursor: 'pointer', padding: 0 }}
            >
              <div className="flex items-center gap-3">
                <div className="card__icon" style={{ backgroundColor: 'var(--warning-muted)' }}>
                  <Wrench style={{ width: 18, height: 18, color: 'var(--warning)' }} />
                </div>
                <div style={{ textAlign: 'left' }}>
                  <h2 style={{ fontSize: 'var(--text-base)', fontWeight: 600, color: 'var(--foreground)' }}>Advanced Options</h2>
                  <p className="page-subtitle">Configure optional agent tools</p>
                </div>
              </div>
              {advancedOpen ? (
                <ChevronDown style={{ width: 20, height: 20, color: 'var(--muted-foreground)' }} />
              ) : (
                <ChevronRight style={{ width: 20, height: 20, color: 'var(--muted-foreground)' }} />
              )}
            </button>

            {advancedOpen && (
              <div style={{ marginTop: 'var(--spacing-4)', paddingTop: 'var(--spacing-4)', borderTop: '1px solid var(--border)' }}>
                {toolsError && (
                  <div className="badge" style={{
                    padding: 'var(--spacing-3)',
                    marginBottom: 'var(--spacing-4)',
                    backgroundColor: 'var(--destructive-muted)',
                    color: 'var(--destructive)'
                  }}>
                    {toolsError}
                  </div>
                )}

                {isLoadingTools ? (
                  <div className="flex items-center justify-center" style={{ padding: 'var(--spacing-6)' }}>
                    <Loader2 className="loading-spinner__icon" style={{ width: 24, height: 24 }} />
                  </div>
                ) : toolSettings ? (
                  <div className="flex flex-col gap-4">
                    <p style={{ fontSize: 'var(--text-sm)', color: 'var(--muted-foreground)' }}>
                      Enable or disable optional tools. Core tools (shell, read, write, edit, memory, web_fetch, todo) are always available.
                    </p>

                    {toolGroups.map((group) => (
                      <div key={group.name}>
                        <div className="flex items-center gap-2" style={{ marginBottom: 'var(--spacing-2)' }}>
                          <group.icon style={{ width: 14, height: 14, color: 'var(--muted-foreground)' }} />
                          <span style={{ fontSize: 'var(--text-xs)', fontWeight: 600, color: 'var(--muted-foreground)', textTransform: 'uppercase', letterSpacing: '0.05em' }}>
                            {group.name}
                          </span>
                        </div>
                        <div className="flex flex-col gap-2">
                          {group.tools.map((tool) => (
                            <label
                              key={tool.key}
                              className="flex items-center gap-3 cursor-pointer"
                              style={{
                                padding: 'var(--spacing-3)',
                                backgroundColor: 'var(--muted)',
                                borderRadius: 'var(--radius-md)',
                                opacity: isSavingTools ? 0.7 : 1,
                              }}
                            >
                              <input
                                type="checkbox"
                                checked={toolSettings[tool.key]}
                                onChange={() => handleToolToggle(tool.key)}
                                disabled={isSavingTools}
                                style={{
                                  width: 16,
                                  height: 16,
                                  accentColor: 'var(--primary)',
                                  cursor: isSavingTools ? 'not-allowed' : 'pointer',
                                }}
                              />
                              <div style={{ flex: 1 }}>
                                <div style={{ fontSize: 'var(--text-sm)', fontWeight: 500, color: 'var(--foreground)' }}>
                                  {tool.label}
                                </div>
                                <div style={{ fontSize: 'var(--text-xs)', color: 'var(--muted-foreground)' }}>
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
