// ============================================================================
// randomId — secure-context-safe UUIDv4 generator
// ============================================================================
// `crypto.randomUUID()` is only defined in **secure contexts** (HTTPS or
// localhost). When the dashboard is loaded over plain HTTP from a LAN IP
// (e.g. a phone hitting `http://192.168.x.x:18791`), the call throws an
// `Uncaught TypeError: crypto.randomUUID is not a function`, which crashes
// the React tree the moment a chat message, plan block, or chip is created.
//
// `crypto.getRandomValues()` is available in non-secure contexts too, so we
// use it to build a v4-shaped string. If the runtime has no `crypto` at all
// (extremely old environments, or some test setups), we fall back to
// `Math.random()` — these IDs are UI keys, not security tokens, so the
// non-cryptographic path is acceptable as a last resort.
//
// Output shape matches `crypto.randomUUID()`:
//     "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx"  (36 chars)

const HEX = "0123456789abcdef";

function hex(n: number): string {
  return HEX[n & 0x0f];
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
 * Generate a v4 UUID string. Prefers `crypto.randomUUID()` when available
 * (secure context), falls back to `crypto.getRandomValues()` (works in any
 * context including http://lan-ip), final fallback is `Math.random()`.
 */
export function randomId(): string {
  if (typeof crypto !== "undefined") {
    if (typeof crypto.randomUUID === "function") {
      try {
        return crypto.randomUUID();
      } catch {
        // Fall through — some browsers throw outside secure contexts even
        // when the function is defined.
      }
    }
    if (typeof crypto.getRandomValues === "function") {
      const bytes = new Uint8Array(16);
      crypto.getRandomValues(bytes);
      return v4FromBytes(bytes);
    }
  }

  // Last-resort, non-crypto fallback. UI keys only — not for security.
  const bytes = new Uint8Array(16);
  for (let i = 0; i < 16; i++) bytes[i] = Math.floor(Math.random() * 256);
  return v4FromBytes(bytes);
}
