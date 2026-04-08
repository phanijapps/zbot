import type { LogSession } from "../../services/transport/types";
import { formatDuration, formatTokens } from "./trace-types";

interface SessionListItemProps {
  session: LogSession;
  isSelected: boolean;
  onClick: () => void;
}

export function SessionListItem({ session, isSelected, onClick }: SessionListItemProps) {
  const title = session.title || session.agent_name || session.agent_id;
  const statusClass = `session-list-item__status session-list-item__status--${session.status}`;
  const itemClass = isSelected
    ? "session-list-item session-list-item--selected"
    : "session-list-item";

  const agentCount = session.child_session_ids?.length || 0;

  return (
    <div className={itemClass} onClick={onClick}>
      <div className="session-list-item__title">{title}</div>
      <div className="session-list-item__meta">
        <span>
          <span className={statusClass} />{" "}
          {agentCount > 0 ? `${agentCount} agent${agentCount > 1 ? "s" : ""}` : "direct"}
          {session.duration_ms ? ` · ${formatDuration(session.duration_ms)}` : ""}
        </span>
        <span>{formatTokens(session.token_count)}</span>
      </div>
    </div>
  );
}
