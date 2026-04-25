// ============================================================================
// randomId — verify the v4 shape, fallback chain, and non-secure-context safety
// ============================================================================

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { randomId } from "./randomId";

const V4_SHAPE = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

describe("randomId", () => {
  it("returns a 36-char string in canonical UUID v4 format", () => {
    const id = randomId();
    expect(id).toHaveLength(36);
    expect(id).toMatch(V4_SHAPE);
  });

  it("produces unique values on consecutive calls", () => {
    const ids = new Set<string>();
    for (let i = 0; i < 1000; i++) ids.add(randomId());
    expect(ids.size).toBe(1000);
  });
});

describe("randomId — fallback chain", () => {
  let originalCrypto: Crypto | undefined;

  beforeEach(() => {
    originalCrypto = globalThis.crypto;
  });

  afterEach(() => {
    if (originalCrypto) {
      Object.defineProperty(globalThis, "crypto", {
        value: originalCrypto,
        configurable: true,
        writable: true,
      });
    }
  });

  it("falls back to crypto.getRandomValues when crypto.randomUUID is unavailable (non-secure context)", () => {
    const mockGetRandomValues = vi.fn((arr: Uint8Array) => {
      for (let i = 0; i < arr.length; i++) arr[i] = i;
      return arr;
    });
    Object.defineProperty(globalThis, "crypto", {
      value: { getRandomValues: mockGetRandomValues },
      configurable: true,
      writable: true,
    });
    const id = randomId();
    expect(mockGetRandomValues).toHaveBeenCalled();
    expect(id).toMatch(V4_SHAPE);
  });

  it("falls back when crypto.randomUUID throws (some browsers throw outside secure contexts)", () => {
    const mockGetRandomValues = vi.fn((arr: Uint8Array) => {
      for (let i = 0; i < arr.length; i++) arr[i] = (i + 1) * 7;
      return arr;
    });
    Object.defineProperty(globalThis, "crypto", {
      value: {
        randomUUID: () => {
          throw new TypeError("crypto.randomUUID is not a function");
        },
        getRandomValues: mockGetRandomValues,
      },
      configurable: true,
      writable: true,
    });
    const id = randomId();
    expect(mockGetRandomValues).toHaveBeenCalled();
    expect(id).toMatch(V4_SHAPE);
  });

  it("falls back to Math.random when crypto is entirely undefined", () => {
    Object.defineProperty(globalThis, "crypto", {
      value: undefined,
      configurable: true,
      writable: true,
    });
    const id = randomId();
    expect(id).toMatch(V4_SHAPE);
  });
});
