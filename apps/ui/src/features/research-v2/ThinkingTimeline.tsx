import type { TimelineEntry } from "./types";

export interface ThinkingTimelineProps {
  entries: TimelineEntry[];
}

function formatTime(at: number): string {
  const d = new Date(at);
  return d.toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

function ToolCallLine({ entry }: { entry: TimelineEntry }) {
  return (
    <>
      <code>{entry.toolName}</code>
      {entry.toolArgsPreview && (
        <span className="thinking-timeline__preview">{entry.toolArgsPreview}</span>
      )}
    </>
  );
}

function ToolResultLine({ entry }: { entry: TimelineEntry }) {
  return (
    <>
      <span className="thinking-timeline__label">↳</span>
      <span className="thinking-timeline__preview">
        {entry.toolResultPreview ?? entry.text}
      </span>
    </>
  );
}

function EntryContent({ entry }: { entry: TimelineEntry }) {
  if (entry.kind === "tool_call" && entry.toolName) {
    return <ToolCallLine entry={entry} />;
  }
  if (entry.kind === "tool_result") {
    return <ToolResultLine entry={entry} />;
  }
  return <>{entry.text}</>;
}

export function ThinkingTimeline({ entries }: ThinkingTimelineProps) {
  if (entries.length === 0) {
    return <div className="thinking-timeline__empty">no intermediate events</div>;
  }
  return (
    <ol className="thinking-timeline">
      {entries.map((e) => (
        <li key={e.id} className={`thinking-timeline__item thinking-timeline__item--${e.kind}`}>
          <span className="thinking-timeline__time">{formatTime(e.at)}</span>
          <span className="thinking-timeline__text">
            <EntryContent entry={e} />
          </span>
        </li>
      ))}
    </ol>
  );
}
