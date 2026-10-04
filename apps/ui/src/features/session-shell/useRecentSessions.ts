import { useEffect, useState } from "react";
import { getTransport } from "@/services/transport";
import type { SessionWithExecutions } from "@/services/transport/types";

export function useRecentSessions(refreshKey: string) {
  const [recents, setRecents] = useState<SessionWithExecutions[]>([]);
  const [unavailable, setUnavailable] = useState(false);
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      const transport = await getTransport();
      const result = await transport.listSessionsFull({root_agent_id: "root", limit: 30});
      if (cancelled) return;
      setUnavailable(!result.success || !result.data);
      if (result.success && result.data) setRecents(result.data.filter(item => !item.parent_session_id && /^[A-Za-z0-9][A-Za-z0-9_-]{0,127}$/.test(item.id)));
      else setRecents([]);
    })().catch(() => { if (!cancelled) { setUnavailable(true); setRecents([]); } });
    return () => { cancelled = true; };
  }, [refreshKey]);
  return {recents, unavailable};
}
