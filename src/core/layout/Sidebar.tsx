// ============================================================================
// SIDEBAR
// Discord-style navigation sidebar with violet accents
// ============================================================================

import { NavLink } from "react-router-dom";
import { MessageSquare, Bot, Zap, Network, Sparkles, Settings, Search } from "lucide-react";
import { cn } from "../../shared/utils";

interface SidebarProps {
  className?: string;
}

// Zero Logo SVG Component (inline for theme switching)
function ZeroLogo({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 120 120"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      className={className}
      aria-label="Zero"
    >
      {/* Icon portion */}
      <circle cx="60" cy="60" r="48" fill="url(#radGrad1)" className="opacity-10" />
      <circle cx="60" cy="60" r="38" stroke="url(#grad1)" strokeWidth="8" fill="none" strokeLinecap="round" />
      <circle cx="60" cy="60" r="28" fill="url(#radGrad2)" className="opacity-15" />
      <circle cx="60" cy="60" r="22" fill="url(#radGrad3)" className="opacity-12" />
      <circle cx="60" cy="60" r="16" fill="url(#radGrad4)" className="opacity-10" />
      <circle cx="60" cy="60" r="5" fill="#5b21b6" />

      {/* Agent indicators */}
      <circle cx="60" cy="22" r="4" fill="#7c3aed" className="opacity-90" />
      <circle cx="98" cy="60" r="4" fill="#059669" className="opacity-90" />
      <circle cx="60" cy="98" r="4" fill="#db2777" className="opacity-90" />
      <circle cx="22" cy="60" r="4" fill="#d97706" className="opacity-90" />

      {/* Gradients */}
      <defs>
        <linearGradient id="grad1" x1="0%" y1="0%" x2="100%" y2="100%">
          <stop offset="0%" stopColor="#7c3aed" stopOpacity="1" className="dark:stop-color-violet-400" />
          <stop offset="50%" stopColor="#4f46e5" stopOpacity="1" className="dark:stop-color-indigo-400" />
          <stop offset="100%" stopColor="#6d28d9" stopOpacity="1" className="dark:stop-color-purple-400" />
        </linearGradient>
        <radialGradient id="radGrad1">
          <stop offset="0%" stopColor="#7c3aed" stopOpacity="0.5" />
          <stop offset="100%" stopColor="#7c3aed" stopOpacity="0" />
        </radialGradient>
        <radialGradient id="radGrad2">
          <stop offset="0%" stopColor="#6d28d9" stopOpacity="0" />
          <stop offset="100%" stopColor="#6d28d9" stopOpacity="1" />
        </radialGradient>
        <radialGradient id="radGrad3">
          <stop offset="0%" stopColor="#4f46e5" stopOpacity="0" />
          <stop offset="100%" stopColor="#4f46e5" stopOpacity="1" />
        </radialGradient>
        <radialGradient id="radGrad4">
          <stop offset="0%" stopColor="#7c3aed" stopOpacity="0" />
          <stop offset="100%" stopColor="#7c3aed" stopOpacity="1" />
        </radialGradient>
      </defs>
    </svg>
  );
}

export function Sidebar({ className }: SidebarProps) {
  const menuItems = [
    { path: "/", icon: MessageSquare, label: "Chat" },
    { path: "/search", icon: Search, label: "Search" },
    { path: "/agents", icon: Bot, label: "Agents" },
    { path: "/skills", icon: Sparkles, label: "Skills" },
    { path: "/mcp", icon: Network, label: "MCP" },
    { path: "/providers", icon: Zap, label: "Providers" },
  ];

  return (
    <aside className={cn("w-[72px] bg-sidebar flex flex-col items-center py-3 gap-2 border-r border-sidebar-border", className)} aria-label="Main navigation">
      {/* Logo */}
      <button
        className="mb-2 cursor-pointer group bg-transparent border-0 p-0"
        aria-label="Zero home"
      >
        <div className="w-12 h-12 flex items-center justify-center transition-all group-hover:scale-110">
          <ZeroLogo className="w-full h-full" />
        </div>
      </button>

      {/* Separator */}
      <div className="w-8 h-[2px] bg-sidebar-border rounded-full my-1" aria-hidden="true" />

      {/* Menu Items */}
      <nav className="flex flex-col gap-2 flex-1" aria-label="Primary navigation">
        {menuItems.map((item) => (
          <NavLink
            key={item.path}
            to={item.path}
            aria-label={item.label}
            className={({ isActive }) =>
              cn(
                "w-12 h-12 rounded-2xl flex items-center justify-center transition-all relative group",
                isActive
                  ? "bg-sidebar-primary text-sidebar-primary-foreground rounded-xl"
                  : "bg-sidebar-accent text-sidebar-foreground hover:text-sidebar-primary-foreground hover:bg-sidebar-primary hover:rounded-xl"
              )
            }
          >
            <item.icon className="size-5" aria-hidden="true" />
            {/* Tooltip with arrow */}
            <div className="absolute left-full ml-4 px-3 py-2 bg-black text-white text-sm rounded-lg opacity-0 group-hover:opacity-100 pointer-events-none whitespace-nowrap z-50 shadow-xl" role="tooltip">
              {item.label}
              <div className="absolute right-full top-1/2 -translate-y-1/2 border-4 border-transparent border-r-black" aria-hidden="true" />
            </div>
          </NavLink>
        ))}
      </nav>

      {/* Settings at bottom */}
      <NavLink
        to="/settings"
        aria-label="Settings"
        className={({ isActive }) =>
          cn(
            "w-12 h-12 rounded-2xl flex items-center justify-center transition-all relative group mb-1",
            isActive
              ? "bg-sidebar-primary text-sidebar-primary-foreground rounded-xl"
              : "bg-sidebar-accent text-sidebar-foreground hover:text-sidebar-primary-foreground hover:bg-sidebar-primary hover:rounded-xl"
          )
        }
      >
        <Settings className="size-5" aria-hidden="true" />
        {/* Tooltip with arrow */}
        <div className="absolute left-full ml-4 px-3 py-2 bg-black text-white text-sm rounded-lg opacity-0 group-hover:opacity-100 pointer-events-none whitespace-nowrap z-50 shadow-xl" role="tooltip">
          Settings
          <div className="absolute right-full top-1/2 -translate-y-1/2 border-4 border-transparent border-r-black" aria-hidden="true" />
        </div>
      </NavLink>
    </aside>
  );
}
