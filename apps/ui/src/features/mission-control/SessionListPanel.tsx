// ============================================================================
// MISSION CONTROL — SessionListPanel
// Left rail: search + status filter chips + scrollable list of sessions.
// Pure rendering; data + selection are wired by MissionControlPage.
// ============================================================================

import type { LogSession } from "@/services/transport/types";
import type { SessionFilters } from "./types";
import type { SessionTokenIndex } from "./useSessionTokens";
import { formatDuration } from "../logs/trace-types";

interface SessionListPanelProps {
  sessions: LogSession[];
  selectedId: string | null;
  filters: SessionFilters;
  loading?: boolean;
  /** Optional — when supplied, rows show per-session in/out token counts. */
  tokenIndex?: SessionTokenIndex;
  onSearchChange(value: string): void;
  onStatusToggle(key: keyof SessionFilters["status"]): void;
  onSelect(sessionId: string): void;
}

export function SessionListPanel({
  sessions,
  selectedId,
  filters,
  loading,
  tokenIndex,
  onSearchChange,
  onStatusToggle,
  onSelect,
}: SessionListPanelProps) {
  const filtered = applyFilters(sessions, filters);

  return (
    <aside className="session-list-panel" aria-label="Sessions">
      <header className="session-list-panel__head">
        <input
          type="search"
          className="session-list-panel__search"
          placeholder="search sessions…"
          value={filters.search}
          onChange={(e) => onSearchChange(e.target.value)}
          aria-label="Search sessions"
        />
        <div className="session-list-panel__filters" role="group" aria-label="Filter by status">
          {STATUS_CHIPS.map(({ key, label }) => {
            const on = filters.status[key];
            return (
              <button
                key={key}
                type="button"
                className={`session-list-panel__chip${on ? " session-list-panel__chip--on" : ""}`}
                aria-pressed={on}
                onClick={() => onStatusToggle(key)}
              >
                {label}
              </button>
            );
          })}
        </div>
      </header>
      <div className="session-list-panel__list">
        {loading && sessions.length === 0 && (
          <div className="session-list-panel__empty">Loading sessions…</div>
        )}
        {!loading && filtered.length === 0 && (
          <div className="session-list-panel__empty">No sessions match these filters.</div>
        )}
        {filtered.map((s) => (
          <SessionRow
            key={s.session_id}
            session={s}
            active={s.session_id === selectedId}
            tokens={tokenIndex?.byRootExecId.get(s.session_id)}
            onSelect={() => onSelect(s.session_id)}
          />
        ))}
      </div>
    </aside>
  );
}

interface SessionRowProps {
  session: LogSession;
  active: boolean;
  tokens?: { in: number; out: number; total: number };
  onSelect(): void;
}

function SessionRow({ session, active, tokens, onSelect }: SessionRowProps) {
  const status = mapStatusVariant(session.status);
  const title = session.title || session.agent_name || session.session_id;
  const duration = formatDuration(session.duration_ms);
  const subagentCount = session.child_session_ids?.length ?? 0;

  return (
    <button
      type="button"
      className={`session-list-panel__row${active ? " session-list-panel__row--active" : ""}`}
      onClick={onSelect}
      aria-current={active ? "true" : undefined}
    >
      <span
        className={`session-list-panel__icon session-list-panel__icon--${status}`}
        aria-hidden="true"
      >
        {STATUS_ICONS[status]}
      </span>
      <span className="session-list-panel__main">
        <span className="session-list-panel__title">{title}</span>
        <span className="session-list-panel__meta">
          #{shortId(session.session_id)} · {session.agent_name}
          {duration && ` · ${duration}`}
          {tokens && tokens.total > 0 && (
            <> · <TokenPair inTok={tokens.in} outTok={tokens.out} compact /></>
          )}
          {subagentCount > 0 && ` · ${subagentCount}↳`}
        </span>
      </span>
    </button>
  );
}

/** Render an in/out token pair: "↓ 24.2k / ↑ 247". Compact for tight rows. */
export function TokenPair({
  inTok,
  outTok,
  compact = false,
}: {
  inTok: number;
  outTok: number;
  compact?: boolean;
}) {
  const fmt = compact ? compactTokens : formatTokensShort;
  return (
    <span className="token-pair">
      <span className="token-pair__in" title={`${inTok.toLocaleString()} input tokens`}>
        ↓ {fmt(inTok)}
      </span>
      <span className="token-pair__sep">/</span>
      <span className="token-pair__out" title={`${outTok.toLocaleString()} output tokens`}>
        ↑ {fmt(outTok)}
      </span>
    </span>
  );
}

/** Compact-tier token formatter for in-line use: "12345" → "12.3k". */
function compactTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return String(n);
}

/** Token formatter for the wider detail-pane header. */
function formatTokensShort(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M tok`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k tok`;
  return `${n} tok`;
}

// ----------------------------------------------------------------------------
// Helpers
// ----------------------------------------------------------------------------

const STATUS_CHIPS: { key: keyof SessionFilters["status"]; label: string }[] = [
  { key: "running", label: "RUNNING" },
  { key: "queued", label: "QUEUED" },
  { key: "completed", label: "DONE" },
  { key: "failed", label: "FAILED" },
  { key: "paused", label: "PAUSED" },
];

const STATUS_ICONS: Record<string, string> = {
  running: "●",
  queued: "◷",
  done: "✓",
  failed: "✗",
  paused: "⏸",
  unknown: "·",
};

type StatusVariant = "running" | "queued" | "done" | "failed" | "paused" | "unknown";

function mapStatusVariant(status: string): StatusVariant {
  switch (status) {
    case "running": return "running";
    case "queued": return "queued";
    case "completed": return "done";
    case "failed":
    case "error":
    case "crashed":
    case "stopped": return "failed";
    case "paused": return "paused";
    default: return "unknown";
  }
}

function shortId(id: string): string {
  return id.length > 8 ? id.slice(-6) : id;
}

/** Apply search + status filter, oldest-first → newest-first sort. */
export function applyFilters(sessions: LogSession[], filters: SessionFilters): LogSession[] {
  const q = filters.search.trim().toLowerCase();
  return sessions
    .filter((s) => {
      const variant = mapStatusVariant(s.status);
      const statusOk =
        (variant === "running" && filters.status.running) ||
        (variant === "queued" && filters.status.queued) ||
        (variant === "done" && filters.status.completed) ||
        (variant === "failed" && filters.status.failed) ||
        (variant === "paused" && filters.status.paused) ||
        (variant === "unknown");
      if (!statusOk) return false;
      if (q === "") return true;
      const haystack = `${s.title ?? ""} ${s.agent_name} ${s.session_id}`.toLowerCase();
      return haystack.includes(q);
    })
    .sort((a, b) => new Date(b.started_at).getTime() - new Date(a.started_at).getTime());
}

export const DEFAULT_FILTERS: SessionFilters = {
  search: "",
  status: {
    running: true,
    queued: true,
    completed: true,
    failed: true,
    paused: false,
  },
};
