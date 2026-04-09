import type { TraceNode } from "./trace-types";
import { formatDuration, formatTokens } from "./trace-types";
import { TraceNodeComponent } from "./TraceNodeComponent";

interface TraceTimelineProps {
  trace: TraceNode | null;
  loading: boolean;
}

export function TraceTimeline({ trace, loading }: TraceTimelineProps) {
  if (loading) {
    return (
      <div className="trace-timeline">
        <div className="trace-timeline__empty">
          <span className="loading-spinner" />
        </div>
      </div>
    );
  }

  if (!trace) {
    return (
      <div className="trace-timeline">
        <div className="trace-timeline__empty">Select a session to view its trace</div>
      </div>
    );
  }

  return (
    <div className="trace-timeline">
      <div className="trace-timeline__header">
        <div className="trace-timeline__title">
          {trace.summary || trace.label}
        </div>
        <div className="trace-timeline__subtitle">
          {formatDuration(trace.durationMs)} · {formatTokens(trace.tokenCount)} · {trace.status}
        </div>
      </div>
      <div className="trace-timeline__tree">
        <TraceNodeComponent node={trace} depth={0} />
      </div>
    </div>
  );
}
