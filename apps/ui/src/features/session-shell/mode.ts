export type SessionMode = "chat" | "research" | "unknown";

export function sessionMode(mode: string | null | undefined): SessionMode {
  if (mode === "fast" || mode === "chat") return "chat";
  if (mode === "deep" || mode === "research") return "research";
  return "unknown";
}
