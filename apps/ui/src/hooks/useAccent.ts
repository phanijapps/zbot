// ============================================================================
// USE ACCENT HOOK
// Persists & live-mutates the futuristic theme accent color (--fx-accent).
// ============================================================================

import { useCallback, useEffect, useState } from "react";

export interface AccentOption {
  id: string;
  label: string;
  hex: string;
}

export const ACCENT_OPTIONS: readonly AccentOption[] = [
  { id: "cyan",    label: "Cyan",    hex: "#7df9ff" },
  { id: "violet",  label: "Violet",  hex: "#a78bff" },
  { id: "amber",   label: "Amber",   hex: "#ffb14a" },
  { id: "magenta", label: "Magenta", hex: "#ff5de8" },
] as const;

export const DEFAULT_ACCENT_ID = "cyan";
const STORAGE_KEY = "agentzero-accent";

function isAccentId(value: string | null): value is AccentOption["id"] {
  return value !== null && ACCENT_OPTIONS.some((opt) => opt.id === value);
}

function readStored(): AccentOption["id"] {
  if (typeof window === "undefined") return DEFAULT_ACCENT_ID;
  const stored = window.localStorage.getItem(STORAGE_KEY);
  return isAccentId(stored) ? stored : DEFAULT_ACCENT_ID;
}

function applyAccent(hex: string): void {
  if (typeof document === "undefined") return;
  document.documentElement.style.setProperty("--fx-accent", hex);
}

/**
 * Returns the active accent + a setter that persists to localStorage and
 * live-mutates `--fx-accent` on `<html>`. Restores stored choice on mount.
 */
export function useAccent() {
  const [accentId, setAccentIdState] = useState<AccentOption["id"]>(readStored);

  useEffect(() => {
    const option = ACCENT_OPTIONS.find((opt) => opt.id === accentId);
    if (option) applyAccent(option.hex);
  }, [accentId]);

  const setAccent = useCallback((id: AccentOption["id"]) => {
    setAccentIdState(id);
    if (typeof window !== "undefined") {
      window.localStorage.setItem(STORAGE_KEY, id);
    }
  }, []);

  const accent = ACCENT_OPTIONS.find((opt) => opt.id === accentId) ?? ACCENT_OPTIONS[0];

  return { accent, accentId, setAccent, options: ACCENT_OPTIONS };
}
