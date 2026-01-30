// ============================================================================
// WEB APP ENTRY POINT
// Main application for standalone web dashboard (no Tauri, no vaults)
// ============================================================================

import { useEffect, useState } from "react";
import { BrowserRouter, Routes, Route, Link, useLocation } from "react-router-dom";
import { Toaster } from "sonner";
import { initializeTransport, getTransport } from "@/services/transport";
import { WebChatPanel } from "./features/agent/WebChatPanel";
import { WebAgentsPanel } from "./features/agent/WebAgentsPanel";
import { WebSkillsPanel } from "./features/skills/WebSkillsPanel";
import { WebCronPanel } from "./features/cron/WebCronPanel";
import { WebIntegrationsPanel } from "./features/integrations/WebIntegrationsPanel";

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
      // Cleanup WebSocket on unmount
      getTransport().then(t => t.disconnect());
    };
  }, []);

  const initializeApp = async () => {
    try {
      // Initialize transport layer
      await initializeTransport();

      // Get transport and check health
      const transport = await getTransport();
      const healthResult = await transport.health();

      if (!healthResult.success) {
        setError(`Cannot connect to gateway: ${healthResult.error}`);
        setConnectionStatus({ connected: false, error: healthResult.error });
        return;
      }

      // Connect to event stream
      const connectResult = await transport.connect();
      if (connectResult.success) {
        setConnectionStatus({ connected: true });
      } else {
        // Still mark as "connected" for HTTP-only usage
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

  // Show loading while initializing
  if (isInitializing) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-[#1a1a1a]">
        <div className="text-center">
          <div className="inline-block animate-spin rounded-full h-8 w-8 border-b-2 border-violet-500 mb-4"></div>
          <p className="text-gray-400">Connecting to gateway...</p>
        </div>
      </div>
    );
  }

  // Show error if connection failed
  if (error && !connectionStatus.connected) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-[#1a1a1a]">
        <div className="text-center max-w-md p-6">
          <div className="text-red-500 text-6xl mb-4">!</div>
          <h1 className="text-xl font-semibold text-white mb-2">Connection Failed</h1>
          <p className="text-gray-400 mb-4">{error}</p>
          <p className="text-gray-500 text-sm mb-6">
            Make sure the AgentZero daemon is running:
            <br />
            <code className="bg-gray-800 px-2 py-1 rounded mt-2 inline-block">zerod</code>
          </p>
          <button
            onClick={() => {
              setError(null);
              setIsInitializing(true);
              initializeApp();
            }}
            className="bg-violet-600 hover:bg-violet-700 text-white px-4 py-2 rounded-lg transition-colors"
          >
            Retry Connection
          </button>
        </div>
      </div>
    );
  }

  // Show main app when connected
  return (
    <BrowserRouter>
      <Toaster
        position="bottom-right"
        theme="dark"
        richColors
        toastOptions={{
          style: {
            fontWeight: 500,
            fontSize: "14px",
            boxShadow: "0 8px 30px rgba(0, 0, 0, 0.5)",
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

function WebAppShell({ children, connectionStatus }: WebAppShellProps) {
  return (
    <div className="flex h-screen bg-[#1a1a1a] text-gray-100">
      {/* Sidebar */}
      <nav className="w-56 bg-[#141414] border-r border-gray-800 flex flex-col">
        <div className="p-4 border-b border-gray-800">
          <img src="/logo-dark.svg" alt="AgentZero" className="h-8" />
        </div>

        <div className="flex-1 py-4">
          <NavLink to="/" label="Chat" />
          <NavLink to="/agents" label="Agents" />
          <NavLink to="/skills" label="Skills" />
          <NavLink to="/cron" label="Schedules" />
          <NavLink to="/integrations" label="Integrations" />
          <NavLink to="/settings" label="Settings" />
        </div>

        {/* Connection status */}
        <div className="p-4 border-t border-gray-800">
          <div className="flex items-center gap-2">
            <div
              className={`w-2 h-2 rounded-full ${
                connectionStatus.connected ? "bg-green-500" : "bg-red-500"
              }`}
            />
            <span className="text-xs text-gray-400">
              {connectionStatus.connected ? "Connected" : "Disconnected"}
            </span>
          </div>
        </div>
      </nav>

      {/* Main content */}
      <main className="flex-1 overflow-hidden">{children}</main>
    </div>
  );
}

function NavLink({ to, label }: { to: string; label: string }) {
  const location = useLocation();
  const isActive = location.pathname === to;

  return (
    <Link
      to={to}
      className={`flex items-center gap-3 px-4 py-2 mx-2 rounded-lg transition-colors ${
        isActive
          ? "bg-violet-500/20 text-violet-400"
          : "text-gray-400 hover:text-white hover:bg-gray-800"
      }`}
    >
      <span>{label}</span>
    </Link>
  );
}

// ============================================================================
// Web Settings Panel (minimal)
// ============================================================================

function WebSettingsPanel() {
  return (
    <div className="p-6">
      <h1 className="text-2xl font-bold mb-6">Settings</h1>

      <div className="space-y-6 max-w-2xl">
        <div className="bg-[#141414] rounded-lg p-4 border border-gray-800">
          <h2 className="text-lg font-semibold mb-2">Gateway Connection</h2>
          <p className="text-gray-400 text-sm mb-4">
            The web dashboard connects to the AgentZero daemon via HTTP and WebSocket.
          </p>
          <div className="grid grid-cols-2 gap-4 text-sm">
            <div>
              <span className="text-gray-500">HTTP API:</span>
              <span className="ml-2 text-gray-300">http://localhost:18791</span>
            </div>
            <div>
              <span className="text-gray-500">WebSocket:</span>
              <span className="ml-2 text-gray-300">ws://localhost:18790</span>
            </div>
          </div>
        </div>

        <div className="bg-[#141414] rounded-lg p-4 border border-gray-800">
          <h2 className="text-lg font-semibold mb-2">Data Location</h2>
          <p className="text-gray-400 text-sm">
            Agent configurations and data are stored in:
          </p>
          <code className="block mt-2 bg-gray-900 px-3 py-2 rounded text-sm text-gray-300">
            ~/Documents/agentzero/
          </code>
        </div>
      </div>
    </div>
  );
}

export default App;
