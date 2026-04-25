// ============================================================================
// WEB APP ENTRY POINT
// Main application for standalone web dashboard (no Tauri, no vaults)
// ============================================================================

import { useEffect, useState } from "react";
import { BrowserRouter, Routes, Route, Navigate, Link, useLocation, useParams } from "react-router-dom";
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
  Menu,
  Search,
} from "lucide-react";
import { initializeTransport, getTransport } from "@/services/transport";
import { SetupWizard, SetupGuard } from "./features/setup";
import { WebAgentsPanel } from "./features/agent/WebAgentsPanel";
import { WebSettingsPanel } from "./features/settings/WebSettingsPanel";
import { WebIntegrationsPanel } from "./features/integrations/WebIntegrationsPanel";
import { WebLogsPanel } from "./features/logs/WebLogsPanel";
import { WebOpsDashboard } from "./features/ops/WebOpsDashboard";
import { MemoryPage } from "./features/memory";
import { ObservatoryPage } from "./features/observatory";
import { QuickChat } from "./features/chat-v2";
import { ResearchPage } from "./features/research-v2";
import { AccentPicker } from "./components/AccentPicker";

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

/** Legacy redirect: /research-v2/:sessionId → /research/:sessionId. */
function ResearchV2Redirect() {
  const { sessionId } = useParams<{ sessionId: string }>();
  return <Navigate to={`/research/${sessionId ?? ""}`} replace />;
}

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
                  <Route path="/" element={<Navigate to="/research" replace />} />
                  <Route path="/dashboard" element={<WebOpsDashboard />} />
                  <Route path="/logs" element={<WebLogsPanel />} />
                  <Route path="/memory" element={<MemoryPage />} />
                  <Route path="/observatory" element={<ObservatoryPage />} />
                  <Route path="/agents" element={<WebAgentsPanel />} />
                  <Route path="/integrations" element={<WebIntegrationsPanel />} />
                  <Route path="/settings" element={<WebSettingsPanel />} />
                  <Route path="/chat" element={<QuickChat />} />
                  {/* Legacy bookmark redirect. */}
                  <Route path="/chat-v2" element={<Navigate to="/chat" replace />} />
                  <Route path="/research" element={<ResearchPage />} />
                  <Route path="/research/:sessionId" element={<ResearchPage />} />
                  {/* Legacy bookmark redirects. */}
                  <Route path="/research-v2" element={<Navigate to="/research" replace />} />
                  <Route path="/research-v2/:sessionId" element={<ResearchV2Redirect />} />
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

// Flat top-bar nav — no groups, no Main/Manage/System split.
// Order matches the design system kit (Research first, Settings last).
export interface NavItem {
  to: string;
  label: string;
  icon: React.ComponentType<{ className?: string; style?: React.CSSProperties }>;
  matchPrefix?: boolean;
  badge?: string;
}

export const navItems: NavItem[] = [
  { to: "/research", label: "Research", icon: Search, matchPrefix: true },
  { to: "/chat", label: "Quick chat", icon: MessageSquare },
  { to: "/dashboard", label: "Dashboard", icon: LayoutDashboard },
  { to: "/agents", label: "Agents", icon: Bot },
  { to: "/memory", label: "Memory", icon: Brain },
  { to: "/logs", label: "Logs", icon: Eye },
  { to: "/observatory", label: "Observatory", icon: Network },
  { to: "/integrations", label: "Integrations", icon: Plug },
  { to: "/settings", label: "Settings", icon: Settings },
];

export function WebAppShell({ children, connectionStatus }: WebAppShellProps) {
  const [sheetOpen, setSheetOpen] = useState(false);
  const location = useLocation();
  // Close the mobile sheet whenever the route changes.
  useEffect(() => { setSheetOpen(false); }, [location.pathname]);

  return (
    <div className="app-shell">
      <span className="app-shell__reticle app-shell__reticle--tl" aria-hidden="true" />
      <span className="app-shell__reticle app-shell__reticle--tr" aria-hidden="true" />
      <span className="app-shell__reticle app-shell__reticle--bl" aria-hidden="true" />
      <span className="app-shell__reticle app-shell__reticle--br" aria-hidden="true" />

      <header className="topbar">
        <Link to="/research" className="topbar__brand" aria-label="z-Bot home">
          <span className="topbar__brand-mark">z</span>
          <span className="topbar__brand-name">z-<b>Bot</b></span>
        </Link>

        <button
          type="button"
          className="topbar__menu-toggle"
          aria-label={sheetOpen ? "Close menu" : "Open menu"}
          aria-expanded={sheetOpen}
          onClick={() => setSheetOpen((v) => !v)}
        >
          <Menu size={18} />
        </button>

        <nav className="topbar__nav" aria-label="Primary">
          {navItems.map((item) => (
            <NavLink key={item.to} to={item.to} label={item.label} icon={item.icon} matchPrefix={item.matchPrefix} badge={item.badge} />
          ))}
        </nav>

        <div className="topbar__right">
          <AccentPicker />
          <div className="connection-status">
            <div className={`connection-status__dot ${
              connectionStatus.connected
                ? 'connection-status__dot--connected'
                : 'connection-status__dot--disconnected'
            }`} />
            <span className="connection-status__text">
              {connectionStatus.connected ? "Connected · zerod" : "Disconnected"}
            </span>
          </div>
        </div>
      </header>

      {sheetOpen && (
        <button
          type="button"
          className="topbar__sheet-backdrop topbar__sheet-backdrop--open"
          aria-label="Close menu"
          onClick={() => setSheetOpen(false)}
        />
      )}
      <nav
        className={`topbar__sheet${sheetOpen ? " topbar__sheet--open" : ""}`}
        aria-label="Mobile primary"
      >
        {navItems.map((item) => (
          <NavLink key={item.to} to={item.to} label={item.label} icon={item.icon} matchPrefix={item.matchPrefix} badge={item.badge} />
        ))}
      </nav>

      <main className="app-shell__main">{children}</main>
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
