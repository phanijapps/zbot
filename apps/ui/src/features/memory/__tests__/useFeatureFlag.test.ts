import { describe, it, expect, vi } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { useFeatureFlag } from "../useFeatureFlag";

vi.mock("@/services/transport", () => ({
  getTransport: async () => ({
    getExecutionSettings: async () => ({
      success: true,
      data: {
        maxParallelAgents: 2,
        setupComplete: true,
        featureFlags: { memory_tab_command_deck: true },
        restartRequired: false,
      },
    }),
  }),
}));

describe("useFeatureFlag", () => {
  it("returns true when flag is enabled", async () => {
    const { result } = renderHook(() =>
      useFeatureFlag("memory_tab_command_deck"),
    );
    await waitFor(() => expect(result.current).toBe(true));
  });

  it("returns false for absent flag name", async () => {
    const { result } = renderHook(() => useFeatureFlag("nonexistent_flag"));
    // Default is false; give effect a chance to run then assert it stays false.
    await waitFor(() => {
      expect(result.current).toBe(false);
    });
  });
});
