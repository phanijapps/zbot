// ============================================================================
// SIDEBAR
// Discord-style navigation sidebar with violet accents
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
    { path: "/mcp", icon: Network, label: "MCP" },
    { path: "/skills", icon: Sparkles, label: "Skills" },
    { path: "/settings", icon: Settings, label: "Settings" },
  ];

  return (
    <aside className={cn("w-[72px] bg-[#1a1a1d] flex flex-col items-center py-3 gap-2 border-r border-black/20", className)}>
      {/* Logo */}
      <div className="mb-2 cursor-pointer group">
        <div className="relative bg-gradient-to-br from-violet-600 to-purple-700 p-3 rounded-2xl transition-all group-hover:rounded-xl">
          <Sparkles className="size-7 text-white" strokeWidth={2.5} fill="white" />
        </div>
      </div>

      {/* Separator */}
      <div className="w-8 h-[2px] bg-white/10 rounded-full my-1" />

      {/* Menu Items */}
      <nav className="flex flex-col gap-2 flex-1">
        {menuItems.map((item) => (
          <NavLink
            key={item.path}
            to={item.path}
            className={({ isActive }) =>
              cn(
                "w-12 h-12 rounded-2xl flex items-center justify-center transition-all relative group",
                isActive
                  ? "bg-violet-600 text-white rounded-xl"
                  : "bg-[#2b2d31] text-gray-400 hover:text-white hover:bg-violet-600 hover:rounded-xl"
              )
            }
          >
            <item.icon className="size-5" />
            {/* Tooltip with arrow */}
            <div className="absolute left-full ml-4 px-3 py-2 bg-black text-white text-sm rounded-lg opacity-0 group-hover:opacity-100 pointer-events-none whitespace-nowrap z-50 shadow-xl">
              {item.label}
              <div className="absolute right-full top-1/2 -translate-y-1/2 border-4 border-transparent border-r-black" />
            </div>
          </NavLink>
        ))}
      </nav>

      {/* Settings at bottom - already included in menuItems above */}
    </aside>
  );
}
