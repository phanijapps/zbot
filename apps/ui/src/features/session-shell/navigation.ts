/** Accept only refresh-safe local conversation destinations, never a redirect URL. */
export function conversationDestination(value: string | null | undefined): string {
  return value && /^\/session(?:\/[A-Za-z0-9][A-Za-z0-9_-]{0,127})?(?:\?mode=(?:chat|research))?$/.test(value)
    ? value : "/session";
}

export function knowledgeDestination(path: "/memory" | "/observatory" | "/agents" | "/settings" | "/integrations", returnTo: string): string {
  const target = conversationDestination(returnTo);
  return target === "/session" ? path : `${path}?returnTo=${encodeURIComponent(target)}`;
}
