// ============================================================================
// USE THEME HOOK
// Manages theme state (light/dark/system) with localStorage persistence
// ============================================================================

import { useState, useEffect, useCallback } from "react";

export type Theme = "light" | "dark" | "system";

const THEME_STORAGE_KEY = "agentzero-theme";

/**
 * Hook for managing application theme.
 * Supports light, dark, and system preferences.
 * Persists choice to localStorage.
 */
export function useTheme() {
  const [theme, setThemeState] = useState<Theme>(() => {
    if (typeof window === "undefined") return "system";
    const stored = localStorage.getItem(THEME_STORAGE_KEY) as Theme | null;
    return stored || "system";
  });

  const [isDark, setIsDark] = useState(false);

  // Apply theme to document
  useEffect(() => {
    const root = document.documentElement;
    const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");

    const applyTheme = () => {
      const shouldBeDark =
        theme === "dark" ||
        (theme === "system" && mediaQuery.matches);

      root.classList.toggle("dark", shouldBeDark);
      setIsDark(shouldBeDark);
    };

    applyTheme();

    // Listen for system preference changes
    const handleChange = () => {
      if (theme === "system") {
        applyTheme();
      }
    };

    mediaQuery.addEventListener("change", handleChange);
    return () => mediaQuery.removeEventListener("change", handleChange);
  }, [theme]);

  const setTheme = useCallback((newTheme: Theme) => {
    setThemeState(newTheme);
    localStorage.setItem(THEME_STORAGE_KEY, newTheme);
  }, []);

  const toggleTheme = useCallback(() => {
    setTheme(theme === "light" ? "dark" : theme === "dark" ? "system" : "light");
  }, [theme, setTheme]);

  return {
    theme,
    setTheme,
    toggleTheme,
    isDark,
  };
}
