import { createContext, useContext, useEffect, useState, ReactNode } from "react";
import { getThemeCss } from "@/services/themes";
import { getSettings } from "@/services/settings";

interface ThemeContextValue {
  theme: string;
  setTheme: (theme: string) => void;
  isLoading: boolean;
}

const ThemeContext = createContext<ThemeContextValue | null>(null);

const THEME_STYLE_ID = "agentzero-theme";

// Check if a color is dark (simple luminance check)
function isDarkColor(color: string): boolean {
  // Handle hex colors
  if (color.startsWith("#")) {
    const hex = color.slice(1);
    const r = parseInt(hex.slice(0, 2), 16);
    const g = parseInt(hex.slice(2, 4), 16);
    const b = parseInt(hex.slice(4, 6), 16);
    // Calculate relative luminance
    const luminance = (0.299 * r + 0.587 * g + 0.114 * b) / 255;
    return luminance < 0.5;
  }
  // Default to dark for unknown formats
  return true;
}

// Parse CSS variables from theme CSS content
function parseThemeVariables(css: string): Map<string, string> {
  const variables = new Map<string, string>();

  // Extract :root block (this is the primary source)
  const rootMatch = css.match(/:root\s*\{([^}]+)\}/);
  if (rootMatch) {
    const rootVars = rootMatch[1].matchAll(/--([^:]+):\s*([^;]+);/g);
    for (const match of rootVars) {
      const name = match[1].trim();
      const value = match[2].trim();
      variables.set(name, value);
    }
  }

  return variables;
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setThemeState] = useState("default");
  const [isLoading, setIsLoading] = useState(true);

  // Load theme from settings on mount
  useEffect(() => {
    loadThemeFromSettings();
  }, []);

  // Apply theme CSS when theme changes
  useEffect(() => {
    applyTheme(theme);
  }, [theme]);

  const loadThemeFromSettings = async () => {
    try {
      const settings = await getSettings();
      if (settings.appearance?.theme) {
        setThemeState(settings.appearance.theme);
      }
    } catch (error) {
      console.error("Failed to load theme from settings:", error);
    } finally {
      setIsLoading(false);
    }
  };

  const applyTheme = async (themeId: string) => {
    try {
      if (themeId === "default") {
        clearTheme();
        // Default theme is dark
        document.body.classList.add("dark");
        document.documentElement.classList.add("dark");
        return;
      }

      const css = await getThemeCss(themeId);
      const vars = parseThemeVariables(css);

      // Apply variables
      for (const [name, value] of vars) {
        document.documentElement.style.setProperty(`--${name}`, value);
        document.body.style.setProperty(`--${name}`, value);
        document.documentElement.style.setProperty(`--color-${name}`, value);
        document.body.style.setProperty(`--color-${name}`, value);
      }

      // Inject style element as fallback
      injectThemeStyle(css);

      // Auto-detect dark/light mode from background color
      const bgColor = vars.get("background") || "#1a1a1a";
      if (isDarkColor(bgColor)) {
        document.body.classList.add("dark");
        document.documentElement.classList.add("dark");
      } else {
        document.body.classList.remove("dark");
        document.documentElement.classList.remove("dark");
      }

    } catch (error) {
      console.error(`Failed to apply theme '${themeId}':`, error);
      clearTheme();
    }
  };

  const clearTheme = () => {
    // Remove style element
    const existing = document.getElementById(THEME_STYLE_ID);
    if (existing) {
      existing.remove();
    }

    // Clear inline variables
    const varsToClean = [
      "font-size", "background", "foreground", "card", "card-foreground",
      "popover", "popover-foreground", "primary", "primary-foreground",
      "secondary", "secondary-foreground", "muted", "muted-foreground",
      "accent", "accent-foreground", "destructive", "destructive-foreground",
      "border", "input", "input-background", "switch-background", "ring",
      "chart-1", "chart-2", "chart-3", "chart-4", "chart-5", "radius",
      "sidebar", "sidebar-foreground", "sidebar-primary", "sidebar-primary-foreground",
      "sidebar-accent", "sidebar-accent-foreground", "sidebar-border", "sidebar-ring"
    ];

    for (const name of varsToClean) {
      document.documentElement.style.removeProperty(`--${name}`);
      document.documentElement.style.removeProperty(`--color-${name}`);
      document.body.style.removeProperty(`--${name}`);
      document.body.style.removeProperty(`--color-${name}`);
    }
  };

  const injectThemeStyle = (css: string) => {
    const existing = document.getElementById(THEME_STYLE_ID);
    if (existing) {
      existing.remove();
    }
    const style = document.createElement("style");
    style.id = THEME_STYLE_ID;
    style.textContent = css;
    document.head.appendChild(style);
  };

  const setTheme = (newTheme: string) => {
    setThemeState(newTheme);
  };

  return (
    <ThemeContext.Provider value={{ theme, setTheme, isLoading }}>
      {children}
    </ThemeContext.Provider>
  );
}

export function useTheme() {
  const context = useContext(ThemeContext);
  if (!context) {
    throw new Error("useTheme must be used within a ThemeProvider");
  }
  return context;
}
