// ============================================================================
// OBSERVATORY V2 — Apple-Vision-style 3D layered graph
// ============================================================================
//
// A new dedicated page that surfaces the memory subsystem as a 3D
// hierarchy. Built on top of:
//
//   - `useHierarchyStats`   — `/api/hierarchy/stats` snapshot (layers,
//                              inter-cluster relations, top aggregates)
//   - `useBeliefNetworkStats` — Belief Network totals + worker stats
//   - `useGraphStats`       — root counts (facts, entities, relationships,
//                              episodes)
//   - `useDistillationStatus` — sessions distilled / failed / skipped
//
// Aesthetic: dark glass background, cream-white glow on aggregates,
// frosted-glass HUD cards, restrained motion. No neon. The 3D viz
// itself lives in `HierarchyShells` (react-three-fiber).
//
// Phase 1 = static snapshot of the hierarchy. Phase 2 will overlay
// live recall (animated LCA walk during agent queries).
// ============================================================================

import { useState } from "react";
import { ArrowUpRight, Layers, Network, Activity, Brain, X } from "lucide-react";
import { Slideover } from "@/components/Slideover";
import { useGraphStats, useDistillationStatus } from "../observatory/graph-hooks";
import { useBeliefNetworkStats } from "../observatory/belief-network/hooks";
import { useHierarchyStats } from "../observatory/hierarchy/hooks";
import { HierarchyPanel } from "../observatory/hierarchy/HierarchyPanel";
import { BeliefNetworkPanel } from "../observatory/belief-network/BeliefNetworkPanel";
import { HierarchyShells } from "./HierarchyShells";
import { useRecallTrace } from "./useRecallTrace";
import type { AggregateSummary } from "../observatory/hierarchy/types";
import "./observatory-v2.css";

export function ObservatoryV2Page() {
  const { stats: graphStats } = useGraphStats();
  const { status: distillStatus } = useDistillationStatus();
  const { stats: beliefStats } = useBeliefNetworkStats();
  const { stats: hierStats, loading: hierLoading } = useHierarchyStats(20);
  const { traces, latest: latestTrace } = useRecallTrace();

  const [hierOpen, setHierOpen] = useState(false);
  const [beliefOpen, setBeliefOpen] = useState(false);
  const [pickedAgg, setPickedAgg] = useState<AggregateSummary | null>(null);

  const hierarchyEnabled = hierStats?.enabled ?? false;
  const layerCounts = hierStats?.summary?.layer_counts ?? [];
  const aggregates = hierStats?.summary?.top_aggregates ?? [];
  const interCluster = hierStats?.summary?.inter_cluster_relations ?? 0;

  const distilled = distillStatus?.success_count ?? 0;
  const failed = distillStatus?.failed_count ?? 0;
  const skipped = distillStatus?.skipped_count ?? 0;
  const totalDistill =
    distilled + failed + skipped +
    (distillStatus?.permanently_failed_count ?? 0);
  const aggCount = aggregates.length;
  const totalAggCount = layerCounts
    .filter(([layer]) => layer > 0)
    .reduce((sum, [, n]) => sum + n, 0);

  return (
    <div className="obs2">
      {/* 3D canvas — fills the page beneath the HUD */}
      <div className="obs2__canvas-wrap">
        <HierarchyShells
          layerCounts={layerCounts}
          aggregates={aggregates}
          interClusterCount={interCluster}
          enabled={hierarchyEnabled && !hierLoading}
          onAggregateClick={setPickedAgg}
          traces={traces}
        />
        {!hierarchyEnabled && !hierLoading && (
          <div className="obs2__empty">
            <div className="obs2__empty-card">
              <Layers size={28} className="obs2__empty-icon" />
              <h2>Hierarchy is dormant</h2>
              <p>
                Enable <code>execution.memory.hierarchy.enabled</code> in
                Settings → Advanced → Memory, then trigger a sleep cycle.
                Layers will materialise here once aggregates form.
              </p>
            </div>
          </div>
        )}
      </div>

      {/* Glass HUD — frosted overlay */}
      <div className="obs2__hud obs2__hud--tl">
        <div className="obs2__hud-row obs2__hud-row--title">
          <span className="obs2__hud-eyebrow">memory · observatory v2</span>
        </div>
        <div className="obs2__hud-stats">
          <Stat label="Facts" value={graphStats?.facts ?? 0} />
          <Stat label="Entities" value={graphStats?.entities ?? 0} />
          <Stat label="Edges" value={graphStats?.relationships ?? 0} />
          <Stat label="Episodes" value={graphStats?.episodes ?? 0} />
        </div>
        {beliefStats?.enabled && (
          <div className="obs2__hud-stats">
            <Stat label="Beliefs" value={beliefStats.totals.total_beliefs} />
            {beliefStats.totals.total_unresolved_contradictions > 0 ? (
              <Stat
                label="Contradicted"
                value={beliefStats.totals.total_unresolved_contradictions}
                tone="warning"
              />
            ) : null}
          </div>
        )}
      </div>

      <div className="obs2__hud obs2__hud--tr">
        <div className="obs2__hud-row">
          <span className="obs2__hud-eyebrow">distillation</span>
          <span className="obs2__hud-value">
            {distilled} / {totalDistill}
          </span>
        </div>
        {failed > 0 && (
          <div className="obs2__hud-row obs2__hud-row--warn">
            <span className="obs2__hud-eyebrow">failed</span>
            <span className="obs2__hud-value">{failed}</span>
          </div>
        )}
      </div>

      {/* Bottom — comprehensive stats strip + detail pills.
          The strip below the pills mirrors the legacy /observatory
          footer so v2 doesn't lose any information when the legacy
          page gets retired. */}
      <div className="obs2__hud obs2__hud--bottom">
        {hierarchyEnabled && (
          <HudPill
            icon={<Layers size={14} aria-hidden />}
            label={`${layerCounts.length} layer${layerCounts.length === 1 ? "" : "s"} · ${totalAggCount} agg`}
            sub={`${aggCount} shown · ${interCluster} inter-cluster`}
            onOpen={() => setHierOpen(true)}
            openLabel="Open hierarchy"
          />
        )}
        {beliefStats?.enabled && (
          <HudPill
            icon={<Brain size={14} aria-hidden />}
            label="Belief network"
            sub={`${beliefStats.totals.total_beliefs} beliefs · ${beliefStats.totals.total_contradictions} contradictions`}
            onOpen={() => setBeliefOpen(true)}
            openLabel="Open belief network"
          />
        )}
        <HudPill
          icon={<Network size={14} aria-hidden />}
          label="Graph snapshot"
          sub={`${graphStats?.entities ?? 0} entities · ${graphStats?.relationships ?? 0} edges`}
        />
        <HudPill
          icon={<Activity size={14} aria-hidden />}
          label={
            hierLoading
              ? "Syncing graph…"
              : latestTrace
                ? "Live recall"
                : "Live"
          }
          sub={
            hierLoading
              ? "fetching layers"
              : latestTrace
                ? `${latestTrace.surfacedItemCount} items · ${formatAge(latestTrace.at)}`
                : "snapshot up to date"
          }
          dim={hierLoading}
        />
      </div>

      {/* Comprehensive footer strip — mirrors the legacy /observatory
          status bar so this page is information-equivalent. Frosted
          single-row card, dense typography, all the counters live. */}
      <div className="obs2__footer">
        <FooterStat label="Distilled" value={`${distilled} / ${totalDistill}`} />
        <FooterStat label="Facts" value={graphStats?.facts ?? 0} />
        <FooterStat label="Entities" value={graphStats?.entities ?? 0} />
        <FooterStat label="Edges" value={graphStats?.relationships ?? 0} />
        <FooterStat label="Episodes" value={graphStats?.episodes ?? 0} />
        {beliefStats?.enabled ? (
          <>
            <FooterStat label="Beliefs" value={beliefStats.totals.total_beliefs} />
            {beliefStats.totals.total_unresolved_contradictions > 0 ? (
              <FooterStat
                label="Contradictions"
                value={`${beliefStats.totals.total_unresolved_contradictions} unresolved`}
                tone="warning"
              />
            ) : beliefStats.totals.total_contradictions > 0 ? (
              <FooterStat
                label="Contradictions"
                value={beliefStats.totals.total_contradictions}
              />
            ) : null}
          </>
        ) : null}
        {hierarchyEnabled && (
          <>
            <FooterStat
              label="Hierarchy"
              value={`${layerCounts.length} layer${layerCounts.length === 1 ? "" : "s"} · ${totalAggCount} agg`}
            />
            <FooterStat label="Inter-cluster" value={interCluster} />
          </>
        )}
        {failed > 0 ? <FooterStat label="Failed" value={failed} tone="error" /> : null}
        {skipped > 0 ? <FooterStat label="Skipped" value={skipped} tone="warning" /> : null}
      </div>

      <Slideover
        open={hierOpen}
        onClose={() => setHierOpen(false)}
        title="Hierarchy details"
        subtitle="Layer breakdown · inter-cluster edges · top aggregates"
      >
        <HierarchyPanel />
      </Slideover>

      <Slideover
        open={beliefOpen}
        onClose={() => setBeliefOpen(false)}
        title="Belief Network details"
        subtitle="Worker stats · activity feed · propagation chain"
      >
        <BeliefNetworkPanel />
      </Slideover>

      {/* Picked-aggregate floating card — appears when the user clicks
          an L1 sphere. Frosted glass, anchored bottom-right, dismissible. */}
      {pickedAgg && (
        <div className="obs2__pick-card">
          <button
            type="button"
            className="obs2__pick-close"
            onClick={() => setPickedAgg(null)}
            aria-label="Close aggregate detail"
          >
            <X size={14} aria-hidden />
          </button>
          <div className="obs2__pick-head">
            <span className="obs2__pick-eyebrow">aggregate · L{pickedAgg.layer}</span>
            <span className="obs2__pick-name">{pickedAgg.name}</span>
          </div>
          <div className="obs2__pick-meta">
            <span className="obs2__pick-meta-chip">
              {pickedAgg.member_count} member{pickedAgg.member_count === 1 ? "" : "s"}
            </span>
          </div>
          {pickedAgg.description && (
            <p className="obs2__pick-desc">{pickedAgg.description}</p>
          )}
        </div>
      )}
    </div>
  );
}

interface StatProps {
  label: string;
  value: number;
  tone?: "warning";
}
function Stat({ label, value, tone }: StatProps) {
  return (
    <div className={`obs2__stat${tone ? ` obs2__stat--${tone}` : ""}`}>
      <span className="obs2__stat-value">{value.toLocaleString()}</span>
      <span className="obs2__stat-label">{label}</span>
    </div>
  );
}

interface FooterStatProps {
  label: string;
  value: number | string;
  tone?: "warning" | "error";
}
function FooterStat({ label, value, tone }: FooterStatProps) {
  return (
    <div className={`obs2__footer-stat${tone ? ` obs2__footer-stat--${tone}` : ""}`}>
      <span className="obs2__footer-stat-label">{label}</span>
      <span className="obs2__footer-stat-value">
        {typeof value === "number" ? value.toLocaleString() : value}
      </span>
    </div>
  );
}

/** Tiny relative-time formatter for "fired N seconds ago". */
function formatAge(ts: number): string {
  const delta = Math.max(0, Date.now() - ts);
  if (delta < 1500) return "just now";
  if (delta < 60_000) return `${Math.floor(delta / 1000)}s ago`;
  if (delta < 3_600_000) return `${Math.floor(delta / 60_000)}m ago`;
  return `${Math.floor(delta / 3_600_000)}h ago`;
}

interface HudPillProps {
  icon: React.ReactNode;
  label: string;
  sub: string;
  onOpen?: () => void;
  openLabel?: string;
  dim?: boolean;
}
function HudPill({ icon, label, sub, onOpen, openLabel, dim }: HudPillProps) {
  return (
    <div className={`obs2__pill${dim ? " obs2__pill--dim" : ""}`}>
      <span className="obs2__pill-icon" aria-hidden>{icon}</span>
      <span className="obs2__pill-body">
        <span className="obs2__pill-label">{label}</span>
        <span className="obs2__pill-sub">{sub}</span>
      </span>
      {onOpen && (
        <button
          type="button"
          className="obs2__pill-open"
          onClick={onOpen}
          aria-label={openLabel ?? "Open details"}
          title={openLabel ?? "Open details"}
        >
          <ArrowUpRight size={14} aria-hidden />
        </button>
      )}
    </div>
  );
}
