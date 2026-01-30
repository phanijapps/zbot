// ============================================================================
// WEB APP ENTRY POINT
// Main application for standalone web dashboard (no Tauri, no vaults)
// ============================================================================

import { useEffect, useState } from "react";
import { BrowserRouter, Routes, Route, Link, useLocation } from "react-router-dom";
import { Toaster } from "sonner";
import {
  MessageSquare,
  Bot,
  Zap,
  Calendar,
  Plug,
  Server,
  Settings,
  Loader2,
  AlertCircle,
  RefreshCw,
} from "lucide-react";
import { initializeTransport, getTransport } from "@/services/transport";
import { WebChatPanel } from "./features/agent/WebChatPanel";
import { WebAgentsPanel } from "./features/agent/WebAgentsPanel";
import { WebSkillsPanel } from "./features/skills/WebSkillsPanel";
import { WebCronPanel } from "./features/cron/WebCronPanel";
import { WebIntegrationsPanel } from "./features/integrations/WebIntegrationsPanel";
import { WebMcpsPanel } from "./features/mcps/WebMcpsPanel";

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

  useEffect(() => {
    let cancelled = false;

    const init = async () => {
      if (cancelled) return;
      await initializeApp();
    };

    init();

    return () => {
      cancelled = true;
      getTransport().then(t => t.disconnect());
    };
  }, []);

  const initializeApp = async () => {
    try {
      await initializeTransport();
      const transport = await getTransport();
      const healthResult = await transport.health();

      if (!healthResult.success) {
        setError(`Cannot connect to gateway: ${healthResult.error}`);
        setConnectionStatus({ connected: false, error: healthResult.error });
        return;
      }

      const connectResult = await transport.connect();
      if (connectResult.success) {
        setConnectionStatus({ connected: true });
      } else {
        setConnectionStatus({ connected: true });
      }
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      setError(errorMessage);
      setConnectionStatus({ connected: false, error: errorMessage });
    } finally {
      setIsInitializing(false);
    }
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
            onClick={() => {
              setError(null);
              setIsInitializing(true);
              initializeApp();
            }}
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
          <Route path="/" element={<WebChatPanel />} />
          <Route path="/agents" element={<WebAgentsPanel />} />
          <Route path="/skills" element={<WebSkillsPanel />} />
          <Route path="/cron" element={<WebCronPanel />} />
          <Route path="/integrations" element={<WebIntegrationsPanel />} />
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

const navItems = [
  { to: "/", label: "Chat", icon: MessageSquare },
  { to: "/agents", label: "Agents", icon: Bot },
  { to: "/skills", label: "Skills", icon: Zap },
  { to: "/cron", label: "Schedules", icon: Calendar },
  { to: "/integrations", label: "Integrations", icon: Plug },
  { to: "/mcps", label: "MCP Servers", icon: Server },
  { to: "/settings", label: "Settings", icon: Settings },
];

function WebAppShell({ children, connectionStatus }: WebAppShellProps) {
  return (
    <div className="app-shell">
      <nav className="sidebar">
        <div className="sidebar__header">
          <div className="sidebar__logo">
            <div className="sidebar__logo-icon">
              <Bot style={{ width: 18, height: 18 }} />
            </div>
            <span className="sidebar__logo-text">AgentZero</span>
          </div>
        </div>

        <div className="sidebar__nav">
          {navItems.map((item) => (
            <NavLink key={item.to} to={item.to} label={item.label} icon={item.icon} />
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
// Web Settings Panel (minimal)
// ============================================================================

function WebSettingsPanel() {
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
        </div>
      </div>
    </div>
  );
}

export default App;
