// ============================================================================
// SIDEBAR
// Modern icon-based navigation sidebar with dark theme
// ============================================================================

import { NavLink } from "react-router-dom";
import { MessageSquare, Bot, Zap, Network, Sparkles, Settings } from "lucide-react";
import { cn } from "../../shared/utils";

interface SidebarProps {
  className?: string;
}

export function Sidebar({ className }: SidebarProps) {
  const menuItems = [
    { path: "/", icon: MessageSquare, label: "Conversations" },
    { path: "/agents", icon: Bot, label: "Agents" },
    { path: "/providers", icon: Zap, label: "Providers" },
    { path: "/mcp", icon: Network, label: "MCP Servers" },
    { path: "/skills", icon: Sparkles, label: "Skills" },
    { path: "/settings", icon: Settings, label: "Settings" },
  ];

  return (
    <aside className={cn("w-16 bg-[#0f0f0f] border-r border-white/5 flex flex-col items-center py-4 gap-2", className)}>
      {/* Logo */}
      <div className="mb-4 relative group cursor-pointer">
        <div className="absolute inset-0 bg-gradient-to-br from-blue-500 to-purple-600 rounded-xl opacity-0 group-hover:opacity-100 blur-md transition-opacity" />
        <div className="relative bg-gradient-to-br from-blue-500 to-purple-600 p-2.5 rounded-xl">
          <Sparkles className="size-6 text-white" strokeWidth={2.5} />
        </div>
      </div>

      {/* Menu Items */}
      <nav className="flex flex-col gap-2">
        {menuItems.map((item) => (
          <NavLink
            key={item.path}
            to={item.path}
            className={({ isActive }) =>
              cn(
                "w-11 h-11 rounded-lg flex items-center justify-center transition-all relative group",
                isActive
                  ? "bg-blue-600 text-white"
                  : "text-gray-400 hover:text-white hover:bg-white/5"
              )
            }
          >
            {({ isActive }) => (
              <>
                {isActive && (
                  <div className="absolute inset-0 bg-blue-500/20 rounded-lg blur-md" />
                )}
                <item.icon className="size-5 relative z-10" />
                <div className="absolute left-full ml-2 px-2 py-1 bg-gray-900 text-white text-xs rounded opacity-0 group-hover:opacity-100 pointer-events-none whitespace-nowrap transition-opacity">
                  {item.label}
                </div>
              </>
            )}
          </NavLink>
        ))}
      </nav>
    </aside>
  );
}
