// ============================================================================
// WEB APP ENTRY POINT
// Main application for standalone web dashboard (no Tauri, no vaults)
// ============================================================================

import { useEffect, useState } from "react";
import { BrowserRouter, Routes, Route, Navigate, Link, useLocation } from "react-router-dom";
import { Toaster } from "sonner";
import {
  Bot,
  Settings,
  Loader2,
  AlertCircle,
  RefreshCw,
  Eye,
  LayoutDashboard,
  Plug,
  Brain,
  Network,
  MessageSquare,
  Search,
} from "lucide-react";
import { initializeTransport, getTransport } from "@/services/transport";
import { SetupWizard, SetupGuard } from "./features/setup";
import { WebChatPanel } from "./features/agent/WebChatPanel";
import { WebAgentsPanel } from "./features/agent/WebAgentsPanel";
import { WebSettingsPanel } from "./features/settings/WebSettingsPanel";
import { WebIntegrationsPanel } from "./features/integrations/WebIntegrationsPanel";
import { WebLogsPanel } from "./features/logs/WebLogsPanel";
import { WebOpsDashboard } from "./features/ops/WebOpsDashboard";
import { MemoryPage } from "./features/memory";
import { ObservatoryPage } from "./features/observatory";
import { FastChat } from "./features/chat/FastChat";
import { QuickChat } from "./features/chat-v2";
// ChatSlider removed — chat is now the home route, no longer in a slide-over
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

        await transport.connect();

        if (cancelled) {
          // If cancelled during connect, disconnect immediately
          transport.disconnect();
          return;
        }

        setConnectionStatus({ connected: true });
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
      <Routes>
          {/* Setup wizard — renders without app shell */}
          <Route path="/setup" element={<SetupWizard />} />

          {/* Main app with sidebar */}
          <Route path="/*" element={
            <SetupGuard>
              <WebAppShell connectionStatus={connectionStatus}>
                <Routes>
                  <Route path="/" element={<WebChatPanel />} />
                  <Route path="/dashboard" element={<WebOpsDashboard />} />
                  <Route path="/logs" element={<WebLogsPanel />} />
                  <Route path="/memory" element={<MemoryPage />} />
                  <Route path="/observatory" element={<ObservatoryPage />} />
                  <Route path="/agents" element={<WebAgentsPanel />} />
                  <Route path="/integrations" element={<WebIntegrationsPanel />} />
                  <Route path="/settings" element={<WebSettingsPanel />} />
                  <Route path="/chat" element={<FastChat />} />
                  <Route path="/chat-v2" element={<QuickChat />} />
                  <Route path="/providers" element={<Navigate to="/settings" replace />} />
                  <Route path="/skills" element={<Navigate to="/agents?tab=skills" replace />} />
                  <Route path="/hooks" element={<Navigate to="/agents?tab=schedules" replace />} />
                  <Route path="/connectors" element={<Navigate to="/integrations?tab=plugins" replace />} />
                  <Route path="/mcps" element={<Navigate to="/integrations" replace />} />
                </Routes>
              </WebAppShell>
            </SetupGuard>
          } />
        </Routes>
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
  matchPrefix?: boolean;
  badge?: string;
}

interface NavGroup {
  label?: string;
  items: NavItem[];
}

const navGroups: NavGroup[] = [
  {
    // Main group - no label
    items: [
      { to: "/chat", label: "Chat", icon: MessageSquare },
      { to: "/chat-v2", label: "Quick Chat", icon: MessageSquare, matchPrefix: true, badge: "v2" },
      { to: "/", label: "Research", icon: Search },
      { to: "/dashboard", label: "Dashboard", icon: LayoutDashboard },
      { to: "/logs", label: "Logs", icon: Eye },
      { to: "/memory", label: "Memory", icon: Brain },
      { to: "/observatory", label: "Observatory", icon: Network },
    ],
  },
  {
    label: "Manage",
    items: [
      { to: "/agents", label: "Agents", icon: Bot },
      { to: "/integrations", label: "Integrations", icon: Plug },
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
                <NavLink key={item.to} to={item.to} label={item.label} icon={item.icon} matchPrefix={item.matchPrefix} badge={item.badge} />
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
    </div>
  );
}

interface NavLinkProps {
  to: string;
  label: string;
  icon: React.ComponentType<{ className?: string; style?: React.CSSProperties }>;
  matchPrefix?: boolean;
  badge?: string;
}

function NavLink({ to, label, icon: Icon, matchPrefix, badge }: NavLinkProps) {
  const location = useLocation();
  const isActive = matchPrefix
    ? location.pathname.startsWith(to)
    : location.pathname === to;

  return (
    <Link
      to={to}
      className={`nav-link ${isActive ? 'nav-link--active' : ''}`}
    >
      <Icon className="nav-link__icon" />
      <span className="nav-link__label">
        {label}
        {badge != null && (
          <span className="nav-link__badge">{badge}</span>
        )}
      </span>
    </Link>
  );
}

export default App;
