// ============================================================================
// HIERARCHY PANEL — slideover content surfaced from the Observatory pill
// ============================================================================
//
// Three sections, top-to-bottom:
//   1. Layer count breakdown ("layer 0: 693", "layer 1: 30", ...)
//   2. Total inter-cluster relations
//   3. Top-N aggregates (name + member_count + description) sorted by size
//
// Loads via useHierarchyStats. Empty / disabled / error states are
// rendered inline rather than at the slideover's seam so the user always
// sees the surface (matches BeliefNetworkPanel's idiom).
// ============================================================================

import { useHierarchyStats } from "./hooks";

export function HierarchyPanel() {
  const { stats, loading, error, refetch } = useHierarchyStats(10);

  if (loading) {
    return (
      <div className="belief-network-panel">
        <p className="belief-network-card__caption">
          Loading hierarchy stats…
        </p>
      </div>
    );
  }

  if (error) {
    return (
      <div className="belief-network-panel">
        <p
          className="belief-network-card__caption"
          style={{ color: "var(--destructive)" }}
        >
          Failed to load hierarchy stats: {error}
        </p>
        <button className="btn btn--ghost btn--sm" onClick={refetch}>
          Retry
        </button>
      </div>
    );
  }

  if (!stats?.enabled) {
    return (
      <div className="belief-network-panel">
        <p className="belief-network-card__caption">
          Hierarchical memory is disabled. Enable it in Settings → Advanced
          → Memory ({"“"}hierarchy{"”"} block, set{" "}
          <span className="mono">enabled: true</span>) and trigger a sleep
          cycle to build the first layer.
        </p>
      </div>
    );
  }

  const { summary } = stats;
  const totalEntities = summary.layer_counts.reduce(
    (sum, [, count]) => sum + count,
    0,
  );

  return (
    <div className="belief-network-panel">
      {/* Section 1: layer breakdown */}
      <div className="belief-network-card">
        <div className="belief-network-card__head">
          <span className="belief-network-card__kind">Layers</span>
          <span className="belief-network-card__caption">
            {summary.layer_counts.length} layer
            {summary.layer_counts.length === 1 ? "" : "s"} · {totalEntities}{" "}
            entities total
          </span>
        </div>
        {summary.layer_counts.length === 0 ? (
          <p className="belief-network-card__caption">
            No entities yet — nothing to cluster.
          </p>
        ) : (
          <ul className="hierarchy-layer-list">
            {summary.layer_counts.map(([layer, count]) => (
              <li
                key={layer}
                className="hierarchy-layer-list__row"
                style={{
                  display: "flex",
                  justifyContent: "space-between",
                  padding: "4px 0",
                  borderBottom: "1px solid var(--border, #1c2535)",
                }}
              >
                <span className="mono">
                  {layer === 0 ? "layer 0 (base)" : `layer ${layer}`}
                </span>
                <span className="mono" style={{ fontWeight: 500 }}>
                  {count}
                </span>
              </li>
            ))}
          </ul>
        )}
      </div>

      {/* Section 2: inter-cluster edge total */}
      <div className="belief-network-card">
        <div className="belief-network-card__head">
          <span className="belief-network-card__kind">Inter-cluster edges</span>
          <span className="belief-network-card__caption">
            LeanRAG λ-gated edges between aggregates
          </span>
        </div>
        <p
          className="mono"
          style={{ fontSize: 22, fontWeight: 500, margin: "4px 0" }}
        >
          {summary.inter_cluster_relations}
        </p>
      </div>

      {/* Section 3: top aggregates */}
      <div className="belief-network-card">
        <div className="belief-network-card__head">
          <span className="belief-network-card__kind">Top aggregates</span>
          <span className="belief-network-card__caption">
            Ranked by member_count · pulled from kg_entities.layer {">"} 0
          </span>
        </div>
        {summary.top_aggregates.length === 0 ? (
          <p className="belief-network-card__caption">
            No aggregates yet — flip{" "}
            <span className="mono">hierarchy.enabled</span> and trigger a
            sleep cycle.
          </p>
        ) : (
          <ul style={{ listStyle: "none", padding: 0, margin: 0 }}>
            {summary.top_aggregates.map((agg) => (
              <li
                key={agg.id}
                style={{
                  padding: "8px 0",
                  borderBottom: "1px solid var(--border, #1c2535)",
                }}
              >
                <div
                  style={{
                    display: "flex",
                    justifyContent: "space-between",
                    gap: 8,
                  }}
                >
                  <span className="mono" style={{ fontWeight: 500 }}>
                    {agg.name}
                  </span>
                  <span
                    className="mono"
                    style={{ color: "var(--muted-foreground)" }}
                  >
                    L{agg.layer} · {agg.member_count} members
                  </span>
                </div>
                {agg.description ? (
                  <p
                    className="belief-network-card__caption"
                    style={{ marginTop: 4 }}
                  >
                    {agg.description}
                  </p>
                ) : null}
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}
