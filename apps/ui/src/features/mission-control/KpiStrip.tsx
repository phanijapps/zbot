// ============================================================================
// MISSION CONTROL — KpiStrip
// Five status counters + a tokens-consumed cell + a 24h delta cell. Reads
// from the computed MissionKpis; styling lives in components.css under the
// `.kpi-strip*` BEM tree.
// ============================================================================

import type { MissionKpis } from "./types";

interface KpiStripProps {
  kpis: MissionKpis;
}

export function KpiStrip({ kpis }: KpiStripProps) {
  return (
    <div className="kpi-strip" role="region" aria-label="Mission Control overview">
      <KpiCell variant="running" icon="●" count={kpis.running} label="Running"
        sub={kpis.runningTokens > 0 ? `${formatTokens(kpis.runningTokens)} streaming` : "—"} />
      <KpiCell variant="queued" icon="◷" count={kpis.queued} label="Queued"
        sub={kpis.queued > 0 ? "waiting to start" : "—"} />
      <KpiCell variant="done" icon="✓" count={kpis.done24h} label="Done · 24h"
        sub={kpis.successRate !== null ? `${kpis.successRate}% success` : "—"} />
      <KpiCell variant="failed" icon="✗" count={kpis.failed24h} label="Failed · 24h"
        sub={kpis.failed24h > 0 ? "needs review" : "all clear"} />
      <KpiCell variant="paused" icon="⏸" count={kpis.paused} label="Paused"
        sub={kpis.paused > 0 ? "manual hold" : "—"} />
      <DeltaCell delta={kpis.delta24h} />
    </div>
  );
}

interface KpiCellProps {
  variant: "running" | "queued" | "done" | "failed" | "paused";
  icon: string;
  count: number;
  label: string;
  sub: string;
}

function KpiCell({ variant, icon, count, label, sub }: KpiCellProps) {
  return (
    <div className={`kpi-strip__cell kpi-strip__cell--${variant}`}>
      <div className="kpi-strip__top">
        <span className="kpi-strip__icon" aria-hidden="true">{icon}</span>
        <span className="kpi-strip__num">{count}</span>
      </div>
      <div className="kpi-strip__label">{label}</div>
      <div className="kpi-strip__sub">{sub}</div>
    </div>
  );
}

interface DeltaCellProps {
  delta: number | null;
}

function DeltaCell({ delta }: DeltaCellProps) {
  let text = "—";
  let cls = "kpi-strip__delta-num";
  if (delta !== null) {
    if (delta > 0) {
      text = `▲ +${delta}%`;
      cls += " kpi-strip__delta-num--up";
    } else if (delta < 0) {
      text = `▼ ${delta}%`;
      cls += " kpi-strip__delta-num--down";
    } else {
      text = "0%";
    }
  }
  return (
    <div className="kpi-strip__cell kpi-strip__cell--delta">
      <div className={cls}>{text}</div>
      <div className="kpi-strip__delta-label">vs 24h ago</div>
    </div>
  );
}

function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M tok`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k tok`;
  return `${n} tok`;
}
