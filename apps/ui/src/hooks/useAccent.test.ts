// ============================================================================
// useAccent — persistence + live mutation tests
// ============================================================================

import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useAccent, ACCENT_OPTIONS, DEFAULT_ACCENT_ID } from "./useAccent";

const STORAGE_KEY = "agentzero-accent";

function getCssVar(): string {
  return document.documentElement.style.getPropertyValue("--fx-accent").trim();
}

describe("useAccent", () => {
  beforeEach(() => {
    window.localStorage.clear();
    document.documentElement.style.removeProperty("--fx-accent");
  });

  afterEach(() => {
    window.localStorage.clear();
    document.documentElement.style.removeProperty("--fx-accent");
  });

  it("defaults to cyan when nothing is stored", () => {
    const { result } = renderHook(() => useAccent());
    expect(result.current.accentId).toBe(DEFAULT_ACCENT_ID);
    expect(result.current.accent.id).toBe("cyan");
    expect(result.current.accent.hex).toBe("#7df9ff");
  });

  it("applies the accent hex to the --fx-accent CSS var on mount", () => {
    renderHook(() => useAccent());
    expect(getCssVar()).toBe("#7df9ff");
  });

  it("exposes exactly four accent options in stable order", () => {
    const { result } = renderHook(() => useAccent());
    expect(result.current.options.map((o) => o.id)).toEqual([
      "cyan",
      "violet",
      "amber",
      "magenta",
    ]);
    expect(result.current.options).toHaveLength(4);
  });

  it("setAccent updates state, persists to localStorage, and live-mutates --fx-accent", () => {
    const { result } = renderHook(() => useAccent());
    act(() => result.current.setAccent("violet"));
    expect(result.current.accentId).toBe("violet");
    expect(result.current.accent.hex).toBe("#a78bff");
    expect(window.localStorage.getItem(STORAGE_KEY)).toBe("violet");
    expect(getCssVar()).toBe("#a78bff");
  });

  it("restores the stored accent on mount", () => {
    window.localStorage.setItem(STORAGE_KEY, "amber");
    const { result } = renderHook(() => useAccent());
    expect(result.current.accentId).toBe("amber");
    expect(getCssVar()).toBe("#ffb14a");
  });

  it("falls back to default when localStorage holds an unknown id", () => {
    window.localStorage.setItem(STORAGE_KEY, "neon-puke");
    const { result } = renderHook(() => useAccent());
    expect(result.current.accentId).toBe(DEFAULT_ACCENT_ID);
  });

  it("each preset hex has a 7-char `#RRGGBB` shape", () => {
    for (const opt of ACCENT_OPTIONS) {
      expect(opt.hex).toMatch(/^#[0-9a-f]{6}$/i);
    }
  });
});
