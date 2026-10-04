import { useSessionDetails } from "./useSessionDetails";
import { InspectorNotice } from "./InspectorNotice";
import type { HookActivity } from "@/services/transport/types";

const statuses: Record<HookActivity["status"], string> = {
  running: "Hook running", completed: "Hook completed", blocked: "Hook blocked",
  failed: "Hook failed", timeout: "Hook timed out", cancelled: "Hook cancelled", skipped: "Hook skipped",
};

export function ActivityPanel({ sessionId, active }: {sessionId?: string; active: boolean}) {
  const {details, loading, error, retry} = useSessionDetails(sessionId, active);

  if (!sessionId) return <p className="session-shell__hint">Select or start a conversation to inspect its recorded activity.</p>;
  return <section className="session-activity" aria-label="Recorded activity">
    <InspectorNotice loading={loading} hasData={Boolean(details)} error={error}
      loadingLabel="Loading activity…" unavailable="Activity is unavailable. Your conversation and previously loaded activity have been kept." retry={retry} />
    {details?.activityTruncated && <p className="session-inspector__notice">Most recent 500 activity records shown. Earlier records are not included.</p>}
    {details?.activity.length === 0 && <p className="session-shell__hint">No recorded activity for this conversation.</p>}
    <ol className="session-activity__list" aria-busy={loading}>
      {details?.activity.map(row => <li key={row.id} className="session-activity__item">
        {row.kind === "hook" && row.hook ? <details className={`session-activity__hook session-activity__hook--${row.hook.status}`}>
          <summary><span className="session-activity__name">{row.hook.hookId}</span><span className="session-activity__status">{statuses[row.hook.status]}</span></summary>
          <dl className="session-activity__metadata">
            <dt>Event</dt><dd>{row.hook.event}</dd>
            <dt>Agent</dt><dd>{row.hook.agentId}</dd>
            <dt>Run</dt><dd>{row.hook.runId ?? "Not yet started"}</dd>
            <dt>Duration</dt><dd>{row.hook.durationMs === null ? "Not available" : `${row.hook.durationMs} ms`}</dd>
            <dt>Exit code</dt><dd>{row.hook.exitCode ?? "Not available"}</dd>
            <dt>Invocation</dt><dd>{row.hook.invocationId}</dd>
          </dl>
        </details> : <p className="session-activity__label">{row.label}</p>}
        <time className="session-activity__time" dateTime={row.occurredAt}>{new Date(row.occurredAt).toLocaleTimeString()}</time>
      </li>)}
    </ol>
  </section>;
}
