// ============================================================================
// randomId — secure-context-safe UUIDv4 generator
// ============================================================================
// `crypto.randomUUID()` is only defined in **secure contexts** (HTTPS or
// localhost). When the dashboard is loaded over plain HTTP from a LAN IP
// (e.g. a phone hitting `http://192.168.x.x:18791`), the call throws an
// `Uncaught TypeError: crypto.randomUUID is not a function`, which crashes
// the React tree the moment a chat message, plan block, or chip is created.
//
// `crypto.getRandomValues()` is available in non-secure contexts on every
// browser since IE11/Safari 5, so we use it as the universal fallback. We
// also probe `window.msCrypto` for legacy IE 11 where the prefix wasn't
// removed. If neither exists we throw rather than silently weakening to
// `Math.random()` — failing loud is preferable to seeding UI ids from a
// non-cryptographic PRNG without warning.
//
// Output shape matches `crypto.randomUUID()`:
//     "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx"  (36 chars)

interface LegacyCryptoWindow {
  crypto?: Crypto;
  msCrypto?: Crypto;
}

const HEX = "0123456789abcdef";

function hex(n: number): string {
  return HEX[n & 0x0f];
}

/** Resolve the platform Crypto interface, honoring legacy IE's `msCrypto`. */
function getCrypto(): Crypto | undefined {
  if (typeof globalThis === "undefined") return undefined;
  const w = globalThis as unknown as LegacyCryptoWindow;
  return w.crypto ?? w.msCrypto;
}

function v4FromBytes(bytes: Uint8Array): string {
  // RFC 4122 v4 — set version + variant bits.
  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;

  let out = "";
  for (let i = 0; i < 16; i++) {
    if (i === 4 || i === 6 || i === 8 || i === 10) out += "-";
    const b = bytes[i];
    out += hex(b >> 4) + hex(b);
  }
  return out;
}

/**
 * Generate a v4 UUID string. Prefers `crypto.randomUUID()` (fast path on
 * secure contexts), falls back to `crypto.getRandomValues()` for all other
 * contexts including plain-HTTP LAN IPs.
 *
 * @throws Error when neither `crypto.randomUUID` nor `crypto.getRandomValues`
 *   is available — this should never happen in any browser that can run the
 *   dashboard, so it's a hard failure rather than a silent degradation.
 */
export function randomId(): string {
  const cryptoApi = getCrypto();
  if (!cryptoApi) {
    throw new Error("randomId: no crypto API available on this platform");
  }

  if (typeof cryptoApi.randomUUID === "function") {
    try {
      return cryptoApi.randomUUID();
    } catch {
      // Some browsers define `randomUUID` but throw outside secure contexts.
      // Fall through to `getRandomValues`, which is always callable.
    }
  }

  if (typeof cryptoApi.getRandomValues !== "function") {
    throw new Error("randomId: crypto.getRandomValues is unavailable");
  }
  const bytes = new Uint8Array(16);
  cryptoApi.getRandomValues(bytes);
  return v4FromBytes(bytes);
}
