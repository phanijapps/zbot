// ============================================================================
// BeliefActivityFeed — colour-coded reverse-chronological event timeline
// ============================================================================
//
// Events come from /api/belief-network/activity already sorted descending by
// timestamp. This component just renders them; sort order is the server's
// responsibility.

import type {
  BeliefActivityEvent,
  BeliefActivityKind,
} from "../types.beliefNetwork";

export interface BeliefActivityFeedProps {
  events: BeliefActivityEvent[];
}

const KIND_LABELS: Record<BeliefActivityKind, string> = {
  synthesized: "Synthesized",
  retracted: "Retracted",
  marked_stale: "Marked stale",
  contradiction_detected: "Contradiction",
  contradiction_resolved: "Resolved",
  propagation_cascade: "Cascade",
};

function kindClass(kind: BeliefActivityKind): string {
  return `belief-activity__kind belief-activity__kind--${kind.replace(/_/g, "-")}`;
}

export function BeliefActivityFeed(props: BeliefActivityFeedProps) {
  const { events } = props;

  if (events.length === 0) {
    return (
      <div
        className="belief-activity"
        data-testid="belief-activity-empty"
      >
        <p className="belief-activity__empty">No recent activity.</p>
      </div>
    );
  }

  return (
    <div className="belief-activity" data-testid="belief-activity">
      <ul className="belief-activity__list">
        {events.map((event, idx) => (
          <li
            key={`${event.timestamp}-${idx}`}
            className="belief-activity__item"
            data-testid="belief-activity-event"
          >
            <span className={kindClass(event.kind)}>
              {KIND_LABELS[event.kind]}
            </span>
            <span className="belief-activity__summary">{event.summary}</span>
            <time
              className="belief-activity__time"
              dateTime={event.timestamp}
            >
              {new Date(event.timestamp).toLocaleString()}
            </time>
          </li>
        ))}
      </ul>
    </div>
  );
}
