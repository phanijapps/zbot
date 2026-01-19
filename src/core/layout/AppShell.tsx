// ============================================================================
// APP SHELL
// Main application shell with sidebar and content area
// ============================================================================

import { Outlet } from "react-router-dom";
import { Sidebar } from "./Sidebar";
import { StatusBar } from "./StatusBar";

interface AppShellProps {
  children?: React.ReactNode;
}

export function AppShell({ children }: AppShellProps) {
  return (
    <div className="flex flex-col h-screen bg-[#313338] text-foreground">
      {/* Main Content Area */}
      <div className="flex flex-1 overflow-hidden">
        {/* Sidebar */}
        <Sidebar />

        {/* Main Content */}
        <main className="flex-1 overflow-auto">
          {children || <Outlet />}
        </main>
      </div>

      {/* Status Bar */}
      <StatusBar />
    </div>
  );
}
