// ============================================================================
// THEME TOGGLE
// Toggle button for switching between light/dark/system themes
// ============================================================================

import { Sun, Moon, Monitor } from "lucide-react";
import { useTheme, type Theme } from "@/hooks/useTheme";

const themeConfig: Record<Theme, { icon: typeof Sun; label: string }> = {
  light: { icon: Sun, label: "Light" },
  dark: { icon: Moon, label: "Dark" },
  system: { icon: Monitor, label: "System" },
};

export function ThemeToggle() {
  const { theme, toggleTheme } = useTheme();
  const { icon: Icon, label } = themeConfig[theme];

  return (
    <button
      onClick={toggleTheme}
      className="theme-toggle"
      title={`Theme: ${label} (click to change)`}
      aria-label={`Current theme: ${label}. Click to change.`}
    >
      <Icon className="theme-toggle__icon" />
      <span className="theme-toggle__label">{label}</span>
    </button>
  );
}
