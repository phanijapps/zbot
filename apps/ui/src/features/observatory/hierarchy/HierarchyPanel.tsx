// ============================================================================
// HIERARCHY PANEL — slideover content surfaced from the Observatory pill
// ============================================================================
//
// Three sections, top-to-bottom:
//   1. Layer count breakdown ("layer 0: 693", "layer 1: 30", ...)
//   2. Total inter-cluster relations
//   3. Top-N aggregates (name + member_count + description) sorted by size
//
// Uses the app's standard `belief-card` + `meta-chip` idioms so the
// surface visually matches the Memory tab's belief cards instead of
// inventing one-off inline styles.
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
      {/* Section 1: layer breakdown — one chip per layer */}
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
          <div
            className="belief-card__meta"
            style={{ flexWrap: "wrap", rowGap: 6 }}
          >
            {summary.layer_counts.map(([layer, count]) => (
              <span
                key={layer}
                className="meta-chip meta-chip--model"
                title={
                  layer === 0
                    ? "Layer 0 — base entities (facts + distillation)"
                    : `Layer ${layer} — aggregate entities`
                }
              >
                L{layer} · {count}
              </span>
            ))}
          </div>
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
        <div className="belief-card__meta">
          <span className="meta-chip meta-chip--mcps">
            {summary.inter_cluster_relations} edge
            {summary.inter_cluster_relations === 1 ? "" : "s"}
          </span>
        </div>
      </div>

      {/* Section 3: top aggregates — one belief-card per row */}
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
          <div
            style={{ display: "flex", flexDirection: "column", gap: 8 }}
            data-testid="hierarchy-aggregate-list"
          >
            {summary.top_aggregates.map((agg) => (
              <div
                key={agg.id}
                className="belief-card"
                style={{ cursor: "default" }}
              >
                <div className="belief-card__head">
                  <span className="belief-card__subject">{agg.name}</span>
                </div>
                {agg.description ? (
                  <p className="belief-card__content">{agg.description}</p>
                ) : null}
                <div className="belief-card__meta">
                  <span
                    className="meta-chip meta-chip--model"
                    title="Hierarchy layer"
                  >
                    L{agg.layer}
                  </span>
                  <span
                    className="meta-chip meta-chip--skills"
                    title="Member count — entities folded into this aggregate"
                  >
                    {agg.member_count} member
                    {agg.member_count === 1 ? "" : "s"}
                  </span>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
