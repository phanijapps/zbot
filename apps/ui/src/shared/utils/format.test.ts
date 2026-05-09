// ============================================================================
// format.ts — unit tests for formatContextWindow
// ============================================================================

import { describe, it, expect } from "vitest";
import { formatContextWindow } from "./format";

describe("formatContextWindow", () => {
  it("returns raw number as string for values < 1000", () => {
    expect(formatContextWindow(0)).toBe("0");
    expect(formatContextWindow(1)).toBe("1");
    expect(formatContextWindow(999)).toBe("999");
  });

  it("formats values >= 1000 and < 1,000,000 with K suffix", () => {
    expect(formatContextWindow(1000)).toBe("1K");
    expect(formatContextWindow(1500)).toBe("2K");
    expect(formatContextWindow(128000)).toBe("128K");
    expect(formatContextWindow(999999)).toBe("1000K");
  });

  it("formats values >= 1,000,000 with M suffix", () => {
    expect(formatContextWindow(1_000_000)).toBe("1M");
    expect(formatContextWindow(1_048_576)).toBe("1M");
    expect(formatContextWindow(2_000_000)).toBe("2M");
    expect(formatContextWindow(8_000_000)).toBe("8M");
  });

  it("rounds correctly at boundaries", () => {
    expect(formatContextWindow(1499)).toBe("1K");
    expect(formatContextWindow(1500)).toBe("2K");
    expect(formatContextWindow(1_499_999)).toBe("1M");
    expect(formatContextWindow(1_500_000)).toBe("2M");
  });
});
