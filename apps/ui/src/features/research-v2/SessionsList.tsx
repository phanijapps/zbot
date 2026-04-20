import { Plus, Trash2 } from "lucide-react";
import type { SessionSummary } from "./types";

// -------------------------------------------------------------------------
// Grouping constants + helpers
// -------------------------------------------------------------------------

const ONE_DAY_MS = 24 * 60 * 60 * 1000;
const SEVEN_DAYS_MS = 7 * ONE_DAY_MS;

type Bucket = "Running" | "Today" | "Yesterday" | "Last week" | "Older";
const BUCKET_ORDER: Bucket[] = ["Running", "Today", "Yesterday", "Last week", "Older"];

// SonarQube: module-level constant — no magic hex in JSX
const STATUS_DOT: Record<SessionSummary["status"], { color: string; label: string }> = {
  running:  { color: "var(--success)",          label: "running"  },
  complete: { color: "var(--muted-foreground)", label: "complete" },
  crashed:  { color: "var(--destructive)",      label: "crashed"  },
  paused:   { color: "var(--warning)",          label: "paused"   },
};

function bucketFor(session: SessionSummary, now: number, startOfTodayMs: number): Bucket {
  if (session.status === "running") return "Running";
  if (session.updatedAt >= startOfTodayMs) return "Today";
  if (session.updatedAt >= startOfTodayMs - ONE_DAY_MS) return "Yesterday";
  if (session.updatedAt >= now - SEVEN_DAYS_MS) return "Last week";
  return "Older";
}

export function groupSessions(
  sessions: SessionSummary[],
  now: number = Date.now(),
): Record<Bucket, SessionSummary[]> {
  const groups: Record<Bucket, SessionSummary[]> = {
    Running: [], Today: [], Yesterday: [], "Last week": [], Older: [],
  };
  const startOfToday = new Date(now);
  startOfToday.setHours(0, 0, 0, 0);
  const startOfTodayMs = startOfToday.getTime();

  for (const s of sessions) {
    groups[bucketFor(s, now, startOfTodayMs)].push(s);
  }
  for (const bucket of BUCKET_ORDER) {
    groups[bucket].sort((a, b) => b.updatedAt - a.updatedAt);
  }
  return groups;
}

// -------------------------------------------------------------------------
// Relative-time helper
// -------------------------------------------------------------------------

function relativeTime(at: number, now: number = Date.now()): string {
  const diff = now - at;
  if (diff < 60_000) return "just now";
  if (diff < 3_600_000) return `${Math.round(diff / 60_000)}m ago`;
  if (diff < ONE_DAY_MS) return `${Math.round(diff / 3_600_000)}h ago`;
  return `${Math.round(diff / ONE_DAY_MS)}d ago`;
}

// -------------------------------------------------------------------------
// Presentation
// -------------------------------------------------------------------------

export interface SessionsListProps {
  sessions: SessionSummary[];
  currentId: string | null;
  onSelect(id: string): void;
  onNew(): void;
  onDelete(id: string): void;
  renderDensity: "expanded" | "condensed";
}

interface SessionsListRowProps {
  session: SessionSummary;
  isActive: boolean;
  onSelect(id: string): void;
  onDelete(id: string): void;
}

function SessionsListRow({ session, isActive, onSelect, onDelete }: SessionsListRowProps) {
  const dot = STATUS_DOT[session.status];
  const label = session.title || session.id;
  return (
    <div
      className={`sessions-list__row${isActive ? " sessions-list__row--active" : ""}`}
      role="button"
      tabIndex={0}
      onClick={() => onSelect(session.id)}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onSelect(session.id);
        }
      }}
    >
      <span
        className="sessions-list__dot"
        style={{ background: dot.color }}
        title={dot.label}
        aria-hidden="true"
      />
      <span className="sessions-list__title">{session.title || "(untitled)"}</span>
      {session.wardName && (
        <span className="sessions-list__ward">{session.wardName}</span>
      )}
      <span className="sessions-list__time">{relativeTime(session.updatedAt)}</span>
      <button
        type="button"
        className="sessions-list__row-delete"
        onClick={(e) => {
          e.stopPropagation();
          onDelete(session.id);
        }}
        aria-label={`Delete session ${label}`}
        title="Delete session"
        data-testid={`sessions-list-delete-${session.id}`}
      >
        <Trash2 size={12} />
      </button>
    </div>
  );
}

interface SessionsListGroupProps {
  bucket: Bucket;
  sessions: SessionSummary[];
  currentId: string | null;
  onSelect(id: string): void;
  onDelete(id: string): void;
}

function SessionsListGroup({ bucket, sessions, currentId, onSelect, onDelete }: SessionsListGroupProps) {
  if (sessions.length === 0) return null;
  return (
    <div className="sessions-list__group">
      <div className="sessions-list__group-title">{bucket}</div>
      {sessions.map((s) => (
        <SessionsListRow
          key={s.id}
          session={s}
          isActive={s.id === currentId}
          onSelect={onSelect}
          onDelete={onDelete}
        />
      ))}
    </div>
  );
}

export function SessionsList({
  sessions,
  currentId,
  onSelect,
  onNew,
  onDelete,
  renderDensity,
}: SessionsListProps) {
  const groups = groupSessions(sessions);
  const isEmpty = sessions.length === 0;
  return (
    <div className={`sessions-list sessions-list--${renderDensity}`}>
      <button
        type="button"
        data-testid="sessions-list-new"
        className="sessions-list__new"
        onClick={onNew}
      >
        <Plus size={14} aria-hidden="true" /> New research
      </button>

      {BUCKET_ORDER.map((bucket) => (
        <SessionsListGroup
          key={bucket}
          bucket={bucket}
          sessions={groups[bucket]}
          currentId={currentId}
          onSelect={onSelect}
          onDelete={onDelete}
        />
      ))}

      {isEmpty && (
        <div className="sessions-list__empty">no research sessions yet</div>
      )}
    </div>
  );
}
