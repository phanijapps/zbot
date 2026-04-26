// ============================================================================
// randomId — verify the v4 shape, fallback chain, and explicit failure when
// no crypto API is reachable.
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
  let originalCrypto: PropertyDescriptor | undefined;

  beforeEach(() => {
    originalCrypto = Object.getOwnPropertyDescriptor(globalThis, "crypto");
  });

  afterEach(() => {
    if (originalCrypto) {
      Object.defineProperty(globalThis, "crypto", originalCrypto);
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

  it("falls back when crypto.randomUUID throws (some browsers define it but throw outside secure contexts)", () => {
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

  it("uses window.msCrypto when window.crypto is missing (legacy IE 11)", () => {
    const mockGetRandomValues = vi.fn((arr: Uint8Array) => {
      for (let i = 0; i < arr.length; i++) arr[i] = (i * 11) & 0xff;
      return arr;
    });
    Object.defineProperty(globalThis, "crypto", {
      value: undefined,
      configurable: true,
      writable: true,
    });
    Object.defineProperty(globalThis, "msCrypto", {
      value: { getRandomValues: mockGetRandomValues },
      configurable: true,
      writable: true,
    });
    try {
      const id = randomId();
      expect(mockGetRandomValues).toHaveBeenCalled();
      expect(id).toMatch(V4_SHAPE);
    } finally {
      Object.defineProperty(globalThis, "msCrypto", {
        value: undefined,
        configurable: true,
        writable: true,
      });
    }
  });

  it("throws explicitly when neither crypto nor msCrypto is available (no silent Math.random fallback)", () => {
    Object.defineProperty(globalThis, "crypto", {
      value: undefined,
      configurable: true,
      writable: true,
    });
    expect(() => randomId()).toThrow(/no crypto API available/);
  });

  it("throws when crypto exists but getRandomValues is missing", () => {
    Object.defineProperty(globalThis, "crypto", {
      value: { /* no randomUUID, no getRandomValues */ },
      configurable: true,
      writable: true,
    });
    expect(() => randomId()).toThrow(/getRandomValues is unavailable/);
  });
});
