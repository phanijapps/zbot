import { useState, useMemo } from "react";
import type { LogSession } from "../../services/transport/types";
import { SessionListItem } from "./SessionListItem";

interface SessionListProps {
  sessions: LogSession[];
  selectedId: string | null;
  onSelect: (sessionId: string) => void;
  loading: boolean;
}

export function SessionList({ sessions, selectedId, onSelect, loading }: SessionListProps) {
  const [filter, setFilter] = useState("");

  const filtered = useMemo(() => {
    if (!filter) return sessions;
    const lower = filter.toLowerCase();
    return sessions.filter(
      (s) =>
        (s.title || "").toLowerCase().includes(lower) ||
        s.agent_id.toLowerCase().includes(lower) ||
        s.agent_name.toLowerCase().includes(lower)
    );
  }, [sessions, filter]);

  return (
    <div className="session-list">
      <div className="session-list__filter">
        <input
          className="session-list__filter-input"
          type="text"
          placeholder="Filter sessions..."
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
        />
      </div>
      <div className="session-list__items">
        {loading && filtered.length === 0 && (
          <div style={{ padding: "var(--spacing-4)", textAlign: "center" }}>
            <span className="loading-spinner" />
          </div>
        )}
        {!loading && filtered.length === 0 && (
          <div style={{ padding: "var(--spacing-4)", textAlign: "center", color: "var(--muted-foreground)", fontSize: "var(--text-sm)" }}>
            No sessions found
          </div>
        )}
        {filtered.map((session) => (
          <SessionListItem
            key={session.session_id}
            session={session}
            isSelected={session.session_id === selectedId}
            onClick={() => onSelect(session.session_id)}
          />
        ))}
      </div>
    </div>
  );
}
